//! Point Manager Module
//!
//! High-performance point manager with optimized data structures and indices.
//! Consolidated from point_manager.rs and optimized_point_manager.rs

use chrono::Utc;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

use super::core::PointData;
use crate::core::config::types::TelemetryType;
use crate::utils::Result;

/// Polling point configuration
#[derive(Debug, Clone)]
pub struct PollingPoint {
    pub id: String,
    pub name: String,
    pub address: u32,
    pub telemetry_type: TelemetryType,
    pub access_mode: String,
}

/// Point manager statistics
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
    /// Number of cache hits
    pub cache_hits: u64,
    /// Number of cache misses
    pub cache_misses: u64,
    /// Last update timestamp
    pub last_update: chrono::DateTime<Utc>,
}

/// Optimized universal point manager with u32 keys and multiple indices
#[derive(Clone, Debug)]
pub struct OptimizedPointManager {
    /// Point configurations indexed by numeric point ID (primary storage)
    points: Arc<RwLock<HashMap<u32, PollingPoint>>>,

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
    stats: Arc<RwLock<PointManagerStats>>,

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
            stats: Arc::new(RwLock::new(PointManagerStats::default())),
            channel_id,
        }
    }

    /// Load point configurations with optimized indices
    pub async fn load_points(&self, configs: Vec<PollingPoint>) -> Result<()> {
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

        // Build indices
        for config in configs {
            let point_id = config.id.parse::<u32>().unwrap_or(config.address); // Use id as point ID, fallback to address

            // Add to name mapping
            name_to_id.insert(config.name.to_string(), point_id);

            // Add to type index
            points_by_type
                .entry(config.telemetry_type)
                .or_insert_with(HashSet::new)
                .insert(point_id);

            // Add to access mode indices
            if config.access_mode.contains('r') {
                readable_points.insert(point_id);
            }
            if config.access_mode.contains('w') {
                writable_points.insert(point_id);
            }

            // Add to enabled points (assume all loaded points are enabled)
            enabled_points.insert(point_id);

            // Store the configuration
            points.insert(point_id, config);
        }

        // Update statistics
        stats.total_points = points.len();
        stats.enabled_points = enabled_points.len();
        stats.points_by_type.clear();
        for (telemetry_type, point_set) in points_by_type.iter() {
            stats
                .points_by_type
                .insert(*telemetry_type, point_set.len());
        }
        stats.last_update = Utc::now();

        info!(
            "[{}] Loaded {} points with optimized indices",
            self.channel_id,
            points.len()
        );

        Ok(())
    }

    /// Get point data by telemetry type
    pub async fn get_point_data_by_type(&self, telemetry_type: &TelemetryType) -> Vec<PointData> {
        let points_by_type = self.points_by_type.read().await;
        let realtime_cache = self.realtime_cache.read().await;

        if let Some(point_ids) = points_by_type.get(telemetry_type) {
            let mut result = Vec::with_capacity(point_ids.len());
            for &point_id in point_ids {
                if let Some(data) = realtime_cache.get(&point_id) {
                    result.push(data.clone());
                }
            }
            result
        } else {
            Vec::new()
        }
    }

    /// Get all point configurations
    pub async fn get_all_point_configs(&self) -> Vec<PollingPoint> {
        let points = self.points.read().await;
        points.values().cloned().collect()
    }

    /// Get enabled points by type
    pub async fn get_enabled_points_by_type(&self, telemetry_type: &TelemetryType) -> Vec<String> {
        let points_by_type = self.points_by_type.read().await;
        let enabled_points = self.enabled_points.read().await;
        let points = self.points.read().await;

        if let Some(type_points) = points_by_type.get(telemetry_type) {
            let mut result = Vec::new();
            for &point_id in type_points {
                if enabled_points.contains(&point_id) {
                    if let Some(config) = points.get(&point_id) {
                        result.push(config.id.to_string());
                    }
                }
            }
            result
        } else {
            Vec::new()
        }
    }

    /// Update point data in realtime cache
    pub async fn update_point_data(&self, point_id: u32, data: PointData) -> Result<()> {
        let mut cache = self.realtime_cache.write().await;
        cache.insert(point_id, data);

        // Update statistics
        let mut stats = self.stats.write().await;
        stats.write_operations += 1;

        Ok(())
    }

    /// Get point data by ID
    pub async fn get_point_data(&self, point_id: u32) -> Option<PointData> {
        let cache = self.realtime_cache.read().await;
        let mut stats = self.stats.write().await;

        if let Some(data) = cache.get(&point_id) {
            stats.cache_hits += 1;
            Some(data.clone())
        } else {
            stats.cache_misses += 1;
            None
        }
    }

    /// Get all point data
    pub async fn get_all_point_data(&self) -> Vec<PointData> {
        let cache = self.realtime_cache.read().await;
        cache.values().cloned().collect()
    }

    /// Get point configuration by ID
    pub async fn get_point_config(&self, point_id: u32) -> Option<PollingPoint> {
        let points = self.points.read().await;
        points.get(&point_id).cloned()
    }

    /// Get point ID by name
    pub async fn get_point_id_by_name(&self, name: &str) -> Option<u32> {
        let name_to_id = self.name_to_id.read().await;
        name_to_id.get(name).copied()
    }

    /// Check if point is readable
    pub async fn is_readable(&self, point_id: u32) -> bool {
        let readable_points = self.readable_points.read().await;
        readable_points.contains(&point_id)
    }

    /// Check if point is writable
    pub async fn is_writable(&self, point_id: u32) -> bool {
        let writable_points = self.writable_points.read().await;
        writable_points.contains(&point_id)
    }

    /// Get statistics
    pub async fn get_stats(&self) -> PointManagerStats {
        let stats = self.stats.read().await;
        stats.clone()
    }

    /// Get readable points for polling
    pub async fn get_readable_points(&self) -> Vec<PollingPoint> {
        let points = self.points.read().await;
        let readable_points = self.readable_points.read().await;
        let enabled_points = self.enabled_points.read().await;

        let mut result = Vec::new();
        for &point_id in readable_points.iter() {
            if enabled_points.contains(&point_id) {
                if let Some(config) = points.get(&point_id) {
                    result.push(config.clone());
                }
            }
        }
        result
    }

    /// Get points by IDs
    pub async fn get_points_by_ids(&self, point_ids: &[u32]) -> Vec<PollingPoint> {
        let points = self.points.read().await;
        let mut result = Vec::with_capacity(point_ids.len());

        for &point_id in point_ids {
            if let Some(config) = points.get(&point_id) {
                result.push(config.clone());
            }
        }
        result
    }

    /// Batch update point data
    pub async fn batch_update_point_data(&self, updates: Vec<(u32, PointData)>) -> Result<()> {
        let mut cache = self.realtime_cache.write().await;
        let mut stats = self.stats.write().await;

        for (point_id, data) in updates {
            cache.insert(point_id, data);
        }

        stats.write_operations += 1;
        Ok(())
    }

    /// Get cache hit rate
    pub async fn get_cache_hit_rate(&self) -> f64 {
        let stats = self.stats.read().await;
        let total = stats.cache_hits + stats.cache_misses;
        if total > 0 {
            stats.cache_hits as f64 / total as f64
        } else {
            0.0
        }
    }

    /// Access point configuration without cloning (for efficiency)
    pub async fn with_point_config<F, R>(&self, point_id: u32, f: F) -> Option<R>
    where
        F: FnOnce(&PollingPoint) -> R,
    {
        let points = self.points.read().await;
        points.get(&point_id).map(f)
    }

    /// Access all point configurations without cloning
    pub async fn with_all_point_configs<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&HashMap<u32, PollingPoint>) -> R,
    {
        let points = self.points.read().await;
        f(&points)
    }

    /// Access statistics without cloning
    pub async fn with_stats<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&PointManagerStats) -> R,
    {
        let stats = self.stats.read().await;
        f(&stats)
    }
}

/// Generate test points for performance testing
pub fn generate_test_points(count: usize) -> Vec<PollingPoint> {
    let mut points = Vec::with_capacity(count);

    for i in 0..count {
        let telemetry_type = match i % 4 {
            0 => TelemetryType::Measurement,
            1 => TelemetryType::Signal,
            2 => TelemetryType::Control,
            _ => TelemetryType::Adjustment,
        };

        let point = PollingPoint {
            id: format!("point_{i}"),
            name: format!("Test Point {i}"),
            address: i as u32 + 1000,
            telemetry_type,
            access_mode: "rw".to_string(),
        };

        points.push(point);
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
        let test_points = generate_test_points(100);

        // Load points
        manager.load_points(test_points).await.unwrap();

        // Test statistics
        let stats = manager.get_stats().await;
        assert_eq!(stats.total_points, 100);
        assert_eq!(stats.enabled_points, 100);

        // Test by type queries
        let telemetry_points = manager
            .get_point_data_by_type(&TelemetryType::Measurement)
            .await;
        assert_eq!(telemetry_points.len(), 0); // No data in cache yet

        let enabled_telemetry = manager
            .get_enabled_points_by_type(&TelemetryType::Measurement)
            .await;
        assert_eq!(enabled_telemetry.len(), 25); // 25% of points are telemetry

        // Test readable points
        let readable_points = manager.get_readable_points().await;
        assert_eq!(readable_points.len(), 100); // All points are readable (rw)
    }

    #[tokio::test]
    async fn test_performance_comparison() {
        let manager = OptimizedPointManager::new("perf_test".to_string());

        // Load large number of points
        let test_points = generate_test_points(10000);
        let start = std::time::Instant::now();
        manager.load_points(test_points).await.unwrap();
        let load_time = start.elapsed();

        // Test query performance
        let start = std::time::Instant::now();
        let _telemetry_points = manager
            .get_enabled_points_by_type(&TelemetryType::Measurement)
            .await;
        let query_time = start.elapsed();

        println!("Load time for 10k points: {load_time:?}");
        println!("Query time for telemetry points: {query_time:?}");

        // Query should be very fast with indices
        assert!(query_time.as_millis() < 10);
    }
}
