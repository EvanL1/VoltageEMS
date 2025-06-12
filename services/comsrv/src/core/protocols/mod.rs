pub mod common;
pub mod modbus;
pub mod iec60870;
pub mod can;

use crate::core::protocols::common::combase::get_global_parser_registry;
use crate::core::protocols::modbus::ModbusPacketParser;
use crate::core::protocols::can::CanPacketParser;
use crate::core::protocols::iec60870::Iec60870PacketParser;

/// Initialize all protocol parsers
/// 
/// Registers all available protocol parsers with the global registry.
/// This function should be called during application startup.
pub fn init_protocol_parsers() {
    let mut registry = get_global_parser_registry().write();

    // Register Modbus parser
    registry.register_parser(ModbusPacketParser::new());

    // Register CAN parser
    registry.register_parser(CanPacketParser::new());

    // Register IEC60870 parser
    registry.register_parser(Iec60870PacketParser::new());

    tracing::info!(
        "Protocol parsers initialized: {:?}",
        registry.registered_protocols()
    );
}
