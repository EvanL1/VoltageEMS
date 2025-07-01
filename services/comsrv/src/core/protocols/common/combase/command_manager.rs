//! Command Manager Module
//!
//! This module contains the universal command manager implementation for handling 
//! four-telemetry commands across all protocols.


use async_trait::async_trait;
use futures::StreamExt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, error, info, trace, warn};
use chrono::Utc;

use super::traits::FourTelemetryOperations;
use super::telemetry::{
    RemoteOperationRequest, RemoteOperationType, PointValueType
};
use super::data_types::PointData;
use crate::utils::Result;

/// Universal Redis command manager for handling four-telemetry commands across all protocols
#[derive(Clone)]
pub struct UniversalCommandManager {
    /// Redis store for command handling
    redis_store: Option<crate::core::storage::redis_storage::RedisStore>,
    /// Channel ID for this communication instance
    channel_id: String,
    /// Command listener task handle
    command_listener_handle: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
    /// Running state (atomic for better performance)
    is_running: Arc<AtomicBool>,
}

impl UniversalCommandManager {
    /// Create a new command manager
    pub fn new(channel_id: String) -> Self {
        Self {
            redis_store: None,
            channel_id,
            command_listener_handle: Arc::new(RwLock::new(None)),
            is_running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Initialize with Redis store
    pub fn with_redis_store(
        mut self,
        redis_store: crate::core::storage::redis_storage::RedisStore,
    ) -> Self {
        self.redis_store = Some(redis_store);
        self
    }

    /// Start command listener
    pub async fn start<T>(&self, four_telemetry_impl: Arc<T>) -> Result<()>
    where
        T: FourTelemetryOperations + 'static,
    {
        if self.redis_store.is_none() {
            // No Redis integration, skip command listener
            return Ok(());
        }

        self.is_running.store(true, Ordering::SeqCst);

        let redis_store = self.redis_store.as_ref().unwrap().clone();
        let channel_id = self.channel_id.clone();
        let is_running = Arc::clone(&self.is_running);

        let handle = tokio::spawn(async move {
            Self::command_listener_loop(redis_store, four_telemetry_impl, channel_id, is_running)
                .await;
        });

        *self.command_listener_handle.write().await = Some(handle);
        info!(
            "Universal command manager started for channel: {}",
            self.channel_id
        );
        Ok(())
    }

    /// Stop command listener
    pub async fn stop(&self) -> Result<()> {
        self.is_running.store(false, Ordering::SeqCst);

        if let Some(handle) = self.command_listener_handle.write().await.take() {
            handle.abort();
        }

        info!(
            "Universal command manager stopped for channel: {}",
            self.channel_id
        );
        Ok(())
    }

    /// Redis command listener loop
    async fn command_listener_loop<T>(
        redis_store: crate::core::storage::redis_storage::RedisStore,
        four_telemetry_impl: Arc<T>,
        channel_id: String,
        is_running: Arc<AtomicBool>,
    ) where
        T: FourTelemetryOperations + 'static,
    {
        info!(
            "Starting Redis command listener for channel: {}",
            channel_id
        );

        // Create PubSub connection
        let mut pubsub = match redis_store.create_pubsub().await {
            Ok(pubsub) => pubsub,
            Err(e) => {
                error!("Failed to create Redis PubSub connection: {}", e);
                return;
            }
        };

        // Subscribe to command channel
        let command_channel = format!("commands:{}", channel_id);
        if let Err(e) = pubsub.subscribe(&command_channel).await {
            error!(
                "Failed to subscribe to command channel {}: {}",
                command_channel, e
            );
            return;
        }

        info!("Subscribed to Redis command channel: {}", command_channel);

        // Listen for commands
        while is_running.load(Ordering::SeqCst) {
            match pubsub.on_message().next().await {
                Some(msg) => {
                    let command_id: String = match msg.get_payload() {
                        Ok(payload) => payload,
                        Err(e) => {
                            warn!("Failed to parse command notification payload: {}", e);
                            continue;
                        }
                    };

                    debug!("Received command notification: {}", command_id);

                    // Process command
                    if let Err(e) = Self::process_redis_command(
                        &redis_store,
                        &four_telemetry_impl,
                        &channel_id,
                        &command_id,
                    )
                    .await
                    {
                        error!("Failed to process command {}: {}", command_id, e);
                    }
                }
                None => {
                    trace!("No message received from Redis PubSub");
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        }

        debug!("Redis command listener loop stopped");
    }

    /// Process a Redis command using four-telemetry operations
    async fn process_redis_command<T>(
        redis_store: &crate::core::storage::redis_storage::RedisStore,
        four_telemetry_impl: &Arc<T>,
        channel_id: &str,
        command_id: &str,
    ) -> Result<()>
    where
        T: FourTelemetryOperations + 'static,
    {
        use crate::core::storage::redis_storage::{CommandResult, CommandType};

        // Get command from Redis
        let command = match redis_store.get_command(channel_id, command_id).await? {
            Some(cmd) => cmd,
            None => {
                warn!("Command {} not found in Redis", command_id);
                return Ok(());
            }
        };

        info!(
            "Processing command: {} for point: {} with value: {}",
            command_id, command.point_name, command.value
        );

        // Convert Redis command to four-telemetry request
        let request = RemoteOperationRequest {
            operation_id: command.command_id.clone(),
            point_name: command.point_name.clone(),
            operation_type: match command.command_type {
                CommandType::RemoteControl => RemoteOperationType::Control {
                    value: command.value != 0.0,
                },
                CommandType::RemoteRegulation => RemoteOperationType::Regulation {
                    value: command.value,
                },
            },
            operator: None,
            description: None,
            timestamp: Utc::now(),
        };

        // Execute command using four-telemetry interface
        let response = match command.command_type {
            CommandType::RemoteControl => four_telemetry_impl.remote_control(request).await,
            CommandType::RemoteRegulation => four_telemetry_impl.remote_regulation(request).await,
        };

        // Convert four-telemetry response to Redis result
        let result = match response {
            Ok(resp) => {
                info!("Command {} executed successfully", command_id);

                CommandResult {
                    command_id: resp.operation_id,
                    success: resp.success,
                    error_message: resp.error_message,
                    execution_time: resp
                        .execution_time
                        .format("%Y-%m-%dT%H:%M:%S%.3fZ")
                        .to_string(),
                    actual_value: resp.actual_value.map(|v| match v {
                        PointValueType::Analog(val) => val,
                        PointValueType::Digital(val) => {
                            if val {
                                1.0
                            } else {
                                0.0
                            }
                        }
                        PointValueType::Measurement(m) => m.value,
                        PointValueType::Signaling(s) => {
                            if s.status {
                                1.0
                            } else {
                                0.0
                            }
                        }
                        PointValueType::Control(c) => {
                            if c.current_state {
                                1.0
                            } else {
                                0.0
                            }
                        }
                        PointValueType::Regulation(r) => r.current_value,
                    }),
                }
            }
            Err(e) => {
                error!("Command {} execution failed: {}", command_id, e);

                CommandResult {
                    command_id: command.command_id.clone(),
                    success: false,
                    error_message: Some(e.to_string()),
                    execution_time: Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
                    actual_value: None,
                }
            }
        };

        // Store result back to Redis
        redis_store
            .set_command_result(channel_id, &result)
            .await?;

        Ok(())
    }

    /// Synchronize data points to Redis
    pub async fn sync_data_to_redis(&self, data_points: &[PointData]) -> Result<()> {
        if let Some(ref redis_store) = self.redis_store {
            // Convert each PointData to RealtimeValue and store
            for point in data_points {
                let realtime_value = crate::core::storage::redis_storage::RealtimeValue {
                    raw: point.value.parse().unwrap_or(0.0),
                    processed: point.value.parse().unwrap_or(0.0),
                    timestamp: point.timestamp.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
                };
                
                let key = format!("{}:point:{}", self.channel_id, point.id);
                redis_store.set_realtime_value(&key, &realtime_value).await?;
            }
            
            debug!(
                "Synced {} data points to Redis for channel: {}",
                data_points.len(),
                self.channel_id
            );
        }
        Ok(())
    }

    /// Check if the command manager is running
    pub fn is_running(&self) -> bool {
        self.is_running.load(Ordering::SeqCst)
    }

    /// Get the channel ID
    pub fn channel_id(&self) -> &str {
        &self.channel_id
    }

    /// Check if Redis integration is enabled
    pub fn has_redis_integration(&self) -> bool {
        self.redis_store.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::collections::HashMap;
    use crate::core::protocols::common::combase::telemetry::{
        MeasurementPoint, SignalingPoint, ControlPoint, RegulationPoint, RemoteOperationResponse,
        ExecutionStatus,
    };

    struct MockFourTelemetryOperations {
        measurement_points: HashMap<String, f64>,
        signaling_points: HashMap<String, bool>,
        control_points: HashMap<String, bool>,
        regulation_points: HashMap<String, f64>,
        should_fail: bool,
    }

    impl MockFourTelemetryOperations {
        fn new() -> Self {
            let mut measurement_points = HashMap::new();
            measurement_points.insert("temp_01".to_string(), 25.5);
            measurement_points.insert("pressure_01".to_string(), 1.2);

            let mut signaling_points = HashMap::new();
            signaling_points.insert("pump_status".to_string(), true);
            signaling_points.insert("alarm_high_temp".to_string(), false);

            Self {
                measurement_points,
                signaling_points,
                control_points: HashMap::new(),
                regulation_points: HashMap::new(),
                should_fail: false,
            }
        }
    }

    #[async_trait]
    impl FourTelemetryOperations for MockFourTelemetryOperations {
        async fn remote_measurement(
            &self,
            point_names: &[String],
        ) -> Result<Vec<(String, PointValueType)>> {
            if self.should_fail {
                return Err(crate::utils::ComSrvError::InvalidOperation("Mock failure".to_string()));
            }

            let mut results = Vec::new();
            for name in point_names {
                if let Some(&value) = self.measurement_points.get(name) {
                    results.push((
                        name.clone(),
                        PointValueType::Measurement(MeasurementPoint {
                            value,
                            unit: "°C".to_string(),
                            timestamp: Utc::now(),
                        }),
                    ));
                }
            }
            Ok(results)
        }

        async fn remote_signaling(
            &self,
            point_names: &[String],
        ) -> Result<Vec<(String, PointValueType)>> {
            if self.should_fail {
                return Err(crate::utils::ComSrvError::InvalidOperation("Mock failure".to_string()));
            }

            let mut results = Vec::new();
            for name in point_names {
                if let Some(&status) = self.signaling_points.get(name) {
                    results.push((
                        name.clone(),
                        PointValueType::Signaling(SignalingPoint {
                            status,
                            status_text: if status { "ON" } else { "OFF" }.to_string(),
                            timestamp: Utc::now(),
                        }),
                    ));
                }
            }
            Ok(results)
        }

        async fn remote_control(
            &self,
            _request: RemoteOperationRequest,
        ) -> Result<RemoteOperationResponse> {
            if self.should_fail {
                return Err(crate::utils::ComSrvError::InvalidOperation("Mock failure".to_string()));
            }

            Ok(RemoteOperationResponse {
                operation_id: "test_control".to_string(),
                success: true,
                error_message: None,
                actual_value: Some(PointValueType::Control(ControlPoint {
                    current_state: true,
                    command_text: "Start".to_string(),
                    execution_status: ExecutionStatus::Success,
                    timestamp: Utc::now(),
                })),
                execution_time: Utc::now(),
            })
        }

        async fn remote_regulation(
            &self,
            _request: RemoteOperationRequest,
        ) -> Result<RemoteOperationResponse> {
            if self.should_fail {
                return Err(crate::utils::ComSrvError::InvalidOperation("Mock failure".to_string()));
            }

            Ok(RemoteOperationResponse {
                operation_id: "test_regulation".to_string(),
                success: true,
                error_message: None,
                actual_value: Some(PointValueType::Regulation(RegulationPoint {
                    current_value: 75.0,
                    unit: "°C".to_string(),
                    in_range: true,
                    timestamp: Utc::now(),
                })),
                execution_time: Utc::now(),
            })
        }

        async fn get_control_points(&self) -> Vec<String> {
            vec!["pump_start".to_string(), "valve_open".to_string()]
        }

        async fn get_regulation_points(&self) -> Vec<String> {
            vec!["temp_setpoint".to_string(), "flow_setpoint".to_string()]
        }

        async fn get_measurement_points(&self) -> Vec<String> {
            vec!["temp_01".to_string(), "pressure_01".to_string()]
        }

        async fn get_signaling_points(&self) -> Vec<String> {
            vec!["pump_status".to_string(), "alarm_high_temp".to_string()]
        }
    }

    #[tokio::test]
    async fn test_universal_command_manager_creation() {
        let manager = UniversalCommandManager::new("test_channel".to_string());
        assert_eq!(manager.channel_id(), "test_channel");
        assert!(!manager.has_redis_integration());
        assert!(!manager.is_running());
    }

    #[tokio::test]
    async fn test_command_manager_without_redis() {
        let manager = UniversalCommandManager::new("test_channel".to_string());
        let four_telemetry = Arc::new(MockFourTelemetryOperations::new());

        // Should succeed but do nothing without Redis
        let result = manager.start(four_telemetry).await;
        assert!(result.is_ok());
        assert!(!manager.is_running());
    }

    #[tokio::test]
    async fn test_sync_data_without_redis() {
        let manager = UniversalCommandManager::new("test_channel".to_string());
        let data_points = vec![
            PointData {
                id: "test_1".to_string(),
                name: "Test Point 1".to_string(),
                value: "123.45".to_string(),
                timestamp: Utc::now(),
                unit: "°C".to_string(),
                description: "Test description".to_string(),
            }
        ];

        // Should succeed but do nothing without Redis
        let result = manager.sync_data_to_redis(&data_points).await;
        assert!(result.is_ok());
    }
} 