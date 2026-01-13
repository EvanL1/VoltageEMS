//! Protocol client factory
//!
//! Create ChannelRuntime implementations from configuration.
//!
//! This module provides factory functions that create protocol client instances
//! (VirtualChannel, ModbusChannel, GpioChannel, CanClient) from comsrv configuration.

#[cfg(feature = "modbus")]
use std::sync::Arc;

use crate::protocols::adapters::virtual_channel::{VirtualChannel, VirtualChannelConfig};
use crate::protocols::core::point::PointConfig;
use crate::protocols::gateway::ChannelRuntime;

#[cfg(feature = "modbus")]
use crate::protocols::adapters::modbus::{ModbusChannel, ModbusChannelConfig, ReconnectConfig};

#[cfg(all(target_os = "linux", feature = "gpio"))]
use crate::protocols::adapters::gpio::{GpioChannel, GpioChannelConfig, GpioPinConfig};

#[cfg(all(feature = "can", target_os = "linux"))]
use crate::protocols::adapters::can::{CanClient, CanConfig, CanPoint};

#[cfg(all(target_os = "linux", feature = "gpio"))]
use crate::core::config::RuntimeChannelConfig;

// ============================================================================
// Virtual Channel Factory
// ============================================================================

/// Create a VirtualChannel that directly implements ChannelRuntime.
///
/// Note: The channel no longer holds a store reference. Storage is handled
/// by the service layer (ChannelManager) after polling.
pub fn create_virtual_channel(
    channel_id: u32,
    channel_name: &str,
    point_configs: Vec<PointConfig>,
) -> Box<dyn ChannelRuntime> {
    let config = VirtualChannelConfig::new(channel_name).with_points(point_configs);
    let channel = VirtualChannel::new(config, channel_id);

    // VirtualChannel now directly implements ChannelRuntime - no wrapper needed
    Box::new(channel)
}

// ============================================================================
// Modbus Channel Factory
// ============================================================================

/// Create a ModbusChannel for TCP mode wrapped as ChannelRuntime.
///
/// Note: The channel no longer holds a store reference. Storage is handled
/// by the service layer (ChannelManager) after polling.
///
/// # Arguments
///
/// * `channel_id` - Unique channel identifier (used for logging)
/// * `host` - Modbus TCP server host address
/// * `port` - Modbus TCP server port
/// * `point_configs` - Point configurations with Modbus addresses
#[cfg(feature = "modbus")]
pub fn create_modbus_channel(
    channel_id: u32,
    host: &str,
    port: u16,
    point_configs: Vec<PointConfig>,
) -> Box<dyn ChannelRuntime> {
    use crate::protocols::core::logging::{ChannelLogConfig, LoggableProtocol, TracingLogHandler};

    let address = format!("{}:{}", host, port);

    let config = ModbusChannelConfig::tcp(&address)
        .with_points(point_configs)
        .with_reconnect(ReconnectConfig::default());

    let channel_name = format!("modbus_tcp_{}", channel_id);
    let mut channel = ModbusChannel::new(config, channel_id, channel_name.clone());
    // Enable tracing logs
    channel.set_log_handler(Arc::new(TracingLogHandler));
    channel.set_log_config(ChannelLogConfig::default());

    // ModbusChannel directly implements ChannelRuntime - no wrapper needed
    Box::new(channel)
}

/// Create a ModbusChannel for RTU (serial) mode wrapped as ChannelRuntime.
///
/// Note: The channel no longer holds a store reference. Storage is handled
/// by the service layer (ChannelManager) after polling.
///
/// # Arguments
///
/// * `channel_id` - Unique channel identifier (used for logging)
/// * `device` - Serial device path (e.g., "/dev/ttyUSB0" on Linux)
/// * `baud_rate` - Serial baud rate (e.g., 9600, 19200, 115200)
/// * `point_configs` - Point configurations with Modbus addresses
#[cfg(feature = "modbus")]
pub fn create_modbus_rtu_channel(
    channel_id: u32,
    device: &str,
    baud_rate: u32,
    point_configs: Vec<PointConfig>,
) -> Box<dyn ChannelRuntime> {
    use crate::protocols::core::logging::{ChannelLogConfig, LoggableProtocol, TracingLogHandler};

    let config = ModbusChannelConfig::rtu(device, baud_rate)
        .with_points(point_configs)
        .with_reconnect(ReconnectConfig::default());

    let channel_name = format!("modbus_rtu_{}", channel_id);
    let mut channel = ModbusChannel::new(config, channel_id, channel_name.clone());
    // Enable tracing logs
    channel.set_log_handler(Arc::new(TracingLogHandler));
    channel.set_log_config(ChannelLogConfig::default());

    // ModbusChannel directly implements ChannelRuntime - no wrapper needed
    Box::new(channel)
}

// ============================================================================
// GPIO Channel Factory
// ============================================================================

/// Create a GpioChannel for digital I/O wrapped as ChannelRuntime.
///
/// Note: Only available on Linux with `gpio` feature enabled.
/// Storage is handled by the service layer (ChannelManager) after polling.
///
/// GPIO pins use explicit `point_type` in `GpioPinConfig`:
/// - Digital inputs (DI) → `PointType::Signal`
/// - Digital outputs (DO) → `PointType::Control`
///
/// # Arguments
///
/// * `channel_id` - Unique channel identifier
/// * `runtime_config` - Channel configuration containing GPIO pin mappings
#[cfg(all(target_os = "linux", feature = "gpio"))]
pub fn create_gpio_channel(
    channel_id: u32,
    runtime_config: &RuntimeChannelConfig,
) -> Box<dyn ChannelRuntime> {
    use std::time::Duration;

    // Use sysfs driver - simpler and works directly with global GPIO numbers
    let mut gpio_config = GpioChannelConfig::new_sysfs("/sys/class/gpio");

    // Get poll interval from parameters
    if let Some(interval_ms) = runtime_config
        .base
        .parameters
        .get("poll_interval_ms")
        .and_then(|v| v.as_u64())
    {
        gpio_config = gpio_config.with_poll_interval(Duration::from_millis(interval_ms));
    }

    // Helper to parse gpio_number from protocol_mappings JSON
    // Expected format: {"gpio_number": 496, ...}
    let parse_gpio_number = |protocol_mappings: &Option<String>| -> Option<u32> {
        let json_str = protocol_mappings.as_ref()?;
        let json: serde_json::Value = serde_json::from_str(json_str).ok()?;
        json.get("gpio_number")?.as_u64().map(|n| n as u32)
    };

    // Configure DI pins from signal points (using sysfs with global GPIO numbers)
    // GpioPinConfig::digital_input_sysfs automatically sets point_type = Signal
    for pt in &runtime_config.signal_points {
        if let Some(gpio_num) = parse_gpio_number(&pt.base.protocol_mappings) {
            let pin_config = GpioPinConfig::digital_input_sysfs(gpio_num, pt.base.point_id)
                .with_active_low(pt.reverse);

            gpio_config = gpio_config.add_pin(pin_config);
        }
    }

    // Configure DO pins from control points (using sysfs with global GPIO numbers)
    // GpioPinConfig::digital_output_sysfs automatically sets point_type = Control
    for pt in &runtime_config.control_points {
        if let Some(gpio_num) = parse_gpio_number(&pt.base.protocol_mappings) {
            let pin_config = GpioPinConfig::digital_output_sysfs(gpio_num, pt.base.point_id)
                .with_active_low(pt.reverse);

            gpio_config = gpio_config.add_pin(pin_config);
        }
    }

    let channel_name = format!("gpio_{}", channel_id);
    // GpioChannel directly implements ChannelRuntime - no wrapper needed
    let channel = GpioChannel::new(gpio_config, channel_id, channel_name);
    Box::new(channel)
}

// ============================================================================
// CAN Channel Factory
// ============================================================================

/// Create a CAN channel with the given configuration wrapped as ChannelRuntime.
///
/// This function creates a CanClient with the specified
/// CAN interface and point configurations.
#[cfg(all(feature = "can", target_os = "linux"))]
pub fn create_can_channel(
    channel_id: u32,
    can_interface: &str,
    points: Vec<CanPoint>,
) -> Box<dyn ChannelRuntime> {
    let config = CanConfig {
        can_interface: can_interface.to_string(),
        bitrate: 250000,
        rx_poll_interval_ms: 50,
        data_read_interval_ms: 1000,
    };

    let channel_name = format!("can_{}", channel_id);
    // CanClient directly implements ChannelRuntime - no wrapper needed
    let mut client = CanClient::new(config, channel_id, channel_name);
    client.add_points(points);

    Box::new(client)
}
