//! Instance Data Logger Module
//!
//! Logs instance runtime data:
//! - Measurement point (M) periodic snapshots (no change tracking)
//! - Action point (A) change tracking (immediate logging with metadata)
//! - Unified snapshot format for easy parsing

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, warn};

/// Data tracker for a single instance
#[derive(Debug)]
pub struct InstanceDataTracker {
    /// Instance name (used for logging)
    pub instance_name: String,
    /// Previous action values: point_id -> value (for change detection)
    #[allow(dead_code)]
    actions: HashMap<String, String>,
    /// Last snapshot time
    last_snapshot: Instant,
    /// Snapshot interval in seconds
    snapshot_interval: Duration,
    /// Verbose logging mode (include point names and extra metadata)
    #[allow(dead_code)]
    verbose: bool,
}

impl InstanceDataTracker {
    /// Create a new tracker for an instance
    pub fn new(instance_name: String, snapshot_interval_secs: u64) -> Self {
        let verbose = std::env::var("INSTANCE_LOG_VERBOSE")
            .unwrap_or_else(|_| "false".to_string())
            .parse()
            .unwrap_or(false);

        Self {
            instance_name,
            actions: HashMap::new(),
            last_snapshot: Instant::now(),
            snapshot_interval: Duration::from_secs(snapshot_interval_secs),
            verbose,
        }
    }

    /// Check if a snapshot is due
    pub fn should_snapshot(&self) -> bool {
        self.last_snapshot.elapsed() >= self.snapshot_interval
    }

    /// Log measurement snapshot (no change tracking)
    /// This method directly logs all current M-point values without comparing to previous state
    pub fn log_measurement_snapshot(
        &mut self,
        measurements: &HashMap<String, f64>,
        actions: &HashMap<String, String>,
    ) {
        // Build measurement data (sorted for consistency)
        let mut m_values: Vec<String> = measurements
            .iter()
            .filter(|(k, _)| !k.starts_with('_'))
            .map(|(k, v)| format!("\"{}\":\"{:.2}\"", k, v))
            .collect();
        m_values.sort();

        // Build action data (sorted for consistency)
        let mut a_values: Vec<String> = actions
            .iter()
            .filter(|(k, _)| !k.starts_with('_'))
            .map(|(k, v)| format!("\"{}\":\"{}\"", k, v))
            .collect();
        a_values.sort();

        let message = format!(
            "SNAPSHOT | M_count={}, A_count={}, uptime={:.0}s\n  M: {{{}}}\n  A: {{{}}}",
            measurements.len(),
            actions.len(),
            self.last_snapshot.elapsed().as_secs_f64(),
            m_values.join(","),
            a_values.join(",")
        );

        if let Err(e) = common::logging::write_to_instance_log(&self.instance_name, &message) {
            warn!(
                "Failed to write instance snapshot for {}: {}",
                self.instance_name, e
            );
        }

        // Update snapshot timestamp
        self.last_snapshot = Instant::now();
    }

    /// Update action data and log changes with metadata (immediate)
    #[allow(dead_code)]
    pub fn update_actions(
        &mut self,
        new_data: &HashMap<String, String>,
        source: &str,
        metadata: Option<&HashMap<String, String>>,
    ) {
        for (point_id, new_value) in new_data {
            // Skip metadata fields
            if point_id.starts_with('_') {
                continue;
            }

            if let Some(old_value) = self.actions.get(point_id) {
                // Log action changes immediately
                if new_value != old_value {
                    let mut message = format!(
                        "A-SET | point_id={}, value={}→{}, source={}",
                        point_id, old_value, new_value, source
                    );

                    // Add metadata if provided
                    if let Some(meta) = metadata {
                        let mut meta_parts: Vec<String> =
                            meta.iter().map(|(k, v)| format!("{}={}", k, v)).collect();
                        meta_parts.sort();
                        if !meta_parts.is_empty() {
                            message.push_str(&format!(", {}", meta_parts.join(", ")));
                        }
                    }

                    if let Err(e) =
                        common::logging::write_to_instance_log(&self.instance_name, &message)
                    {
                        warn!(
                            "Failed to write instance log for {}: {}",
                            self.instance_name, e
                        );
                    }
                }

                // Update tracked value
                self.actions.insert(point_id.clone(), new_value.clone());
            } else {
                // First time seeing this action point
                self.actions.insert(point_id.clone(), new_value.clone());

                let message = format!(
                    "A-INIT | point_id={}, value={}, source={}",
                    point_id, new_value, source
                );
                if let Err(e) =
                    common::logging::write_to_instance_log(&self.instance_name, &message)
                {
                    warn!(
                        "Failed to write instance log for {}: {}",
                        self.instance_name, e
                    );
                }
            }
        }
    }
}

/// Global tracker registry for all instances
pub type TrackerRegistry = Arc<RwLock<HashMap<String, InstanceDataTracker>>>;

/// Create a new tracker registry
pub fn create_tracker_registry() -> TrackerRegistry {
    Arc::new(RwLock::new(HashMap::new()))
}

/// Get or create tracker for an instance
pub async fn get_or_create_tracker(
    registry: &TrackerRegistry,
    instance_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut registry = registry.write().await;

    if !registry.contains_key(instance_name) {
        // Read configuration from environment
        let snapshot_interval = std::env::var("INSTANCE_LOG_INTERVAL")
            .unwrap_or_else(|_| "60".to_string())
            .parse()
            .unwrap_or(60);

        let tracker = InstanceDataTracker::new(instance_name.to_string(), snapshot_interval);

        debug!(
            "Created tracker for instance {} (snapshot_interval={}s)",
            instance_name, snapshot_interval,
        );

        registry.insert(instance_name.to_string(), tracker);
    }

    Ok(())
}

/// Log measurement snapshot for an instance
pub async fn log_snapshot(
    registry: &TrackerRegistry,
    instance_name: &str,
    measurements: &HashMap<String, f64>,
    actions: &HashMap<String, String>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Ensure tracker exists
    get_or_create_tracker(registry, instance_name).await?;

    let mut registry = registry.write().await;

    if let Some(tracker) = registry.get_mut(instance_name) {
        tracker.log_measurement_snapshot(measurements, actions);
    }

    Ok(())
}

/// Check if snapshot is due for an instance
pub async fn should_snapshot(
    registry: &TrackerRegistry,
    instance_name: &str,
) -> Result<bool, Box<dyn std::error::Error>> {
    // Ensure tracker exists
    get_or_create_tracker(registry, instance_name).await?;

    let registry = registry.read().await;

    if let Some(tracker) = registry.get(instance_name) {
        Ok(tracker.should_snapshot())
    } else {
        Ok(false)
    }
}

/// Update actions for an instance and log changes with metadata
#[allow(dead_code)]
pub async fn log_action_changes(
    registry: &TrackerRegistry,
    instance_name: &str,
    data: &HashMap<String, String>,
    source: &str,
    metadata: Option<&HashMap<String, String>>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Ensure tracker exists
    get_or_create_tracker(registry, instance_name).await?;

    let mut registry = registry.write().await;

    if let Some(tracker) = registry.get_mut(instance_name) {
        tracker.update_actions(data, source, metadata);
    }

    Ok(())
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;

    #[test]
    fn test_tracker_creation() {
        let tracker = InstanceDataTracker::new("test_instance".to_string(), 60);
        assert_eq!(tracker.instance_name, "test_instance");
        assert_eq!(tracker.snapshot_interval, Duration::from_secs(60));
    }

    #[test]
    fn test_snapshot_logging() {
        let mut tracker = InstanceDataTracker::new("test".to_string(), 60);

        // Create test data
        let mut measurements = HashMap::new();
        measurements.insert("1".to_string(), 100.5);
        measurements.insert("2".to_string(), 200.3);

        let mut actions = HashMap::new();
        actions.insert("10".to_string(), "1".to_string());
        actions.insert("11".to_string(), "0".to_string());

        // Log snapshot (should not panic)
        tracker.log_measurement_snapshot(&measurements, &actions);
    }

    #[test]
    fn test_action_change_detection() {
        let mut tracker = InstanceDataTracker::new("test".to_string(), 60);

        // Initial update
        let mut data1 = HashMap::new();
        data1.insert("10".to_string(), "0".to_string());
        tracker.update_actions(&data1, "test", None);

        assert_eq!(tracker.actions.len(), 1);

        // Value change
        let mut data2 = HashMap::new();
        data2.insert("10".to_string(), "1".to_string());
        tracker.update_actions(&data2, "test", None);

        // Should still be 1 (updated, not added)
        assert_eq!(tracker.actions.len(), 1);
    }

    #[test]
    fn test_action_with_metadata() {
        let mut tracker = InstanceDataTracker::new("test".to_string(), 60);

        let mut data = HashMap::new();
        data.insert("10".to_string(), "1".to_string());

        let mut metadata = HashMap::new();
        metadata.insert("rule_id".to_string(), "battery_protect".to_string());
        metadata.insert("user".to_string(), "admin".to_string());

        tracker.update_actions(&data, "rulesrv", Some(&metadata));
        assert_eq!(tracker.actions.len(), 1);
    }

    #[test]
    fn test_snapshot_interval() {
        let tracker = InstanceDataTracker::new("test".to_string(), 1);

        // Initially should not need snapshot (just created)
        assert!(!tracker.should_snapshot());

        // Note: We can't test elapsed time in unit tests without sleeping
        // This test mainly verifies the method doesn't panic
    }
}
