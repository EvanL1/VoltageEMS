//! GPIO (General Purpose Input/Output) protocol adapter.
//!
//! Provides direct hardware GPIO control on Linux systems with a pluggable driver architecture.
//!
//! # Platform Support
//!
//! **Linux only**: Supports multiple GPIO backends:
//! - `gpiod`: Modern character device interface (libgpiod v2) - **recommended**
//! - `sysfs`: Legacy sysfs interface (`/sys/class/gpio/`) - for compatibility
//!
//! # Feature Flag
//!
//! Requires `gpio` feature to be enabled.
//!
//! # Driver Architecture
//!
//! The GPIO module uses a trait-based driver system for extensibility:
//!
//! ```text
//!              ┌─────────────────────────┐
//!              │     GpioDriver trait    │
//!              └───────────┬─────────────┘
//!                          │
//!      ┌───────────┬───────┼───────┬───────────┐
//!      ▼           ▼       ▼       ▼           ▼
//!   GpiodDriver  SysfsDriver  (future)  MockDriver  ...
//!   (chardev)    (/sys/)               (testing)
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! use igw::protocols::gpio::{GpioChannel, GpioChannelConfig, GpioPinConfig, GpioDriverType};
//!
//! // Using gpiod (chardev) - recommended
//! let config = GpioChannelConfig::new()
//!     .with_driver(GpioDriverType::Gpiod)
//!     .add_pin(GpioPinConfig::digital_input("gpiochip0", 17, 1))
//!     .add_pin(GpioPinConfig::digital_output("gpiochip0", 18, 101));
//!
//! // Using sysfs - for legacy compatibility
//! let config = GpioChannelConfig::new()
//!     .with_driver(GpioDriverType::Sysfs { base_path: "/sys/class/gpio".into() })
//!     .add_pin(GpioPinConfig::digital_input_sysfs(490, 1))  // GPIO 490
//!     .add_pin(GpioPinConfig::digital_output_sysfs(491, 101));
//!
//! let mut gpio = GpioChannel::new(config);
//! gpio.connect().await?;
//!
//! // Read DI
//! let response = gpio.read(ReadRequest::all()).await?;
//!
//! // Control DO
//! gpio.write_control(&[ControlCommand::latching(101, true)]).await?;
//! ```

use std::borrow::Cow;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use tokio::sync::RwLock;

use tokio_gpiod::{Chip, Options};

use serde::Deserialize;

use crate::protocols::core::data::{DataBatch, DataPoint};
use crate::protocols::core::error::{GatewayError, Result};
use crate::protocols::core::logging::{
    ChannelLogConfig, ChannelLogHandler, ErrorContext, LogContext, LoggableProtocol,
};
use crate::protocols::core::metadata::{
    DriverMetadata, HasMetadata, ParameterMetadata, ParameterType,
};
use crate::protocols::core::slot::AtomicBoolStore;
use crate::protocols::core::traits::{
    AdjustmentCommand, CommunicationMode, ConnectionState, ControlCommand, DataEventReceiver,
    Diagnostics, PointFailure, PollResult, Protocol, ProtocolCapabilities, ProtocolClient,
    WriteResult,
};
use crate::protocols::gateway::ChannelRuntime;

// ============================================================================
// GPIO Driver Trait (Extensible Driver Architecture)
// ============================================================================

/// GPIO driver type selection.
///
/// Determines which backend is used for GPIO operations.
#[derive(Debug, Clone)]
pub enum GpioDriverType {
    /// Modern character device interface (libgpiod v2).
    /// Uses `/dev/gpiochipN` devices. **Recommended for new projects.**
    Gpiod,

    /// Legacy sysfs interface.
    /// Uses `/sys/class/gpio/` filesystem. For compatibility with older systems.
    Sysfs {
        /// Base path for sysfs GPIO (default: "/sys/class/gpio")
        base_path: String,
    },
}

impl Default for GpioDriverType {
    fn default() -> Self {
        Self::Gpiod
    }
}

// ============================================================================
// Strongly-typed mapping configs for JSON deserialization
// ============================================================================

/// GPIO point mapping configuration (deserialized from protocol_mappings JSON).
///
/// # Required Fields
/// - `gpio_number`: The GPIO pin number. This field is **required** and
///   deserialization will fail if missing.
///
/// # Optional Fields
/// - `gpio_chip`: GPIO chip name (default: "gpiochip0")
/// - `active_low`: Invert the logic level (default: false)
/// - `debounce_us`: Debounce time in microseconds (default: 0)
///
/// # Example JSON
/// ```json
/// {
///     "gpio_number": 496,
///     "gpio_chip": "gpiochip0",
///     "active_low": false
/// }
/// ```
#[derive(Debug, Clone, Deserialize)]
pub struct GpioMappingConfig {
    /// GPIO pin number (sysfs number or chip offset). **Required field**.
    pub gpio_number: u32,

    /// GPIO chip name (default: "gpiochip0").
    #[serde(default = "default_gpio_chip")]
    pub gpio_chip: String,

    /// Active low - invert the logic level.
    #[serde(default)]
    pub active_low: bool,

    /// Debounce time in microseconds.
    #[serde(default)]
    pub debounce_us: u64,
}

fn default_gpio_chip() -> String {
    "gpiochip0".to_string()
}

impl GpioMappingConfig {
    /// Convert to igw GpioPinConfig for input (DI).
    ///
    /// If `gpio_chip` is "gpiochip0" (default) and `gpio_number` >= 32,
    /// the driver will auto-resolve the global GPIO number to the correct chip.
    pub fn to_input_pin_config(&self, point_id: u32) -> GpioPinConfig {
        GpioPinConfig::digital_input(&self.gpio_chip, self.gpio_number, point_id)
            .with_gpio_number(self.gpio_number) // Enable auto-resolution
            .with_active_low(self.active_low)
            .with_debounce(self.debounce_us)
    }

    /// Convert to igw GpioPinConfig for output (DO).
    ///
    /// If `gpio_chip` is "gpiochip0" (default) and `gpio_number` >= 32,
    /// the driver will auto-resolve the global GPIO number to the correct chip.
    pub fn to_output_pin_config(&self, point_id: u32) -> GpioPinConfig {
        GpioPinConfig::digital_output(&self.gpio_chip, self.gpio_number, point_id)
            .with_gpio_number(self.gpio_number) // Enable auto-resolution
            .with_active_low(self.active_low)
    }
}

/// GPIO channel parameters configuration (deserialized from parameters_json).
///
/// # Example JSON
/// ```json
/// {
///     "driver": "gpiod",
///     "gpio_chip": "gpiochip0",
///     "poll_interval_ms": 200
/// }
/// ```
#[derive(Debug, Clone, Deserialize)]
pub struct GpioChannelParamsConfig {
    /// Driver type: "gpiod" or "sysfs".
    #[serde(default = "default_driver")]
    pub driver: String,

    /// Sysfs base path (only for sysfs driver).
    #[serde(default = "default_sysfs_path")]
    pub sysfs_base_path: String,

    /// Poll interval in milliseconds.
    #[serde(default = "default_poll_interval")]
    pub poll_interval_ms: u64,
}

fn default_driver() -> String {
    "gpiod".to_string()
}

fn default_sysfs_path() -> String {
    "/sys/class/gpio".to_string()
}

fn default_poll_interval() -> u64 {
    200
}

impl GpioChannelParamsConfig {
    /// Get the GPIO driver type from configuration.
    pub fn driver_type(&self) -> GpioDriverType {
        match self.driver.to_lowercase().as_str() {
            "sysfs" => GpioDriverType::Sysfs {
                base_path: self.sysfs_base_path.clone(),
            },
            _ => GpioDriverType::Gpiod,
        }
    }

    /// Convert to GpioChannelConfig.
    pub fn to_config(&self) -> GpioChannelConfig {
        GpioChannelConfig {
            driver: self.driver_type(),
            pins: Vec::new(), // Pins are added via point configs
            poll_interval: std::time::Duration::from_millis(self.poll_interval_ms),
        }
    }
}

/// GPIO driver trait - extensible interface for GPIO backends.
///
/// Implement this trait to add support for new GPIO backends (e.g., BDaq, custom hardware).
///
/// # Example
///
/// ```rust,ignore
/// pub struct MyCustomDriver { /* ... */ }
///
/// #[async_trait]
/// impl GpioDriver for MyCustomDriver {
///     fn name(&self) -> &'static str { "my-custom" }
///     async fn read_pin(&self, gpio_num: u32) -> Result<bool> { /* ... */ }
///     async fn write_pin(&self, gpio_num: u32, value: bool) -> Result<()> { /* ... */ }
/// }
/// ```
#[async_trait]
pub trait GpioDriver: Send + Sync {
    /// Driver name for diagnostics.
    fn name(&self) -> &'static str;

    /// Read a GPIO pin value.
    ///
    /// # Arguments
    /// * `pin` - Pin configuration (contains chip/gpio_number, direction, etc.)
    ///
    /// # Returns
    /// Raw pin value (before active_low adjustment).
    async fn read_pin(&self, pin: &GpioPinConfig) -> Result<bool>;

    /// Write a GPIO pin value.
    ///
    /// # Arguments
    /// * `pin` - Pin configuration
    /// * `value` - Value to write (before active_low adjustment)
    async fn write_pin(&self, pin: &GpioPinConfig, value: bool) -> Result<()>;

    /// Initialize an output pin (export and set direction to out).
    ///
    /// Called during connect() phase to pre-configure all output pins.
    /// This ensures GPIO direction is set at startup, not lazily on first write.
    ///
    /// Default implementation does nothing (for drivers like gpiod that don't need it).
    async fn init_output_pin(&self, _pin: &GpioPinConfig) -> Result<()> {
        Ok(())
    }

    /// Initialize the driver (optional).
    async fn init(&mut self) -> Result<()> {
        Ok(())
    }

    /// Shutdown the driver (optional).
    async fn shutdown(&mut self) -> Result<()> {
        Ok(())
    }
}

// ============================================================================
// GPIO Number Resolution (Global → Chip + Line)
// ============================================================================

/// GPIO chip information for mapping global numbers to chip + line.
#[derive(Debug, Clone)]
struct GpioChipInfo {
    /// Chip name (e.g., "gpiochip495")
    name: String,
    /// Base GPIO number
    base: u32,
    /// Number of lines
    ngpio: u32,
}

/// Resolve a global GPIO number to chip name and line offset.
///
/// Scans `/sys/class/gpio/gpiochipN` directories to find the chip containing
/// the given GPIO number.
///
/// # Example
/// ```text
/// Global GPIO 503 on a system with:
///   gpiochip495: base=495, ngpio=16
/// Resolves to: ("gpiochip495", 8)  // 503 - 495 = 8
/// ```
fn resolve_gpio_to_chip_line(gpio_number: u32) -> Result<(String, u32)> {
    let gpio_path = std::path::Path::new("/sys/class/gpio");

    if !gpio_path.exists() {
        return Err(GatewayError::Protocol(
            "GPIO sysfs not available at /sys/class/gpio".into(),
        ));
    }

    let mut chips: Vec<GpioChipInfo> = Vec::new();

    // Scan all gpiochipN directories
    if let Ok(entries) = std::fs::read_dir(gpio_path) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.starts_with("gpiochip") {
                continue;
            }

            let chip_path = entry.path();

            // Read base
            let base_path = chip_path.join("base");
            let base: u32 = std::fs::read_to_string(&base_path)
                .ok()
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(0);

            // Read ngpio
            let ngpio_path = chip_path.join("ngpio");
            let ngpio: u32 = std::fs::read_to_string(&ngpio_path)
                .ok()
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(0);

            chips.push(GpioChipInfo { name, base, ngpio });
        }
    }

    // Find the chip that contains this GPIO number
    for chip in &chips {
        if gpio_number >= chip.base && gpio_number < chip.base + chip.ngpio {
            let line = gpio_number - chip.base;
            tracing::debug!(
                "GPIO {} resolved to chip '{}' line {} (base={}, ngpio={})",
                gpio_number,
                chip.name,
                line,
                chip.base,
                chip.ngpio
            );
            return Ok((chip.name.clone(), line));
        }
    }

    Err(GatewayError::Protocol(format!(
        "GPIO {} not found in any chip. Available chips: {:?}",
        gpio_number,
        chips
            .iter()
            .map(|c| format!("{}(base={},n={})", c.name, c.base, c.ngpio))
            .collect::<Vec<_>>()
    )))
}

// ============================================================================
// Gpiod Driver (Modern chardev interface)
// ============================================================================

/// Gpiod driver using libgpiod v2 character device interface.
///
/// Uses `/dev/gpiochipN` for GPIO access. This is the recommended driver for modern Linux systems.
///
/// **Auto-resolution**: If `gpio_number` is provided without a specific chip,
/// the driver will automatically resolve the global GPIO number to the correct
/// chip and line offset by scanning `/sys/class/gpio/gpiochipN`.
pub struct GpiodDriver;

impl GpiodDriver {
    /// Create a new gpiod driver.
    pub fn new() -> Self {
        Self
    }

    /// Resolve chip and line for a pin, handling global GPIO number auto-conversion.
    ///
    /// Returns `Cow<'_, str>` to avoid cloning when the chip name is already in the config.
    fn resolve_chip_line(pin: &GpioPinConfig) -> Result<(Cow<'_, str>, u32)> {
        // If gpio_number is provided and chip is default/empty, auto-resolve
        if let Some(gpio_num) = pin.gpio_number {
            if pin.chip.is_empty() || pin.chip == "gpiochip0" {
                // Check if it's actually on gpiochip0
                if gpio_num < 32 {
                    // Likely actually on gpiochip0 - borrow from config
                    return Ok((Cow::Borrowed(&pin.chip), pin.pin));
                }
                // Auto-resolve global GPIO number to chip + line (requires allocation)
                let (chip, line) = resolve_gpio_to_chip_line(gpio_num)?;
                return Ok((Cow::Owned(chip), line));
            }
        }

        // Use the configured chip directly - no allocation!
        Ok((Cow::Borrowed(&pin.chip), pin.pin))
    }
}

impl HasMetadata for GpiodDriver {
    #[allow(clippy::disallowed_methods)] // json! macro
    fn metadata() -> DriverMetadata {
        DriverMetadata {
            name: "gpiod",
            display_name: "Gpiod (Recommended)",
            description: "Modern character device interface using /dev/gpiochipN. Recommended for new projects.",
            is_recommended: true,
            example_config: serde_json::json!({
                "driver": "gpiod",
                "gpio_chip": "gpiochip6",
                "poll_interval_ms": 200,
                "pins": [
                    { "chip": "gpiochip6", "pin": 0, "direction": "input", "point_id": 1 },
                    { "chip": "gpiochip6", "pin": 1, "direction": "output", "point_id": 101 }
                ]
            }),
            parameters: vec![
                ParameterMetadata::optional(
                    "driver",
                    "Driver",
                    "GPIO driver type: 'gpiod' or 'sysfs'",
                    ParameterType::String,
                    serde_json::json!("gpiod"),
                ),
                ParameterMetadata::optional(
                    "gpio_chip",
                    "GPIO Chip",
                    "Default GPIO chip device name (e.g., gpiochip0, gpiochip6)",
                    ParameterType::String,
                    serde_json::json!("gpiochip0"),
                ),
                ParameterMetadata::optional(
                    "poll_interval_ms",
                    "Poll Interval (ms)",
                    "Polling interval for input pins in milliseconds",
                    ParameterType::Integer,
                    serde_json::json!(200),
                ),
                ParameterMetadata::required(
                    "pins",
                    "Pin Configuration",
                    "Array of GPIO pin configurations with chip, pin, direction, and point_id",
                    ParameterType::Array,
                ),
            ],
        }
    }
}

impl Default for GpiodDriver {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl GpioDriver for GpiodDriver {
    fn name(&self) -> &'static str {
        "gpiod"
    }

    async fn read_pin(&self, pin: &GpioPinConfig) -> Result<bool> {
        // Auto-resolve global GPIO number to chip + line
        let (chip_name, line) = Self::resolve_chip_line(pin)?;

        let chip = Chip::new(&*chip_name).await.map_err(|e| {
            GatewayError::Protocol(format!("Failed to open GPIO chip '{}': {}", chip_name, e))
        })?;

        let opts = Options::input([line]).consumer("igw");
        let lines = chip.request_lines(opts).await.map_err(|e| {
            GatewayError::Protocol(format!(
                "Failed to request GPIO line {} on chip '{}': {}",
                line, chip_name, e
            ))
        })?;

        let values = lines.get_values([false]).await.map_err(|e| {
            GatewayError::Protocol(format!("Failed to read GPIO line {}: {}", line, e))
        })?;

        Ok(values[0])
    }

    async fn write_pin(&self, pin: &GpioPinConfig, value: bool) -> Result<()> {
        // Auto-resolve global GPIO number to chip + line
        let (chip_name, line) = Self::resolve_chip_line(pin)?;

        let chip = Chip::new(&*chip_name).await.map_err(|e| {
            GatewayError::Protocol(format!("Failed to open GPIO chip '{}': {}", chip_name, e))
        })?;

        let opts = Options::output([line]).consumer("igw").values([value]);
        let lines = chip.request_lines(opts).await.map_err(|e| {
            GatewayError::Protocol(format!(
                "Failed to request GPIO line {} on chip '{}': {}",
                line, chip_name, e
            ))
        })?;

        lines.set_values([value]).await.map_err(|e| {
            GatewayError::Protocol(format!("Failed to write GPIO line {}: {}", line, e))
        })?;

        Ok(())
    }
}

// ============================================================================
// Sysfs Driver (Legacy interface)
// ============================================================================

/// Sysfs driver using legacy `/sys/class/gpio/` interface.
///
/// This driver is provided for compatibility with:
/// - Older Linux kernels (< 4.8)
/// - Industrial devices that use sysfs (e.g., Advantech ECU series)
/// - Systems where GPIO is already exported via sysfs
///
/// **Note**: sysfs GPIO is deprecated since Linux 4.8. Use `GpiodDriver` for new projects.
pub struct SysfsDriver {
    base_path: String,
}

impl SysfsDriver {
    /// Create a new sysfs driver.
    ///
    /// # Arguments
    /// * `base_path` - Path to sysfs GPIO (typically "/sys/class/gpio")
    pub fn new(base_path: impl Into<String>) -> Self {
        Self {
            base_path: base_path.into(),
        }
    }

    /// Get the path for a GPIO's value file.
    fn value_path(&self, gpio_num: u32) -> std::path::PathBuf {
        std::path::PathBuf::from(&self.base_path)
            .join(format!("gpio{}", gpio_num))
            .join("value")
    }

    /// Get the path for a GPIO's direction file.
    fn direction_path(&self, gpio_num: u32) -> std::path::PathBuf {
        std::path::PathBuf::from(&self.base_path)
            .join(format!("gpio{}", gpio_num))
            .join("direction")
    }

    /// Ensure GPIO is exported and set to output direction.
    /// Only sets direction if not already "out" to avoid unnecessary writes.
    async fn ensure_output(&self, gpio_num: u32) -> Result<()> {
        let gpio_path = std::path::PathBuf::from(&self.base_path).join(format!("gpio{}", gpio_num));

        // Export if not already exported
        if !gpio_path.exists() {
            let export_path = std::path::PathBuf::from(&self.base_path).join("export");
            tokio::fs::write(&export_path, gpio_num.to_string())
                .await
                .map_err(|e| {
                    GatewayError::Protocol(format!("Failed to export GPIO {}: {}", gpio_num, e))
                })?;
            // Wait for sysfs to create the GPIO directory
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }

        // Check current direction, only set if not already "out"
        let direction_path = self.direction_path(gpio_num);
        let current_dir = tokio::fs::read_to_string(&direction_path)
            .await
            .unwrap_or_default();

        if current_dir.trim() != "out" {
            tokio::fs::write(&direction_path, "out")
                .await
                .map_err(|e| {
                    GatewayError::Protocol(format!(
                        "Failed to set GPIO {} direction to out: {}",
                        gpio_num, e
                    ))
                })?;
        }

        Ok(())
    }
}

impl HasMetadata for SysfsDriver {
    #[allow(clippy::disallowed_methods)] // json! macro
    fn metadata() -> DriverMetadata {
        DriverMetadata {
            name: "sysfs",
            display_name: "Sysfs (Legacy)",
            description: "Legacy sysfs interface using /sys/class/gpio/. For compatibility with older systems.",
            is_recommended: false,
            example_config: serde_json::json!({
                "driver": "sysfs",
                "gpio_base_path": "/sys/class/gpio",
                "poll_interval_ms": 200,
                "pins": [
                    { "gpio_number": 490, "direction": "input", "point_id": 1 },
                    { "gpio_number": 491, "direction": "output", "point_id": 101 }
                ]
            }),
            parameters: vec![
                ParameterMetadata::optional(
                    "driver",
                    "Driver",
                    "GPIO driver type: 'gpiod' or 'sysfs'",
                    ParameterType::String,
                    serde_json::json!("sysfs"),
                ),
                ParameterMetadata::optional(
                    "gpio_base_path",
                    "GPIO Base Path",
                    "Base path for sysfs GPIO interface",
                    ParameterType::String,
                    serde_json::json!("/sys/class/gpio"),
                ),
                ParameterMetadata::optional(
                    "poll_interval_ms",
                    "Poll Interval (ms)",
                    "Polling interval for input pins in milliseconds",
                    ParameterType::Integer,
                    serde_json::json!(200),
                ),
                ParameterMetadata::required(
                    "pins",
                    "Pin Configuration",
                    "Array of GPIO pin configurations with gpio_number, direction, and point_id",
                    ParameterType::Array,
                ),
            ],
        }
    }
}

impl Default for SysfsDriver {
    fn default() -> Self {
        Self::new("/sys/class/gpio")
    }
}

#[async_trait]
impl GpioDriver for SysfsDriver {
    fn name(&self) -> &'static str {
        "sysfs"
    }

    async fn read_pin(&self, pin: &GpioPinConfig) -> Result<bool> {
        let gpio_num = pin.gpio_number.ok_or_else(|| {
            GatewayError::Protocol(format!(
                "GPIO number not set for pin {} (required for sysfs driver)",
                pin.point_id
            ))
        })?;

        // Read value (GPIO should already be exported and configured by OS/device tree)
        let value_str = tokio::fs::read_to_string(self.value_path(gpio_num))
            .await
            .map_err(|e| {
                GatewayError::Protocol(format!("Failed to read GPIO {}: {}", gpio_num, e))
            })?;

        let value = value_str.trim() == "1";
        Ok(value)
    }

    async fn write_pin(&self, pin: &GpioPinConfig, value: bool) -> Result<()> {
        let gpio_num = pin.gpio_number.ok_or_else(|| {
            GatewayError::Protocol(format!(
                "GPIO number not set for pin {} (required for sysfs driver)",
                pin.point_id
            ))
        })?;

        // Ensure GPIO is exported and set as output (only if not already)
        self.ensure_output(gpio_num).await?;

        // Write value
        let value_str = if value { "1" } else { "0" };
        tokio::fs::write(self.value_path(gpio_num), value_str)
            .await
            .map_err(|e| {
                GatewayError::Protocol(format!("Failed to write GPIO {}: {}", gpio_num, e))
            })?;

        Ok(())
    }

    async fn init_output_pin(&self, pin: &GpioPinConfig) -> Result<()> {
        let gpio_num = pin.gpio_number.ok_or_else(|| {
            GatewayError::Protocol(format!(
                "GPIO number not set for pin {} (required for sysfs driver)",
                pin.point_id
            ))
        })?;

        // Export and set direction to out at startup
        self.ensure_output(gpio_num).await?;

        tracing::info!(
            gpio_num = gpio_num,
            point_id = pin.point_id,
            "Initialized GPIO output pin"
        );

        Ok(())
    }
}

// ============================================================================
// Pin Configuration
// ============================================================================

/// GPIO pin direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpioDirection {
    /// Input pin (DI - Digital Input).
    Input,
    /// Output pin (DO - Digital Output).
    Output,
}

/// GPIO pin configuration.
///
/// Supports both gpiod (chip + pin) and sysfs (gpio_number) addressing.
#[derive(Debug, Clone)]
pub struct GpioPinConfig {
    /// GPIO chip name (e.g., "gpiochip0") - for gpiod driver.
    pub chip: String,

    /// Pin number/offset on the GPIO chip - for gpiod driver.
    pub pin: u32,

    /// Global GPIO number (e.g., 490) - for sysfs driver.
    /// This is the number used in `/sys/class/gpio/gpioN/`.
    pub gpio_number: Option<u32>,

    /// Pin direction.
    pub direction: GpioDirection,

    /// Point ID for SCADA mapping (matches DataPoint/ControlCommand IDs).
    pub point_id: u32,

    /// Active low (invert logic).
    pub active_low: bool,

    /// Debounce time for inputs (microseconds).
    pub debounce_us: Option<u64>,
}

impl GpioPinConfig {
    /// Create a digital input configuration for gpiod driver.
    ///
    /// # Arguments
    /// * `chip` - GPIO chip name (e.g., "gpiochip0")
    /// * `pin` - Pin offset on the chip
    /// * `point_id` - SCADA point ID
    pub fn digital_input(chip: impl Into<String>, pin: u32, point_id: u32) -> Self {
        Self {
            chip: chip.into(),
            pin,
            gpio_number: None,
            direction: GpioDirection::Input,
            point_id,
            active_low: false,
            debounce_us: Some(1000), // 1ms default debounce
        }
    }

    /// Create a digital output configuration for gpiod driver.
    pub fn digital_output(chip: impl Into<String>, pin: u32, point_id: u32) -> Self {
        Self {
            chip: chip.into(),
            pin,
            gpio_number: None,
            direction: GpioDirection::Output,
            point_id,
            active_low: false,
            debounce_us: None,
        }
    }

    /// Create a digital input configuration for sysfs driver.
    ///
    /// # Arguments
    /// * `gpio_number` - Global GPIO number (e.g., 490 for `/sys/class/gpio/gpio490/`)
    /// * `point_id` - SCADA point ID
    pub fn digital_input_sysfs(gpio_number: u32, point_id: u32) -> Self {
        Self {
            chip: String::new(),
            pin: 0,
            gpio_number: Some(gpio_number),
            direction: GpioDirection::Input,
            point_id,
            active_low: false,
            debounce_us: Some(1000),
        }
    }

    /// Create a digital output configuration for sysfs driver.
    pub fn digital_output_sysfs(gpio_number: u32, point_id: u32) -> Self {
        Self {
            chip: String::new(),
            pin: 0,
            gpio_number: Some(gpio_number),
            direction: GpioDirection::Output,
            point_id,
            active_low: false,
            debounce_us: None,
        }
    }

    /// Set GPIO number (for sysfs driver or dual-mode configuration).
    pub fn with_gpio_number(mut self, gpio_number: u32) -> Self {
        self.gpio_number = Some(gpio_number);
        self
    }

    /// Set active low mode.
    pub fn with_active_low(mut self, active_low: bool) -> Self {
        self.active_low = active_low;
        self
    }

    /// Set debounce time.
    pub fn with_debounce(mut self, debounce_us: u64) -> Self {
        self.debounce_us = Some(debounce_us);
        self
    }
}

/// GPIO channel configuration.
#[derive(Debug, Clone)]
pub struct GpioChannelConfig {
    /// Driver type selection.
    pub driver: GpioDriverType,

    /// Pin configurations.
    pub pins: Vec<GpioPinConfig>,

    /// Polling interval for inputs.
    pub poll_interval: Duration,
}

impl Default for GpioChannelConfig {
    fn default() -> Self {
        Self {
            driver: GpioDriverType::default(),
            pins: Vec::new(),
            poll_interval: Duration::from_millis(100),
        }
    }
}

impl GpioChannelConfig {
    /// Create a new configuration with default gpiod driver.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new configuration with sysfs driver.
    pub fn new_sysfs(base_path: impl Into<String>) -> Self {
        Self {
            driver: GpioDriverType::Sysfs {
                base_path: base_path.into(),
            },
            pins: Vec::new(),
            poll_interval: Duration::from_millis(100),
        }
    }

    /// Set the driver type.
    pub fn with_driver(mut self, driver: GpioDriverType) -> Self {
        self.driver = driver;
        self
    }

    /// Add a pin configuration.
    pub fn add_pin(mut self, pin: GpioPinConfig) -> Self {
        self.pins.push(pin);
        self
    }

    /// Set poll interval.
    pub fn with_poll_interval(mut self, interval: Duration) -> Self {
        self.poll_interval = interval;
        self
    }

    /// Get input pins.
    pub fn input_pins(&self) -> impl Iterator<Item = &GpioPinConfig> {
        self.pins
            .iter()
            .filter(|p| p.direction == GpioDirection::Input)
    }

    /// Get output pins.
    pub fn output_pins(&self) -> impl Iterator<Item = &GpioPinConfig> {
        self.pins
            .iter()
            .filter(|p| p.direction == GpioDirection::Output)
    }
}

/// GPIO channel diagnostics.
#[derive(Debug, Default)]
struct GpioDiagnostics {
    read_count: u64,
    write_count: u64,
    error_count: u64,
    last_error: Option<String>,
}

/// GPIO channel adapter.
///
/// Provides digital input/output control via pluggable GPIO drivers.
/// Supports both modern gpiod (chardev) and legacy sysfs backends.
///
/// The service layer (comsrv) is responsible for persistence.
pub struct GpioChannel {
    /// Channel unique identifier.
    channel_id: u32,
    /// Channel instance name.
    name: String,
    config: GpioChannelConfig,
    /// Pluggable GPIO driver (trait object for extensibility)
    driver: Box<dyn GpioDriver>,
    state: Arc<std::sync::RwLock<ConnectionState>>,
    diagnostics: Arc<RwLock<GpioDiagnostics>>,
    poll_task: Option<tokio::task::JoinHandle<()>>,
    /// Output states cache (for read-back) - lock-free atomic storage
    output_states: AtomicBoolStore,
    /// Logging context
    log_ctx: LogContext,
}

impl GpioChannel {
    /// Create a new GPIO channel with the configured driver.
    ///
    /// GPIO channels are always "connected" since they operate on local hardware
    /// without requiring external network connections (unlike Modbus TCP).
    pub fn new(config: GpioChannelConfig, channel_id: u32, name: String) -> Self {
        // Collect output pin IDs for lock-free AtomicBoolStore
        let output_pin_ids: Vec<u32> = config.output_pins().map(|p| p.point_id).collect();

        // Create driver based on configuration
        let driver: Box<dyn GpioDriver> = match &config.driver {
            GpioDriverType::Gpiod => Box::new(GpiodDriver::new()),
            GpioDriverType::Sysfs { base_path } => Box::new(SysfsDriver::new(base_path.clone())),
        };
        Self {
            channel_id,
            name,
            config,
            driver,
            // GPIO is always "connected" - it's local hardware, no external connection needed
            state: Arc::new(std::sync::RwLock::new(ConnectionState::Connected)),
            diagnostics: Arc::new(RwLock::new(GpioDiagnostics::default())),
            poll_task: None,
            output_states: AtomicBoolStore::from_pins(&output_pin_ids),
            log_ctx: LogContext::new(channel_id),
        }
    }

    /// Create a GPIO channel with a custom driver.
    ///
    /// This allows using custom driver implementations beyond the built-in ones.
    /// GPIO channels are always "connected" since they operate on local hardware.
    pub fn with_driver(
        config: GpioChannelConfig,
        driver: Box<dyn GpioDriver>,
        channel_id: u32,
        name: String,
    ) -> Self {
        // Collect output pin IDs for lock-free AtomicBoolStore
        let output_pin_ids: Vec<u32> = config.output_pins().map(|p| p.point_id).collect();

        Self {
            channel_id,
            name,
            config,
            driver,
            // GPIO is always "connected" - it's local hardware, no external connection needed
            state: Arc::new(std::sync::RwLock::new(ConnectionState::Connected)),
            diagnostics: Arc::new(RwLock::new(GpioDiagnostics::default())),
            poll_task: None,
            output_states: AtomicBoolStore::from_pins(&output_pin_ids),
            log_ctx: LogContext::new(channel_id),
        }
    }

    /// Get the driver name.
    pub fn driver_name(&self) -> &'static str {
        self.driver.name()
    }

    /// Get the configured poll interval.
    pub fn poll_interval(&self) -> Duration {
        self.config.poll_interval
    }

    fn set_state(&self, state: ConnectionState) {
        if let Ok(mut s) = self.state.write() {
            *s = state;
        }
    }

    fn get_state(&self) -> ConnectionState {
        self.state
            .read()
            .map(|s| *s)
            .unwrap_or(ConnectionState::Error)
    }

    /// Read a single GPIO pin using the configured driver.
    async fn read_pin(&self, pin_config: &GpioPinConfig) -> Result<DataPoint> {
        let raw_value = self.driver.read_pin(pin_config).await?;
        let adjusted = if pin_config.active_low {
            !raw_value
        } else {
            raw_value
        };
        Ok(DataPoint::new(pin_config.point_id, adjusted))
    }

    /// Write to a single GPIO pin using the configured driver.
    async fn write_pin(&self, pin_config: &GpioPinConfig, value: bool) -> Result<()> {
        let adjusted = if pin_config.active_low { !value } else { value };
        self.driver.write_pin(pin_config, adjusted).await?;

        // Update internal state cache for feedback (lock-free atomic operation)
        self.output_states.set(pin_config.point_id, adjusted);

        Ok(())
    }

    /// Read output state (for feedback).
    fn read_output_state(&self, point_id: u32) -> Option<bool> {
        // Lock-free atomic read
        self.output_states.get(point_id)
    }
}

impl ProtocolCapabilities for GpioChannel {
    fn name(&self) -> &'static str {
        "GPIO"
    }

    fn supported_modes(&self) -> &[CommunicationMode] {
        &[CommunicationMode::Polling]
    }
}

impl LoggableProtocol for GpioChannel {
    fn set_log_handler(&mut self, handler: Arc<dyn ChannelLogHandler>) {
        self.log_ctx.set_handler(handler);
    }

    fn set_log_config(&mut self, config: ChannelLogConfig) {
        self.log_ctx.set_config(config);
    }

    fn log_config(&self) -> &ChannelLogConfig {
        self.log_ctx.config()
    }
}

// Helper methods for GpioChannel
impl GpioChannel {
    /// Read all GPIO pins and return batch with failures.
    ///
    /// This method reads all input pins and output states, collecting any failures.
    async fn read_all(&self) -> (DataBatch, Vec<PointFailure>) {
        let mut batch = DataBatch::new();
        let mut failures = Vec::new();

        // Read all input pins
        for pin in self.config.input_pins() {
            match self.read_pin(pin).await {
                Ok(point) => batch.add(point),
                Err(e) => {
                    failures.push(PointFailure::with_error(pin.point_id, e.to_string()));
                    let mut diag = self.diagnostics.write().await;
                    diag.error_count += 1;
                    diag.last_error = Some(e.to_string());
                },
            }
        }

        // Also include output states as feedback (lock-free read)
        for pin in self.config.output_pins() {
            if let Some(state) = self.read_output_state(pin.point_id) {
                batch.add(DataPoint::new(pin.point_id, state));
            }
        }

        {
            let mut diag = self.diagnostics.write().await;
            diag.read_count += 1;
        }

        (batch, failures)
    }
}

impl Protocol for GpioChannel {
    fn connection_state(&self) -> ConnectionState {
        self.get_state()
    }

    #[allow(clippy::disallowed_methods)] // json! macro
    async fn diagnostics(&self) -> Result<Diagnostics> {
        let diag = self.diagnostics.read().await;
        let input_count = self.config.input_pins().count();
        let output_count = self.config.output_pins().count();

        Ok(Diagnostics {
            protocol: self.name().to_string(),
            connection_state: self.get_state(),
            read_count: diag.read_count,
            write_count: diag.write_count,
            error_count: diag.error_count,
            last_error: diag.last_error.clone(),
            extra: serde_json::json!({
                "input_pins": input_count,
                "output_pins": output_count,
            }),
        })
    }
}

impl ProtocolClient for GpioChannel {
    async fn connect(&mut self) -> Result<()> {
        let start = Instant::now();

        // Initialize all output pins at startup (export + set direction to out)
        let mut output_count = 0usize;
        for pin in self.config.output_pins() {
            output_count += 1;
            if let Err(e) = self.driver.init_output_pin(pin).await {
                tracing::warn!(
                    point_id = pin.point_id,
                    gpio_number = ?pin.gpio_number,
                    error = %e,
                    "Failed to initialize GPIO output pin (will retry on first write)"
                );
            }
        }

        self.set_state(ConnectionState::Connected);
        self.log_ctx
            .log_connected("gpio", start.elapsed().as_millis() as u64)
            .await;

        tracing::info!(
            driver = self.driver.name(),
            output_pins = output_count,
            input_pins = self.config.input_pins().count(),
            elapsed_ms = start.elapsed().as_millis(),
            "GPIO channel connected"
        );

        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        if let Some(task) = self.poll_task.take() {
            task.abort();
        }
        self.set_state(ConnectionState::Disconnected);
        self.log_ctx.log_disconnected(None).await;
        Ok(())
    }

    async fn poll_once(&mut self) -> PollResult {
        let start = Instant::now();
        let (batch, failures) = self.read_all().await;

        // Log error summary (avoids log flooding with many failed pins)
        if !failures.is_empty() {
            let first_errors: Vec<_> = failures.iter().take(3).collect();
            let error_msg = format!(
                "GPIO read: {} point(s) failed, first errors: {:?}",
                failures.len(),
                first_errors
            );
            self.log_ctx
                .log_error(error_msg, ErrorContext::Polling)
                .await;
        }

        // Log poll cycle (pass count instead of cloning batch)
        self.log_ctx
            .log_poll_cycle(
                batch.len(),
                start.elapsed().as_millis() as u64,
                batch.len(),
                failures.len(),
            )
            .await;

        if failures.is_empty() {
            PollResult::success(batch)
        } else {
            PollResult::partial(batch, failures)
        }
    }

    async fn write_control(&mut self, commands: &[ControlCommand]) -> Result<WriteResult> {
        let start = Instant::now();

        if !self.get_state().is_connected() {
            return Err(GatewayError::NotConnected);
        }

        let mut success_count = 0;
        let mut failures = Vec::new();

        for cmd in commands {
            // Find corresponding output pin
            let pin = self
                .config
                .pins
                .iter()
                .find(|p| p.point_id == cmd.id && p.direction == GpioDirection::Output);

            match pin {
                Some(p) => match self.write_pin(p, cmd.value).await {
                    Ok(()) => success_count += 1,
                    Err(e) => failures.push((cmd.id, e.to_string())),
                },
                None => {
                    failures.push((cmd.id, "Output pin not found".into()));
                },
            }
        }

        {
            let mut diag = self.diagnostics.write().await;
            diag.write_count += success_count as u64;
        }

        let result = WriteResult {
            success_count,
            failures,
        };

        // Log control write
        self.log_ctx
            .log_control_write(
                commands,
                Ok(result.clone()),
                start.elapsed().as_millis() as u64,
            )
            .await;

        Ok(result)
    }

    async fn write_adjustment(
        &mut self,
        _adjustments: &[AdjustmentCommand],
    ) -> Result<WriteResult> {
        // GPIO doesn't support analog output
        Err(GatewayError::Unsupported(
            "GPIO does not support analog adjustment".into(),
        ))
    }
}

// ============================================================================
// ChannelRuntime implementation (direct, no wrapper needed)
// ============================================================================

#[async_trait]
impl ChannelRuntime for GpioChannel {
    fn id(&self) -> u32 {
        self.channel_id
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn protocol(&self) -> &str {
        "gpio"
    }

    fn is_event_driven(&self) -> bool {
        false // GPIO is polling-only
    }

    async fn connect(&mut self) -> Result<()> {
        <Self as ProtocolClient>::connect(self).await
    }

    async fn disconnect(&mut self) -> Result<()> {
        <Self as ProtocolClient>::disconnect(self).await
    }

    async fn poll_once(&mut self) -> PollResult {
        <Self as ProtocolClient>::poll_once(self).await
    }

    async fn write_control(&mut self, commands: &[(u32, f64)]) -> Result<usize> {
        let cmds: Vec<_> = commands
            .iter()
            .map(|(id, value)| ControlCommand::latching(*id, *value != 0.0))
            .collect();
        let result = <Self as ProtocolClient>::write_control(self, &cmds).await?;
        Ok(result.success_count)
    }

    async fn write_adjustment(&mut self, adjustments: &[(u32, f64)]) -> Result<usize> {
        let adjs: Vec<_> = adjustments
            .iter()
            .map(|(id, value)| AdjustmentCommand::new(*id, *value))
            .collect();
        let result = <Self as ProtocolClient>::write_adjustment(self, &adjs).await?;
        Ok(result.success_count)
    }

    fn subscribe(&self) -> Option<DataEventReceiver> {
        None // GPIO is polling-only
    }

    async fn start_events(&mut self) -> Result<()> {
        Ok(()) // No-op for polling channel
    }

    async fn stop_events(&mut self) -> Result<()> {
        Ok(()) // No-op for polling channel
    }

    async fn diagnostics(&self) -> Result<Diagnostics> {
        <Self as Protocol>::diagnostics(self).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_gpio_channel_connect() {
        let config = GpioChannelConfig::new()
            .add_pin(GpioPinConfig::digital_input("gpiochip0", 17, 1))
            .add_pin(GpioPinConfig::digital_output("gpiochip0", 18, 101));

        let mut gpio = GpioChannel::new(config);

        // GPIO is always connected on creation (local hardware, no external connection)
        assert_eq!(gpio.connection_state(), ConnectionState::Connected);

        // connect() is idempotent for GPIO
        gpio.connect().await.unwrap();
        assert_eq!(gpio.connection_state(), ConnectionState::Connected);

        // disconnect() still works for explicit shutdown
        gpio.disconnect().await.unwrap();
        assert_eq!(gpio.connection_state(), ConnectionState::Disconnected);
    }

    #[tokio::test]
    async fn test_gpio_write_control() {
        let config =
            GpioChannelConfig::new().add_pin(GpioPinConfig::digital_output("gpiochip0", 18, 101));

        let mut gpio = GpioChannel::new(config);
        gpio.connect().await.unwrap();

        let result = gpio
            .write_control(&[ControlCommand::latching(101, true)])
            .await
            .unwrap();

        assert_eq!(result.success_count, 1);
        assert!(result.failures.is_empty());

        // Check output state (lock-free read)
        let state = gpio.read_output_state(101);
        assert_eq!(state, Some(true));
    }

    #[tokio::test]
    async fn test_gpio_diagnostics() {
        let config = GpioChannelConfig::new()
            .add_pin(GpioPinConfig::digital_input("gpiochip0", 17, 1))
            .add_pin(GpioPinConfig::digital_output("gpiochip0", 18, 101));

        let gpio = GpioChannel::new(config);
        let diag = gpio.diagnostics().await.unwrap();

        assert_eq!(diag.protocol, "GPIO");
        assert_eq!(diag.extra["input_pins"], 1);
        assert_eq!(diag.extra["output_pins"], 1);
    }
}
