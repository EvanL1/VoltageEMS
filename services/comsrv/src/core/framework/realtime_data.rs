//! Optimized real-time data structures
//!
//! This module contains streamlined data structures for real-time telemetry,
//! separating static configuration from dynamic values.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Simplified real-time point value
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealtimeValue {
    /// Raw value from device (before scaling)
    pub raw: f64,
    /// Processed engineering value (after scaling)
    pub value: f64,
    /// Timestamp in milliseconds since epoch
    pub ts: i64,
}

impl RealtimeValue {
    /// Create new realtime value
    pub fn new(raw: f64, value: f64) -> Self {
        Self {
            raw,
            value,
            ts: Utc::now().timestamp_millis(),
        }
    }

    /// Create with specific timestamp
    pub fn with_timestamp(raw: f64, value: f64, timestamp: DateTime<Utc>) -> Self {
        Self {
            raw,
            value,
            ts: timestamp.timestamp_millis(),
        }
    }

    /// Convert to compact JSON string for Redis storage
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Parse from JSON string
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Get timestamp as DateTime
    pub fn timestamp(&self) -> DateTime<Utc> {
        DateTime::from_timestamp_millis(self.ts).unwrap_or_else(Utc::now)
    }
}

/// Static point configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointConfig {
    /// Point name
    pub name: String,
    /// Unit of measurement
    pub unit: String,
    /// Telemetry type
    pub telemetry_type: String,
    /// Description
    pub description: String,
    /// Scale factor for value conversion
    pub scale: f64,
    /// Offset for value conversion
    pub offset: f64,
    /// Protocol address
    pub address: String,
}

impl PointConfig {
    /// Apply scaling to convert raw value to engineering value
    pub fn apply_scaling(&self, raw_value: f64) -> f64 {
        raw_value * self.scale + self.offset
    }

    /// Reverse scaling to convert engineering value to raw value
    pub fn reverse_scaling(&self, eng_value: f64) -> f64 {
        (eng_value - self.offset) / self.scale
    }
}

/// Batch of realtime values for efficient updates
#[derive(Debug, Clone)]
pub struct RealtimeBatch {
    /// Channel ID
    pub channel_id: u16,
    /// Point values mapped by point ID
    pub values: HashMap<String, RealtimeValue>,
    /// Batch timestamp
    pub batch_time: DateTime<Utc>,
}

impl RealtimeBatch {
    /// Create new batch
    pub fn new(channel_id: u16) -> Self {
        Self {
            channel_id,
            values: HashMap::new(),
            batch_time: Utc::now(),
        }
    }

    /// Add a point value to the batch
    pub fn add_point(&mut self, point_id: String, raw: f64, value: f64) {
        self.values.insert(point_id, RealtimeValue::new(raw, value));
    }

    /// Add with custom timestamp
    pub fn add_point_with_time(
        &mut self,
        point_id: String,
        raw: f64,
        value: f64,
        timestamp: DateTime<Utc>,
    ) {
        self.values.insert(
            point_id,
            RealtimeValue::with_timestamp(raw, value, timestamp),
        );
    }

    /// Get the number of points in the batch
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Check if batch is empty
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Convert to Redis hash fields
    pub fn to_redis_fields(&self) -> Result<Vec<(String, String)>, serde_json::Error> {
        let mut fields = Vec::with_capacity(self.values.len());

        for (point_id, value) in &self.values {
            fields.push((point_id.clone(), value.to_json()?));
        }

        Ok(fields)
    }
}

/// Configuration collection for a channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelConfig {
    /// Channel ID
    pub channel_id: u16,
    /// Channel name
    pub channel_name: String,
    /// Point configurations mapped by point ID
    pub points: HashMap<String, PointConfig>,
}

impl ChannelConfig {
    /// Create new channel configuration
    pub fn new(channel_id: u16, channel_name: String) -> Self {
        Self {
            channel_id,
            channel_name,
            points: HashMap::new(),
        }
    }

    /// Add a point configuration
    pub fn add_point(&mut self, point_id: String, config: PointConfig) {
        self.points.insert(point_id, config);
    }

    /// Get point configuration
    pub fn get_point(&self, point_id: &str) -> Option<&PointConfig> {
        self.points.get(point_id)
    }

    /// Convert to Redis hash fields for storage
    pub fn to_redis_fields(&self) -> Result<Vec<(String, String)>, serde_json::Error> {
        let mut fields = Vec::with_capacity(self.points.len());

        for (point_id, config) in &self.points {
            fields.push((point_id.clone(), serde_json::to_string(config)?));
        }

        Ok(fields)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_realtime_value() {
        let val = RealtimeValue::new(1000.0, 100.0);
        assert_eq!(val.raw, 1000.0);
        assert_eq!(val.value, 100.0);
        assert!(val.ts > 0);
    }

    #[test]
    fn test_point_config_scaling() {
        let config = PointConfig {
            name: "Temperature".to_string(),
            unit: "°C".to_string(),
            telemetry_type: "Measurement".to_string(),
            description: "Test temperature".to_string(),
            scale: 0.1,
            offset: -273.15,
            address: "1:3:1000".to_string(),
        };

        // Test scaling: raw 2731.5 -> 0°C
        assert_eq!(config.apply_scaling(2731.5), 0.0);

        // Test reverse scaling: 0°C -> raw 2731.5
        assert_eq!(config.reverse_scaling(0.0), 2731.5);
    }

    #[test]
    fn test_realtime_batch() {
        let mut batch = RealtimeBatch::new(1);
        batch.add_point("point_1".to_string(), 1000.0, 100.0);
        batch.add_point("point_2".to_string(), 2000.0, 200.0);

        assert_eq!(batch.len(), 2);
        assert!(!batch.is_empty());

        let fields = batch.to_redis_fields().unwrap();
        assert_eq!(fields.len(), 2);
    }
}
