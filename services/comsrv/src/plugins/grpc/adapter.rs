//! gRPC Plugin Adapter
//!
//! Implements `ComBase` trait to call remote plugins via gRPC

use crate::core::combase::{ChannelStatus, ComBase, PointData, PointDataMap, RedisValue};
use crate::core::config::ChannelConfig;
use crate::utils::error::{ComSrvError, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

use super::client::GrpcPluginClient;
use super::proto::{
    command_value, point_data, BatchReadRequest, CommandValue, EncodeRequest,
    PointData as ProtoPointData,
};

/// gRPC Plugin Adapter
#[derive(Debug)]
pub struct GrpcPluginAdapter {
    /// Plugin client
    client: Arc<Mutex<GrpcPluginClient>>,
    /// Channel configuration
    channel_config: Option<ChannelConfig>,
    /// Connection status
    connected: bool,
    /// Plugin endpoint
    endpoint: String,
    /// Protocol type
    protocol_type: String,
    /// Connection parameters cache
    connection_params: HashMap<String, String>,
}

impl GrpcPluginAdapter {
    /// Create new gRPC plugin adapter
    pub async fn new(endpoint: &str, protocol_type: &str) -> Result<Self> {
        info!(
            "Creating gRPC plugin adapter for {} at {}",
            protocol_type, endpoint
        );

        let client = GrpcPluginClient::new(endpoint).await?;

        Ok(Self {
            client: Arc::new(Mutex::new(client)),
            channel_config: None,
            connected: false,
            endpoint: endpoint.to_string(),
            protocol_type: protocol_type.to_string(),
            connection_params: HashMap::new(),
        })
    }

    /// Convert protobuf `PointData` to internal format
    fn convert_proto_point(&self, proto_point: ProtoPointData) -> Result<(u32, PointData)> {
        let value = match proto_point.value {
            Some(point_data::Value::FloatValue(v)) => RedisValue::Float(v),
            Some(point_data::Value::IntValue(v)) => RedisValue::Integer(v),
            Some(point_data::Value::BoolValue(v)) => RedisValue::Bool(v),
            Some(point_data::Value::StringValue(s)) => RedisValue::String(s),
            None => RedisValue::Null,
        };

        Ok((
            proto_point.point_id,
            PointData {
                value,
                timestamp: proto_point.timestamp as u64,
            },
        ))
    }

    /// Build connection parameters
    fn build_connection_params(&self) -> HashMap<String, String> {
        let mut params = self.connection_params.clone();

        // Extract connection parameters from channel configuration
        if let Some(config) = &self.channel_config {
            // Add common parameters
            params.insert("channel_id".to_string(), config.id.to_string());
            params.insert("channel_name".to_string(), config.name.clone());

            // Add protocol-specific parameters
            for (key, value) in &config.parameters {
                // Convert YAML Value to string
                let value_str = match value {
                    serde_yaml::Value::String(s) => s.clone(),
                    serde_yaml::Value::Number(n) => n.to_string(),
                    serde_yaml::Value::Bool(b) => b.to_string(),
                    _ => serde_json::to_string(value).unwrap_or_default(),
                };
                params.insert(key.clone(), value_str);
            }
        }

        params
    }
}

#[async_trait]
impl ComBase for GrpcPluginAdapter {
    fn name(&self) -> &'static str {
        "gRPC Plugin Adapter"
    }

    fn protocol_type(&self) -> &str {
        &self.protocol_type
    }

    fn is_connected(&self) -> bool {
        self.connected
    }

    async fn get_status(&self) -> ChannelStatus {
        let mut status = ChannelStatus {
            is_connected: self.connected,
            ..Default::default()
        };

        // Get health status
        if let Ok(mut client) = self.client.try_lock() {
            if let Ok(health) = client.health_check().await {
                if !health.healthy {
                    status.last_error = Some(health.message);
                }
            }
        }

        status
    }

    async fn initialize(&mut self, channel_config: &ChannelConfig) -> Result<()> {
        info!(
            "Initializing gRPC plugin adapter for channel {}",
            channel_config.id
        );

        self.channel_config = Some(channel_config.clone());
        self.connection_params = self.build_connection_params();

        // Get plugin information
        let mut client = self.client.lock().await;
        let info = client.get_info().await?;

        info!(
            "Connected to plugin: {} v{} ({})",
            info.name, info.version, info.protocol_type
        );

        // Verify protocol type match
        if info.protocol_type != self.protocol_type {
            return Err(ComSrvError::config(format!(
                "Protocol type mismatch: expected {}, got {}",
                self.protocol_type, info.protocol_type
            )));
        }

        Ok(())
    }

    async fn connect(&mut self) -> Result<()> {
        info!("Connecting to gRPC plugin at {}", self.endpoint);

        // Health check
        let mut client = self.client.lock().await;
        let health = client.health_check().await?;

        if !health.healthy {
            return Err(ComSrvError::protocol(format!(
                "Plugin is not healthy: {}",
                health.message
            )));
        }

        self.connected = true;
        info!("Successfully connected to gRPC plugin");

        // Send initial BatchRead request to trigger plugin polling
        if let Some(channel_config) = &self.channel_config {
            info!(
                "Triggering initial batch read to start plugin polling for channel {}",
                channel_config.id
            );

            // Read telemetry type points to trigger polling
            let _ = self.read_four_telemetry("telemetry").await;
        }

        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        info!("Disconnecting from gRPC plugin");
        self.connected = false;
        Ok(())
    }

    async fn read_four_telemetry(&self, telemetry_type: &str) -> Result<PointDataMap> {
        if !self.connected {
            return Err(ComSrvError::NotConnected);
        }

        let channel_config = self
            .channel_config
            .as_ref()
            .ok_or_else(|| ComSrvError::config("No channel config"))?;

        // Build batch read request
        // Get point list based on telemetry_type
        let point_ids: Vec<u32> = match telemetry_type {
            "telemetry" => channel_config.telemetry_points.keys().copied().collect(),
            "signal" => channel_config.signal_points.keys().copied().collect(),
            "control" => channel_config.control_points.keys().copied().collect(),
            "adjustment" => channel_config.adjustment_points.keys().copied().collect(),
            _ => {
                warn!("Unknown telemetry type: {}", telemetry_type);
                vec![]
            }
        };

        if point_ids.is_empty() {
            debug!(
                "No points configured for telemetry type: {}",
                telemetry_type
            );
            return Ok(PointDataMap::new());
        }

        debug!(
            "Batch reading {} points for telemetry type {}",
            point_ids.len(),
            telemetry_type
        );

        // Build read parameters
        // TODO: Get Modbus parameters for each point from configuration
        // Currently simplified, let plugin use default configuration
        let read_params = HashMap::new();

        let request = BatchReadRequest {
            channel_id: u32::from(channel_config.id),
            point_ids,
            connection_params: self.connection_params.clone(),
            read_params,
        };

        let mut client = self.client.lock().await;
        let response = client.batch_read(request).await?;

        if !response.error.is_empty() {
            return Err(ComSrvError::protocol(response.error));
        }

        // Convert response data
        let mut results = PointDataMap::new();
        for point in response.points {
            match self.convert_proto_point(point) {
                Ok((id, data)) => {
                    results.insert(id, data);
                }
                Err(e) => {
                    warn!("Failed to convert point data: {}", e);
                }
            }
        }

        debug!("Read {} points from plugin", results.len());
        Ok(results)
    }

    async fn control(&mut self, commands: Vec<(u32, RedisValue)>) -> Result<Vec<(u32, bool)>> {
        if !self.connected {
            return Err(ComSrvError::NotConnected);
        }

        let mut results = Vec::new();

        for (point_id, value) in commands {
            let float_value = match value {
                RedisValue::Float(v) => v,
                RedisValue::Integer(v) => v as f64,
                RedisValue::Bool(v) => {
                    if v {
                        1.0
                    } else {
                        0.0
                    }
                }
                _ => {
                    results.push((point_id, false));
                    continue;
                }
            };

            // Build encode request
            let request = EncodeRequest {
                channel_id: self.channel_config.as_ref().map_or(0, |c| u32::from(c.id)),
                point_id,
                value: Some(CommandValue {
                    value: Some(command_value::Value::FloatValue(float_value)),
                }),
                context: self.connection_params.clone(),
            };

            let mut client = self.client.lock().await;
            match client.encode_command(request).await {
                Ok(response) if response.error.is_empty() => {
                    // TODO: Actually send encoded data to device
                    debug!(
                        "Encoded command for point {}: {} bytes",
                        point_id,
                        response.encoded_data.len()
                    );
                    results.push((point_id, true));
                }
                Ok(response) => {
                    warn!(
                        "Command encoding failed for point {}: {}",
                        point_id, response.error
                    );
                    results.push((point_id, false));
                }
                Err(e) => {
                    warn!("Command encoding error for point {}: {}", point_id, e);
                    results.push((point_id, false));
                }
            }
        }

        Ok(results)
    }

    async fn adjustment(
        &mut self,
        adjustments: Vec<(u32, RedisValue)>,
    ) -> Result<Vec<(u32, bool)>> {
        // Adjustment command implementation is similar to control command
        self.control(adjustments).await
    }
}
