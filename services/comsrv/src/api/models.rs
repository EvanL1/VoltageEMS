use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// 新的简化模型
// ============================================================================

/// 控制命令（遥控）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlCommand {
    pub point_id: u32,
    pub value: u8, // 0 或 1
}

/// 调节命令（遥调）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdjustmentCommand {
    pub point_id: u32,
    pub value: f64,
}

/// service status response
#[derive(Debug, Clone, Serialize)]
pub struct ServiceStatus {
    pub name: String,
    pub version: String,
    pub uptime: u64,
    pub start_time: DateTime<Utc>,
    pub channels: u32,
    pub active_channels: u32,
}

/// channel status response for list endpoint
#[derive(Debug, Clone, Serialize)]
pub struct ChannelStatusResponse {
    pub id: u16,
    pub name: String,
    pub protocol: String,
    pub connected: bool,
    pub last_update: DateTime<Utc>,
    pub error_count: u32,
    pub last_error: Option<String>,
}

/// channel status response - Enhanced version combining API and `ComBase` requirements
#[derive(Debug, Clone, Serialize)]
pub struct ChannelStatus {
    pub id: u16,
    pub name: String,
    pub protocol: String,
    pub connected: bool,
    pub running: bool,
    pub last_update: DateTime<Utc>,
    pub error_count: u32,
    pub last_error: Option<String>,
    pub statistics: HashMap<String, serde_json::Value>,
}

impl From<crate::core::combase::ChannelStatus> for ChannelStatus {
    /// Convert from `ComBase` `ChannelStatus` to API `ChannelStatus`
    fn from(status: crate::core::combase::ChannelStatus) -> Self {
        Self {
            id: 0,                           // Will be filled by handler
            name: "Unknown".to_string(),     // Will be filled by handler
            protocol: "Unknown".to_string(), // Will be filled by handler
            connected: status.is_connected,
            running: status.is_connected, // Use is_connected as running status
            last_update: DateTime::<Utc>::from_timestamp(
                status.last_update.try_into().unwrap_or(0),
                0,
            )
            .unwrap_or_else(Utc::now),
            error_count: status.error_count.try_into().unwrap_or(u32::MAX),
            last_error: status.last_error,
            statistics: HashMap::new(), // Will be filled by handler
        }
    }
}

/// service health status
#[derive(Debug, Clone, Serialize)]
pub struct HealthStatus {
    pub status: String,
    pub uptime: u64,
    pub memory_usage: u64,
    pub cpu_usage: f64,
}

/// channel operation request
#[derive(Debug, Clone, Deserialize)]
pub struct ChannelOperation {
    pub operation: String, // "start", "stop", "restart"
}

/// error response
#[derive(Debug, Clone, Serialize)]
pub struct ErrorResponse {
    pub status: u16,
    pub message: String,
}

/// Protocol factory information
#[derive(Debug, Clone, Serialize)]
pub struct ProtocolFactoryInfo {
    pub protocol_type: String,
    pub supported: bool,
    pub default_config: Option<serde_json::Value>,
    pub config_schema: Option<serde_json::Value>,
}

/// Protocol factory status
#[derive(Debug, Clone, Serialize)]
pub struct ProtocolFactoryStatus {
    pub supported_protocols: Vec<String>,
    pub total_channels: u32,
    pub active_channels: u32,
    pub channel_distribution: HashMap<String, u32>, // protocol -> count
}

/// Channel creation request
#[derive(Debug, Clone, Deserialize)]
pub struct ChannelCreateRequest {
    pub name: String,
    pub description: String,
    pub protocol: String,
    pub parameters: HashMap<String, serde_json::Value>,
}

/// Channel configuration update request
#[derive(Debug, Clone, Deserialize)]
pub struct ChannelConfigUpdateRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub parameters: Option<HashMap<String, serde_json::Value>>,
}

/// API response wrapper
#[derive(Debug, Clone, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn error(message: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use serde_json::json;

    #[test]
    fn test_service_status_serialization() {
        let start_time = Utc::now();
        let status = ServiceStatus {
            name: "TestService".to_string(),
            version: "1.0.0".to_string(),
            uptime: 3600,
            start_time,
            channels: 5,
            active_channels: 3,
        };

        let serialized = serde_json::to_string(&status).unwrap();
        assert!(serialized.contains("TestService"));
        assert!(serialized.contains("1.0.0"));
        assert!(serialized.contains("3600"));
    }

    #[test]
    fn test_channel_status_serialization() {
        let now = Utc::now();
        let mut parameters = HashMap::new();
        parameters.insert("timeout".to_string(), json!(5000));
        parameters.insert("slave_id".to_string(), json!(1));

        let status = ChannelStatus {
            id: 1,
            name: "Test Channel".to_string(),
            protocol: "ModbusTcp".to_string(),
            connected: true,
            running: true,
            last_update: now,
            error_count: 0,
            last_error: None,
            statistics: parameters,
        };

        let serialized = serde_json::to_string(&status).unwrap();
        assert!(serialized.contains('1'));
        assert!(serialized.contains("ModbusTcp"));
        assert!(serialized.contains("true"));
    }

    #[test]
    fn test_health_status_serialization() {
        let health = HealthStatus {
            status: "OK".to_string(),
            uptime: 7200,
            memory_usage: 1_024_000,
            cpu_usage: 15.5,
        };

        let serialized = serde_json::to_string(&health).unwrap();
        assert!(serialized.contains("OK"));
        assert!(serialized.contains("7200"));
        assert!(serialized.contains("15.5"));
    }

    #[test]
    fn test_channel_operation_deserialization() {
        let json_data = r#"{"operation": "start"}"#;
        let operation: ChannelOperation = serde_json::from_str(json_data).unwrap();
        assert_eq!(operation.operation, "start");

        let json_data = r#"{"operation": "stop"}"#;
        let operation: ChannelOperation = serde_json::from_str(json_data).unwrap();
        assert_eq!(operation.operation, "stop");

        let json_data = r#"{"operation": "restart"}"#;
        let operation: ChannelOperation = serde_json::from_str(json_data).unwrap();
        assert_eq!(operation.operation, "restart");
    }

    #[test]
    fn test_error_response_serialization() {
        let error = ErrorResponse {
            status: 404,
            message: "Not found".to_string(),
        };

        let serialized = serde_json::to_string(&error).unwrap();
        assert!(serialized.contains("404"));
        assert!(serialized.contains("Not found"));
    }

    #[test]
    fn test_api_response_success() {
        let data = "test data".to_string();
        let response = ApiResponse::success(data);

        assert!(response.success);
        assert!(response.data.is_some());
        assert_eq!(response.data.clone().unwrap(), "test data");
        assert!(response.error.is_none());

        let serialized = serde_json::to_string(&response).unwrap();
        assert!(serialized.contains("true"));
        assert!(serialized.contains("test data"));
    }

    #[test]
    fn test_api_response_error() {
        let response: ApiResponse<String> = ApiResponse::error("Something went wrong".to_string());

        assert!(!response.success);
        assert!(response.data.is_none());
        assert!(response.error.is_some());
        assert_eq!(response.error.clone().unwrap(), "Something went wrong");

        let serialized = serde_json::to_string(&response).unwrap();
        assert!(serialized.contains("false"));
        assert!(serialized.contains("Something went wrong"));
    }

    #[test]
    fn test_channel_status_with_empty_parameters() {
        let now = Utc::now();
        let status = ChannelStatus {
            id: 1,
            name: "Simple Channel".to_string(),
            protocol: "Virtual".to_string(),
            connected: false,
            running: false,
            last_update: now,
            error_count: 0,
            last_error: Some("Connection timeout".to_string()),
            statistics: HashMap::new(),
        };

        let serialized = serde_json::to_string(&status).unwrap();
        assert!(serialized.contains('1'));
        assert!(serialized.contains("false"));
        assert!(serialized.contains("Connection timeout"));
    }

    #[test]
    fn test_combase_channel_status_conversion() {
        let combase_status = crate::core::combase::ChannelStatus {
            is_connected: true,
            last_error: Some("Test error".to_string()),
            last_update: 1_234_567_890,
            success_count: 100,
            error_count: 5,
            reconnect_count: 2,
            points_count: 50,
            last_read_duration_ms: Some(100),
            average_read_duration_ms: Some(95.5),
        };
        let api_status = ChannelStatus::from(combase_status);

        assert_eq!(api_status.id, 0); // Default value
        assert_eq!(api_status.name, "Unknown");
        assert_eq!(api_status.protocol, "Unknown");
        assert!(api_status.connected);
        assert_eq!(api_status.error_count, 5);
        assert_eq!(api_status.last_error, Some("Test error".to_string()));
        assert!(api_status.statistics.is_empty());
    }

    #[test]
    fn test_protocol_factory_info_serialization() {
        let factory_info = ProtocolFactoryInfo {
            protocol_type: "ModbusTcp".to_string(),
            supported: true,
            default_config: Some(json!({"host": "127.0.0.1", "port": 502})),
            config_schema: Some(json!({"type": "object", "properties": {}})),
        };

        let serialized = serde_json::to_string(&factory_info).unwrap();
        assert!(serialized.contains("ModbusTcp"));
        assert!(serialized.contains("true"));
        assert!(serialized.contains("127.0.0.1"));
    }

    #[test]
    fn test_protocol_factory_status_serialization() {
        let mut distribution = HashMap::new();
        distribution.insert("ModbusTcp".to_string(), 3);
        distribution.insert("ModbusRtu".to_string(), 2);

        let factory_status = ProtocolFactoryStatus {
            supported_protocols: vec!["ModbusTcp".to_string(), "ModbusRtu".to_string()],
            total_channels: 5,
            active_channels: 4,
            channel_distribution: distribution,
        };

        let serialized = serde_json::to_string(&factory_status).unwrap();
        assert!(serialized.contains("ModbusTcp"));
        assert!(serialized.contains("ModbusRtu"));
        assert!(serialized.contains("\"total_channels\":5"));
        assert!(serialized.contains("\"active_channels\":4"));
    }

    // #[test]
    // fn test_channel_create_request_deserialization() {
    //     let json_data = r#"{
    //         "name": "Test Channel",
    //         "description": "Test channel for Modbus TCP",
    //         "protocol": "ModbusTcp",
    //         "parameters": {
    //             "host": "192.168.1.100",
    //             "port": 502,
    //             "slave_id": 1
    //         }
    //     }"#;
    //
    //     let request: ChannelCreateRequest = serde_json::from_str(json_data).unwrap();
    //     assert_eq!(request.name, "Test Channel");
    //     assert_eq!(request.description, "Test channel for Modbus TCP");
    //     assert_eq!(request.protocol, "ModbusTcp");
    //     assert_eq!(request.parameters.len(), 3);
    //     assert_eq!(
    //         request.parameters.get("host"),
    //         Some(&json!("192.168.1.100"))
    //     );
    //     assert_eq!(request.parameters.get("port"), Some(&json!(502)));
    //     assert_eq!(request.parameters.get("slave_id"), Some(&json!(1)));
    // }

    #[test]
    fn test_channel_config_update_request_deserialization() {
        let json_data = r#"{
            "name": "Updated Channel Name",
            "parameters": {
                "timeout": 5000
            }
        }"#;

        let request: ChannelConfigUpdateRequest = serde_json::from_str(json_data).unwrap();
        assert_eq!(request.name, Some("Updated Channel Name".to_string()));
        assert!(request.description.is_none());
        assert!(request.parameters.is_some());

        let params = request.parameters.unwrap();
        assert_eq!(params.get("timeout"), Some(&json!(5000)));
    }
}
