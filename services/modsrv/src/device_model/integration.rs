use crate::error::Result;
use crate::redis_handler::RedisHandler;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::RwLock;

use super::{
    dataflow::DataUpdate, CalculationDefinition, CalculationEngine, CalculationExpression,
    CollectionType, CommandType, Constraints, DataFlowConfig, DataFlowProcessor, DataType,
    DeviceInstance, DeviceModel, DeviceType, InstanceManager, ModelRegistry, PropertyDefinition,
    TelemetryDefinition, TelemetryMapping, TelemetryValue,
};
use crate::cache::ModelCacheManager;
// engine module removed - using device model system directly

/// Integrated device model system
pub struct DeviceModelSystem {
    registry: Arc<ModelRegistry>,
    instance_manager: Arc<InstanceManager>,
    calculation_engine: Arc<CalculationEngine>,
    dataflow_processor: Arc<DataFlowProcessor>,
    cache_manager: Arc<ModelCacheManager>,
    redis_client: Arc<RedisHandler>,
    update_receiver: Arc<RwLock<Option<mpsc::Receiver<DataUpdate>>>>,
}

impl DeviceModelSystem {
    pub async fn new(
        redis_client: Arc<RedisHandler>,
        cache_manager: Arc<ModelCacheManager>,
        _config: DataFlowConfig,
    ) -> Result<Self> {
        let registry = Arc::new(ModelRegistry::new());
        let calculation_engine = Arc::new(CalculationEngine::new());
        let instance_manager = Arc::new(InstanceManager::new(registry.clone()));

        let (dataflow_processor, update_receiver) = DataFlowProcessor::new(
            redis_client.clone(),
            instance_manager.clone(),
            calculation_engine.clone(),
        );

        let system = Self {
            registry,
            instance_manager,
            calculation_engine,
            dataflow_processor: Arc::new(dataflow_processor),
            cache_manager,
            redis_client,
            update_receiver: Arc::new(RwLock::new(Some(update_receiver))),
        };

        // Start background update processor
        let update_processor = system.clone();
        tokio::spawn(async move {
            if let Err(e) = update_processor.process_updates().await {
                tracing::error!("Update processor error: {}", e);
            }
        });

        Ok(system)
    }

    /// Start the device model system
    pub async fn start(&self) -> Result<()> {
        // Start dataflow processor
        self.dataflow_processor.start().await?;

        // Load models from configuration
        self.load_models_from_config().await?;

        // Initialize instances from Redis
        self.initialize_instances().await?;

        tracing::info!("Device model system started");
        Ok(())
    }

    /// Stop the device model system
    pub async fn stop(&self) -> Result<()> {
        self.dataflow_processor.stop().await?;
        tracing::info!("Device model system stopped");
        Ok(())
    }

    /// Register a device model
    pub async fn register_model(&self, model: DeviceModel) -> Result<()> {
        self.registry.register_model(model).await
    }

    /// Create a device instance
    pub async fn create_instance(
        &self,
        model_id: &str,
        instance_id: String,
        name: String,
        initial_properties: HashMap<String, Value>,
    ) -> Result<String> {
        let instance = self
            .instance_manager
            .create_instance(
                model_id,
                &instance_id,
                &name,
                Some(initial_properties),
                None,
            )
            .await?;
        let id = instance.instance_id.clone();

        // Set up data subscriptions for the instance
        if let Some(instance) = self.instance_manager.get_instance(&id).await {
            let model = self.instance_manager.get_model(&instance.model_id).await?;
            let mut point_mappings = HashMap::new();

            // Map telemetry points to Redis keys
            for telemetry in &model.telemetry {
                // Use the telemetry mapping to create Redis key
                let redis_key = format!("point:{}", telemetry.mapping.point_id);
                point_mappings.insert(telemetry.identifier.clone(), redis_key);
            }

            // Subscribe instance to data updates
            self.dataflow_processor
                .subscribe_instance(
                    id.clone(),
                    point_mappings,
                    tokio::time::Duration::from_secs(1),
                )
                .await?;
        }

        Ok(id)
    }

    /// Get device instance
    pub async fn get_instance(&self, instance_id: &str) -> Result<Option<DeviceInstance>> {
        Ok(self.instance_manager.get_instance(instance_id).await)
    }

    /// Update device property
    pub async fn update_property(
        &self,
        instance_id: &str,
        property_name: &str,
        value: Value,
    ) -> Result<()> {
        let mut properties = HashMap::new();
        properties.insert(property_name.to_string(), value);
        self.instance_manager
            .update_instance_properties(instance_id, properties)
            .await
    }

    /// Get telemetry data
    pub async fn get_telemetry(
        &self,
        instance_id: &str,
        telemetry_name: &str,
    ) -> Result<Option<TelemetryValue>> {
        if let Some(device_data) = self.instance_manager.get_device_data(instance_id).await {
            Ok(device_data.telemetry.get(telemetry_name).cloned())
        } else {
            Ok(None)
        }
    }

    /// Execute command on device
    pub async fn execute_command(
        &self,
        instance_id: &str,
        command_name: &str,
        parameters: HashMap<String, Value>,
    ) -> Result<Value> {
        // Get instance and model
        let instance = self
            .instance_manager
            .get_instance(instance_id)
            .await
            .ok_or_else(|| {
                crate::error::ModelSrvError::NotFound(format!("Instance {} not found", instance_id))
            })?;

        let model = self.instance_manager.get_model(&instance.model_id).await?;

        // Find command definition
        let command = model
            .commands
            .iter()
            .find(|c| c.identifier == command_name)
            .ok_or_else(|| {
                crate::error::ModelSrvError::NotFound(format!("Command {} not found", command_name))
            })?;

        // Validate parameters
        for param in &command.input_params {
            if param.required && !parameters.contains_key(&param.name) {
                return Err(crate::error::ModelSrvError::ValidationError(format!(
                    "Required parameter {} missing",
                    param.name
                )));
            }
        }

        // Execute command based on type
        match &command.command_type {
            CommandType::Control => {
                // Send control command via Redis
                let channel = format!("cmd:{}:control", instance_id);
                let message = serde_json::json!({
                    "command": command_name,
                    "parameters": parameters,
                    "timestamp": chrono::Utc::now().timestamp_millis(),
                });

                self.redis_client
                    .publish(&channel, message.to_string())
                    .await?;
                Ok(serde_json::json!({"status": "sent"}))
            }
            _ => {
                // For other command types, send via appropriate channel
                let channel = format!("cmd:{}:{:?}", instance_id, command.command_type);
                let message = serde_json::json!({
                    "command": command_name,
                    "parameters": parameters,
                    "timestamp": chrono::Utc::now().timestamp_millis(),
                });

                self.redis_client
                    .publish(&channel, message.to_string())
                    .await?;
                Ok(serde_json::json!({"status": "sent"}))
            }
        }
    }

    /// Process data updates in background
    async fn process_updates(&self) -> Result<()> {
        let mut receiver = {
            let mut rx_lock = self.update_receiver.write().await;
            rx_lock.take().ok_or_else(|| {
                crate::error::ModelSrvError::InternalError(
                    "Update receiver already taken".to_string(),
                )
            })?
        };

        while let Some(update) = receiver.recv().await {
            if let Err(e) = self.dataflow_processor.process_update(update).await {
                tracing::error!("Failed to process update: {}", e);
            }
        }

        Ok(())
    }

    /// Load models from configuration
    async fn load_models_from_config(&self) -> Result<()> {
        // TODO: Load models from config files or database
        // For now, register some example models

        // Example: Power meter model
        let power_meter = DeviceModel {
            id: "power_meter_v1".to_string(),
            name: "Power Meter V1".to_string(),
            description: "Three-phase power meter".to_string(),
            version: "1.0.0".to_string(),
            device_type: DeviceType::PowerMeter,
            properties: vec![PropertyDefinition {
                identifier: "rated_voltage".to_string(),
                name: "Rated Voltage".to_string(),
                data_type: DataType::Float64,
                required: false,
                default_value: Some(serde_json::json!(380)),
                constraints: Some(Constraints {
                    min: Some(100.0),
                    max: Some(1000.0),
                    enum_values: None,
                    pattern: None,
                }),
                unit: Some("V".to_string()),
                description: None,
            }],
            telemetry: vec![
                TelemetryDefinition {
                    identifier: "voltage_a".to_string(),
                    name: "Voltage Phase A".to_string(),
                    data_type: DataType::Float64,
                    collection_type: CollectionType::Periodic { interval_ms: 1000 },
                    mapping: TelemetryMapping {
                        channel_id: 1,
                        point_type: "YC".to_string(),
                        point_id: 1001,
                        scale: None,
                        offset: None,
                    },
                    transform: None,
                    unit: Some("V".to_string()),
                    description: None,
                },
                TelemetryDefinition {
                    identifier: "current_a".to_string(),
                    name: "Current Phase A".to_string(),
                    data_type: DataType::Float64,
                    collection_type: CollectionType::Periodic { interval_ms: 1000 },
                    mapping: TelemetryMapping {
                        channel_id: 1,
                        point_type: "YC".to_string(),
                        point_id: 1002,
                        scale: None,
                        offset: None,
                    },
                    transform: None,
                    unit: Some("A".to_string()),
                    description: None,
                },
                TelemetryDefinition {
                    identifier: "power_a".to_string(),
                    name: "Power Phase A".to_string(),
                    data_type: DataType::Float64,
                    collection_type: CollectionType::Periodic { interval_ms: 1000 },
                    mapping: TelemetryMapping {
                        channel_id: 1,
                        point_type: "YC".to_string(),
                        point_id: 1003,
                        scale: None,
                        offset: None,
                    },
                    transform: None,
                    unit: Some("kW".to_string()),
                    description: None,
                },
            ],
            commands: vec![],
            events: vec![],
            calculations: vec![CalculationDefinition {
                identifier: "total_power".to_string(),
                name: "Total Power".to_string(),
                inputs: vec![
                    "power_a".to_string(),
                    "power_b".to_string(),
                    "power_c".to_string(),
                ],
                outputs: vec!["total_power".to_string()],
                expression: CalculationExpression::BuiltIn {
                    function: "sum".to_string(),
                    args: vec![],
                },
                condition: None,
                description: Some("Calculate total power across all phases".to_string()),
            }],
            metadata: HashMap::new(),
        };

        self.registry.register_model(power_meter).await?;

        Ok(())
    }

    /// Initialize instances from Redis
    async fn initialize_instances(&self) -> Result<()> {
        // TODO: Load existing instances from Redis
        Ok(())
    }
}

impl Clone for DeviceModelSystem {
    fn clone(&self) -> Self {
        Self {
            registry: self.registry.clone(),
            instance_manager: self.instance_manager.clone(),
            calculation_engine: self.calculation_engine.clone(),
            dataflow_processor: self.dataflow_processor.clone(),
            cache_manager: self.cache_manager.clone(),
            redis_client: self.redis_client.clone(),
            update_receiver: self.update_receiver.clone(),
        }
    }
}
