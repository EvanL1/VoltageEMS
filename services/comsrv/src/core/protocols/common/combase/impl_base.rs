//! ComBase Implementation
//!
//! This module contains the reference implementation of the ComBase trait.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::info;

use super::data_types::{ChannelStatus, PointData};
use super::traits::{ComBase, ProtocolLogger};
use super::point_manager::{UniversalPointManager, UniversalPointConfig};
use super::telemetry::TelemetryType;
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

/// Reference implementation of ComBase trait with integrated UniversalPointManager
pub struct ComBaseImpl {
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
    /// Universal point manager for unified data access (optional)
    point_manager: Option<UniversalPointManager>,
}

impl std::fmt::Debug for ComBaseImpl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ComBaseImpl")
            .field("name", &self.name)
            .field("protocol_type", &self.protocol_type)
            .field("channel_id", &self.config.id)
            .field("running", &self.running)
            .field("has_logger", &self.logger.is_some())
            .field("has_point_manager", &self.point_manager.is_some())
            .finish()
    }
}

impl ComBaseImpl {
    /// Create a new ComBase implementation with UniversalPointManager
    pub fn new_with_point_manager(name: &str, protocol_type: &str, config: ChannelConfig) -> Self {
        let channel_id = config.id.to_string();
        let point_manager = Some(UniversalPointManager::new(channel_id.clone()));
        
        Self {
            name: name.to_string(),
            protocol_type: protocol_type.to_string(),
            config,
            running: Arc::new(RwLock::new(false)),
            status: Arc::new(RwLock::new(ChannelStatus::new(&channel_id))),
            logger: None,
            point_manager,
        }
    }

    /// Create a new ComBase implementation without UniversalPointManager (for legacy protocols)
    pub fn new(name: &str, protocol_type: &str, config: ChannelConfig) -> Self {
        let channel_id = config.id.to_string();
        Self {
            name: name.to_string(),
            protocol_type: protocol_type.to_string(),
            config,
            running: Arc::new(RwLock::new(false)),
            status: Arc::new(RwLock::new(ChannelStatus::new(&channel_id))),
            logger: None,
            point_manager: None,
        }
    }

    /// Load point configurations into the universal point manager
    pub async fn load_point_configs(&self, configs: Vec<UniversalPointConfig>) -> Result<()> {
        if let Some(ref point_manager) = self.point_manager {
            point_manager.load_points(configs).await?;
            info!("Loaded {} point configurations for channel {}", 
                  point_manager.get_stats().await.total_points, 
                  point_manager.channel_id());
        } else {
            return Err(ComSrvError::InvalidOperation(
                "No point manager available for loading point configurations".to_string(),
            ));
        }
        Ok(())
    }

    /// Get the point manager if available
    pub fn get_point_manager_ref(&self) -> Option<&UniversalPointManager> {
        self.point_manager.as_ref()
    }

    /// Check if this implementation has a point manager
    pub fn has_point_manager(&self) -> bool {
        self.point_manager.is_some()
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
impl ComBase for ComBaseImpl {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

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
        let mut params = HashMap::new();
        params.insert("name".to_string(), self.name.clone());
        params.insert("protocol".to_string(), self.protocol_type.clone());
        params.insert("channel_id".to_string(), self.config.id.to_string());
        params.insert("has_point_manager".to_string(), self.has_point_manager().to_string());
        
        if let Some(ref point_manager) = self.point_manager {
            params.insert("point_manager_channel".to_string(), point_manager.channel_id().to_string());
        }
        
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
        if let Some(ref point_manager) = self.point_manager {
            point_manager.get_all_point_data().await
        } else {
            // Fallback to empty list for protocols without point manager
            Vec::new()
        }
    }

    async fn read_point(&self, point_id: &str) -> Result<PointData> {
        if let Some(ref point_manager) = self.point_manager {
            point_manager.get_point_data(point_id).await.ok_or_else(|| {
                ComSrvError::NotFound(format!("Point not found: {}", point_id))
            })
        } else {
            Err(ComSrvError::InvalidOperation(
                "No point manager available for point reading".to_string(),
            ))
        }
    }

    async fn write_point(&mut self, _point_id: &str, _value: &str) -> Result<()> {
        Err(ComSrvError::InvalidOperation(
            "Base implementation does not support point writing - protocols should override this method".to_string(),
        ))
    }

    async fn get_diagnostics(&self) -> HashMap<String, String> {
        let mut diagnostics = HashMap::new();
        diagnostics.insert("service_name".to_string(), self.name.to_string());
        diagnostics.insert("protocol_type".to_string(), self.protocol_type.to_string());
        diagnostics.insert("running".to_string(), self.is_running().await.to_string());
        diagnostics.insert("has_point_manager".to_string(), self.has_point_manager().to_string());
        
        // Extract status information before async operations to avoid Send trait issues
        let (connected, last_response_time, last_error, last_update) = {
            let status = self.status.read();
            (
                status.is_connected(),
                status.response_time(),
                status.error_ref().to_string(),
                status.last_update().to_rfc3339()
            )
        };
        
        diagnostics.insert("connected".to_string(), connected.to_string());
        diagnostics.insert("last_response_time".to_string(), last_response_time.to_string());
        diagnostics.insert("last_error".to_string(), last_error);
        diagnostics.insert("last_update".to_string(), last_update);
        
        // Add point manager diagnostics if available
        if let Some(ref point_manager) = self.point_manager {
            let stats = point_manager.get_stats().await;
            diagnostics.insert("total_points".to_string(), stats.total_points.to_string());
            diagnostics.insert("enabled_points".to_string(), stats.enabled_points.to_string());
            diagnostics.insert("read_operations".to_string(), stats.read_operations.to_string());
            diagnostics.insert("write_operations".to_string(), stats.write_operations.to_string());
            diagnostics.insert("validation_errors".to_string(), stats.validation_errors.to_string());
            diagnostics.insert("last_point_update".to_string(), stats.last_update.to_rfc3339());
        }
        
        diagnostics
    }

    /// Get the universal point manager if available
    async fn get_point_manager(&self) -> Option<UniversalPointManager> {
        self.point_manager.clone()
    }

    /// Get points by telemetry type using unified point manager
    async fn get_points_by_telemetry_type(&self, telemetry_type: &TelemetryType) -> Vec<PointData> {
        if let Some(ref point_manager) = self.point_manager {
            point_manager.get_point_data_by_type(telemetry_type).await
        } else {
            Vec::new()
        }
    }

    /// Get all point configurations using unified point manager
    async fn get_all_point_configs(&self) -> Vec<UniversalPointConfig> {
        if let Some(ref point_manager) = self.point_manager {
            point_manager.get_all_point_configs().await
        } else {
            Vec::new()
        }
    }

    /// Get enabled points by telemetry type using unified point manager
    async fn get_enabled_points_by_type(&self, telemetry_type: &TelemetryType) -> Vec<String> {
        if let Some(ref point_manager) = self.point_manager {
            point_manager.get_enabled_points_by_type(telemetry_type).await
        } else {
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::{ChannelConfig, ProtocolType, ChannelParameters};
    use crate::core::config::types::ChannelLoggingConfig;
    use crate::core::protocols::common::combase::telemetry::TelemetryType;

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
    async fn test_combase_impl_creation_with_point_manager() {
        let config = create_test_config();
        let service = ComBaseImpl::new_with_point_manager("Test Service", "test_protocol", config);
        
        assert_eq!(service.name(), "Test Service");
        assert_eq!(service.protocol_type(), "test_protocol");
        assert_eq!(service.channel_id(), "1");
        assert!(!service.is_running().await);
        assert!(service.has_point_manager());
        
        // Test unified point manager access
        let point_manager = service.get_point_manager().await;
        assert!(point_manager.is_some());
        
        let manager = point_manager.unwrap();
        assert_eq!(manager.channel_id(), "1");
    }

    #[tokio::test]
    async fn test_combase_impl_creation_without_point_manager() {
        let config = create_test_config();
        let service = ComBaseImpl::new("Test Service", "test_protocol", config);
        
        assert_eq!(service.name(), "Test Service");
        assert!(!service.has_point_manager());
        
        // Test that methods return empty/error when no point manager
        let all_points = service.get_all_points().await;
        assert!(all_points.is_empty());
        
        let point_result = service.read_point("test_point").await;
        assert!(point_result.is_err());
    }

    #[tokio::test]
    async fn test_load_point_configs() {
        let config = create_test_config();
        let service = ComBaseImpl::new_with_point_manager("Test Service", "test_protocol", config);
        
        let point_configs = vec![
            UniversalPointConfig::new(1001, "Temperature", TelemetryType::Telemetry),
            UniversalPointConfig::new(1002, "Pressure", TelemetryType::Telemetry),
        ];
        
        let result = service.load_point_configs(point_configs).await;
        assert!(result.is_ok());
        
        // Verify points were loaded
        let all_configs = service.get_all_point_configs().await;
        assert_eq!(all_configs.len(), 2);
        
        let stats = service.get_point_manager().await.unwrap().get_stats().await;
        assert_eq!(stats.total_points, 2);
        assert_eq!(stats.enabled_points, 2);
    }

    #[tokio::test]
    async fn test_get_points_by_telemetry_type() {
        let config = create_test_config();
        let service = ComBaseImpl::new_with_point_manager("Test Service", "test_protocol", config);
        
        let point_configs = vec![
            UniversalPointConfig::new(1001, "Temperature", TelemetryType::Telemetry),
            UniversalPointConfig::new(2001, "Pump Control", TelemetryType::Control),
        ];
        
        service.load_point_configs(point_configs).await.unwrap();
        
        // Test getting points by type
        let telemetry_points = service.get_points_by_telemetry_type(&TelemetryType::Telemetry).await;
        // Note: This will be empty until actual point data is added to cache
        // But the method should not error
        
        let enabled_telemetry = service.get_enabled_points_by_type(&TelemetryType::Telemetry).await;
        assert_eq!(enabled_telemetry.len(), 1);
        assert_eq!(enabled_telemetry[0], "1001");
        
        let enabled_control = service.get_enabled_points_by_type(&TelemetryType::Control).await;
        assert_eq!(enabled_control.len(), 1);
        assert_eq!(enabled_control[0], "2001");
    }

    #[tokio::test]
    async fn test_diagnostics_with_point_manager() {
        let config = create_test_config();
        let service = ComBaseImpl::new_with_point_manager("Test Service", "test_protocol", config);
        
        let diagnostics = service.get_diagnostics().await;
        assert_eq!(diagnostics.get("has_point_manager"), Some(&"true".to_string()));
        assert_eq!(diagnostics.get("total_points"), Some(&"0".to_string()));
        assert_eq!(diagnostics.get("enabled_points"), Some(&"0".to_string()));
    }

    #[tokio::test]
    async fn test_unified_data_access_interface() {
        let config = create_test_config();
        let service = ComBaseImpl::new_with_point_manager("Unified Test Service", "unified_protocol", config);
        
        // Test 1: Load diverse point configurations
        let point_configs = vec![
            UniversalPointConfig::new(1001, "Temperature Sensor", TelemetryType::Telemetry),
            UniversalPointConfig::new(1002, "Pressure Sensor", TelemetryType::Telemetry),
            UniversalPointConfig::new(2001, "Pump Status", TelemetryType::Signaling),
            UniversalPointConfig::new(2002, "Valve Status", TelemetryType::Signaling),
            UniversalPointConfig::new(3001, "Pump Control", TelemetryType::Control),
            UniversalPointConfig::new(4001, "Setpoint Adjust", TelemetryType::Setpoint),
        ];
        
        service.load_point_configs(point_configs).await.unwrap();
        
        // Test 2: Verify unified point access works
        let all_configs = service.get_all_point_configs().await;
        assert_eq!(all_configs.len(), 6);
        
        // Test 3: Query points by telemetry type
        let telemetry_points = service.get_enabled_points_by_type(&TelemetryType::Telemetry).await;
        assert_eq!(telemetry_points.len(), 2);
        assert!(telemetry_points.contains(&"1001".to_string()));
        assert!(telemetry_points.contains(&"1002".to_string()));
        
        let signaling_points = service.get_enabled_points_by_type(&TelemetryType::Signaling).await;
        assert_eq!(signaling_points.len(), 2);
        
        let control_points = service.get_enabled_points_by_type(&TelemetryType::Control).await;
        assert_eq!(control_points.len(), 1);
        assert_eq!(control_points[0], "3001");
        
        let setpoint_points = service.get_enabled_points_by_type(&TelemetryType::Setpoint).await;
        assert_eq!(setpoint_points.len(), 1);
        assert_eq!(setpoint_points[0], "4001");
        
        // Test 4: Check statistics
        let diagnostics = service.get_diagnostics().await;
        assert_eq!(diagnostics.get("total_points"), Some(&"6".to_string()));
        assert_eq!(diagnostics.get("enabled_points"), Some(&"6".to_string()));
        
        // Test 5: Test ComBase trait methods work with UniversalPointManager
        assert!(service.has_point_manager());
        let point_manager = service.get_point_manager().await;
        assert!(point_manager.is_some());
        
        let manager = point_manager.unwrap();
        let stats = manager.get_stats().await;
        assert_eq!(stats.total_points, 6);
        assert_eq!(stats.enabled_points, 6);
        
        // Verify points by type distribution
        assert_eq!(stats.points_by_type.get(&TelemetryType::Telemetry), Some(&2));
        assert_eq!(stats.points_by_type.get(&TelemetryType::Signaling), Some(&2));
        assert_eq!(stats.points_by_type.get(&TelemetryType::Control), Some(&1));
        assert_eq!(stats.points_by_type.get(&TelemetryType::Setpoint), Some(&1));
    }

    #[tokio::test]
    async fn test_legacy_protocol_compatibility() {
        // Test that protocols without UniversalPointManager still work
        let config = create_test_config();
        let service = ComBaseImpl::new("Legacy Protocol", "legacy", config);
        
        assert!(!service.has_point_manager());
        
        // These should return empty/error gracefully
        let all_points = service.get_all_points().await;
        assert!(all_points.is_empty());
        
        let all_configs = service.get_all_point_configs().await;
        assert!(all_configs.is_empty());
        
        let enabled_points = service.get_enabled_points_by_type(&TelemetryType::Telemetry).await;
        assert!(enabled_points.is_empty());
        
        let telemetry_data = service.get_points_by_telemetry_type(&TelemetryType::Telemetry).await;
        assert!(telemetry_data.is_empty());
        
        // Point reading should fail gracefully
        let read_result = service.read_point("any_point").await;
        assert!(read_result.is_err());
        
        // Load point configs should fail gracefully
        let load_result = service.load_point_configs(vec![]).await;
        assert!(load_result.is_err());
    }
} 