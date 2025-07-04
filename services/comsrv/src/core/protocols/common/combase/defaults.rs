//! Default values and constants for the communication service
//!
//! This module provides system-wide default values to minimize configuration requirements

use super::telemetry::TelemetryType;
use crate::core::protocols::modbus::common::ModbusFunctionCode;

/// System default values
pub mod defaults {
    // Modbus protocol defaults
    pub const DEFAULT_SLAVE_ID: u8 = 1;
    pub const DEFAULT_TIMEOUT_MS: u32 = 3000;
    pub const DEFAULT_RETRY_COUNT: u32 = 3;
    pub const DEFAULT_RETRY_INTERVAL_MS: u32 = 1000;
    
    // Data processing defaults
    pub const DEFAULT_SCALE: f64 = 1.0;
    pub const DEFAULT_OFFSET: f64 = 0.0;
    pub const DEFAULT_REVERSE: u8 = 0;
    
    // Polling defaults
    pub const DEFAULT_POLLING_INTERVAL_MS: u64 = 1000;
    pub const DEFAULT_BATCH_SIZE: u32 = 100;
    pub const DEFAULT_MAX_POINTS_PER_CYCLE: u32 = 100;
    pub const DEFAULT_POINT_READ_DELAY_MS: u64 = 10;
    
    // Modbus function codes
    pub const FC_READ_COILS: u8 = 1;
    pub const FC_READ_DISCRETE_INPUTS: u8 = 2;
    pub const FC_READ_HOLDING_REGISTERS: u8 = 3;
    pub const FC_READ_INPUT_REGISTERS: u8 = 4;
    pub const FC_WRITE_SINGLE_COIL: u8 = 5;
    pub const FC_WRITE_SINGLE_REGISTER: u8 = 6;
    pub const FC_WRITE_MULTIPLE_COILS: u8 = 15;
    pub const FC_WRITE_MULTIPLE_REGISTERS: u8 = 16;
    
    // Channel defaults
    pub const DEFAULT_CHANNEL_ENABLED: bool = true;
    pub const DEFAULT_CHANNEL_LOG_LEVEL: &str = "info";
    pub const DEFAULT_CHANNEL_LOG_DIR: &str = "logs/channels";
    pub const DEFAULT_MAX_LOG_FILE_SIZE: u64 = 10485760; // 10MB
    pub const DEFAULT_MAX_LOG_FILES: u32 = 5;
    pub const DEFAULT_LOG_RETENTION_DAYS: u32 = 7;
    
    // Connection defaults
    pub const DEFAULT_TCP_PORT: u16 = 502;
    pub const DEFAULT_SERIAL_BAUD: u32 = 9600;
    pub const DEFAULT_SERIAL_DATA_BITS: u8 = 8;
    pub const DEFAULT_SERIAL_STOP_BITS: u8 = 1;
    pub const DEFAULT_SERIAL_PARITY: &str = "none";
}

/// Get default data type for telemetry type
pub fn get_default_data_type(telemetry_type: &TelemetryType) -> &'static str {
    match telemetry_type {
        TelemetryType::Telemetry | TelemetryType::Setpoint => "float32",
        TelemetryType::Signaling | TelemetryType::Control => "bool",
    }
}

/// Get default function code for telemetry type
pub fn get_default_function_code(telemetry_type: &TelemetryType) -> ModbusFunctionCode {
    match telemetry_type {
        TelemetryType::Telemetry => ModbusFunctionCode::Read03,
        TelemetryType::Signaling => ModbusFunctionCode::Read02,
        TelemetryType::Control => ModbusFunctionCode::Write05,
        TelemetryType::Setpoint => ModbusFunctionCode::Write06,
    }
}

/// Get default unit for common measurement types
pub fn get_default_unit(signal_name: &str) -> Option<&'static str> {
    let name_lower = signal_name.to_lowercase();
    
    // Voltage related
    if name_lower.contains("voltage") || name_lower.contains("电压") {
        return Some("V");
    }
    
    // Current related
    if name_lower.contains("current") || name_lower.contains("电流") {
        return Some("A");
    }
    
    // Power related
    if name_lower.contains("power") || name_lower.contains("功率") {
        if name_lower.contains("reactive") || name_lower.contains("无功") {
            return Some("kVar");
        } else if name_lower.contains("apparent") || name_lower.contains("视在") {
            return Some("kVA");
        } else {
            return Some("kW");
        }
    }
    
    // Energy related
    if name_lower.contains("energy") || name_lower.contains("电能") || name_lower.contains("电量") {
        return Some("kWh");
    }
    
    // Frequency
    if name_lower.contains("frequency") || name_lower.contains("频率") {
        return Some("Hz");
    }
    
    // Temperature
    if name_lower.contains("temperature") || name_lower.contains("温度") {
        return Some("°C");
    }
    
    // Pressure
    if name_lower.contains("pressure") || name_lower.contains("压力") {
        return Some("bar");
    }
    
    // Flow
    if name_lower.contains("flow") || name_lower.contains("流量") {
        return Some("m³/h");
    }
    
    // Percentage
    if name_lower.contains("percent") || name_lower.contains("百分") || name_lower.contains("率") {
        return Some("%");
    }
    
    None
}

/// Get default scale for common units
pub fn get_default_scale(unit: Option<&str>, signal_name: &str) -> f64 {
    if let Some(unit_str) = unit {
        match unit_str {
            "kW" | "kVar" | "kVA" => {
                // If register stores W, convert to kW
                if signal_name.to_lowercase().contains("register") {
                    return 0.001;
                }
            }
            "MW" => {
                // If register stores W, convert to MW
                if signal_name.to_lowercase().contains("register") {
                    return 0.000001;
                }
            }
            _ => {}
        }
    }
    defaults::DEFAULT_SCALE
}

/// Channel parameter defaults based on protocol
pub fn get_channel_defaults(protocol: &str) -> std::collections::HashMap<String, String> {
    let mut params = std::collections::HashMap::new();
    
    match protocol.to_lowercase().as_str() {
        "modbus_tcp" => {
            params.insert("port".to_string(), defaults::DEFAULT_TCP_PORT.to_string());
            params.insert("timeout".to_string(), defaults::DEFAULT_TIMEOUT_MS.to_string());
            params.insert("retry_count".to_string(), defaults::DEFAULT_RETRY_COUNT.to_string());
            params.insert("retry_interval".to_string(), defaults::DEFAULT_RETRY_INTERVAL_MS.to_string());
        }
        "modbus_rtu" => {
            params.insert("baud_rate".to_string(), defaults::DEFAULT_SERIAL_BAUD.to_string());
            params.insert("data_bits".to_string(), defaults::DEFAULT_SERIAL_DATA_BITS.to_string());
            params.insert("stop_bits".to_string(), defaults::DEFAULT_SERIAL_STOP_BITS.to_string());
            params.insert("parity".to_string(), defaults::DEFAULT_SERIAL_PARITY.to_string());
            params.insert("timeout".to_string(), defaults::DEFAULT_TIMEOUT_MS.to_string());
            params.insert("retry_count".to_string(), defaults::DEFAULT_RETRY_COUNT.to_string());
        }
        "iec104" => {
            params.insert("port".to_string(), "2404".to_string());
            params.insert("k".to_string(), "12".to_string());
            params.insert("w".to_string(), "8".to_string());
            params.insert("t1".to_string(), "15".to_string());
            params.insert("t2".to_string(), "10".to_string());
            params.insert("t3".to_string(), "20".to_string());
        }
        "can" => {
            params.insert("bitrate".to_string(), "500000".to_string());
            params.insert("timeout".to_string(), "1000".to_string());
        }
        _ => {}
    }
    
    params
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_data_type() {
        assert_eq!(get_default_data_type(&TelemetryType::Telemetry), "float32");
        assert_eq!(get_default_data_type(&TelemetryType::Setpoint), "float32");
        assert_eq!(get_default_data_type(&TelemetryType::Signaling), "bool");
        assert_eq!(get_default_data_type(&TelemetryType::Control), "bool");
    }

    #[test]
    fn test_default_function_code() {
        assert_eq!(get_default_function_code(&TelemetryType::Telemetry), ModbusFunctionCode::Read03);
        assert_eq!(get_default_function_code(&TelemetryType::Signaling), ModbusFunctionCode::Read02);
        assert_eq!(get_default_function_code(&TelemetryType::Control), ModbusFunctionCode::Write05);
        assert_eq!(get_default_function_code(&TelemetryType::Setpoint), ModbusFunctionCode::Write06);
    }

    #[test]
    fn test_default_unit() {
        assert_eq!(get_default_unit("voltage_a"), Some("V"));
        assert_eq!(get_default_unit("current_phase_b"), Some("A"));
        assert_eq!(get_default_unit("active_power"), Some("kW"));
        assert_eq!(get_default_unit("reactive_power"), Some("kVar"));
        assert_eq!(get_default_unit("temperature_sensor"), Some("°C"));
        assert_eq!(get_default_unit("unknown_signal"), None);
    }

    #[test]
    fn test_channel_defaults() {
        let modbus_tcp_defaults = get_channel_defaults("modbus_tcp");
        assert_eq!(modbus_tcp_defaults.get("port"), Some(&"502".to_string()));
        assert_eq!(modbus_tcp_defaults.get("timeout"), Some(&"3000".to_string()));
        
        let modbus_rtu_defaults = get_channel_defaults("modbus_rtu");
        assert_eq!(modbus_rtu_defaults.get("baud_rate"), Some(&"9600".to_string()));
        assert_eq!(modbus_rtu_defaults.get("parity"), Some(&"none".to_string()));
    }
}