//! DI/DO (Digital Input/Output) Protocol - Low-level Implementation
//!
//! Provides GPIO-based digital I/O for local hardware (e.g., ECU-1170).
//! Supports sysfs driver for Linux GPIO access.
//!
//! # Architecture
//!
//! This module provides the low-level GPIO operations:
//! - `GpioDriver` trait for abstraction
//! - `SysfsGpioDriver` for Linux sysfs GPIO access
//! - `GpioPoint` for point configuration
//!
//! The service layer (`comsrv/protocols/dido.rs`) wraps this with
//! `ComClient` trait implementation.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, warn};

/// GPIO point configuration
#[derive(Debug, Clone)]
pub struct GpioPoint {
    pub point_id: u32,
    pub gpio_number: u32,
    pub reverse: bool,
}

/// GPIO driver trait for abstraction
pub trait GpioDriver: Send + Sync {
    /// Read GPIO value
    fn read(&self, gpio_number: u32) -> Result<bool, String>;

    /// Write GPIO value
    fn write(&self, gpio_number: u32, value: bool) -> Result<(), String>;

    /// Setup GPIO with direction
    fn setup(&self, gpio_number: u32, direction: &str) -> Result<(), String>;

    /// Check if driver is available
    fn is_available(&self) -> bool;
}

/// Sysfs GPIO driver for Linux
pub struct SysfsGpioDriver {
    base_path: String,
}

impl SysfsGpioDriver {
    pub fn new(base_path: &str) -> Self {
        Self {
            base_path: base_path.to_string(),
        }
    }
}

impl Default for SysfsGpioDriver {
    fn default() -> Self {
        Self::new("/sys/class/gpio")
    }
}

impl GpioDriver for SysfsGpioDriver {
    fn read(&self, gpio_number: u32) -> Result<bool, String> {
        let path = format!("{}/gpio{}/value", self.base_path, gpio_number);
        match std::fs::read_to_string(&path) {
            Ok(content) => {
                let value = content.trim() == "1";
                Ok(value)
            },
            Err(e) => {
                debug!("Failed to read GPIO {}: {}", gpio_number, e);
                Err(format!("GPIO {} read failed: {}", gpio_number, e))
            },
        }
    }

    fn write(&self, gpio_number: u32, value: bool) -> Result<(), String> {
        let path = format!("{}/gpio{}/value", self.base_path, gpio_number);
        let content = if value { "1" } else { "0" };
        match std::fs::write(&path, content) {
            Ok(()) => Ok(()),
            Err(e) => {
                warn!("Failed to write GPIO {}: {}", gpio_number, e);
                Err(format!("GPIO {} write failed: {}", gpio_number, e))
            },
        }
    }

    fn setup(&self, gpio_number: u32, direction: &str) -> Result<(), String> {
        // Export GPIO if not already exported
        let export_path = format!("{}/export", self.base_path);
        let gpio_path = format!("{}/gpio{}", self.base_path, gpio_number);

        if !Path::new(&gpio_path).exists() {
            if let Err(e) = std::fs::write(&export_path, gpio_number.to_string()) {
                // Ignore "device busy" errors (already exported)
                if !e.to_string().contains("Device or resource busy") {
                    warn!("Failed to export GPIO {}: {}", gpio_number, e);
                }
            }
        }

        // Set direction
        let direction_path = format!("{}/direction", gpio_path);
        if let Err(e) = std::fs::write(&direction_path, direction) {
            warn!(
                "Failed to set GPIO {} direction to {}: {}",
                gpio_number, direction, e
            );
        }

        Ok(())
    }

    fn is_available(&self) -> bool {
        Path::new(&self.base_path).exists()
    }
}

/// DI/DO controller for managing GPIO points
pub struct DiDoController {
    driver: Box<dyn GpioDriver>,
    di_points: Vec<GpioPoint>,
    do_points: Vec<GpioPoint>,
    di_cache: Arc<RwLock<HashMap<u32, bool>>>,
    do_cache: Arc<RwLock<HashMap<u32, bool>>>,
}

impl DiDoController {
    /// Create a new DI/DO controller with the given driver
    pub fn new(driver: Box<dyn GpioDriver>) -> Self {
        Self {
            driver,
            di_points: Vec::new(),
            do_points: Vec::new(),
            di_cache: Arc::new(RwLock::new(HashMap::new())),
            do_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create with default sysfs driver
    pub fn with_sysfs(base_path: &str) -> Self {
        Self::new(Box::new(SysfsGpioDriver::new(base_path)))
    }

    /// Check if the driver is available
    pub fn is_available(&self) -> bool {
        self.driver.is_available()
    }

    /// Add a DI (input) point
    pub fn add_di_point(&mut self, point: GpioPoint) {
        // Setup GPIO as input
        let _ = self.driver.setup(point.gpio_number, "in");
        self.di_points.push(point);
    }

    /// Add a DO (output) point
    pub fn add_do_point(&mut self, point: GpioPoint) {
        // Setup GPIO as output
        let _ = self.driver.setup(point.gpio_number, "out");
        self.do_points.push(point);
    }

    /// Get DI point count
    pub fn di_count(&self) -> usize {
        self.di_points.len()
    }

    /// Get DO point count
    pub fn do_count(&self) -> usize {
        self.do_points.len()
    }

    /// Poll all DI points and update cache
    pub async fn poll_di(&self) {
        let mut cache = self.di_cache.write().await;

        for point in &self.di_points {
            match self.driver.read(point.gpio_number) {
                Ok(raw_value) => {
                    // Apply reverse logic
                    let value = if point.reverse { !raw_value } else { raw_value };
                    cache.insert(point.point_id, value);
                },
                Err(_) => {
                    // Keep previous value on error
                },
            }
        }
    }

    /// Read all cached DI values
    pub async fn read_di_cache(&self) -> HashMap<u32, bool> {
        self.di_cache.read().await.clone()
    }

    /// Read all cached DO values
    pub async fn read_do_cache(&self) -> HashMap<u32, bool> {
        self.do_cache.read().await.clone()
    }

    /// Write a DO point
    pub async fn write_do(&self, point_id: u32, value: bool) -> Result<(), String> {
        // Find the GPIO point
        let point = self
            .do_points
            .iter()
            .find(|p| p.point_id == point_id)
            .ok_or_else(|| format!("DO point {} not found", point_id))?;

        // Apply reverse logic
        let gpio_value = if point.reverse { !value } else { value };

        // Write to GPIO
        self.driver.write(point.gpio_number, gpio_value)?;

        // Update cache
        self.do_cache.write().await.insert(point_id, value);

        debug!(
            "DO point {} (GPIO {}) set to {}",
            point_id, point.gpio_number, value
        );

        Ok(())
    }
}

impl std::fmt::Debug for DiDoController {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DiDoController")
            .field("di_points", &self.di_points.len())
            .field("do_points", &self.do_points.len())
            .finish()
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod tests {
    use super::*;

    /// Mock GPIO driver for testing
    struct MockGpioDriver {
        values: std::sync::Mutex<HashMap<u32, bool>>,
    }

    impl MockGpioDriver {
        fn new() -> Self {
            Self {
                values: std::sync::Mutex::new(HashMap::new()),
            }
        }
    }

    impl GpioDriver for MockGpioDriver {
        fn read(&self, gpio_number: u32) -> Result<bool, String> {
            let values = self.values.lock().expect("lock poisoned");
            Ok(*values.get(&gpio_number).unwrap_or(&false))
        }

        fn write(&self, gpio_number: u32, value: bool) -> Result<(), String> {
            let mut values = self.values.lock().expect("lock poisoned");
            values.insert(gpio_number, value);
            Ok(())
        }

        fn setup(&self, _gpio_number: u32, _direction: &str) -> Result<(), String> {
            Ok(())
        }

        fn is_available(&self) -> bool {
            true
        }
    }

    #[tokio::test]
    async fn test_controller_creation() {
        let controller = DiDoController::new(Box::new(MockGpioDriver::new()));
        assert_eq!(controller.di_count(), 0);
        assert_eq!(controller.do_count(), 0);
    }

    #[tokio::test]
    async fn test_add_points() {
        let mut controller = DiDoController::new(Box::new(MockGpioDriver::new()));

        controller.add_di_point(GpioPoint {
            point_id: 1,
            gpio_number: 496,
            reverse: false,
        });

        controller.add_do_point(GpioPoint {
            point_id: 1,
            gpio_number: 504,
            reverse: false,
        });

        assert_eq!(controller.di_count(), 1);
        assert_eq!(controller.do_count(), 1);
    }

    #[tokio::test]
    async fn test_write_do() {
        let controller = DiDoController::new(Box::new(MockGpioDriver::new()));

        // Add DO point
        let mut controller = controller;
        controller.add_do_point(GpioPoint {
            point_id: 1,
            gpio_number: 504,
            reverse: false,
        });

        // Write and verify
        let result = controller.write_do(1, true).await;
        assert!(result.is_ok());

        let cache = controller.read_do_cache().await;
        assert_eq!(cache.get(&1), Some(&true));
    }

    #[tokio::test]
    async fn test_read_di_with_reverse() {
        let mock = MockGpioDriver::new();
        // Preset GPIO 496 value to true
        mock.values.lock().expect("lock").insert(496, true);

        let mut controller = DiDoController::new(Box::new(mock));

        // Add DI point with reverse = true (inverted logic)
        controller.add_di_point(GpioPoint {
            point_id: 1,
            gpio_number: 496,
            reverse: true,
        });

        // Poll to update cache
        controller.poll_di().await;

        // Verify reverse logic: GPIO is true, but reverse=true so cache should be false
        let cache = controller.read_di_cache().await;
        assert_eq!(cache.get(&1), Some(&false));
    }

    #[tokio::test]
    async fn test_poll_di_updates_cache() {
        let mock = MockGpioDriver::new();
        let mut controller = DiDoController::new(Box::new(mock));

        controller.add_di_point(GpioPoint {
            point_id: 1,
            gpio_number: 496,
            reverse: false,
        });
        controller.add_di_point(GpioPoint {
            point_id: 2,
            gpio_number: 497,
            reverse: false,
        });

        // Initial poll - all values should be false (MockGpioDriver default)
        controller.poll_di().await;
        let cache = controller.read_di_cache().await;
        assert_eq!(cache.get(&1), Some(&false));
        assert_eq!(cache.get(&2), Some(&false));

        // Verify cache contains both points
        assert_eq!(cache.len(), 2);
    }

    #[tokio::test]
    async fn test_write_do_with_reverse() {
        let mock = MockGpioDriver::new();
        let mock_values = mock.values.lock().expect("lock").clone();
        drop(mock_values);

        let mut controller = DiDoController::new(Box::new(mock));

        // Add DO point with reverse = true
        controller.add_do_point(GpioPoint {
            point_id: 1,
            gpio_number: 504,
            reverse: true,
        });

        // Write true, but with reverse=true, GPIO should receive false
        let result = controller.write_do(1, true).await;
        assert!(result.is_ok());

        // Cache stores the logical value (true)
        let cache = controller.read_do_cache().await;
        assert_eq!(cache.get(&1), Some(&true));
    }

    #[tokio::test]
    async fn test_point_not_found_error() {
        let mock = MockGpioDriver::new();
        let controller = DiDoController::new(Box::new(mock));

        // Try to write without adding any points
        let result = controller.write_do(999, true).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }
}
