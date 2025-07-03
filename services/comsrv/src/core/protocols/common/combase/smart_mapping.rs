//! Smart Protocol Mapping with Defaults
//!
//! This module provides intelligent protocol mapping with automatic defaults

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use super::telemetry::TelemetryType;
use super::defaults::{defaults, get_default_function_code};

/// Smart protocol mapping that provides intelligent defaults
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartProtocolMapping {
    pub point_id: u32,
    pub signal_name: String,
    
    /// Protocol parameters with smart defaults
    /// Common keys: slave_id, function_code, address, data_format
    #[serde(default)]
    pub protocol_params: HashMap<String, String>,
}

impl SmartProtocolMapping {
    /// Create a new mapping with minimal required fields
    pub fn new(point_id: u32, signal_name: String, address: u16) -> Self {
        let mut protocol_params = HashMap::new();
        protocol_params.insert("address".to_string(), address.to_string());
        
        Self {
            point_id,
            signal_name,
            protocol_params,
        }
    }
    
    /// Get slave_id with default
    pub fn get_slave_id(&self) -> u8 {
        self.protocol_params
            .get("slave_id")
            .and_then(|s| s.parse::<u8>().ok())
            .unwrap_or(defaults::DEFAULT_SLAVE_ID)
    }
    
    /// Get function_code with intelligent default based on telemetry type
    pub fn get_function_code(&self, telemetry_type: &TelemetryType) -> u8 {
        self.protocol_params
            .get("function_code")
            .and_then(|s| s.parse::<u8>().ok())
            .unwrap_or_else(|| get_default_function_code(telemetry_type).into())
    }
    
    /// Get register address
    pub fn get_register_address(&self) -> Result<u16, String> {
        self.protocol_params
            .get("address")
            .or(self.protocol_params.get("register_address"))
            .ok_or_else(|| "No address specified in protocol_params".to_string())
            .and_then(|s| s.parse::<u16>()
                .map_err(|_| format!("Invalid address format: {}", s)))
    }
    
    /// Get data format with default based on telemetry type
    pub fn get_data_format(&self, telemetry_type: &TelemetryType) -> String {
        self.protocol_params
            .get("data_format")
            .cloned()
            .unwrap_or_else(|| {
                match telemetry_type {
                    TelemetryType::Telemetry | TelemetryType::Setpoint => "float32".to_string(),
                    TelemetryType::Signaling | TelemetryType::Control => "bool".to_string(),
                }
            })
    }
    
    /// Get timeout with default
    pub fn get_timeout_ms(&self) -> u32 {
        self.protocol_params
            .get("timeout")
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(defaults::DEFAULT_TIMEOUT_MS)
    }
    
    /// Build address string in format "slave_id:function_code:register_address"
    pub fn build_address_string(&self, telemetry_type: &TelemetryType) -> Result<String, String> {
        let slave_id = self.get_slave_id();
        let function_code = self.get_function_code(telemetry_type);
        let register_address = self.get_register_address()?;
        
        Ok(format!("{}:{}:{}", slave_id, function_code, register_address))
    }
    
    /// Create from minimal CSV data
    pub fn from_csv_minimal(
        point_id: u32,
        signal_name: String,
        register_address: u16,
        slave_id: Option<u8>,
        function_code: Option<u8>,
    ) -> Self {
        let mut protocol_params = HashMap::new();
        
        // Always include address
        protocol_params.insert("address".to_string(), register_address.to_string());
        
        // Only include non-default values
        if let Some(sid) = slave_id {
            if sid != defaults::DEFAULT_SLAVE_ID {
                protocol_params.insert("slave_id".to_string(), sid.to_string());
            }
        }
        
        if let Some(fc) = function_code {
            protocol_params.insert("function_code".to_string(), fc.to_string());
        }
        
        Self {
            point_id,
            signal_name,
            protocol_params,
        }
    }
}

/// Simplified CSV mapping record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimplifiedMappingRecord {
    pub point_id: u32,
    pub signal_name: String,
    pub register_address: u16,
    
    /// Optional fields - will use defaults if not provided
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slave_id: Option<u8>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_code: Option<u8>,
    
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_format: Option<String>,
}

impl SimplifiedMappingRecord {
    /// Convert to SmartProtocolMapping
    pub fn to_smart_mapping(&self) -> SmartProtocolMapping {
        SmartProtocolMapping::from_csv_minimal(
            self.point_id,
            self.signal_name.clone(),
            self.register_address,
            self.slave_id,
            self.function_code,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_smart_defaults() {
        let mapping = SmartProtocolMapping::new(10001, "voltage_a".to_string(), 100);
        
        assert_eq!(mapping.get_slave_id(), 1);
        assert_eq!(mapping.get_function_code(&TelemetryType::Telemetry), 3);
        assert_eq!(mapping.get_function_code(&TelemetryType::Control), 5);
        assert_eq!(mapping.get_register_address().unwrap(), 100);
    }

    #[test]
    fn test_minimal_csv_record() {
        let record = SimplifiedMappingRecord {
            point_id: 10001,
            signal_name: "voltage_a".to_string(),
            register_address: 100,
            slave_id: None,
            function_code: None,
            data_format: None,
        };
        
        let mapping = record.to_smart_mapping();
        assert_eq!(mapping.get_slave_id(), 1); // Should use default
        assert_eq!(mapping.get_data_format(&TelemetryType::Telemetry), "float32");
    }

    #[test]
    fn test_build_address_string() {
        let mapping = SmartProtocolMapping::new(10001, "voltage_a".to_string(), 100);
        let address = mapping.build_address_string(&TelemetryType::Telemetry).unwrap();
        assert_eq!(address, "1:3:100");
    }
}