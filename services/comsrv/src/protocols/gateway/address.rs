//! Protocol address parsing.
//!
//! Converts shorthand address strings to `ProtocolAddress` enum variants.

use crate::protocols::core::error::{GatewayError, Result};
use crate::protocols::core::point::{
    Iec104Address, ModbusAddress, OpcUaAddress, ProtocolAddress, VirtualAddress,
};

#[cfg(feature = "gpio")]
use crate::protocols::core::point::GpioAddress;

#[cfg(feature = "can")]
use crate::protocols::core::point::CanAddress;

#[cfg(feature = "dl645")]
use crate::protocols::core::point::Dl645Address;

/// Parse a shorthand address string into a `ProtocolAddress`.
///
/// # Address Formats
///
/// - **Modbus**: `"slave_id:register"` or `"slave_id:register:function_code"`
///   - Example: `"1:100"` → slave_id=1, register=100, function_code=3 (default)
///   - Example: `"1:100:4"` → slave_id=1, register=100, function_code=4
///
/// - **IEC104**: `"ioa"` or `"ioa:type_id"`
///   - Example: `"1001"` → ioa=1001
///   - Example: `"1001:13"` → ioa=1001, type_id=13
///
/// - **OPC UA**: Standard OPC UA node ID format
///   - Example: `"ns=2;i=1234"` → namespace=2, node_id="i=1234"
///   - Example: `"ns=2;s=Temperature"` → namespace=2, node_id="s=Temperature"
///   - Example: `"i=1234"` → namespace=0, node_id="i=1234"
///
/// - **CAN**: `"can_id:byte_offset:bit_pos:bit_len"`
///   - Example: `"0x100:0:0:16"` → can_id=0x100, byte_offset=0, bit_pos=0, bit_len=16
///
/// - **GPIO**: `"pin_number"` or `"pin_number:direction"`
///   - Example: `"17"` → pin=17, direction=input (default)
///   - Example: `"18:output"` → pin=18, direction=output
///
/// - **Virtual**: Any string key
///   - Example: `"temperature"` → key="temperature"
pub fn parse_address(protocol: &str, address: &str) -> Result<ProtocolAddress> {
    // Use eq_ignore_ascii_case to avoid String allocation from to_lowercase()
    if protocol.eq_ignore_ascii_case("modbus") {
        parse_modbus_address(address)
    } else if protocol.eq_ignore_ascii_case("iec104") {
        parse_iec104_address(address)
    } else if protocol.eq_ignore_ascii_case("opcua") {
        parse_opcua_address(address)
    } else if protocol.eq_ignore_ascii_case("can") {
        parse_can_address(address)
    } else if protocol.eq_ignore_ascii_case("virtual") {
        Ok(ProtocolAddress::Virtual(VirtualAddress::new(address)))
    } else {
        #[cfg(feature = "gpio")]
        if protocol.eq_ignore_ascii_case("gpio") {
            return parse_gpio_address(address);
        }
        #[cfg(feature = "dl645")]
        if protocol.eq_ignore_ascii_case("dl645") {
            return parse_dl645_address(address);
        }
        Err(GatewayError::Config(format!(
            "Unknown protocol: {}",
            protocol
        )))
    }
}

/// Parse Modbus address: "slave_id:register" or "slave_id:register:function_code"
fn parse_modbus_address(address: &str) -> Result<ProtocolAddress> {
    // Use splitn to avoid Vec allocation
    let mut parts = address.splitn(3, ':');

    let slave_id_str = parts
        .next()
        .ok_or_else(|| GatewayError::Config("Missing slave_id".into()))?;
    let register_str = parts.next().ok_or_else(|| {
        GatewayError::Config(format!(
            "Invalid Modbus address format: {}. Expected 'slave_id:register'",
            address
        ))
    })?;
    let fc_str = parts.next(); // Optional third part

    let slave_id = slave_id_str
        .parse::<u8>()
        .map_err(|_| GatewayError::Config(format!("Invalid slave_id: {}", slave_id_str)))?;
    let register = register_str
        .parse::<u16>()
        .map_err(|_| GatewayError::Config(format!("Invalid register: {}", register_str)))?;

    match fc_str {
        None => Ok(ProtocolAddress::Modbus(ModbusAddress::holding_register(
            slave_id,
            register,
            crate::protocols::core::point::DataFormat::default(),
        ))),
        Some(fc) => {
            let function_code = fc
                .parse::<u8>()
                .map_err(|_| GatewayError::Config(format!("Invalid function_code: {}", fc)))?;

            Ok(ProtocolAddress::Modbus(ModbusAddress {
                slave_id,
                register,
                function_code,
                format: crate::protocols::core::point::DataFormat::default(),
                byte_order: crate::protocols::core::point::ByteOrder::default(),
                bit_position: None,
            }))
        },
    }
}

/// Parse IEC104 address: "ioa" or "ioa:type_id"
fn parse_iec104_address(address: &str) -> Result<ProtocolAddress> {
    // Use split_once to avoid Vec allocation
    match address.split_once(':') {
        None => {
            // Just "ioa"
            let ioa = address
                .parse::<u32>()
                .map_err(|_| GatewayError::Config(format!("Invalid IOA: {}", address)))?;

            Ok(ProtocolAddress::Iec104(Iec104Address {
                ioa,
                type_id: 0, // Will be inferred from data
                common_address: 1,
            }))
        },
        Some((ioa_str, type_id_str)) => {
            let ioa = ioa_str
                .parse::<u32>()
                .map_err(|_| GatewayError::Config(format!("Invalid IOA: {}", ioa_str)))?;
            let type_id = type_id_str
                .parse::<u8>()
                .map_err(|_| GatewayError::Config(format!("Invalid type_id: {}", type_id_str)))?;

            Ok(ProtocolAddress::Iec104(Iec104Address {
                ioa,
                type_id,
                common_address: 1,
            }))
        },
    }
}

/// Parse OPC UA address: "ns=N;i=ID" or "ns=N;s=Name" or "i=ID"
fn parse_opcua_address(address: &str) -> Result<ProtocolAddress> {
    let (namespace_index, node_id_str) = if address.starts_with("ns=") {
        // Has namespace prefix
        if let Some(semi_pos) = address.find(';') {
            let ns_str = &address[3..semi_pos];
            let ns_idx = ns_str
                .parse()
                .map_err(|_| GatewayError::Config(format!("Invalid namespace: {}", ns_str)))?;
            (ns_idx, &address[semi_pos + 1..])
        } else {
            return Err(GatewayError::Config(format!(
                "Invalid OPC UA address format: {}. Expected 'ns=N;i=ID' or 'ns=N;s=Name'",
                address
            )));
        }
    } else {
        // No namespace prefix, default to 0
        (0u16, address)
    };

    // Validate node ID format
    if !node_id_str.starts_with("i=")
        && !node_id_str.starts_with("s=")
        && !node_id_str.starts_with("g=")
        && !node_id_str.starts_with("b=")
    {
        return Err(GatewayError::Config(format!(
            "Invalid OPC UA node ID: {}. Expected 'i=N', 's=Name', 'g=GUID', or 'b=Base64'",
            node_id_str
        )));
    }

    Ok(ProtocolAddress::OpcUa(OpcUaAddress {
        node_id: node_id_str.to_string(), // Single allocation at the end
        namespace_index,
    }))
}

/// Parse CAN address: "can_id:byte_offset:bit_pos:bit_len"
#[cfg(feature = "can")]
fn parse_can_address(address: &str) -> Result<ProtocolAddress> {
    let can_addr = CanAddress::parse(address)?;
    Ok(ProtocolAddress::Can(can_addr))
}

/// Parse CAN address (fallback when `can` feature is disabled).
#[cfg(not(feature = "can"))]
fn parse_can_address(address: &str) -> Result<ProtocolAddress> {
    // Store as Generic when CAN feature is disabled
    Ok(ProtocolAddress::Generic(address.to_string()))
}

/// Parse GPIO address: "pin_number" or "chip:pin" or "chip:pin:direction"
#[cfg(feature = "gpio")]
fn parse_gpio_address(address: &str) -> Result<ProtocolAddress> {
    // Use splitn to avoid Vec allocation
    let mut parts = address.splitn(3, ':');

    let first = parts
        .next()
        .ok_or_else(|| GatewayError::Config("Empty GPIO address".into()))?;

    match parts.next() {
        None => {
            // Just pin number, default chip
            let pin = first
                .parse::<u32>()
                .map_err(|_| GatewayError::Config(format!("Invalid GPIO pin: {}", first)))?;
            Ok(ProtocolAddress::Gpio(GpioAddress::digital_input(
                "gpiochip0",
                pin,
            )))
        },
        Some(pin_str) => {
            let chip = first.to_string();
            let pin = pin_str
                .parse::<u32>()
                .map_err(|_| GatewayError::Config(format!("Invalid GPIO pin: {}", pin_str)))?;

            match parts.next() {
                None => {
                    // chip:pin
                    Ok(ProtocolAddress::Gpio(GpioAddress::digital_input(chip, pin)))
                },
                Some(dir) => {
                    // chip:pin:direction - use eq_ignore_ascii_case to avoid allocation
                    let addr = if dir.eq_ignore_ascii_case("input")
                        || dir.eq_ignore_ascii_case("in")
                        || dir.eq_ignore_ascii_case("di")
                    {
                        GpioAddress::digital_input(chip, pin)
                    } else if dir.eq_ignore_ascii_case("output")
                        || dir.eq_ignore_ascii_case("out")
                        || dir.eq_ignore_ascii_case("do")
                    {
                        GpioAddress::digital_output(chip, pin)
                    } else {
                        return Err(GatewayError::Config(format!(
                            "Invalid GPIO direction: {}. Expected 'input' or 'output'",
                            dir
                        )));
                    };
                    Ok(ProtocolAddress::Gpio(addr))
                },
            }
        },
    }
}

/// Parse DL/T 645 address: "meter_addr:data_id"
///
/// Format:
/// - meter_addr: 12-digit BCD meter address
/// - data_id: 8-character hex data identifier
///
/// Example: "123456789012:00010000" for total positive active energy
#[cfg(feature = "dl645")]
fn parse_dl645_address(address: &str) -> Result<ProtocolAddress> {
    let dl645_addr = Dl645Address::parse(address)?;
    Ok(ProtocolAddress::Dl645(dl645_addr))
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // unwrap in tests
mod tests {
    use super::*;

    #[test]
    fn test_parse_modbus_address() {
        let addr = parse_modbus_address("1:100").unwrap();
        if let ProtocolAddress::Modbus(m) = addr {
            assert_eq!(m.slave_id, 1);
            assert_eq!(m.register, 100);
            assert_eq!(m.function_code, 3);
        } else {
            panic!("Expected Modbus address");
        }
    }

    #[test]
    fn test_parse_modbus_address_with_function() {
        let addr = parse_modbus_address("2:200:4").unwrap();
        if let ProtocolAddress::Modbus(m) = addr {
            assert_eq!(m.slave_id, 2);
            assert_eq!(m.register, 200);
            assert_eq!(m.function_code, 4);
        } else {
            panic!("Expected Modbus address");
        }
    }

    #[test]
    fn test_parse_iec104_address() {
        let addr = parse_iec104_address("1001").unwrap();
        if let ProtocolAddress::Iec104(i) = addr {
            assert_eq!(i.ioa, 1001);
        } else {
            panic!("Expected IEC104 address");
        }
    }

    #[test]
    fn test_parse_opcua_address() {
        let addr = parse_opcua_address("ns=2;i=1234").unwrap();
        if let ProtocolAddress::OpcUa(o) = addr {
            assert_eq!(o.namespace_index, 2);
            assert_eq!(o.node_id, "i=1234");
        } else {
            panic!("Expected OPC UA address");
        }
    }

    #[test]
    fn test_parse_opcua_address_no_namespace() {
        let addr = parse_opcua_address("i=1234").unwrap();
        if let ProtocolAddress::OpcUa(o) = addr {
            assert_eq!(o.namespace_index, 0);
            assert_eq!(o.node_id, "i=1234");
        } else {
            panic!("Expected OPC UA address");
        }
    }

    #[test]
    fn test_parse_virtual_address() {
        let addr = parse_address("virtual", "temperature").unwrap();
        if let ProtocolAddress::Virtual(v) = addr {
            assert_eq!(v.tag, "temperature");
        } else {
            panic!("Expected Virtual address");
        }
    }
}
