//! CAN Client Implementation
//!
//! This module provides CAN bus client functionality including message
//! transmission, reception, filtering, and data extraction.

use super::common::*;
use super::frame::CanFrame;
use crate::core::config::ChannelConfig;
use crate::core::protocols::common::combase::DefaultProtocol;
use crate::core::protocols::common::traits::ComBase;
use crate::core::protocols::common::{ChannelStatus, PointData};
use crate::utils::{ComSrvError, Result};
use crate::utils::hex::format_hex;
use async_trait::async_trait;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};

/// CAN client trait defining the interface for CAN bus communication
///
/// This trait provides a standardized interface for CAN bus operations
/// across different CAN interface implementations (SocketCAN, Peak CAN, etc.).
#[async_trait]
pub trait CanClient: ComBase {
    /// Send a CAN frame
    ///
    /// # Arguments
    ///
    /// * `frame` - CAN frame to send
    ///
    /// # Returns
    ///
    /// `Ok(())` if successful, `Err` if transmission failed
    async fn send_frame(&self, frame: &CanFrame) -> Result<()>;

    /// Receive CAN frames with optional filtering
    ///
    /// # Arguments
    ///
    /// * `filter_ids` - Optional list of CAN IDs to filter (None = receive all)
    /// * `timeout_ms` - Timeout in milliseconds (0 = non-blocking)
    ///
    /// # Returns
    ///
    /// Vector of received CAN frames
    async fn receive_frames(
        &self,
        filter_ids: Option<&[u32]>,
        timeout_ms: u64,
    ) -> Result<Vec<CanFrame>>;

    /// Read data from CAN message using mapping configuration
    ///
    /// # Arguments
    ///
    /// * `mapping` - CAN message mapping configuration
    ///
    /// # Returns
    ///
    /// JSON value containing the extracted data
    async fn read_data(&self, mapping: &CanMessageMapping) -> Result<serde_json::Value>;

    /// Write data to CAN bus using mapping configuration
    ///
    /// # Arguments
    ///
    /// * `mapping` - CAN message mapping configuration
    /// * `value` - JSON value to encode and send
    async fn write_data(
        &self,
        mapping: &CanMessageMapping,
        value: &serde_json::Value,
    ) -> Result<()>;

    /// Get CAN bus statistics
    ///
    /// # Returns
    ///
    /// Current bus statistics
    async fn get_statistics(&self) -> CanStatistics;

    /// Reset CAN bus statistics
    async fn reset_statistics(&self);
}

/// Base implementation for CAN clients
///
/// `CanClientBase` provides common functionality shared across different
/// CAN client implementations. It handles connection management, status
/// tracking, message mapping, and frame processing.
#[derive(Debug)]
pub struct CanClientBase {
    /// Base communication implementation
    pub base: DefaultProtocol,
    /// CAN interface configuration
    interface_type: CanInterfaceType,
    /// CAN bit rate
    bit_rate: CanBitRate,
    /// Connection timeout in milliseconds
    timeout_ms: u64,
    /// Current connection status
    connected: Arc<RwLock<bool>>,
    /// CAN message mappings
    message_mappings: Arc<RwLock<Vec<CanMessageMapping>>>,
    /// CAN bus statistics
    statistics: Arc<RwLock<CanStatistics>>,
    /// Message filters (CAN IDs to monitor)
    message_filters: Arc<RwLock<Vec<u32>>>,
}

impl CanClientBase {
    /// Create a new CAN client base implementation
    ///
    /// # Arguments
    ///
    /// * `name` - Human-readable name for the client instance
    /// * `config` - Channel configuration containing CAN parameters
    ///
    /// # Configuration Parameters
    ///
    /// - `interface_type`: CAN interface type (default: SocketCan("can0"))
    /// - `bit_rate`: CAN bit rate (default: 500000)
    /// - `timeout`: Connection timeout in milliseconds (default: 1000)
    ///
    /// # Returns
    ///
    /// New `CanClientBase` instance
    pub fn new(name: &str, config: ChannelConfig) -> Self {
        // Parse interface type
        let interface_type = config
            .parameters
            .get("interface")
            .and_then(|v| v.as_str().map(|s| s.to_string()))
            .map(|s| match s.as_str() {
                s if s.starts_with("socketcan:") => CanInterfaceType::SocketCan(
                    s.strip_prefix("socketcan:").unwrap_or("can0").to_string(),
                ),
                s if s.starts_with("peak:") => CanInterfaceType::PeakCan(
                    s.strip_prefix("peak:")
                        .unwrap_or("PCAN_USBBUS1")
                        .to_string(),
                ),
                s if s.starts_with("virtual:") => CanInterfaceType::Virtual(
                    s.strip_prefix("virtual:").unwrap_or("vcan0").to_string(),
                ),
                s if s.starts_with("usb:") => {
                    CanInterfaceType::UsbCan(s.strip_prefix("usb:").unwrap_or("USB1").to_string())
                }
                _ => CanInterfaceType::SocketCan("can0".to_string()),
            })
            .unwrap_or(CanInterfaceType::SocketCan("can0".to_string()));

        // Parse bit rate
        let bit_rate = config
            .parameters
            .get("bit_rate")
            .and_then(|v| v.as_u64())
            .map(|rate| match rate {
                10000 => CanBitRate::Kbps10,
                20000 => CanBitRate::Kbps20,
                50000 => CanBitRate::Kbps50,
                100000 => CanBitRate::Kbps100,
                125000 => CanBitRate::Kbps125,
                250000 => CanBitRate::Kbps250,
                500000 => CanBitRate::Kbps500,
                800000 => CanBitRate::Kbps800,
                1000000 => CanBitRate::Mbps1,
                _ => CanBitRate::Kbps500, // Default
            })
            .unwrap_or(CanBitRate::Kbps500);

        let timeout_ms = config
            .parameters
            .get("timeout")
            .and_then(|v| v.as_u64())
            .unwrap_or(1000);

        Self {
            base: DefaultProtocol::new(name, &config.protocol.to_string(), config),
            interface_type,
            bit_rate,
            timeout_ms,
            connected: Arc::new(RwLock::new(false)),
            message_mappings: Arc::new(RwLock::new(Vec::new())),
            statistics: Arc::new(RwLock::new(CanStatistics::default())),
            message_filters: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Get the CAN interface type
    pub fn interface_type(&self) -> &CanInterfaceType {
        &self.interface_type
    }

    /// Get the CAN bit rate
    pub fn bit_rate(&self) -> CanBitRate {
        self.bit_rate
    }

    /// Get the timeout value
    pub fn timeout_ms(&self) -> u64 {
        self.timeout_ms
    }

    /// Load CAN message mappings
    ///
    /// # Arguments
    ///
    /// * `mappings` - Vector of CAN message mappings to load
    pub async fn load_message_mappings(&self, mappings: Vec<CanMessageMapping>) {
        let mut msg_mappings = self.message_mappings.write().await;
        *msg_mappings = mappings;

        // Update message filters based on mappings
        let mut filters = self.message_filters.write().await;
        filters.clear();
        for mapping in msg_mappings.iter() {
            if !filters.contains(&mapping.can_id) {
                filters.push(mapping.can_id);
            }
        }

        info!(
            "Loaded {} CAN message mappings with {} unique IDs",
            msg_mappings.len(),
            filters.len()
        );
    }

    /// Get all CAN message mappings
    pub async fn get_message_mappings(&self) -> Vec<CanMessageMapping> {
        self.message_mappings.read().await.clone()
    }

    /// Find a CAN message mapping by name
    ///
    /// # Arguments
    ///
    /// * `name` - Name of the mapping to find
    ///
    /// # Returns
    ///
    /// `Some(mapping)` if found, `None` otherwise
    pub async fn find_mapping(&self, name: &str) -> Option<CanMessageMapping> {
        let mappings = self.message_mappings.read().await;
        mappings.iter().find(|m| m.name == name).cloned()
    }

    /// Extract data from CAN frame using mapping configuration
    ///
    /// # Arguments
    ///
    /// * `frame` - CAN frame containing data
    /// * `mapping` - Message mapping configuration
    ///
    /// # Returns
    ///
    /// Extracted and scaled data value
    pub fn extract_data(
        &self,
        frame: &CanFrame,
        mapping: &CanMessageMapping,
    ) -> Result<serde_json::Value> {
        let config = &mapping.data_config;
        let data = &frame.data;

        // Validate data length
        if (config.start_byte as usize) >= data.len() {
            return Err(ComSrvError::InvalidData(format!(
                "Start byte {} exceeds frame data length {}",
                config.start_byte,
                data.len()
            )));
        }

        let raw_value = match config.data_type {
            CanDataType::Bool => {
                let byte_val = data[config.start_byte as usize];
                let bit_mask = 1 << config.bit_offset;
                serde_json::Value::Bool((byte_val & bit_mask) != 0)
            }
            CanDataType::UInt8 => serde_json::Value::Number(serde_json::Number::from(
                data[config.start_byte as usize],
            )),
            CanDataType::Int8 => {
                let val = data[config.start_byte as usize] as i8;
                serde_json::Value::Number(serde_json::Number::from(val))
            }
            CanDataType::UInt16 => {
                if (config.start_byte as usize + 1) >= data.len() {
                    return Err(ComSrvError::InvalidData(
                        "Insufficient data for UInt16".to_string(),
                    ));
                }
                let val = match config.byte_order {
                    CanByteOrder::BigEndian => u16::from_be_bytes([
                        data[config.start_byte as usize],
                        data[config.start_byte as usize + 1],
                    ]),
                    CanByteOrder::LittleEndian => u16::from_le_bytes([
                        data[config.start_byte as usize],
                        data[config.start_byte as usize + 1],
                    ]),
                };
                serde_json::Value::Number(serde_json::Number::from(val))
            }
            CanDataType::UInt32 => {
                if (config.start_byte as usize + 3) >= data.len() {
                    return Err(ComSrvError::InvalidData(
                        "Insufficient data for UInt32".to_string(),
                    ));
                }
                let val = match config.byte_order {
                    CanByteOrder::BigEndian => u32::from_be_bytes([
                        data[config.start_byte as usize],
                        data[config.start_byte as usize + 1],
                        data[config.start_byte as usize + 2],
                        data[config.start_byte as usize + 3],
                    ]),
                    CanByteOrder::LittleEndian => u32::from_le_bytes([
                        data[config.start_byte as usize],
                        data[config.start_byte as usize + 1],
                        data[config.start_byte as usize + 2],
                        data[config.start_byte as usize + 3],
                    ]),
                };
                serde_json::Value::Number(serde_json::Number::from(val))
            }
            CanDataType::Float32 => {
                if (config.start_byte as usize + 3) >= data.len() {
                    return Err(ComSrvError::InvalidData(
                        "Insufficient data for Float32".to_string(),
                    ));
                }
                let val = match config.byte_order {
                    CanByteOrder::BigEndian => f32::from_be_bytes([
                        data[config.start_byte as usize],
                        data[config.start_byte as usize + 1],
                        data[config.start_byte as usize + 2],
                        data[config.start_byte as usize + 3],
                    ]),
                    CanByteOrder::LittleEndian => f32::from_le_bytes([
                        data[config.start_byte as usize],
                        data[config.start_byte as usize + 1],
                        data[config.start_byte as usize + 2],
                        data[config.start_byte as usize + 3],
                    ]),
                };
                serde_json::Value::Number(
                    serde_json::Number::from_f64(val as f64).unwrap_or(serde_json::Number::from(0)),
                )
            }
            CanDataType::Raw => {
                let start = config.start_byte as usize;
                let end = std::cmp::min(start + config.data_type.size_bytes(), data.len());
                let raw_data = &data[start..end];
                serde_json::Value::Array(
                    raw_data
                        .iter()
                        .map(|&b| serde_json::Value::Number(serde_json::Number::from(b)))
                        .collect(),
                )
            }
            CanDataType::String(max_len) => {
                let start = config.start_byte as usize;
                let end = std::cmp::min(start + max_len, data.len());
                let string_data = &data[start..end];

                // Find null terminator or use full length
                let null_pos = string_data
                    .iter()
                    .position(|&b| b == 0)
                    .unwrap_or(string_data.len());
                let string_bytes = &string_data[..null_pos];

                match String::from_utf8(string_bytes.to_vec()) {
                    Ok(s) => serde_json::Value::String(s),
                    Err(_) => serde_json::Value::String(format_hex(string_bytes)),
                }
            }
            _ => {
                return Err(ComSrvError::InvalidData(format!(
                    "Unsupported data type: {:?}",
                    config.data_type
                )))
            }
        };

        // Apply scaling and offset
        match &raw_value {
            serde_json::Value::Number(num) => {
                if let Some(val) = num.as_f64() {
                    let scaled_value = val * mapping.scale + mapping.offset;
                    Ok(serde_json::Value::Number(
                        serde_json::Number::from_f64(scaled_value)
                            .unwrap_or(serde_json::Number::from(0)),
                    ))
                } else {
                    Ok(raw_value)
                }
            }
            _ => Ok(raw_value),
        }
    }

    /// Update statistics
    pub async fn update_statistics(
        &self,
        messages_sent: u64,
        messages_received: u64,
        error_count: u64,
    ) {
        let stats = self.statistics.read().await;
        stats
            .messages_sent
            .fetch_add(messages_sent, Ordering::Relaxed);
        stats
            .messages_received
            .fetch_add(messages_received, Ordering::Relaxed);
        stats
            .error_messages
            .fetch_add(error_count, Ordering::Relaxed);

        if error_count > 0 {
            drop(stats);
            let mut stats = self.statistics.write().await;
            stats.last_error_time = Some(std::time::SystemTime::now());
        }
    }
}

#[async_trait]
impl ComBase for CanClientBase {
    fn name(&self) -> &str {
        "CAN"
    }

    fn channel_id(&self) -> String {
        "CAN".to_string()
    }

    fn protocol_type(&self) -> &str {
        "CAN"
    }

    fn get_parameters(&self) -> HashMap<String, String> {
        let mut params = HashMap::new();
        params.insert(
            "interface".to_string(),
            format!("{:?}", self.interface_type),
        );
        params.insert("bit_rate".to_string(), format!("{:?}", self.bit_rate));
        params.insert("timeout_ms".to_string(), self.timeout_ms.to_string());
        params
    }

    async fn is_running(&self) -> bool {
        *self.connected.read().await
    }

    async fn start(&mut self) -> Result<()> {
        // Initialize CAN interface here
        *self.connected.write().await = true;
        info!("Starting CAN client on interface {:?}", self.interface_type);
        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        // Clean up CAN interface here
        *self.connected.write().await = false;
        info!("Stopping CAN client");
        Ok(())
    }

    async fn status(&self) -> ChannelStatus {
        ChannelStatus {
            id: self.channel_id().to_string(),
            connected: *self.connected.read().await,
            last_response_time: 0.0,
            last_error: String::new(),
            last_update_time: Utc::now(),
        }
    }

    async fn get_all_points(&self) -> Vec<PointData> {
        let mappings = self.message_mappings.read().await;
        let mut points = Vec::new();

        for mapping in mappings.iter() {
            let point_data = PointData {
                id: mapping.name.clone(),
                name: mapping
                    .display_name
                    .clone()
                    .unwrap_or_else(|| mapping.name.clone()),
                value: "null".to_string(),
                timestamp: Utc::now(),
                unit: mapping.unit.clone().unwrap_or_default(),
                description: mapping.description.clone().unwrap_or_default(),
                telemetry_type: Some(crate::core::protocols::common::TelemetryType::Telemetry),
                channel_id: None,
            };
            points.push(point_data);
        }

        points
    }

    async fn update_status(&mut self, status: ChannelStatus) -> Result<()> {
        // Update connection status
        *self.connected.write().await = status.connected;
        Ok(())
    }

    async fn read_point(&self, point_id: &str) -> Result<PointData> {
        // Find point in mappings
        let mappings = self.message_mappings.read().await;
        if let Some(mapping) = mappings.iter().find(|m| m.name == point_id) {
            Ok(PointData {
                id: mapping.name.clone(),
                name: mapping
                    .display_name
                    .clone()
                    .unwrap_or_else(|| mapping.name.clone()),
                value: "0".to_string(), // Default value, would be from actual CAN data
                timestamp: Utc::now(),
                unit: mapping.unit.clone().unwrap_or_default(),
                description: mapping.description.clone().unwrap_or_default(),
                telemetry_type: Some(crate::core::protocols::common::TelemetryType::Telemetry),
                channel_id: None,
            })
        } else {
            Err(ComSrvError::InvalidParameter(format!(
                "Point {} not found",
                point_id
            )))
        }
    }

    async fn write_point(&mut self, point_id: &str, value: &str) -> Result<()> {
        // Find mapping and send CAN message
        let mappings = self.message_mappings.read().await;
        if let Some(mapping) = mappings.iter().find(|m| m.name == point_id) {
            info!(
                "Writing CAN point {} = {} to ID 0x{:X}",
                point_id, value, mapping.can_id
            );
            // TODO: Encode value and send CAN frame
            Ok(())
        } else {
            Err(ComSrvError::InvalidParameter(format!(
                "Point {} not found",
                point_id
            )))
        }
    }

    async fn get_diagnostics(&self) -> HashMap<String, String> {
        let mut diag = HashMap::new();
        diag.insert("protocol".to_string(), "CAN".to_string());
        diag.insert(
            "interface".to_string(),
            format!("{:?}", self.interface_type),
        );
        diag.insert("bit_rate".to_string(), format!("{:?}", self.bit_rate));
        diag.insert("connected".to_string(), self.is_running().await.to_string());

        let stats = self.statistics.read().await;
        diag.insert(
            "messages_sent".to_string(),
            stats.get_messages_sent().to_string(),
        );
        diag.insert(
            "messages_received".to_string(),
            stats.get_messages_received().to_string(),
        );
        diag.insert(
            "error_messages".to_string(),
            stats.get_error_messages().to_string(),
        );
        diag.insert(
            "bus_utilization".to_string(),
            format!("{:.2}%", stats.get_bus_utilization() * 100.0),
        );
        diag
    }
}

// Placeholder CanClient implementation - would need actual CAN interface integration
#[async_trait]
impl CanClient for CanClientBase {
    async fn send_frame(&self, frame: &CanFrame) -> Result<()> {
        // This would be implemented with actual CAN interface
        debug!(
            "Sending CAN frame: ID=0x{:X}, len={}",
            frame.id.raw(),
            frame.data.len()
        );

        // Update statistics
        self.update_statistics(1, 0, 0).await;

        Ok(())
    }

    async fn receive_frames(
        &self,
        filter_ids: Option<&[u32]>,
        _timeout_ms: u64,
    ) -> Result<Vec<CanFrame>> {
        // This would be implemented with actual CAN interface
        debug!("Receiving CAN frames with filter: {filter_ids:?}");

        // Return empty for now - actual implementation would read from CAN interface
        Ok(Vec::new())
    }

    async fn read_data(&self, mapping: &CanMessageMapping) -> Result<serde_json::Value> {
        // This would typically receive a frame with the specified CAN ID
        // For now, return a mock value
        debug!("Reading CAN data for mapping: {}", mapping.name);

        Ok(serde_json::Value::Null)
    }

    async fn write_data(
        &self,
        mapping: &CanMessageMapping,
        value: &serde_json::Value,
    ) -> Result<()> {
        // This would encode the value into a CAN frame and send it
        debug!(
            "Writing CAN data for mapping: {}, value: {:?}",
            mapping.name, value
        );

        Ok(())
    }

    async fn get_statistics(&self) -> CanStatistics {
        self.statistics.read().await.clone()
    }

    async fn reset_statistics(&self) {
        let mut stats = self.statistics.write().await;
        *stats = CanStatistics::default();
    }
}

#[cfg(test)]
mod tests {
    use super::super::frame::CanFrame;
    use super::*;
    use crate::core::config::ProtocolType;

    fn create_test_can_config() -> ChannelConfig {
        let mut parameters = std::collections::HashMap::new();
        parameters.insert(
            "interface".to_string(),
            serde_yaml::Value::String("socketcan:can0".to_string()),
        );
        parameters.insert(
            "bit_rate".to_string(),
            serde_yaml::Value::Number(serde_yaml::Number::from(500000)),
        );
        parameters.insert(
            "timeout".to_string(),
            serde_yaml::Value::Number(serde_yaml::Number::from(1000)),
        );

        ChannelConfig {
            id: 1,
            name: "Test CAN Channel".to_string(),
            description: Some("Test channel for CAN client".to_string()),
            protocol: "can".to_string(),
            parameters,
            logging: Default::default(),
            table_config: None,
            points: Vec::new(),
            combined_points: Vec::new(),
        }
    }

    #[tokio::test]
    async fn test_can_client_creation() {
        let config = create_test_can_config();
        let client = CanClientBase::new("TestCANClient", config);

        assert_eq!(client.name(), "TestCANClient");
        assert_eq!(client.bit_rate(), CanBitRate::Kbps500);
        assert_eq!(client.timeout_ms(), 1000);
        assert!(matches!(
            client.interface_type(),
            CanInterfaceType::SocketCan(_)
        ));
    }

    #[tokio::test]
    async fn test_data_extraction() {
        let config = create_test_can_config();
        let client = CanClientBase::new("TestCANClient", config);

        // Create test frame
        let frame = CanFrame {
            id: CanId::Standard(0x123),
            data: vec![0x12, 0x34, 0x56, 0x78],
            rtr: false,
            err: false,
        };

        // Create test mapping for UInt16 extraction
        let mapping = CanMessageMapping {
            name: "test_value".to_string(),
            display_name: Some("Test Value".to_string()),
            can_id: 0x123,
            frame_format: CanFrameFormat::Standard,
            data_config: CanDataConfig {
                data_type: CanDataType::UInt16,
                start_byte: 0,
                bit_offset: 0,
                bit_length: 16,
                byte_order: CanByteOrder::BigEndian,
            },
            scale: 1.0,
            offset: 0.0,
            unit: None,
            description: None,
            access_mode: "read".to_string(),
            transmission_rate: 0.0,
        };

        let result = client.extract_data(&frame, &mapping);
        assert!(result.is_ok());

        let value = result.unwrap();
        if let serde_json::Value::Number(num) = value {
            assert_eq!(num.as_u64().unwrap(), 0x1234);
        } else {
            panic!("Expected numeric value");
        }
    }

    #[tokio::test]
    async fn test_message_mapping_management() {
        let config = create_test_can_config();
        let client = CanClientBase::new("TestCANClient", config);

        let mappings = vec![CanMessageMapping {
            name: "engine_rpm".to_string(),
            display_name: Some("Engine RPM".to_string()),
            can_id: 0x123,
            frame_format: CanFrameFormat::Standard,
            data_config: CanDataConfig {
                data_type: CanDataType::UInt16,
                start_byte: 0,
                bit_offset: 0,
                bit_length: 16,
                byte_order: CanByteOrder::BigEndian,
            },
            scale: 0.25,
            offset: 0.0,
            unit: Some("RPM".to_string()),
            description: Some("Engine speed".to_string()),
            access_mode: "read".to_string(),
            transmission_rate: 10.0,
        }];

        client.load_message_mappings(mappings.clone()).await;

        let loaded_mappings = client.get_message_mappings().await;
        assert_eq!(loaded_mappings.len(), 1);
        assert_eq!(loaded_mappings[0].name, "engine_rpm");

        let found_mapping = client.find_mapping("engine_rpm").await;
        assert!(found_mapping.is_some());
        assert_eq!(found_mapping.unwrap().can_id, 0x123);
    }

    #[tokio::test]
    async fn test_statistics() {
        let config = create_test_can_config();
        let client = CanClientBase::new("TestCANClient", config);

        let initial_stats = client.get_statistics().await;
        assert_eq!(initial_stats.messages_sent.load(Ordering::SeqCst), 0);
        assert_eq!(initial_stats.messages_received.load(Ordering::SeqCst), 0);

        client.update_statistics(5, 10, 1).await;

        let updated_stats = client.get_statistics().await;
        assert_eq!(updated_stats.messages_sent.load(Ordering::SeqCst), 5);
        assert_eq!(updated_stats.messages_received.load(Ordering::SeqCst), 10);
        assert_eq!(updated_stats.error_messages.load(Ordering::SeqCst), 1);
        assert!(updated_stats.last_error_time.is_some());

        client.reset_statistics().await;

        let reset_stats = client.get_statistics().await;
        assert_eq!(reset_stats.messages_sent.load(Ordering::SeqCst), 0);
        assert_eq!(reset_stats.messages_received.load(Ordering::SeqCst), 0);
        assert_eq!(reset_stats.error_messages.load(Ordering::SeqCst), 0);
    }
}
