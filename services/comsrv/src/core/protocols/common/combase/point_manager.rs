//! Point Manager Module
//!
//! This module contains the universal point manager implementation for handling
//! data points across all protocols with four-telemetry classification.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use super::data_types::PointData;
use super::telemetry::{TelemetryType, PointValueType};
use super::defaults::{get_default_data_type, get_default_unit, get_default_scale};
use crate::utils::Result;

/// 默认缩放因子函数
fn default_scale() -> f64 {
    1.0
}

/// 默认true值函数
fn default_true() -> bool {
    true
}

/// 默认false值函数  
fn default_false() -> bool {
    false
}

/// Point configuration for universal management
/// Follows the format specified in the functional specification document
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UniversalPointConfig {
    /// 点位唯一标识符（必需，数字，与四遥点表中点位对应）
    pub point_id: u32,
    /// 点位中文名称（可选）
    #[serde(default)]
    pub name: Option<String>,
    /// 详细描述（可选）
    #[serde(default)]
    pub description: Option<String>,
    /// 工程单位（可选）
    #[serde(default)]
    pub unit: Option<String>,
    /// 数据类型（可选，如未提供则根据telemetry_type自动推断）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_type: Option<String>,
    /// 缩放因子（必需，默认1.0）
    #[serde(default = "default_scale")]
    pub scale: f64,
    /// 偏移（必需，默认为0）
    #[serde(default)]
    pub offset: f64,
    /// 是否反位（仅遥信/遥控使用，0不开启，1开启）
    #[serde(default)]
    pub reverse: u8,
    /// Telemetry type classification
    pub telemetry_type: TelemetryType,
    /// Whether this point is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Whether this point is readable
    #[serde(default = "default_true")]
    pub readable: bool,
    /// Whether this point is writable
    #[serde(default = "default_false")]
    pub writable: bool,
}

impl UniversalPointConfig {
    /// Create a new point configuration with smart defaults
    pub fn new(point_id: u32, name: &str, telemetry_type: TelemetryType) -> Self {
        let is_writable = matches!(telemetry_type, TelemetryType::Control | TelemetryType::Setpoint);
        
        // Try to infer unit from signal name
        let unit = get_default_unit(name);
        
        // Get default scale based on unit
        let scale = get_default_scale(unit, name);
        
        Self {
            point_id,
            name: Some(name.to_string()),
            description: None,
            unit: unit.map(|u| u.to_string()),
            data_type: None, // Will be inferred from telemetry_type
            scale,
            offset: 0.0,
            reverse: 0,
            telemetry_type,
            enabled: true,
            readable: true,
            writable: is_writable,
        }
    }
    
    /// Get the actual data type (inferred if not specified)
    pub fn get_data_type(&self) -> String {
        self.data_type.clone().unwrap_or_else(|| {
            get_default_data_type(&self.telemetry_type).to_string()
        })
    }

    /// Validate the point configuration
    pub fn validate(&self) -> Result<()> {
        if self.point_id == 0 {
            return Err(crate::utils::ComSrvError::InvalidParameter(
                "Point ID cannot be 0".to_string(),
            ));
        }

        // data_type is now optional, so we validate the resolved type
        let data_type = self.get_data_type();
        if data_type.is_empty() {
            return Err(crate::utils::ComSrvError::InvalidParameter(
                "Data type cannot be empty".to_string(),
            ));
        }

        if self.reverse > 1 {
            return Err(crate::utils::ComSrvError::InvalidParameter(
                "Reverse must be 0 or 1".to_string(),
            ));
        }

        Ok(())
    }

    /// Apply scale and offset to raw value for analog points
    /// Point_data = source_data * scale + offset
    pub fn process_value(&self, raw_value: f64) -> f64 {
        raw_value * self.scale + self.offset
    }

    /// Apply reverse logic for digital points
    /// reverse=1时，point_data = 1 - source_data (即取反)
    pub fn process_digital_value(&self, source_data: bool) -> bool {
        if self.reverse == 1 {
            !source_data
        } else {
            source_data
        }
    }

    /// Get point ID as string for compatibility
    pub fn id(&self) -> String {
        self.point_id.to_string()
    }

    /// Get point name or default
    pub fn get_name(&self) -> String {
        self.name.clone().unwrap_or_else(|| format!("Point_{}", self.point_id))
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
#[derive(Clone, Debug)]
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
                .push(config.id());

            // Store configuration
            points.insert(config.id(), config);
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
                processed.to_string()
            }
            PointValueType::Digital(val) => {
                let processed = config.process_digital_value(val);
                processed.to_string()
            },
            PointValueType::Measurement(ref point) => point.value.to_string(),
            PointValueType::Signaling(ref point) => point.status.to_string(),
            PointValueType::Control(ref point) => point.current_state.to_string(),
            PointValueType::Regulation(ref point) => point.current_value.to_string(),
        };

        let point_data = PointData {
            id: point_id.to_string(),
            name: config.get_name(),
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
    pub async fn validate_point_value(&self, point_id: &str, _value: f64) -> Result<()> {
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

        // Note: Value validation can be added here if needed
        // let processed_value = config.process_value(value);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config() -> UniversalPointConfig {
        UniversalPointConfig::new(
            1001,
            "Test Temperature",
            TelemetryType::Telemetry,
        )
    }

    #[test]
    fn test_point_config_creation() {
        let config = create_test_config();
        assert_eq!(config.point_id, 1001);
        assert_eq!(config.get_name(), "Test Temperature");
        assert_eq!(config.telemetry_type, TelemetryType::Telemetry);
        assert!(config.enabled);
        assert!(config.readable);
    }

    #[test]
    fn test_point_config_validation() {
        let config = create_test_config();
        assert!(config.validate().is_ok());

        let mut invalid_config = config.clone();
        invalid_config.point_id = 0;
        assert!(invalid_config.validate().is_err());
    }

    #[test]
    fn test_value_processing() {
        let mut config = create_test_config();
        config.scale = 0.1;
        config.offset = 10.0;

        let processed = config.process_value(100.0);
        assert_eq!(processed, 20.0); // 100 * 0.1 + 10 = 20
    }

    #[test]
    fn test_digital_value_processing() {
        let mut config = create_test_config();
        config.reverse = 1;

        let processed = config.process_digital_value(true);
        assert_eq!(processed, false); // reverse = 1, so true -> false
        
        let processed2 = config.process_digital_value(false);
        assert_eq!(processed2, true); // reverse = 1, so false -> true
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

        let config = manager.get_point_config("1001").await;
        assert!(config.is_some());
    }

    #[tokio::test]
    async fn test_points_by_type() {
        let manager = UniversalPointManager::new("test_channel".to_string());
        let configs = vec![create_test_config()];

        manager.load_points(configs).await.unwrap();

        let telemetry_points = manager.get_points_by_type(&TelemetryType::Telemetry).await;
        assert_eq!(telemetry_points.len(), 1);
        assert_eq!(telemetry_points[0], "1001");
    }

    #[tokio::test]
    async fn test_point_value_update() {
        let manager = UniversalPointManager::new("test_channel".to_string());
        let configs = vec![create_test_config()];

        manager.load_points(configs).await.unwrap();

        let value = PointValueType::Analog(25.5);
        manager
            .update_point_value("1001", value)
            .await
            .unwrap();

        let point_data = manager.get_point_data("1001").await;
        assert!(point_data.is_some());

        let data = point_data.unwrap();
        assert_eq!(data.value, "25.5");
    }
} 