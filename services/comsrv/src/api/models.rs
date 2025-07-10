use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use utoipa::ToSchema;

/// service status response
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ServiceStatus {
    pub name: String,
    pub version: String,
    pub uptime: u64,
    pub start_time: DateTime<Utc>,
    pub channels: u32,
    pub active_channels: u32,
}

/// channel status response for list endpoint
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ChannelStatusResponse {
    pub id: u16,
    pub name: String,
    pub protocol: String,
    pub connected: bool,
    pub last_update: DateTime<Utc>,
    pub error_count: u32,
    pub last_error: Option<String>,
}

/// channel status response - Enhanced version combining API and ComBase requirements
#[derive(Debug, Clone, Serialize, ToSchema)]
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

impl From<crate::core::framework::ChannelStatus> for ChannelStatus {
    /// Convert from ComBase ChannelStatus to API ChannelStatus
    fn from(status: crate::core::framework::ChannelStatus) -> Self {
        Self {
            id: status.id.parse().unwrap_or(0), // Convert string ID to u16
            name: "Unknown".to_string(), // ComBase doesn't provide name, will be filled by handler
            protocol: "Unknown".to_string(), // ComBase doesn't provide protocol, will be filled by handler
            connected: status.connected,
            running: false, // Will be filled by handler
            last_update: status.last_update_time,
            error_count: if status.has_error() { 1 } else { 0 }, // Estimate from error state
            last_error: if status.has_error() {
                Some(status.last_error)
            } else {
                None
            },
            statistics: HashMap::new(), // ComBase doesn't provide statistics, will be filled by handler
        }
    }
}

/// service health status
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct HealthStatus {
    pub status: String,
    pub uptime: u64,
    pub memory_usage: u64,
    pub cpu_usage: f64,
}

/// channel operation request
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct ChannelOperation {
    pub operation: String, // "start", "stop", "restart"
}

/// point value read response
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PointValue {
    pub id: String,
    pub name: String,
    pub value: serde_json::Value,
    pub timestamp: DateTime<Utc>,
    pub unit: String,
    pub description: String,
}

impl From<crate::core::framework::types::PointData> for PointValue {
    /// Convert from protocols common PointData to API PointValue
    fn from(point: crate::core::framework::types::PointData) -> Self {
        Self {
            id: point.id,
            name: point.name,
            value: serde_json::Value::String(point.value),
            timestamp: point.timestamp,
            unit: point.unit,
            description: point.description,
        }
    }
}

/// point table data response containing all points
#[derive(Debug, Clone, Serialize)]
pub struct PointTableData {
    pub channel_id: String,
    pub points: Vec<PointValue>,
    pub timestamp: DateTime<Utc>,
}

/// point value write request
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct WritePointRequest {
    pub value: serde_json::Value,
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
#[derive(Debug, Clone, Serialize, ToSchema)]
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

/// Enhanced point with configuration information
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TelemetryPoint {
    /// Point ID from CSV configuration
    pub point_id: u32,
    /// Point name from CSV
    pub name: String,
    /// Description from CSV
    pub description: String,
    /// Engineering unit from CSV
    pub unit: String,
    /// Data type from CSV (uint16, float32, etc.)
    pub data_type: String,
    /// Scale factor from CSV
    pub scale: f64,
    /// Offset value from CSV
    pub offset: f64,
    /// Current real-time value (changes)
    pub current_value: Option<serde_json::Value>,
    /// Last update timestamp
    pub last_update: Option<DateTime<Utc>>,
    /// Point status (connected, error, etc.)
    pub status: String,
    /// Protocol mapping information (serialized as JSON)
    pub protocol_mapping: Option<serde_json::Value>,
}

/// Protocol mapping trait for different industrial protocols
pub trait ProtocolMapping: Send + Sync + std::fmt::Debug {
    /// Get protocol type name
    fn protocol_type(&self) -> &str;

    /// Get unique mapping identifier for this point
    fn mapping_id(&self) -> String;

    /// Get polling interval in milliseconds (if applicable)
    fn polling_interval(&self) -> Option<u32>;

    /// Get protocol-specific parameters as key-value pairs
    fn get_parameters(&self) -> std::collections::HashMap<String, String>;

    /// Serialize to JSON for API response
    fn to_json(&self) -> serde_json::Value;

    /// Validate mapping configuration
    fn validate(&self) -> Result<(), String>;
}

/// Default function code for Modbus (Read Holding Registers)
fn default_function_code() -> u8 {
    3
}

/// Modbus protocol mapping implementation
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ModbusMapping {
    /// Point ID that links to telemetry table
    pub point_id: u32,
    /// Register address for Modbus
    pub address: u32,
    /// Function code for Modbus (1=coils, 2=discrete, 3=holding, 4=input)
    #[serde(default = "default_function_code")]
    pub function_code: u8,
    /// Slave/Unit ID
    pub slave_id: Option<u8>,
    /// Data format (AB, ABCD, DCBA, etc.)
    pub data_format: String,
    /// Number of bytes/registers
    pub number_of_bytes: u16,
    /// Polling interval in milliseconds
    pub polling_interval: Option<u32>,
}

impl ProtocolMapping for ModbusMapping {
    fn protocol_type(&self) -> &str {
        "modbus"
    }

    fn mapping_id(&self) -> String {
        format!("modbus_{}_{}", self.slave_id.unwrap_or(1), self.address)
    }

    fn polling_interval(&self) -> Option<u32> {
        self.polling_interval
    }

    fn get_parameters(&self) -> std::collections::HashMap<String, String> {
        let mut params = std::collections::HashMap::new();
        params.insert("point_id".to_string(), self.point_id.to_string());
        params.insert("address".to_string(), self.address.to_string());
        params.insert("function_code".to_string(), self.function_code.to_string());
        if let Some(sid) = self.slave_id {
            params.insert("slave_id".to_string(), sid.to_string());
        }
        params.insert("data_format".to_string(), self.data_format.clone());
        params.insert(
            "number_of_bytes".to_string(),
            self.number_of_bytes.to_string(),
        );
        params
    }

    fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }

    fn validate(&self) -> Result<(), String> {
        if self.point_id == 0 {
            return Err("Point ID must be > 0".to_string());
        }
        if self.address > 65535 {
            return Err("Modbus address must be <= 65535".to_string());
        }
        // Modbus function codes: 1=ReadCoils, 2=ReadDiscreteInputs, 3=ReadHoldingRegisters, 4=ReadInputRegisters
        // 5=WriteSingleCoil, 6=WriteSingleRegister, 15=WriteMultipleCoils, 16=WriteMultipleRegisters
        match self.function_code {
            1..=6 | 15 | 16 => {} // Valid function codes
            _ => return Err("Invalid Modbus function code, supported: 1-6, 15, 16".to_string()),
        }
        if self.number_of_bytes == 0 || self.number_of_bytes > 125 {
            return Err("Number of bytes must be 1-125".to_string());
        }
        Ok(())
    }
}

/// CAN protocol mapping implementation
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CanMapping {
    /// Point ID that links to telemetry table
    pub point_id: u32,
    /// CAN ID (29-bit for extended, 11-bit for standard)
    pub can_id: u32,
    /// Extended CAN ID flag
    pub extended: bool,
    /// Data byte position (0-7)
    pub byte_position: u8,
    /// Bit position within byte (0-7, optional for digital signals)
    pub bit_position: Option<u8>,
    /// Data length (1, 2, 4, or 8 bytes for multi-byte values)
    pub data_length: u8,
    /// Byte order (big_endian or little_endian)
    pub byte_order: String,
    /// Message polling interval
    pub polling_interval: Option<u32>,
}

impl ProtocolMapping for CanMapping {
    fn protocol_type(&self) -> &str {
        "can"
    }

    fn mapping_id(&self) -> String {
        format!("can_{:X}_{}", self.can_id, self.byte_position)
    }

    fn polling_interval(&self) -> Option<u32> {
        self.polling_interval
    }

    fn get_parameters(&self) -> std::collections::HashMap<String, String> {
        let mut params = std::collections::HashMap::new();
        params.insert("point_id".to_string(), self.point_id.to_string());
        params.insert("can_id".to_string(), format!("0x{:X}", self.can_id));
        params.insert("extended".to_string(), self.extended.to_string());
        params.insert("byte_position".to_string(), self.byte_position.to_string());
        if let Some(bit_pos) = self.bit_position {
            params.insert("bit_position".to_string(), bit_pos.to_string());
        }
        params.insert("data_length".to_string(), self.data_length.to_string());
        params.insert("byte_order".to_string(), self.byte_order.clone());
        params
    }

    fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }

    fn validate(&self) -> Result<(), String> {
        if self.point_id == 0 {
            return Err("Point ID must be > 0".to_string());
        }
        if !self.extended && self.can_id > 0x7FF {
            return Err("Standard CAN ID must be <= 0x7FF".to_string());
        }
        if self.extended && self.can_id > 0x1FFFFFFF {
            return Err("Extended CAN ID must be <= 0x1FFFFFFF".to_string());
        }
        if self.byte_position > 7 {
            return Err("Byte position must be 0-7".to_string());
        }
        if let Some(bit_pos) = self.bit_position {
            if bit_pos > 7 {
                return Err("Bit position must be 0-7".to_string());
            }
        }
        if ![1, 2, 4, 8].contains(&self.data_length) {
            return Err("Data length must be 1, 2, 4, or 8 bytes".to_string());
        }
        Ok(())
    }
}

/// IEC 60870-5-104 protocol mapping implementation  
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct IecMapping {
    /// Point ID that links to telemetry table
    pub point_id: u32,
    /// Information Object Address (IOA)
    pub ioa: u32,
    /// Common Address of ASDU (CA)
    pub ca: u16,
    /// Type identification
    pub type_id: u8,
    /// Cause of transmission
    pub cot: Option<u8>,
    /// Qualifier of interrogation
    pub qoi: Option<u8>,
    /// Update interval for polling
    pub polling_interval: Option<u32>,
}

impl ProtocolMapping for IecMapping {
    fn protocol_type(&self) -> &str {
        "iec60870"
    }

    fn mapping_id(&self) -> String {
        format!("iec_{}_{}", self.ca, self.ioa)
    }

    fn polling_interval(&self) -> Option<u32> {
        self.polling_interval
    }

    fn get_parameters(&self) -> std::collections::HashMap<String, String> {
        let mut params = std::collections::HashMap::new();
        params.insert("point_id".to_string(), self.point_id.to_string());
        params.insert("ioa".to_string(), self.ioa.to_string());
        params.insert("ca".to_string(), self.ca.to_string());
        params.insert("type_id".to_string(), self.type_id.to_string());
        if let Some(cot) = self.cot {
            params.insert("cot".to_string(), cot.to_string());
        }
        if let Some(qoi) = self.qoi {
            params.insert("qoi".to_string(), qoi.to_string());
        }
        params
    }

    fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }

    fn validate(&self) -> Result<(), String> {
        if self.point_id == 0 {
            return Err("Point ID must be > 0".to_string());
        }
        if self.ioa > 0xFFFFFF {
            return Err("IOA must be <= 0xFFFFFF".to_string());
        }
        if self.ca == 0 {
            return Err("Common Address must be > 0".to_string());
        }
        if self.type_id == 0 {
            return Err("Type ID must be > 0".to_string());
        }
        Ok(())
    }
}

/// Four-telemetry table view for frontend display
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TelemetryTableView {
    /// Channel ID
    pub channel_id: u16,
    /// Channel name
    pub channel_name: String,
    /// Telemetry points (遥测 - analog measurements)
    pub telemetry: Vec<TelemetryPoint>,
    /// Signal points (遥信 - digital status)  
    pub signal: Vec<TelemetryPoint>,
    /// Adjustment points (遥调 - analog setpoints)
    pub adjustment: Vec<TelemetryPoint>,
    /// Control points (遥控 - digital commands)
    pub control: Vec<TelemetryPoint>,
    /// Last refresh timestamp
    pub timestamp: DateTime<Utc>,
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
        assert!(serialized.contains("1"));
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
            id: "temp_001".to_string(),
            name: "temperature".to_string(),
            value: json!(23.5),
            timestamp: now,
            unit: "°C".to_string(),
            description: "Temperature sensor".to_string(),
        };

        let serialized = serde_json::to_string(&point).unwrap();
        assert!(serialized.contains("temperature"));
        assert!(serialized.contains("23.5"));
        assert!(serialized.contains("temp_001"));
        assert!(serialized.contains("°C"));
    }

    #[test]
    fn test_point_table_data_serialization() {
        let now = Utc::now();
        let points = vec![
            PointValue {
                id: "point1_id".to_string(),
                name: "point1".to_string(),
                value: json!(100),
                timestamp: now,
                unit: "unit".to_string(),
                description: "Test point 1".to_string(),
            },
            PointValue {
                id: "point2_id".to_string(),
                name: "point2".to_string(),
                value: json!("active"),
                timestamp: now,
                unit: "status".to_string(),
                description: "Test point 2".to_string(),
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

    // #[test]
    // fn test_write_point_request_deserialization() {
    //     let json_data = r#"{"value": 42}"#;
    //     let request: WritePointRequest = serde_json::from_str(json_data).unwrap();
    //     assert_eq!(request.value, json!(42));
    //
    //     let json_data = r#"{"value": "hello"}"#;
    //     let request: WritePointRequest = serde_json::from_str(json_data).unwrap();
    //     assert_eq!(request.value, json!("hello"));
    //
    //     let json_data = r#"{"value": true}"#;
    //     let request: WritePointRequest = serde_json::from_str(json_data).unwrap();
    //     assert_eq!(request.value, json!(true));
    // }

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
            id: "int_001".to_string(),
            name: "int_value".to_string(),
            value: json!(42),
            timestamp: now,
            unit: "count".to_string(),
            description: "Integer test point".to_string(),
        };

        let float_point = PointValue {
            id: "float_001".to_string(),
            name: "float_value".to_string(),
            value: json!(3.14159),
            timestamp: now,
            unit: "ratio".to_string(),
            description: "Float test point".to_string(),
        };

        let bool_point = PointValue {
            id: "bool_001".to_string(),
            name: "bool_value".to_string(),
            value: json!(false),
            timestamp: now,
            unit: "state".to_string(),
            description: "Boolean test point".to_string(),
        };

        let string_point = PointValue {
            id: "string_001".to_string(),
            name: "string_value".to_string(),
            value: json!("test string"),
            timestamp: now,
            unit: "text".to_string(),
            description: "String test point".to_string(),
        };

        let array_point = PointValue {
            id: "array_001".to_string(),
            name: "array_value".to_string(),
            value: json!([1, 2, 3, 4, 5]),
            timestamp: now,
            unit: "list".to_string(),
            description: "Array test point".to_string(),
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
        assert!(serialized.contains("1"));
        assert!(serialized.contains("false"));
        assert!(serialized.contains("Connection timeout"));
    }

    #[test]
    fn test_combase_channel_status_conversion() {
        let combase_status = crate::core::framework::ChannelStatus::new("test_001");
        let api_status = ChannelStatus::from(combase_status);

        assert_eq!(api_status.id, 0); // Updated: test_001 cannot parse as u16, so returns 0
        assert_eq!(api_status.name, "Unknown");
        assert_eq!(api_status.protocol, "Unknown");
        assert!(!api_status.connected);
        assert_eq!(api_status.error_count, 0);
        assert!(api_status.last_error.is_none());
        assert!(api_status.statistics.is_empty());
    }

    #[test]
    fn test_combase_point_data_conversion() {
        let combase_point = crate::core::framework::types::PointData {
            id: "1".to_string(),
            name: "Temperature".to_string(),
            value: "25.5".to_string(),
            timestamp: Utc::now(),
            unit: "°C".to_string(),
            description: "Ambient temperature".to_string(),
            telemetry_type: None,
            channel_id: None,
        };

        let api_point = PointValue::from(combase_point);

        assert_eq!(api_point.id, "1");
        assert_eq!(api_point.name, "Temperature");
        assert_eq!(
            api_point.value,
            serde_json::Value::String("25.5".to_string())
        );
        assert_eq!(api_point.unit, "°C");
        assert_eq!(api_point.description, "Ambient temperature");
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
