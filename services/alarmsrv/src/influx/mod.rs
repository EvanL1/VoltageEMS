//! InfluxDB historical storage module for alarm data
//! 
//! This module provides InfluxDB integration for storing alarm history,
//! based on the hissrv implementation but adapted for alarm-specific data.

pub mod client;
pub mod storage;
pub mod config;

pub use client::InfluxDBClient;
pub use storage::AlarmHistoryStorage;
pub use config::InfluxDBConfig;

use chrono::{DateTime, Utc};
use std::collections::HashMap;

/// Data value types supported by InfluxDB
#[derive(Debug, Clone)]
pub enum DataValue {
    Float(f64),
    Integer(i64),
    String(String),
    Boolean(bool),
}

impl DataValue {
    /// Convert to InfluxDB Line Protocol format
    pub fn to_line_protocol(&self) -> String {
        match self {
            DataValue::Float(f) => f.to_string(),
            DataValue::Integer(i) => format!("{}i", i),
            DataValue::String(s) => format!("\"{}\"", s.replace('"', "\\\"")),
            DataValue::Boolean(b) => b.to_string(),
        }
    }
}

/// Alarm data point for InfluxDB storage
#[derive(Debug, Clone)]
pub struct AlarmDataPoint {
    /// Measurement name (always "alarms" for alarm data)
    pub measurement: String,
    /// Tags for indexing and filtering
    pub tags: HashMap<String, String>,
    /// Fields containing actual data
    pub fields: HashMap<String, DataValue>,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

impl AlarmDataPoint {
    /// Create new alarm data point
    pub fn new(
        tags: HashMap<String, String>,
        fields: HashMap<String, DataValue>,
        timestamp: DateTime<Utc>,
    ) -> Self {
        Self {
            measurement: "alarms".to_string(),
            tags,
            fields,
            timestamp,
        }
    }

    /// Create alarm data point from alarm data
    pub fn from_alarm_data(
        alarm_id: &str,
        level: &str,
        status: &str,
        title: &str,
        description: &str,
        module_id: Option<&str>,
        point_name: Option<&str>,
        timestamp: DateTime<Utc>,
    ) -> Self {
        let mut tags = HashMap::new();
        tags.insert("alarm_id".to_string(), alarm_id.to_string());
        tags.insert("level".to_string(), level.to_string());
        tags.insert("status".to_string(), status.to_string());
        
        if let Some(module) = module_id {
            tags.insert("module_id".to_string(), module.to_string());
        }
        
        if let Some(point) = point_name {
            tags.insert("point_name".to_string(), point.to_string());
        }

        let mut fields = HashMap::new();
        fields.insert("title".to_string(), DataValue::String(title.to_string()));
        fields.insert("description".to_string(), DataValue::String(description.to_string()));

        Self::new(tags, fields, timestamp)
    }

    /// Convert to InfluxDB Line Protocol format
    pub fn to_line_protocol(&self) -> String {
        let mut line = self.measurement.clone();

        // Add tags
        if !self.tags.is_empty() {
            line.push(',');
            let tag_parts: Vec<String> = self
                .tags
                .iter()
                .map(|(k, v)| format!("{}={}", escape_tag_key(k), escape_tag_value(v)))
                .collect();
            line.push_str(&tag_parts.join(","));
        }

        // Add fields
        line.push(' ');
        let field_parts: Vec<String> = self
            .fields
            .iter()
            .map(|(k, v)| format!("{}={}", escape_field_key(k), v.to_line_protocol()))
            .collect();
        line.push_str(&field_parts.join(","));

        // Add timestamp (nanoseconds)
        line.push(' ');
        line.push_str(&self.timestamp.timestamp_nanos_opt().unwrap_or(0).to_string());

        line
    }
}

/// Escape tag key for InfluxDB Line Protocol
fn escape_tag_key(key: &str) -> String {
    key.replace(' ', "\\ ")
        .replace(',', "\\,")
        .replace('=', "\\=")
}

/// Escape tag value for InfluxDB Line Protocol
fn escape_tag_value(value: &str) -> String {
    value.replace(' ', "\\ ")
         .replace(',', "\\,")
         .replace('=', "\\=")
}

/// Escape field key for InfluxDB Line Protocol
fn escape_field_key(key: &str) -> String {
    key.replace(' ', "\\ ")
        .replace(',', "\\,")
        .replace('=', "\\=")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_value_to_line_protocol() {
        assert_eq!(DataValue::Float(3.14).to_line_protocol(), "3.14");
        assert_eq!(DataValue::Integer(42).to_line_protocol(), "42i");
        assert_eq!(DataValue::String("hello".to_string()).to_line_protocol(), "\"hello\"");
        assert_eq!(DataValue::Boolean(true).to_line_protocol(), "true");
    }

    #[test]
    fn test_alarm_data_point_to_line_protocol() {
        let point = AlarmDataPoint::from_alarm_data(
            "alarm_001",
            "Critical",
            "New",
            "High Temperature",
            "Temperature exceeded 90°C",
            Some("device_001"),
            Some("temperature_sensor_1"),
            DateTime::from_timestamp(1642681200, 0).unwrap(),
        );

        let line = point.to_line_protocol();
        assert!(line.contains("alarms"));
        assert!(line.contains("alarm_id=alarm_001"));
        assert!(line.contains("level=Critical"));
        assert!(line.contains("status=New"));
        assert!(line.contains("module_id=device_001"));
        assert!(line.contains("point_name=temperature_sensor_1"));
        assert!(line.contains("title=\"High Temperature\""));
        assert!(line.contains("1642681200000000000"));
    }

    #[test]
    fn test_escape_functions() {
        assert_eq!(escape_tag_key("test key"), "test\\ key");
        assert_eq!(escape_tag_value("value,with=spaces"), "value\\,with\\=spaces");
        assert_eq!(escape_field_key("field key"), "field\\ key");
    }
}