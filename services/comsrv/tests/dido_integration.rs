//! DI/DO Protocol Integration Tests
//!
//! Uses `/tmp` directory to simulate sysfs GPIO filesystem, enabling tests
//! to run in environments without GPIO hardware.
//!
//! # Running Tests
//! ```bash
//! cargo test --package comsrv --test dido_integration --features integration
//! ```
//!
//! # Important Notes
//!
//! GPIO filesystem simulation must be set up **before** protocol initialization,
//! because `SysfsGpioDriver.setup()` checks if the GPIO directory exists
//! to determine whether to export.

#![cfg(feature = "integration")]
#![allow(clippy::disallowed_methods)]

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use comsrv::core::channels::{ComBase, ComClient, ProtocolValue};
use comsrv::core::config::FourRemote;
use comsrv::protocols::dido::DiDoProtocol;
use voltage_config::comsrv::{
    ChannelConfig, ChannelCore, ControlPoint, GpioMapping, Point, RuntimeChannelConfig, SignalPoint,
};

/// Temporary GPIO filesystem simulator
///
/// Creates a simulated sysfs GPIO structure, enabling DI/DO protocol testing
/// on macOS/Linux without actual hardware.
///
/// **Important**: Must call `setup_gpio()` before `DiDoProtocol::initialize()`,
/// because initialization calls `SysfsGpioDriver::setup()` to configure GPIO direction.
pub struct TempGpioFs {
    base_path: PathBuf,
}

impl TempGpioFs {
    /// Create a temporary GPIO filesystem
    ///
    /// Uses process ID and timestamp to create a unique temporary directory,
    /// avoiding conflicts between tests
    pub fn new() -> Self {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base_path = PathBuf::from(format!(
            "/tmp/test_gpio_{}_{}",
            std::process::id(),
            timestamp
        ));

        // Clean up any existing old directory
        let _ = fs::remove_dir_all(&base_path);

        // Create base directory
        fs::create_dir_all(&base_path).expect("Failed to create temp GPIO directory");

        // Create export and unexport files
        fs::write(base_path.join("export"), "").expect("Failed to create export file");
        fs::write(base_path.join("unexport"), "").expect("Failed to create unexport file");

        Self { base_path }
    }

    /// Set up a GPIO point (create directory and files)
    ///
    /// **Must be called before `DiDoProtocol::initialize()`**
    pub fn setup_gpio(&self, gpio_number: u32, direction: &str, initial_value: bool) {
        let gpio_dir = self.base_path.join(format!("gpio{}", gpio_number));
        fs::create_dir_all(&gpio_dir).expect("Failed to create GPIO directory");

        // Write direction file
        fs::write(gpio_dir.join("direction"), direction).expect("Failed to write direction");

        // Write value file
        let value = if initial_value { "1" } else { "0" };
        fs::write(gpio_dir.join("value"), value).expect("Failed to write value");
    }

    /// Read GPIO value
    pub fn read_value(&self, gpio_number: u32) -> bool {
        let path = self.base_path.join(format!("gpio{}/value", gpio_number));
        match fs::read_to_string(&path) {
            Ok(content) => content.trim() == "1",
            Err(_) => false, // Return false when GPIO doesn't exist
        }
    }

    /// Write GPIO value (simulate external signal change)
    ///
    /// Creates the GPIO directory if it doesn't exist
    pub fn write_value(&self, gpio_number: u32, value: bool) {
        let gpio_dir = self.base_path.join(format!("gpio{}", gpio_number));
        // Ensure directory exists
        if !gpio_dir.exists() {
            fs::create_dir_all(&gpio_dir).expect("Failed to create GPIO directory");
            fs::write(gpio_dir.join("direction"), "in").expect("Failed to write direction");
        }

        let path = gpio_dir.join("value");
        let content = if value { "1" } else { "0" };
        fs::write(&path, content).expect("Failed to write GPIO value");
    }

    /// Get the base path
    pub fn base_path(&self) -> &str {
        self.base_path.to_str().expect("Invalid path")
    }
}

impl Drop for TempGpioFs {
    /// Automatically clean up temporary directory
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base_path);
    }
}

impl Default for TempGpioFs {
    fn default() -> Self {
        Self::new()
    }
}

/// Create RuntimeChannelConfig for testing
fn create_test_config(id: u32, gpio_base_path: &str) -> RuntimeChannelConfig {
    let mut parameters = HashMap::new();
    parameters.insert(
        "gpio_base_path".to_string(),
        serde_json::json!(gpio_base_path),
    );
    parameters.insert("di_poll_interval_ms".to_string(), serde_json::json!(50)); // Fast polling

    let config = ChannelConfig {
        core: ChannelCore {
            id,
            name: format!("Test DI/DO {}", id),
            description: Some("Integration test DI/DO".to_string()),
            protocol: "di_do".to_string(),
            enabled: true,
        },
        parameters,
        logging: Default::default(),
    };

    let mut runtime_config = RuntimeChannelConfig::from_base(config);

    // Add Signal points (DI)
    runtime_config.signal_points.push(SignalPoint {
        base: Point {
            point_id: 1,
            signal_name: "DI1_Status".to_string(),
            description: Some("Test DI point 1".to_string()),
            unit: None,
        },
        reverse: false,
    });
    runtime_config.signal_points.push(SignalPoint {
        base: Point {
            point_id: 2,
            signal_name: "DI2_Status_Reversed".to_string(),
            description: Some("Test DI point 2 (with reverse)".to_string()),
            unit: None,
        },
        reverse: true,
    });

    // Add Control points (DO)
    runtime_config.control_points.push(ControlPoint {
        base: Point {
            point_id: 1,
            signal_name: "DO1_Control".to_string(),
            description: Some("Test DO point 1".to_string()),
            unit: None,
        },
        control_type: "momentary".to_string(),
        on_value: 1,
        off_value: 0,
        pulse_duration_ms: None,
    });

    // Add GPIO mappings
    runtime_config.gpio_mappings.push(GpioMapping {
        channel_id: id,
        point_id: 1,
        telemetry_type: "S".to_string(),
        gpio_number: 496,
    });
    runtime_config.gpio_mappings.push(GpioMapping {
        channel_id: id,
        point_id: 2,
        telemetry_type: "S".to_string(),
        gpio_number: 497,
    });
    runtime_config.gpio_mappings.push(GpioMapping {
        channel_id: id,
        point_id: 1,
        telemetry_type: "C".to_string(),
        gpio_number: 504,
    });

    runtime_config
}

/// Full DI/DO flow integration test
#[tokio::test]
async fn test_dido_full_flow_with_mock_gpio() {
    // 1. Create temporary GPIO filesystem
    let gpio_fs = TempGpioFs::new();
    gpio_fs.setup_gpio(496, "in", false); // DI1
    gpio_fs.setup_gpio(497, "in", true); // DI2 (initially true)
    gpio_fs.setup_gpio(504, "out", false); // DO1

    // 2. Create RuntimeChannelConfig pointing to temporary directory
    let config = create_test_config(4, gpio_fs.base_path());

    // 3. Create and initialize protocol
    let mut protocol =
        DiDoProtocol::from_runtime_config(&config).expect("Failed to create protocol");
    protocol
        .initialize(Arc::new(config))
        .await
        .expect("Failed to initialize");
    protocol.connect().await.expect("Failed to connect");

    // Wait for first poll to complete
    // poll_interval is 50ms, waiting 200ms should be sufficient
    tokio::time::sleep(Duration::from_millis(200)).await;

    // 4. Verify DI reading
    let signals = protocol
        .read_four_telemetry(FourRemote::Signal)
        .await
        .expect("Failed to read signals");

    // DI1 (point_id=1): GPIO=false, reverse=false → result should be false
    if let Some(di1) = signals.get(&1) {
        match &di1.value {
            ProtocolValue::Bool(v) => assert!(!*v, "DI1 should be false"),
            _ => panic!("DI1 should be Bool type"),
        }
    }

    // DI2 (point_id=2): GPIO=true, reverse=true → result should be false (reversed)
    if let Some(di2) = signals.get(&2) {
        match &di2.value {
            ProtocolValue::Bool(v) => assert!(!*v, "DI2 should be false (reversed)"),
            _ => panic!("DI2 should be Bool type"),
        }
    }

    // 5. Simulate DI1 signal change
    gpio_fs.write_value(496, true);
    tokio::time::sleep(Duration::from_millis(200)).await; // Wait for polling

    // Re-read
    let signals = protocol
        .read_four_telemetry(FourRemote::Signal)
        .await
        .expect("Failed to read signals after change");

    if let Some(di1) = signals.get(&1) {
        match &di1.value {
            ProtocolValue::Bool(v) => assert!(*v, "DI1 should now be true"),
            _ => panic!("DI1 should be Bool type"),
        }
    }

    // 6. Execute DO control
    let results = protocol
        .control(vec![(1, ProtocolValue::Bool(true))])
        .await
        .expect("Failed to execute control");

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, 1);
    assert!(results[0].1, "Control command should succeed");

    // 7. Verify DO state
    assert!(
        gpio_fs.read_value(504),
        "GPIO 504 should be true after control"
    );

    // 8. Read Control telemetry (DO state)
    let controls = protocol
        .read_four_telemetry(FourRemote::Control)
        .await
        .expect("Failed to read controls");

    if let Some(do1) = controls.get(&1) {
        match &do1.value {
            ProtocolValue::Bool(v) => assert!(*v, "DO1 should be true in cache"),
            _ => panic!("DO1 should be Bool type"),
        }
    }

    // 9. Disconnect
    protocol.disconnect().await.expect("Failed to disconnect");
}

/// Test behavior when GPIO doesn't exist
///
/// When GPIO directory doesn't exist:
/// - `initialize()` attempts setup but doesn't fail
/// - `control()` attempts to write, may fail due to missing directory
#[tokio::test]
async fn test_dido_gpio_not_exists() {
    // Create temporary directory without setting up GPIO points
    let gpio_fs = TempGpioFs::new();
    // Don't call setup_gpio, simulating non-existent GPIO

    let config = create_test_config(4, gpio_fs.base_path());
    let mut protocol =
        DiDoProtocol::from_runtime_config(&config).expect("Failed to create protocol");

    protocol
        .initialize(Arc::new(config))
        .await
        .expect("Initialize should succeed even without GPIO");

    protocol
        .connect()
        .await
        .expect("Connect should succeed (simulation mode)");

    // Control command will attempt to write to GPIO
    let results = protocol
        .control(vec![(1, ProtocolValue::Bool(true))])
        .await
        .expect("Control should not panic");

    // Note: SysfsGpioDriver's setup may create directory (via export), so it might succeed
    // Here we only verify it doesn't panic
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, 1);
    // Result may succeed or fail depending on whether setup created the directory

    protocol
        .disconnect()
        .await
        .expect("Disconnect should succeed");
}

/// Test polling multiple DI points
///
/// Verifies:
/// 1. Multiple DI points can be polled correctly
/// 2. GPIO value changes are detected
/// 3. Reverse logic is correctly applied
#[tokio::test]
async fn test_dido_multiple_di_polling() {
    let gpio_fs = TempGpioFs::new();
    gpio_fs.setup_gpio(496, "in", false);
    gpio_fs.setup_gpio(497, "in", false);

    let config = create_test_config(4, gpio_fs.base_path());
    let mut protocol =
        DiDoProtocol::from_runtime_config(&config).expect("Failed to create protocol");

    protocol
        .initialize(Arc::new(config))
        .await
        .expect("Failed to initialize");
    protocol.connect().await.expect("Failed to connect");

    // Wait for polling - use longer time to ensure at least one poll completes
    // poll_interval is 50ms, waiting 200ms should be sufficient
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Initial state: GPIO 496=false, GPIO 497=false
    // DI1: reverse=false → false
    // DI2: reverse=true → true
    let signals = protocol
        .read_four_telemetry(FourRemote::Signal)
        .await
        .unwrap();

    // Verify we have 2 signal points
    assert_eq!(
        signals.len(),
        2,
        "Expected 2 signals, got {}. Signals: {:?}",
        signals.len(),
        signals.keys().collect::<Vec<_>>()
    );

    // Change GPIO 496 value
    gpio_fs.write_value(496, true);
    tokio::time::sleep(Duration::from_millis(200)).await;

    let signals = protocol
        .read_four_telemetry(FourRemote::Signal)
        .await
        .unwrap();

    // DI1 should become true (GPIO=true, reverse=false)
    if let Some(di1) = signals.get(&1) {
        match &di1.value {
            ProtocolValue::Bool(v) => assert!(*v, "DI1 should be true after GPIO change"),
            _ => panic!("Should be bool"),
        }
    }

    // DI2 should remain true (GPIO=false, reverse=true)
    if let Some(di2) = signals.get(&2) {
        match &di2.value {
            ProtocolValue::Bool(v) => assert!(*v, "DI2 with reverse should be true"),
            _ => panic!("Should be bool"),
        }
    }

    protocol.disconnect().await.unwrap();
}
