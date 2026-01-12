//! Channel factory.
//!
//! Creates `ChannelRuntime` instances from configuration.

use crate::protocols::core::error::{GatewayError, Result};
use crate::protocols::core::point::PointConfig;

use super::config::ChannelConfig;
use super::parse_address;
use super::runtime::ChannelRuntime;

/// Create a channel from configuration.
pub fn create_channel(config: &ChannelConfig) -> Result<Box<dyn ChannelRuntime>> {
    let protocol = &config.protocol;

    // Use eq_ignore_ascii_case to avoid String allocation from to_lowercase()
    #[cfg(feature = "modbus")]
    if protocol.eq_ignore_ascii_case("modbus") {
        return create_modbus_channel(config);
    }

    #[cfg(feature = "iec104")]
    if protocol.eq_ignore_ascii_case("iec104") {
        return create_iec104_channel(config);
    }

    #[cfg(feature = "opcua")]
    if protocol.eq_ignore_ascii_case("opcua") {
        return create_opcua_channel(config);
    }

    #[cfg(all(feature = "can", target_os = "linux"))]
    if protocol.eq_ignore_ascii_case("can") {
        return create_can_channel(config);
    }

    #[cfg(all(feature = "gpio", target_os = "linux"))]
    if protocol.eq_ignore_ascii_case("gpio") {
        return create_gpio_channel(config);
    }

    if protocol.eq_ignore_ascii_case("virtual") {
        return create_virtual_channel(config);
    }

    Err(GatewayError::Config(format!(
        "Unsupported protocol: {}. Check if the required feature is enabled.",
        protocol
    )))
}

/// Convert PointDef list to PointConfig list.
fn build_point_configs(config: &ChannelConfig) -> Result<Vec<PointConfig>> {
    // Pre-allocate with upper bound (some points may be disabled)
    let mut points = Vec::with_capacity(config.points.len());

    for point_def in &config.points {
        if !point_def.enabled {
            continue;
        }

        let address = parse_address(&config.protocol, &point_def.address)?;

        points.push(PointConfig {
            id: point_def.id,
            name: Some(point_def.name.clone()),
            address,
            transform: point_def.transform.clone(),
            poll_group: None,
            enabled: true,
        });
    }

    Ok(points)
}

// ============================================================================
// Protocol-specific channel creators
// ============================================================================

#[cfg(feature = "modbus")]
fn create_modbus_channel(config: &ChannelConfig) -> Result<Box<dyn ChannelRuntime>> {
    use crate::protocols::adapters::modbus::ModbusChannelParamsConfig;

    // Parse parameters
    let params: ModbusChannelParamsConfig = serde_json::from_value(config.parameters.clone())
        .map_err(|e| GatewayError::Config(format!("Invalid Modbus parameters: {}", e)))?;

    // Build channel config
    let channel_config = params.to_channel_config();

    // Build point configs
    let points = build_point_configs(config)?;
    let channel_config = channel_config.with_points(points);

    // ModbusChannel directly implements ChannelRuntime - no wrapper needed
    let channel = crate::protocols::adapters::modbus::ModbusChannel::new(
        channel_config,
        config.id,
        config.name.clone(),
    );

    Ok(Box::new(channel))
}

#[cfg(feature = "iec104")]
fn create_iec104_channel(config: &ChannelConfig) -> Result<Box<dyn ChannelRuntime>> {
    use crate::protocols::adapters::iec104::Iec104ParamsConfig;

    // Parse parameters
    let params: Iec104ParamsConfig = serde_json::from_value(config.parameters.clone())
        .map_err(|e| GatewayError::Config(format!("Invalid IEC104 parameters: {}", e)))?;

    // Build point configs
    let points = build_point_configs(config)?;

    // Build channel config
    let channel_config = params.to_config().with_points(points);

    // Iec104Channel directly implements ChannelRuntime - no wrapper needed
    let channel = crate::protocols::adapters::iec104::Iec104Channel::new(
        channel_config,
        config.id,
        config.name.clone(),
    );

    Ok(Box::new(channel))
}

#[cfg(feature = "opcua")]
fn create_opcua_channel(config: &ChannelConfig) -> Result<Box<dyn ChannelRuntime>> {
    use crate::protocols::adapters::opcua::OpcUaParamsConfig;

    // Parse parameters
    let params: OpcUaParamsConfig = serde_json::from_value(config.parameters.clone())
        .map_err(|e| GatewayError::Config(format!("Invalid OPC UA parameters: {}", e)))?;

    // Build point configs
    let points = build_point_configs(config)?;

    // Build channel config
    let channel_config = params.to_config().with_points(points);

    // OpcUaChannel directly implements ChannelRuntime - no wrapper needed
    let channel = crate::protocols::adapters::opcua::OpcUaChannel::new(
        channel_config,
        config.id,
        config.name.clone(),
    );

    Ok(Box::new(channel))
}

#[cfg(all(feature = "can", target_os = "linux"))]
fn create_can_channel(config: &ChannelConfig) -> Result<Box<dyn ChannelRuntime>> {
    use crate::protocols::adapters::can::CanChannelParamsConfig;

    // Parse parameters
    let params: CanChannelParamsConfig = serde_json::from_value(config.parameters.clone())
        .map_err(|e| GatewayError::Config(format!("Invalid CAN parameters: {}", e)))?;

    // Build channel config
    let channel_config = params.to_config();

    // CanClient directly implements ChannelRuntime - no wrapper needed
    let channel = crate::protocols::adapters::can::CanClient::new(
        channel_config,
        config.id,
        config.name.clone(),
    );

    Ok(Box::new(channel))
}

#[cfg(all(feature = "gpio", target_os = "linux"))]
fn create_gpio_channel(config: &ChannelConfig) -> Result<Box<dyn ChannelRuntime>> {
    use crate::protocols::adapters::gpio::GpioChannelParamsConfig;

    // Parse parameters
    let params: GpioChannelParamsConfig = serde_json::from_value(config.parameters.clone())
        .map_err(|e| GatewayError::Config(format!("Invalid GPIO parameters: {}", e)))?;

    // Build channel config
    let channel_config = params.to_config();

    // GpioChannel directly implements ChannelRuntime - no wrapper needed
    let channel = crate::protocols::adapters::gpio::GpioChannel::new(
        channel_config,
        config.id,
        config.name.clone(),
    );

    Ok(Box::new(channel))
}

fn create_virtual_channel(config: &ChannelConfig) -> Result<Box<dyn ChannelRuntime>> {
    use crate::protocols::adapters::virtual_channel::{VirtualChannel, VirtualChannelParamsConfig};

    // Parse parameters (optional for virtual)
    let params: VirtualChannelParamsConfig =
        serde_json::from_value(config.parameters.clone()).unwrap_or_default();

    // Build point configs
    let points = build_point_configs(config)?;

    // Build channel config
    let channel_config = params.to_config().with_points(points);

    // VirtualChannel now directly implements ChannelRuntime - no wrapper needed
    let channel = VirtualChannel::new(channel_config, config.id);

    Ok(Box::new(channel))
}
