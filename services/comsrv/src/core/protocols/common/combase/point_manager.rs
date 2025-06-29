//! Point Manager Module
//!
//! This module contains the universal point manager implementation for handling
//! data points across all protocols with four-telemetry classification.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use super::data_types::PointData;
use super::telemetry::{TelemetryType, PointValueType};
use crate::utils::Result;

/// Point configuration for universal management
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UniversalPointConfig {
    /// Point identifier
    pub id: String,
    /// Human-readable point name
    pub name: String,
    /// Point description
    pub description: Option<String>,
    /// Telemetry type classification
    pub telemetry_type: TelemetryType,
    /// Engineering unit for analog points
    pub unit: Option<String>,
    /// Protocol-specific address or register
    pub address: String,
    /// Data type (bool, i16, i32, f32, f64, etc.)
    pub data_type: String,
    /// Scale factor for analog values
    pub scale_factor: Option<f64>,
    /// Offset for analog values
    pub offset: Option<f64>,
    /// Minimum valid value
    pub min_value: Option<f64>,
    /// Maximum valid value
    pub max_value: Option<f64>,
    /// Whether this point is enabled
    pub enabled: bool,
    /// Whether this point is readable
    pub readable: bool,
    /// Whether this point is writable
    pub writable: bool,
    /// Additional metadata
    pub metadata: HashMap<String, String>,
}

impl UniversalPointConfig {
    /// Create a new point configuration
    pub fn new(id: &str, name: &str, telemetry_type: TelemetryType, address: &str) -> Self {
        let is_writable = matches!(telemetry_type, TelemetryType::Control | TelemetryType::Setpoint);
        Self {
            id: id.to_string(),
            name: name.to_string(),
            description: None,
            telemetry_type,
            unit: None,
            address: address.to_string(),
            data_type: "f64".to_string(),
            scale_factor: None,
            offset: None,
            min_value: None,
            max_value: None,
            enabled: true,
            readable: true,
            writable: is_writable,
            metadata: HashMap::new(),
        }
    }

    /// Validate the point configuration
    pub fn validate(&self) -> Result<()> {
        if self.id.is_empty() {
            return Err(crate::utils::ComSrvError::InvalidParameter(
                "Point ID cannot be empty".to_string(),
            ));
        }

        if self.name.is_empty() {
            return Err(crate::utils::ComSrvError::InvalidParameter(
                "Point name cannot be empty".to_string(),
            ));
        }

        if self.address.is_empty() {
            return Err(crate::utils::ComSrvError::InvalidParameter(
                "Point address cannot be empty".to_string(),
            ));
        }

        // Validate min/max values
        if let (Some(min), Some(max)) = (self.min_value, self.max_value) {
            if min >= max {
                return Err(crate::utils::ComSrvError::InvalidParameter(
                    "Minimum value must be less than maximum value".to_string(),
                ));
            }
        }

        Ok(())
    }

    /// Apply scale and offset to raw value
    pub fn process_value(&self, raw_value: f64) -> f64 {
        let scaled = raw_value * self.scale_factor.unwrap_or(1.0);
        scaled + self.offset.unwrap_or(0.0)
    }

    /// Check if value is within valid range
    pub fn is_value_valid(&self, value: f64) -> bool {
        if let Some(min) = self.min_value {
            if value < min {
                return false;
            }
        }
        if let Some(max) = self.max_value {
            if value > max {
                return false;
            }
        }
        true
    }
}

/// Point management statistics
#[derive(Debug, Clone, Default)]
pub struct PointManagerStats {
    /// Total number of points
    pub total_points: usize,
    /// Number of enabled points
    pub enabled_points: usize,
    /// Number of points by telemetry type
    pub points_by_type: HashMap<TelemetryType, usize>,
    /// Number of read operations
    pub read_operations: u64,
    /// Number of write operations
    pub write_operations: u64,
    /// Number of validation errors
    pub validation_errors: u64,
    /// Last update timestamp
    pub last_update: chrono::DateTime<Utc>,
}

/// Universal point manager for all protocols
#[derive(Clone)]
pub struct UniversalPointManager {
    /// Point configurations indexed by point ID
    points: Arc<RwLock<HashMap<String, UniversalPointConfig>>>,
    /// Point cache for quick lookups
    point_cache: Arc<RwLock<HashMap<String, PointData>>>,
    /// Points grouped by telemetry type
    points_by_type: Arc<RwLock<HashMap<TelemetryType, Vec<String>>>>,
    /// Statistics
    stats: Arc<RwLock<PointManagerStats>>,
    /// Channel ID
    channel_id: String,
}

impl UniversalPointManager {
    /// Create a new point manager
    pub fn new(channel_id: String) -> Self {
        Self {
            points: Arc::new(RwLock::new(HashMap::new())),
            point_cache: Arc::new(RwLock::new(HashMap::new())),
            points_by_type: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(PointManagerStats::default())),
            channel_id,
        }
    }

    /// Load point configurations
    pub async fn load_points(&self, configs: Vec<UniversalPointConfig>) -> Result<()> {
        let mut points = self.points.write().await;
        let mut points_by_type = self.points_by_type.write().await;
        let mut stats = self.stats.write().await;

        // Clear existing data
        points.clear();
        points_by_type.clear();

        let mut enabled_count = 0;
        let mut type_counts: HashMap<TelemetryType, usize> = HashMap::new();

        for config in configs {
            // Validate configuration
            config.validate()?;

            if config.enabled {
                enabled_count += 1;
            }

            // Count by type
            *type_counts.entry(config.telemetry_type.clone()).or_insert(0) += 1;

            // Group by telemetry type
            points_by_type
                .entry(config.telemetry_type.clone())
                .or_insert_with(Vec::new)
                .push(config.id.clone());

            // Store configuration
            points.insert(config.id.clone(), config);
        }

        // Update statistics
        stats.total_points = points.len();
        stats.enabled_points = enabled_count;
        stats.points_by_type = type_counts;
        stats.last_update = Utc::now();

        info!(
            "Loaded {} points for channel {}: {} enabled",
            stats.total_points, self.channel_id, stats.enabled_points
        );

        Ok(())
    }

    /// Get point configuration by ID
    pub async fn get_point_config(&self, point_id: &str) -> Option<UniversalPointConfig> {
        self.points.read().await.get(point_id).cloned()
    }

    /// Get all point configurations
    pub async fn get_all_point_configs(&self) -> Vec<UniversalPointConfig> {
        self.points.read().await.values().cloned().collect()
    }

    /// Get points by telemetry type
    pub async fn get_points_by_type(&self, telemetry_type: &TelemetryType) -> Vec<String> {
        self.points_by_type
            .read()
            .await
            .get(telemetry_type)
            .cloned()
            .unwrap_or_default()
    }

    /// Get enabled points by telemetry type
    pub async fn get_enabled_points_by_type(&self, telemetry_type: &TelemetryType) -> Vec<String> {
        let points = self.points.read().await;
        let point_ids = self.get_points_by_type(telemetry_type).await;

        point_ids
            .into_iter()
            .filter(|id| {
                points
                    .get(id)
                    .map(|config| config.enabled)
                    .unwrap_or(false)
            })
            .collect()
    }

    /// Update point value in cache
    pub async fn update_point_value(&self, point_id: &str, value: PointValueType) -> Result<()> {
        let config = self.get_point_config(point_id).await.ok_or_else(|| {
            crate::utils::ComSrvError::NotFound(format!("Point not found: {}", point_id))
        })?;

        if !config.enabled {
            return Err(crate::utils::ComSrvError::InvalidOperation(
                format!("Point {} is disabled", point_id),
            ));
        }

        // Convert PointValueType to string representation
        let value_str = match value {
            PointValueType::Analog(val) => {
                let processed = config.process_value(val);
                if !config.is_value_valid(processed) {
                    warn!("Value {} for point {} is out of range", processed, point_id);
                    self.stats.write().await.validation_errors += 1;
                }
                processed.to_string()
            }
            PointValueType::Digital(val) => val.to_string(),
            PointValueType::Measurement(ref point) => point.value.to_string(),
            PointValueType::Signaling(ref point) => point.status.to_string(),
            PointValueType::Control(ref point) => point.current_state.to_string(),
            PointValueType::Regulation(ref point) => point.current_value.to_string(),
        };

        let point_data = PointData {
            id: point_id.to_string(),
            name: config.name.clone(),
            value: value_str,
            timestamp: Utc::now(),
            unit: config.unit.unwrap_or_default(),
            description: config.description.unwrap_or_default(),
        };

        self.point_cache
            .write()
            .await
            .insert(point_id.to_string(), point_data);

        self.stats.write().await.read_operations += 1;

        debug!("Updated point {} with new value", point_id);
        Ok(())
    }

    /// Get point data from cache
    pub async fn get_point_data(&self, point_id: &str) -> Option<PointData> {
        self.point_cache.read().await.get(point_id).cloned()
    }

    /// Get all cached point data
    pub async fn get_all_point_data(&self) -> Vec<PointData> {
        self.point_cache.read().await.values().cloned().collect()
    }

    /// Get cached point data by telemetry type
    pub async fn get_point_data_by_type(&self, telemetry_type: &TelemetryType) -> Vec<PointData> {
        let point_ids = self.get_points_by_type(telemetry_type).await;
        let cache = self.point_cache.read().await;

        point_ids
            .into_iter()
            .filter_map(|id| cache.get(&id).cloned())
            .collect()
    }

    /// Clear point cache
    pub async fn clear_cache(&self) {
        self.point_cache.write().await.clear();
        debug!("Cleared point cache for channel {}", self.channel_id);
    }

    /// Remove point configuration
    pub async fn remove_point(&self, point_id: &str) -> Result<()> {
        let mut points = self.points.write().await;
        let mut points_by_type = self.points_by_type.write().await;
        let mut cache = self.point_cache.write().await;

        if let Some(config) = points.remove(point_id) {
            // Remove from type grouping
            if let Some(type_points) = points_by_type.get_mut(&config.telemetry_type) {
                type_points.retain(|id| id != point_id);
            }

            // Remove from cache
            cache.remove(point_id);

            info!("Removed point {} from channel {}", point_id, self.channel_id);
            Ok(())
        } else {
            Err(crate::utils::ComSrvError::NotFound(format!(
                "Point not found: {}",
                point_id
            )))
        }
    }

    /// Get statistics
    pub async fn get_stats(&self) -> PointManagerStats {
        self.stats.read().await.clone()
    }

    /// Get channel ID
    pub fn channel_id(&self) -> &str {
        &self.channel_id
    }

    /// Check if point exists and is enabled
    pub async fn is_point_enabled(&self, point_id: &str) -> bool {
        self.points
            .read()
            .await
            .get(point_id)
            .map(|config| config.enabled)
            .unwrap_or(false)
    }

    /// Check if point is readable
    pub async fn is_point_readable(&self, point_id: &str) -> bool {
        self.points
            .read()
            .await
            .get(point_id)
            .map(|config| config.enabled && config.readable)
            .unwrap_or(false)
    }

    /// Check if point is writable
    pub async fn is_point_writable(&self, point_id: &str) -> bool {
        self.points
            .read()
            .await
            .get(point_id)
            .map(|config| config.enabled && config.writable)
            .unwrap_or(false)
    }

    /// Validate point value before writing
    pub async fn validate_point_value(&self, point_id: &str, value: f64) -> Result<()> {
        let config = self.get_point_config(point_id).await.ok_or_else(|| {
            crate::utils::ComSrvError::NotFound(format!("Point not found: {}", point_id))
        })?;

        if !config.enabled {
            return Err(crate::utils::ComSrvError::InvalidOperation(
                format!("Point {} is disabled", point_id),
            ));
        }

        if !config.writable {
            return Err(crate::utils::ComSrvError::InvalidOperation(
                format!("Point {} is not writable", point_id),
            ));
        }

        let processed_value = config.process_value(value);
        if !config.is_value_valid(processed_value) {
                    return Err(crate::utils::ComSrvError::InvalidParameter(format!(
            "Value {} is out of valid range for point {}",
            processed_value, point_id
        )));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config() -> UniversalPointConfig {
        UniversalPointConfig::new(
            "test_point_01",
            "Test Temperature",
            TelemetryType::Telemetry,
            "40001",
        )
    }

    #[test]
    fn test_point_config_creation() {
        let config = create_test_config();
        assert_eq!(config.id, "test_point_01");
        assert_eq!(config.name, "Test Temperature");
        assert_eq!(config.telemetry_type, TelemetryType::Telemetry);
        assert_eq!(config.address, "40001");
        assert!(config.enabled);
        assert!(config.readable);
    }

    #[test]
    fn test_point_config_validation() {
        let config = create_test_config();
        assert!(config.validate().is_ok());

        let mut invalid_config = config.clone();
        invalid_config.id = "".to_string();
        assert!(invalid_config.validate().is_err());
    }

    #[test]
    fn test_value_processing() {
        let mut config = create_test_config();
        config.scale_factor = Some(0.1);
        config.offset = Some(10.0);

        let processed = config.process_value(100.0);
        assert_eq!(processed, 20.0); // 100 * 0.1 + 10 = 20
    }

    #[test]
    fn test_value_validation() {
        let mut config = create_test_config();
        config.min_value = Some(0.0);
        config.max_value = Some(100.0);

        assert!(config.is_value_valid(50.0));
        assert!(!config.is_value_valid(-10.0));
        assert!(!config.is_value_valid(150.0));
    }

    #[tokio::test]
    async fn test_point_manager_creation() {
        let manager = UniversalPointManager::new("test_channel".to_string());
        assert_eq!(manager.channel_id(), "test_channel");

        let stats = manager.get_stats().await;
        assert_eq!(stats.total_points, 0);
    }

    #[tokio::test]
    async fn test_load_points() {
        let manager = UniversalPointManager::new("test_channel".to_string());
        let configs = vec![create_test_config()];

        manager.load_points(configs).await.unwrap();

        let stats = manager.get_stats().await;
        assert_eq!(stats.total_points, 1);
        assert_eq!(stats.enabled_points, 1);

        let config = manager.get_point_config("test_point_01").await;
        assert!(config.is_some());
    }

    #[tokio::test]
    async fn test_points_by_type() {
        let manager = UniversalPointManager::new("test_channel".to_string());
        let configs = vec![create_test_config()];

        manager.load_points(configs).await.unwrap();

        let telemetry_points = manager.get_points_by_type(&TelemetryType::Telemetry).await;
        assert_eq!(telemetry_points.len(), 1);
        assert_eq!(telemetry_points[0], "test_point_01");
    }

    #[tokio::test]
    async fn test_point_value_update() {
        let manager = UniversalPointManager::new("test_channel".to_string());
        let configs = vec![create_test_config()];

        manager.load_points(configs).await.unwrap();

        let value = PointValueType::Analog(25.5);
        manager
            .update_point_value("test_point_01", value)
            .await
            .unwrap();

        let point_data = manager.get_point_data("test_point_01").await;
        assert!(point_data.is_some());

        let data = point_data.unwrap();
        assert_eq!(data.value, "25.5");
    }
} 