//! Optimized Point Manager Module
//! 
//! High-performance point manager using u32 keys and multiple indices

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use super::data_types::PointData;
use super::telemetry::{TelemetryType, PointValueType};
use super::point_manager::UniversalPointConfig;
use crate::utils::Result;

/// Optimized point manager statistics
#[derive(Debug, Clone, Default)]
pub struct OptimizedPointManagerStats {
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
    /// Number of cache hits
    pub cache_hits: u64,
    /// Number of cache misses
    pub cache_misses: u64,
    /// Last update timestamp
    pub last_update: chrono::DateTime<Utc>,
}

/// Optimized universal point manager with u32 keys and multiple indices
#[derive(Clone)]
pub struct OptimizedPointManager {
    /// Point configurations indexed by numeric point ID (primary storage)
    points: Arc<RwLock<HashMap<u32, UniversalPointConfig>>>,
    
    /// Real-time point data cache indexed by numeric point ID
    realtime_cache: Arc<RwLock<HashMap<u32, PointData>>>,
    
    /// Points grouped by telemetry type using HashSet for O(1) lookups
    points_by_type: Arc<RwLock<HashMap<TelemetryType, HashSet<u32>>>>,
    
    /// Name to ID mapping for fast name-based lookups
    name_to_id: Arc<RwLock<HashMap<String, u32>>>,
    
    /// Enabled points set for quick filtering
    enabled_points: Arc<RwLock<HashSet<u32>>>,
    
    /// Readable points set
    readable_points: Arc<RwLock<HashSet<u32>>>,
    
    /// Writable points set
    writable_points: Arc<RwLock<HashSet<u32>>>,
    
    /// Statistics
    stats: Arc<RwLock<OptimizedPointManagerStats>>,
    
    /// Channel ID
    channel_id: String,
}

impl OptimizedPointManager {
    /// Create a new optimized point manager
    pub fn new(channel_id: String) -> Self {
        Self {
            points: Arc::new(RwLock::new(HashMap::with_capacity(10000))),
            realtime_cache: Arc::new(RwLock::new(HashMap::with_capacity(10000))),
            points_by_type: Arc::new(RwLock::new(HashMap::new())),
            name_to_id: Arc::new(RwLock::new(HashMap::new())),
            enabled_points: Arc::new(RwLock::new(HashSet::new())),
            readable_points: Arc::new(RwLock::new(HashSet::new())),
            writable_points: Arc::new(RwLock::new(HashSet::new())),
            stats: Arc::new(RwLock::new(OptimizedPointManagerStats::default())),
            channel_id,
        }
    }

    /// Load point configurations with optimized indices
    pub async fn load_points(&self, configs: Vec<UniversalPointConfig>) -> Result<()> {
        let mut points = self.points.write().await;
        let mut points_by_type = self.points_by_type.write().await;
        let mut name_to_id = self.name_to_id.write().await;
        let mut enabled_points = self.enabled_points.write().await;
        let mut readable_points = self.readable_points.write().await;
        let mut writable_points = self.writable_points.write().await;
        let mut stats = self.stats.write().await;

        // Clear existing data
        points.clear();
        points_by_type.clear();
        name_to_id.clear();
        enabled_points.clear();
        readable_points.clear();
        writable_points.clear();

        let mut type_counts: HashMap<TelemetryType, usize> = HashMap::new();

        for config in configs {
            // Validate configuration
            config.validate()?;

            let point_id = config.point_id;

            // Update indices
            if config.enabled {
                enabled_points.insert(point_id);
            }
            if config.readable {
                readable_points.insert(point_id);
            }
            if config.writable {
                writable_points.insert(point_id);
            }

            // Count by type
            *type_counts.entry(config.telemetry_type.clone()).or_insert(0) += 1;

            // Group by telemetry type
            points_by_type
                .entry(config.telemetry_type.clone())
                .or_insert_with(HashSet::new)
                .insert(point_id);

            // Name to ID mapping
            if let Some(name) = &config.name {
                name_to_id.insert(name.clone(), point_id);
            }

            // Store configuration
            points.insert(point_id, config);
        }

        // Update statistics
        stats.total_points = points.len();
        stats.enabled_points = enabled_points.len();
        stats.points_by_type = type_counts;
        stats.last_update = Utc::now();

        info!(
            "Loaded {} points for channel {}: {} enabled, {} readable, {} writable",
            stats.total_points, 
            self.channel_id, 
            enabled_points.len(),
            readable_points.len(),
            writable_points.len()
        );

        Ok(())
    }

    /// Get point configuration by numeric ID (O(1) lookup)
    pub async fn get_point_config(&self, point_id: u32) -> Option<UniversalPointConfig> {
        self.points.read().await.get(&point_id).cloned()
    }

    /// Get point configuration by string ID (for compatibility)
    pub async fn get_point_config_by_string(&self, point_id: &str) -> Option<UniversalPointConfig> {
        if let Ok(id) = point_id.parse::<u32>() {
            self.get_point_config(id).await
        } else {
            None
        }
    }

    /// Get point configuration by name (O(1) lookup via name index)
    pub async fn get_point_config_by_name(&self, name: &str) -> Option<UniversalPointConfig> {
        let name_to_id = self.name_to_id.read().await;
        if let Some(&point_id) = name_to_id.get(name) {
            self.get_point_config(point_id).await
        } else {
            None
        }
    }

    /// Get all point configurations
    pub async fn get_all_point_configs(&self) -> Vec<UniversalPointConfig> {
        self.points.read().await.values().cloned().collect()
    }

    /// Get points by telemetry type (O(1) lookup)
    pub async fn get_points_by_type(&self, telemetry_type: &TelemetryType) -> Vec<u32> {
        self.points_by_type
            .read()
            .await
            .get(telemetry_type)
            .map(|set| set.iter().copied().collect())
            .unwrap_or_default()
    }

    /// Get enabled points by telemetry type (O(1) set intersection)
    pub async fn get_enabled_points_by_type(&self, telemetry_type: &TelemetryType) -> Vec<u32> {
        let points_by_type = self.points_by_type.read().await;
        let enabled_points = self.enabled_points.read().await;
        
        if let Some(type_points) = points_by_type.get(telemetry_type) {
            type_points
                .intersection(&*enabled_points)
                .copied()
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Update point value in cache with optimized access
    pub async fn update_point_value(&self, point_id: u32, value: PointValueType) -> Result<()> {
        // Quick enabled check via HashSet
        if !self.enabled_points.read().await.contains(&point_id) {
            return Err(crate::utils::ComSrvError::InvalidOperation(
                format!("Point {} is disabled", point_id),
            ));
        }

        let config = self.get_point_config(point_id).await.ok_or_else(|| {
            crate::utils::ComSrvError::NotFound(format!("Point not found: {}", point_id))
        })?;

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

        self.realtime_cache
            .write()
            .await
            .insert(point_id, point_data);

        self.stats.write().await.write_operations += 1;

        debug!("Updated point {} with new value", point_id);
        Ok(())
    }

    /// Batch update multiple point values
    pub async fn batch_update_values(&self, updates: Vec<(u32, PointValueType)>) -> Result<()> {
        let enabled_points = self.enabled_points.read().await;
        let points = self.points.read().await;
        let mut cache = self.realtime_cache.write().await;
        let mut stats = self.stats.write().await;
        
        let timestamp = Utc::now();
        
        for (point_id, value) in updates {
            // Quick enabled check
            if !enabled_points.contains(&point_id) {
                continue;
            }
            
            if let Some(config) = points.get(&point_id) {
                let value_str = match value {
                    PointValueType::Analog(val) => config.process_value(val).to_string(),
                    PointValueType::Digital(val) => config.process_digital_value(val).to_string(),
                    PointValueType::Measurement(ref point) => point.value.to_string(),
                    PointValueType::Signaling(ref point) => point.status.to_string(),
                    PointValueType::Control(ref point) => point.current_state.to_string(),
                    PointValueType::Regulation(ref point) => point.current_value.to_string(),
                };
                
                let point_data = PointData {
                    id: point_id.to_string(),
                    name: config.get_name(),
                    value: value_str,
                    timestamp,
                    unit: config.unit.clone().unwrap_or_default(),
                    description: config.description.clone().unwrap_or_default(),
                };
                
                cache.insert(point_id, point_data);
                stats.write_operations += 1;
            }
        }
        
        Ok(())
    }

    /// Get point data from cache by numeric ID
    pub async fn get_point_data(&self, point_id: u32) -> Option<PointData> {
        let mut stats = self.stats.write().await;
        let cache = self.realtime_cache.read().await;
        
        if let Some(data) = cache.get(&point_id) {
            stats.cache_hits += 1;
            Some(data.clone())
        } else {
            stats.cache_misses += 1;
            None
        }
    }

    /// Get all cached point data
    pub async fn get_all_point_data(&self) -> Vec<PointData> {
        self.realtime_cache.read().await.values().cloned().collect()
    }

    /// Get cached point data by telemetry type
    pub async fn get_point_data_by_type(&self, telemetry_type: &TelemetryType) -> Vec<PointData> {
        let points_by_type = self.points_by_type.read().await;
        let cache = self.realtime_cache.read().await;
        
        if let Some(point_ids) = points_by_type.get(telemetry_type) {
            point_ids
                .iter()
                .filter_map(|&id| cache.get(&id).cloned())
                .collect()
        } else {
            Vec::new()
        }
    }

    /// Check if point exists and is enabled (O(1) lookup)
    pub async fn is_point_enabled(&self, point_id: u32) -> bool {
        self.enabled_points.read().await.contains(&point_id)
    }

    /// Check if point is readable (O(1) lookup)
    pub async fn is_point_readable(&self, point_id: u32) -> bool {
        self.readable_points.read().await.contains(&point_id)
    }

    /// Check if point is writable (O(1) lookup)
    pub async fn is_point_writable(&self, point_id: u32) -> bool {
        self.writable_points.read().await.contains(&point_id)
    }

    /// Get statistics
    pub async fn get_stats(&self) -> OptimizedPointManagerStats {
        self.stats.read().await.clone()
    }

    /// Get memory usage estimate
    pub async fn get_memory_usage(&self) -> usize {
        let points_size = self.points.read().await.len() * std::mem::size_of::<(u32, UniversalPointConfig)>();
        let cache_size = self.realtime_cache.read().await.len() * std::mem::size_of::<(u32, PointData)>();
        let indices_size = self.enabled_points.read().await.len() * std::mem::size_of::<u32>() * 3; // enabled, readable, writable
        
        points_size + cache_size + indices_size
    }

    /// Clear all data (for testing or reset)
    pub async fn clear_all(&self) {
        self.points.write().await.clear();
        self.realtime_cache.write().await.clear();
        self.points_by_type.write().await.clear();
        self.name_to_id.write().await.clear();
        self.enabled_points.write().await.clear();
        self.readable_points.write().await.clear();
        self.writable_points.write().await.clear();
        *self.stats.write().await = OptimizedPointManagerStats::default();
    }
}

/// Generate test points for demonstration
pub fn generate_test_points(count: usize) -> Vec<UniversalPointConfig> {
    let mut points = Vec::with_capacity(count);
    
    // Generate different types of points
    for i in 0..count {
        let telemetry_type = match i % 4 {
            0 => TelemetryType::Telemetry,
            1 => TelemetryType::Signaling,
            2 => TelemetryType::Control,
            _ => TelemetryType::Setpoint,
        };
        
        let point_id = 1000 + i as u32;
        let name = format!("Point_{:04}", point_id);
        
        let mut config = UniversalPointConfig::new(point_id, &name, telemetry_type.clone());
        
        // Add some variety to the configurations
        if i % 10 == 0 {
            config.enabled = false; // 10% disabled
        }
        
        if telemetry_type.is_analog() {
            config.unit = Some(match i % 3 {
                0 => "V".to_string(),
                1 => "A".to_string(),
                _ => "kW".to_string(),
            });
            config.scale = 1.0 + (i % 5) as f64 * 0.1;
            config.offset = (i % 3) as f64 * 10.0;
        }
        
        config.description = Some(format!("Test point for {:?}", telemetry_type));
        
        points.push(config);
    }
    
    points
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_optimized_point_manager() {
        let manager = OptimizedPointManager::new("test_channel".to_string());
        
        // Generate test points
        let points = generate_test_points(1000);
        
        // Load points
        manager.load_points(points).await.unwrap();
        
        // Check stats
        let stats = manager.get_stats().await;
        assert_eq!(stats.total_points, 1000);
        assert_eq!(stats.enabled_points, 900); // 90% enabled
        
        // Test O(1) lookups
        assert!(manager.is_point_enabled(1005).await);
        assert!(manager.is_point_readable(1005).await);
        
        // Test batch update
        let updates: Vec<(u32, PointValueType)> = (1000..1010)
            .map(|id| (id, PointValueType::Analog(100.0 + id as f64)))
            .collect();
        
        manager.batch_update_values(updates).await.unwrap();
        
        // Check cache
        let data = manager.get_point_data(1005).await;
        assert!(data.is_some());
    }

    #[tokio::test]
    async fn test_performance_comparison() {
        use std::time::Instant;
        
        let manager = OptimizedPointManager::new("perf_test".to_string());
        let points = generate_test_points(10000);
        
        let start = Instant::now();
        manager.load_points(points).await.unwrap();
        let load_time = start.elapsed();
        
        println!("Load 10000 points: {:?}", load_time);
        
        // Test lookup performance
        let start = Instant::now();
        for i in 0..1000 {
            let _ = manager.is_point_enabled(1000 + i).await;
        }
        let lookup_time = start.elapsed();
        
        println!("1000 enabled checks: {:?}", lookup_time);
        
        // Test batch update performance
        let updates: Vec<(u32, PointValueType)> = (1000..2000)
            .map(|id| (id, PointValueType::Analog(id as f64)))
            .collect();
        
        let start = Instant::now();
        manager.batch_update_values(updates).await.unwrap();
        let update_time = start.elapsed();
        
        println!("1000 batch updates: {:?}", update_time);
    }
}