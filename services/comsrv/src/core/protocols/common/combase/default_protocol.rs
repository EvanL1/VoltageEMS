//! Default Protocol Implementation
//!
//! This module contains the default reference implementation of the ComBase trait.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::info;

use super::data_types::{ChannelStatus, PointData};
use super::traits::{ComBase, ProtocolLogger};
use crate::core::config::ChannelConfig;
use crate::utils::{ComSrvError, Result};

/// Packet parsing result
#[derive(Debug, Clone)]
pub struct PacketParseResult {
    pub success: bool,
    pub protocol: String,
    pub direction: String,
    pub hex_data: String,
    pub parsed_data: Option<String>,
    pub error_message: Option<String>,
    pub timestamp: DateTime<Utc>,
}

impl PacketParseResult {
    pub fn success(protocol: &str, direction: &str, hex_data: &str, parsed_data: &str) -> Self {
        Self {
            success: true,
            protocol: protocol.to_string(),
            direction: direction.to_string(),
            hex_data: hex_data.to_string(),
            parsed_data: Some(parsed_data.to_string()),
            error_message: None,
            timestamp: Utc::now(),
        }
    }

    pub fn failure(protocol: &str, direction: &str, hex_data: &str, error: &str) -> Self {
        Self {
            success: false,
            protocol: protocol.to_string(),
            direction: direction.to_string(),
            hex_data: hex_data.to_string(),
            parsed_data: None,
            error_message: Some(error.to_string()),
            timestamp: Utc::now(),
        }
    }
}

/// Default implementation of ComBase trait
pub struct DefaultProtocol {
    /// Service name
    name: String,
    /// Protocol type
    protocol_type: String,
    /// Channel configuration
    config: ChannelConfig,
    /// Running state
    running: Arc<RwLock<bool>>,
    /// Channel status
    status: Arc<RwLock<ChannelStatus>>,
    /// Protocol logger
    logger: Option<Arc<dyn ProtocolLogger>>,
}

impl std::fmt::Debug for DefaultProtocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DefaultProtocol")
            .field("name", &self.name)
            .field("protocol_type", &self.protocol_type)
            .field("running", &self.running)

            .field("logger", &"<logger>")
            .finish()
    }
}

impl DefaultProtocol {
    /// Create a new default protocol implementation
    pub fn new(name: &str, protocol_type: &str, config: ChannelConfig) -> Self {
        let channel_id = config.id.to_string();
        Self {
            name: name.to_string(),
            protocol_type: protocol_type.to_string(),
            config,
            running: Arc::new(RwLock::new(false)),
            status: Arc::new(RwLock::new(ChannelStatus::new(&channel_id))),
            logger: None,
        }
    }

    /// Set protocol logger
    pub fn set_logger(&mut self, logger: Arc<dyn ProtocolLogger>) {
        self.logger = Some(logger);
    }

    /// Measure execution time for an async operation
    pub async fn measure_execution<F, Fut, R>(&self, operation: F) -> (R, Duration)
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = R>,
    {
        let start = Instant::now();
        let result = operation().await;
        let duration = start.elapsed();
        (result, duration)
    }

    /// Measure execution time and return result
    pub async fn measure_result_execution<F, Fut, R>(&self, operation: F) -> Result<(R, Duration)>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<R>>,
    {
        let start = Instant::now();
        let result = operation().await?;
        let duration = start.elapsed();
        Ok((result, duration))
    }

    /// Set error status
    pub async fn set_error(&mut self, error_message: &str) {
        let mut status = self.status.write();
        status.last_error = error_message.to_string();
        status.last_update_time = Utc::now();
    }
}

#[async_trait]
impl ComBase for DefaultProtocol {

    fn name(&self) -> &str {
        &self.name
    }

    fn channel_id(&self) -> String {
        self.config.id.to_string()
    }

    fn protocol_type(&self) -> &str {
        &self.protocol_type
    }

    fn get_parameters(&self) -> HashMap<String, String> {
        // Extract parameters from config
        let mut params = HashMap::new();
        params.insert("name".to_string(), self.name.clone());
        params.insert("protocol".to_string(), self.protocol_type.clone());
        params.insert("channel_id".to_string(), self.config.id.to_string());
        params
    }

    async fn is_running(&self) -> bool {
        *self.running.read()
    }

    async fn start(&mut self) -> Result<()> {
        info!("Starting communication service: {}", self.name);
        *self.running.write() = true;
        
        // Update status
        let mut status = self.status.write();
        status.connected = true;
        status.last_update_time = Utc::now();
        status.last_error.clear();
        
        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        info!("Stopping communication service: {}", self.name);
        *self.running.write() = false;
        
        // Update status
        let mut status = self.status.write();
        status.connected = false;
        status.last_update_time = Utc::now();
        
        Ok(())
    }

    async fn status(&self) -> ChannelStatus {
        self.status.read().clone()
    }

    async fn update_status(&mut self, new_status: ChannelStatus) -> Result<()> {
        *self.status.write() = new_status;
        Ok(())
    }

    async fn get_all_points(&self) -> Vec<PointData> {
        // Base implementation returns empty list
        // Protocol-specific implementations should override this
        Vec::new()
    }

    async fn read_point(&self, _point_id: &str) -> Result<PointData> {
        Err(ComSrvError::InvalidOperation(
            "Base implementation does not support point reading".to_string(),
        ))
    }

    async fn write_point(&mut self, _point_id: &str, _value: &str) -> Result<()> {
        Err(ComSrvError::InvalidOperation(
            "Base implementation does not support point writing".to_string(),
        ))
    }

    async fn get_diagnostics(&self) -> HashMap<String, String> {
        let mut diagnostics = HashMap::new();
        diagnostics.insert("service_name".to_string(), self.name.to_string());
        diagnostics.insert("protocol_type".to_string(), self.protocol_type.to_string());
        diagnostics.insert("running".to_string(), self.is_running().await.to_string());
        
        let status = self.status.read();
        diagnostics.insert("connected".to_string(), status.is_connected().to_string());
        diagnostics.insert("last_response_time".to_string(), status.response_time().to_string());
        diagnostics.insert("last_error".to_string(), status.error_ref().to_string());
        diagnostics.insert("last_update".to_string(), status.last_update().to_rfc3339());
        
        diagnostics
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::{ChannelConfig, ProtocolType, ChannelParameters};
    use crate::core::config::types::ChannelLoggingConfig;

    fn create_test_config() -> ChannelConfig {
        ChannelConfig {
            id: 1,
            name: "test_channel".to_string(),
            description: Some("Test channel".to_string()),
            protocol: ProtocolType::Virtual,
            parameters: ChannelParameters::Generic(HashMap::new()),
            logging: ChannelLoggingConfig::default(),
        }
    }

    #[tokio::test]
    async fn test_combase_impl_creation() {
        let config = create_test_config();
        let service = DefaultProtocol::new("Test Service", "test_protocol", config);
        
        assert_eq!(service.name(), "Test Service");
        assert_eq!(service.protocol_type(), "test_protocol");
        assert_eq!(service.channel_id(), "1");
        assert!(!service.is_running().await);
    }

    #[tokio::test]
    async fn test_combase_impl_lifecycle() {
        let config = create_test_config();
        let mut service = DefaultProtocol::new("Test Service", "test_protocol", config);
        
        // Test start
        service.start().await.unwrap();
        assert!(service.is_running().await);
        
        let status = service.status().await;
        assert!(status.connected);
        
        // Test stop
        service.stop().await.unwrap();
        assert!(!service.is_running().await);
        
        let status = service.status().await;
        assert!(!status.connected);
    }
} 