use crate::error::{ModelSrvError, Result};
use crate::redis_handler::RedisHandler;
use futures::StreamExt;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::RwLock;
use tokio::time::{interval, Duration};

use super::calculation::CalculationEngine;
use super::{DeviceModel, InstanceManager};

/// Data flow processor for real-time device data processing
pub struct DataFlowProcessor {
    redis_client: Arc<RedisHandler>,
    instance_manager: Arc<InstanceManager>,
    calculation_engine: Arc<CalculationEngine>,
    subscriptions: Arc<RwLock<HashMap<String, DataSubscription>>>,
    update_channel: mpsc::Sender<DataUpdate>,
    is_running: Arc<RwLock<bool>>,
}

/// Data subscription for a device instance
#[derive(Clone)]
struct DataSubscription {
    _instance_id: String,
    point_mappings: HashMap<String, String>, // telemetry_name -> redis_key
    _update_interval: Duration,
}

/// Data update message
#[derive(Debug, Clone)]
pub struct DataUpdate {
    pub instance_id: String,
    pub telemetry_name: String,
    pub value: Value,
    pub timestamp: i64,
}

impl DataFlowProcessor {
    pub fn new(
        redis_client: Arc<RedisHandler>,
        instance_manager: Arc<InstanceManager>,
        calculation_engine: Arc<CalculationEngine>,
    ) -> (Self, mpsc::Receiver<DataUpdate>) {
        let (tx, rx) = mpsc::channel(1000);

        let processor = Self {
            redis_client,
            instance_manager,
            calculation_engine,
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
            update_channel: tx,
            is_running: Arc::new(RwLock::new(false)),
        };

        (processor, rx)
    }

    /// Start the data flow processor
    pub async fn start(&self) -> Result<()> {
        {
            let mut is_running = self.is_running.write().await;
            if *is_running {
                return Ok(()); // Already running
            }
            *is_running = true;
        }

        // Start Redis subscription handler
        let redis_handler = self.clone();
        tokio::spawn(async move {
            if let Err(e) = redis_handler.run_redis_subscriber().await {
                tracing::error!("Redis subscriber error: {}", e);
            }
        });

        // Start polling handler for non-pub/sub data
        let polling_handler = self.clone();
        tokio::spawn(async move {
            if let Err(e) = polling_handler.run_polling_handler().await {
                tracing::error!("Polling handler error: {}", e);
            }
        });

        Ok(())
    }

    /// Stop the data flow processor
    pub async fn stop(&self) -> Result<()> {
        let mut is_running = self.is_running.write().await;
        *is_running = false;
        Ok(())
    }

    /// Subscribe a device instance to data updates
    pub async fn subscribe_instance(
        &self,
        instance_id: String,
        point_mappings: HashMap<String, String>,
        update_interval: Duration,
    ) -> Result<()> {
        let subscription = DataSubscription {
            _instance_id: instance_id.clone(),
            point_mappings,
            _update_interval: update_interval,
        };

        let mut subscriptions = self.subscriptions.write().await;
        subscriptions.insert(instance_id, subscription);

        Ok(())
    }

    /// Unsubscribe a device instance
    pub async fn unsubscribe_instance(&self, instance_id: &str) -> Result<()> {
        let mut subscriptions = self.subscriptions.write().await;
        subscriptions.remove(instance_id);
        Ok(())
    }

    /// Process a data update
    pub async fn process_update(&self, update: DataUpdate) -> Result<()> {
        // Extract numeric value from JSON
        let numeric_value = match &update.value {
            Value::Number(n) => n.as_f64().unwrap_or(0.0),
            Value::String(s) => s.parse::<f64>().unwrap_or(0.0),
            _ => 0.0,
        };

        // Update instance telemetry
        self.instance_manager
            .update_telemetry(
                &update.instance_id,
                &update.telemetry_name,
                numeric_value,
                None,
            )
            .await?;

        // Get instance to check for calculations
        let instance = self
            .instance_manager
            .get_instance(&update.instance_id)
            .await
            .ok_or_else(|| ModelSrvError::instance_not_found(&update.instance_id))?;

        // Get the model to find calculations that depend on this telemetry
        let model = self.instance_manager.get_model(&instance.model_id).await?;
        for calc in &model.calculations {
            if calc.inputs.contains(&update.telemetry_name) {
                // Execute calculation
                self.execute_calculation(&instance.instance_id, &model, calc.identifier.clone())
                    .await?;
            }
        }

        Ok(())
    }

    /// Execute a calculation for an instance
    pub(super) async fn execute_calculation(
        &self,
        instance_id: &str,
        model: &DeviceModel,
        calculation_id: String,
    ) -> Result<()> {
        // Get calculation definition
        let _calc = model
            .calculations
            .iter()
            .find(|c| c.identifier == calculation_id)
            .ok_or_else(|| {
                crate::error::ModelSrvError::NotFound(format!(
                    "Calculation {} not found",
                    calculation_id
                ))
            })?;

        // Get device data to prepare telemetry values
        let device_data = self
            .instance_manager
            .get_device_data(instance_id)
            .await
            .ok_or_else(|| ModelSrvError::instance_not_found(instance_id))?;

        // Execute the calculation
        let instance = self
            .instance_manager
            .get_instance(instance_id)
            .await
            .ok_or_else(|| ModelSrvError::instance_not_found(instance_id))?;

        let results = self
            .calculation_engine
            .execute_model_calculations(model, &instance, &device_data.telemetry)
            .await?;

        // Store results
        if let Some(calc_result) = results.get(&calculation_id) {
            for (output_name, output_value) in &calc_result.outputs {
                if let Some(num_val) = output_value.as_f64() {
                    self.instance_manager
                        .update_telemetry(instance_id, output_name, num_val, None)
                        .await?;
                }
            }
        }

        Ok(())
    }

    /// Run Redis subscriber for real-time updates
    async fn run_redis_subscriber(&self) -> Result<()> {
        let mut pubsub = self.redis_client.get_async_pubsub().await?;

        // Subscribe to point update channel
        pubsub.subscribe("point:update").await?;

        while *self.is_running.read().await {
            let msg = pubsub.on_message().next().await;
            match msg {
                Some(msg) => {
                    if let Ok(payload) = msg.get_payload::<String>() {
                        // Parse update message
                        if let Ok(update_data) =
                            serde_json::from_str::<PointUpdateMessage>(&payload)
                        {
                            // Find subscriptions for this point
                            let subscriptions = self.subscriptions.read().await;
                            for (instance_id, sub) in subscriptions.iter() {
                                if let Some(telemetry_name) = sub
                                    .point_mappings
                                    .iter()
                                    .find(|(_, point_id)| **point_id == update_data.point_id)
                                    .map(|(name, _)| name.clone())
                                {
                                    let update = DataUpdate {
                                        instance_id: instance_id.clone(),
                                        telemetry_name,
                                        value: update_data.value.clone(),
                                        timestamp: update_data.timestamp,
                                    };

                                    if let Err(e) = self.update_channel.send(update).await {
                                        tracing::error!("Failed to send update: {}", e);
                                    }
                                }
                            }
                        }
                    }
                }
                None => {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        }

        Ok(())
    }

    /// Run polling handler for non-pub/sub data
    async fn run_polling_handler(&self) -> Result<()> {
        let mut interval = interval(Duration::from_secs(1));

        while *self.is_running.read().await {
            interval.tick().await;

            let subscriptions = self.subscriptions.read().await.clone();
            for (instance_id, sub) in subscriptions {
                // Check if it's time to poll
                for (telemetry_name, redis_key) in &sub.point_mappings {
                    // Get data from Redis - 现在主键直接存储数值字符串
                    if let Ok(Some(data)) = self.redis_client.get::<String>(redis_key).await {
                        // 尝试解析为浮点数
                        if let Ok(value) = data.parse::<f64>() {
                            let update = DataUpdate {
                                instance_id: instance_id.clone(),
                                telemetry_name: telemetry_name.clone(),
                                value: serde_json::Value::Number(
                                    serde_json::Number::from_f64(value)
                                        .unwrap_or(serde_json::Number::from(0)),
                                ),
                                timestamp: chrono::Utc::now().timestamp_millis(), // 如需精确时间戳，可从:ts键读取
                            };

                            if let Err(e) = self.update_channel.send(update).await {
                                tracing::error!("Failed to send polled update: {}", e);
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

impl Clone for DataFlowProcessor {
    fn clone(&self) -> Self {
        Self {
            redis_client: self.redis_client.clone(),
            instance_manager: self.instance_manager.clone(),
            calculation_engine: self.calculation_engine.clone(),
            subscriptions: self.subscriptions.clone(),
            update_channel: self.update_channel.clone(),
            is_running: self.is_running.clone(),
        }
    }
}

/// Point update message from Redis pub/sub
#[derive(Debug, Clone, serde::Deserialize)]
struct PointUpdateMessage {
    point_id: String,
    value: Value,
    timestamp: i64,
    _quality: Option<u8>,
}

/// Data flow configuration
#[derive(Debug, Clone, serde::Deserialize)]
pub struct DataFlowConfig {
    pub enable_pubsub: bool,
    pub enable_polling: bool,
    pub polling_interval_ms: u64,
    pub update_buffer_size: usize,
}

impl Default for DataFlowConfig {
    fn default() -> Self {
        Self {
            enable_pubsub: true,
            enable_polling: true,
            polling_interval_ms: 1000,
            update_buffer_size: 1000,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_data_flow_processor() {
        // TODO: Add tests
    }
}
