use crate::core::protocols::common::{
    combase::{
        FourTelemetryOperations, PointValueType, 
        RemoteOperationRequest, RemoteOperationResponse
    },
};
use crate::utils::error::Result;
use async_trait::async_trait;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::RwLock;
use std::sync::Arc;

/// Virtual Protocol Configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VirtualProtocolConfig {
    /// Protocol identifier
    pub protocol_id: String,
    /// Protocol name
    pub name: String,
    /// Update interval in milliseconds
    pub update_interval_ms: u64,
}

/// Virtual Protocol Client
pub struct VirtualProtocolClient {
    /// Protocol configuration
    config: VirtualProtocolConfig,
    /// Four telemetry tables
    tables: Arc<RwLock<FourTelemetryTables>>,
    /// Connection status
    is_connected: Arc<RwLock<bool>>,
}

/// Four Telemetry Tables for Virtual Protocol
#[derive(Debug, Default)]
pub struct FourTelemetryTables {
    /// Analog measurements (YC - 遥测)
    pub analog_measurements: HashMap<u32, f64>,
    /// Digital inputs (YX - 遥信)
    pub digital_inputs: HashMap<u32, bool>,
    /// Digital outputs (YK - 遥控)
    pub digital_outputs: HashMap<u32, bool>,
    /// Analog outputs (YT - 遥调)
    pub analog_outputs: HashMap<u32, f64>,
}

impl VirtualProtocolClient {
    /// Create new virtual protocol client
    pub fn new(config: VirtualProtocolConfig) -> Self {
        Self {
            config,
            tables: Arc::new(RwLock::new(FourTelemetryTables::default())),
            is_connected: Arc::new(RwLock::new(false)),
        }
    }
    
    /// Set an analog measurement value
    pub async fn set_analog_measurement(&self, point_id: u32, value: f64) {
        let mut tables = self.tables.write().await;
        tables.analog_measurements.insert(point_id, value);
    }
    
    /// Set a digital input value
    pub async fn set_digital_input(&self, point_id: u32, value: bool) {
        let mut tables = self.tables.write().await;
        tables.digital_inputs.insert(point_id, value);
    }
}

#[async_trait]
impl FourTelemetryOperations for VirtualProtocolClient {
    /// Read analog measurement values (遥测)
    async fn remote_measurement(&self, point_names: &[String]) -> Result<Vec<(String, PointValueType)>> {
        let tables = self.tables.read().await;
        let mut results = Vec::new();
        
        for point_name in point_names {
            // Extract point ID from name (assuming format like "point_1")
            if let Some(point_id) = extract_point_id(point_name) {
                if let Some(&value) = tables.analog_measurements.get(&point_id) {
                    results.push((point_name.clone(), PointValueType::Analog(value)));
                }
            }
        }
        
        Ok(results)
    }

    /// Read digital status values (遥信)
    async fn remote_signaling(&self, point_names: &[String]) -> Result<Vec<(String, PointValueType)>> {
        let tables = self.tables.read().await;
        let mut results = Vec::new();
        
        for point_name in point_names {
            if let Some(point_id) = extract_point_id(point_name) {
                if let Some(&value) = tables.digital_inputs.get(&point_id) {
                    results.push((point_name.clone(), PointValueType::Digital(value)));
                }
            }
        }
        
        Ok(results)
    }

    /// Execute digital control operations (遥控)
    async fn remote_control(&self, request: RemoteOperationRequest) -> Result<RemoteOperationResponse> {
        let mut tables = self.tables.write().await;
        
        if let Some(point_id) = extract_point_id(&request.point_name) {
            // For now, assume control is digital
            let control_value = true; // Simplified
            tables.digital_outputs.insert(point_id, control_value);
            
            return Ok(RemoteOperationResponse {
                operation_id: request.operation_id,
                success: true,
                error_message: None,
                actual_value: Some(PointValueType::Digital(control_value)),
                execution_time: Utc::now(),
            });
        }
        
        Ok(RemoteOperationResponse {
            operation_id: request.operation_id,
            success: false,
            error_message: Some("Invalid point name".to_string()),
            actual_value: None,
            execution_time: Utc::now(),
        })
    }

    /// Execute analog regulation operations (遥调)
    async fn remote_regulation(&self, request: RemoteOperationRequest) -> Result<RemoteOperationResponse> {
        let mut tables = self.tables.write().await;
        
        if let Some(point_id) = extract_point_id(&request.point_name) {
            // For now, assume regulation is analog
            let regulation_value = 0.0; // Simplified
            tables.analog_outputs.insert(point_id, regulation_value);
            
            return Ok(RemoteOperationResponse {
                operation_id: request.operation_id,
                success: true,
                error_message: None,
                actual_value: Some(PointValueType::Analog(regulation_value)),
                execution_time: Utc::now(),
            });
        }
        
        Ok(RemoteOperationResponse {
            operation_id: request.operation_id,
            success: false,
            error_message: Some("Invalid point name".to_string()),
            actual_value: None,
            execution_time: Utc::now(),
        })
    }

    /// Get all available remote control points (遥控点)
    async fn get_control_points(&self) -> Vec<String> {
        let tables = self.tables.read().await;
        tables.digital_outputs.keys()
            .map(|&id| format!("virtual_control_{}", id))
            .collect()
    }

    /// Get all available remote regulation points (遥调点)
    async fn get_regulation_points(&self) -> Vec<String> {
        let tables = self.tables.read().await;
        tables.analog_outputs.keys()
            .map(|&id| format!("virtual_regulation_{}", id))
            .collect()
    }

    /// Get all available measurement points (遥测点)
    async fn get_measurement_points(&self) -> Vec<String> {
        let tables = self.tables.read().await;
        tables.analog_measurements.keys()
            .map(|&id| format!("virtual_measurement_{}", id))
            .collect()
    }

    /// Get all available signaling points (遥信点)
    async fn get_signaling_points(&self) -> Vec<String> {
        let tables = self.tables.read().await;
        tables.digital_inputs.keys()
            .map(|&id| format!("virtual_signaling_{}", id))
            .collect()
    }
}

/// Extract point ID from point name
/// Assumes format like "virtual_measurement_1" or "point_1"
fn extract_point_id(point_name: &str) -> Option<u32> {
    point_name.split('_')
        .last()
        .and_then(|id_str| id_str.parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_virtual_protocol_measurements() {
        let config = VirtualProtocolConfig {
            protocol_id: "virtual_test".to_string(),
            name: "Virtual Test Protocol".to_string(),
            update_interval_ms: 1000,
        };
        
        let client = VirtualProtocolClient::new(config);
        
        // Set test data
        client.set_analog_measurement(1, 123.45).await;
        client.set_digital_input(2, true).await;
        
        // Test measurement reading
        let measurements = client.remote_measurement(&["virtual_measurement_1".to_string()]).await.unwrap();
        assert_eq!(measurements.len(), 1);
        assert_eq!(measurements[0].0, "virtual_measurement_1");
        if let PointValueType::Analog(value) = measurements[0].1 {
            assert_eq!(value, 123.45);
        } else {
            panic!("Expected analog value");
        }
        
        // Test signaling reading
        let signals = client.remote_signaling(&["virtual_signaling_2".to_string()]).await.unwrap();
        assert_eq!(signals.len(), 1);
        assert_eq!(signals[0].0, "virtual_signaling_2");
        if let PointValueType::Digital(value) = signals[0].1 {
            assert!(value);
        } else {
            panic!("Expected digital value");
        }
    }
    
    #[test]
    fn test_extract_point_id() {
        assert_eq!(extract_point_id("virtual_measurement_1"), Some(1));
        assert_eq!(extract_point_id("point_123"), Some(123));
        assert_eq!(extract_point_id("invalid"), None);
    }
} 