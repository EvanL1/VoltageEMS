use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use std::collections::HashMap;

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

/// channel status response
#[derive(Debug, Clone, Serialize)]
pub struct ChannelStatus {
    pub id: String,
    pub name: String,
    pub protocol: String,
    pub connected: bool,
    pub last_response_time: f64,
    pub last_error: String,
    pub last_update_time: DateTime<Utc>,
    pub parameters: HashMap<String, serde_json::Value>,
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
    pub operation: String,  // "start", "stop", "restart"
}

/// point value read response
#[derive(Debug, Clone, Serialize)]
pub struct PointValue {
    pub name: String,
    pub value: serde_json::Value,
    pub quality: bool,
    pub timestamp: DateTime<Utc>,
}

/// point table data response containing all points
#[derive(Debug, Clone, Serialize)]
pub struct PointTableData {
    pub channel_id: String,
    pub points: Vec<PointValue>,
    pub timestamp: DateTime<Utc>,
}

/// point value write request
#[derive(Debug, Clone, Deserialize)]
pub struct WritePointRequest {
    pub value: serde_json::Value,
}

/// error response
#[derive(Debug, Clone, Serialize)]
pub struct ErrorResponse {
    pub status: u16,
    pub message: String,
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
    use serde_json::{json, Value};

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
            id: "channel_1".to_string(),
            name: "Test Channel".to_string(),
            protocol: "ModbusTcp".to_string(),
            connected: true,
            last_response_time: 123.456,
            last_error: "".to_string(),
            last_update_time: now,
            parameters,
        };

        let serialized = serde_json::to_string(&status).unwrap();
        assert!(serialized.contains("channel_1"));
        assert!(serialized.contains("ModbusTcp"));
        assert!(serialized.contains("true"));
    }

    #[test]
    fn test_health_status_serialization() {
        let health = HealthStatus {
            status: "OK".to_string(),
            uptime: 7200,
            memory_usage: 1024000,
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
    fn test_point_value_serialization() {
        let now = Utc::now();
        let point = PointValue {
            name: "temperature".to_string(),
            value: json!(23.5),
            quality: true,
            timestamp: now,
        };

        let serialized = serde_json::to_string(&point).unwrap();
        assert!(serialized.contains("temperature"));
        assert!(serialized.contains("23.5"));
        assert!(serialized.contains("true"));
    }

    #[test]
    fn test_point_table_data_serialization() {
        let now = Utc::now();
        let points = vec![
            PointValue {
                name: "point1".to_string(),
                value: json!(100),
                quality: true,
                timestamp: now,
            },
            PointValue {
                name: "point2".to_string(),
                value: json!("active"),
                quality: false,
                timestamp: now,
            },
        ];

        let table_data = PointTableData {
            channel_id: "channel_1".to_string(),
            points,
            timestamp: now,
        };

        let serialized = serde_json::to_string(&table_data).unwrap();
        assert!(serialized.contains("channel_1"));
        assert!(serialized.contains("point1"));
        assert!(serialized.contains("point2"));
    }

    #[test]
    fn test_write_point_request_deserialization() {
        let json_data = r#"{"value": 42}"#;
        let request: WritePointRequest = serde_json::from_str(json_data).unwrap();
        assert_eq!(request.value, json!(42));

        let json_data = r#"{"value": "hello"}"#;
        let request: WritePointRequest = serde_json::from_str(json_data).unwrap();
        assert_eq!(request.value, json!("hello"));

        let json_data = r#"{"value": true}"#;
        let request: WritePointRequest = serde_json::from_str(json_data).unwrap();
        assert_eq!(request.value, json!(true));
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
    fn test_complex_point_value_types() {
        let now = Utc::now();
        
        // Test with different value types
        let int_point = PointValue {
            name: "int_value".to_string(),
            value: json!(42),
            quality: true,
            timestamp: now,
        };

        let float_point = PointValue {
            name: "float_value".to_string(),
            value: json!(3.14159),
            quality: true,
            timestamp: now,
        };

        let bool_point = PointValue {
            name: "bool_value".to_string(),
            value: json!(false),
            quality: true,
            timestamp: now,
        };

        let string_point = PointValue {
            name: "string_value".to_string(),
            value: json!("test string"),
            quality: true,
            timestamp: now,
        };

        let array_point = PointValue {
            name: "array_value".to_string(),
            value: json!([1, 2, 3, 4, 5]),
            quality: true,
            timestamp: now,
        };

        // All should serialize without error
        assert!(serde_json::to_string(&int_point).is_ok());
        assert!(serde_json::to_string(&float_point).is_ok());
        assert!(serde_json::to_string(&bool_point).is_ok());
        assert!(serde_json::to_string(&string_point).is_ok());
        assert!(serde_json::to_string(&array_point).is_ok());
    }

    #[test]
    fn test_channel_status_with_empty_parameters() {
        let now = Utc::now();
        let status = ChannelStatus {
            id: "simple_channel".to_string(),
            name: "Simple Channel".to_string(),
            protocol: "Virtual".to_string(),
            connected: false,
            last_response_time: 0.0,
            last_error: "Connection timeout".to_string(),
            last_update_time: now,
            parameters: HashMap::new(),
        };

        let serialized = serde_json::to_string(&status).unwrap();
        assert!(serialized.contains("simple_channel"));
        assert!(serialized.contains("false"));
        assert!(serialized.contains("Connection timeout"));
    }
} 