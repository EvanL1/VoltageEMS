pub mod common;
pub mod modbus;
// TODO: 暂时屏蔽，等核心组件稳定后再启用
// pub mod iec60870;
// pub mod can;

use crate::core::protocols::common::combase::get_global_parser_registry;
use crate::core::protocols::modbus::ModbusPacketParser;
// TODO: 暂时屏蔽，等核心组件稳定后再启用
// use crate::core::protocols::can::CanPacketParser;
// use crate::core::protocols::iec60870::Iec60870PacketParser;

/// Initialize all protocol parsers
/// 
/// Registers all available protocol parsers with the global registry.
/// This function should be called during application startup.
pub fn init_protocol_parsers() {
    let mut registry = get_global_parser_registry().write();

    // Register Modbus parser
    registry.register_parser(ModbusPacketParser::new());

    // TODO: 暂时屏蔽，等核心组件稳定后再启用
    // Register CAN parser
    // registry.register_parser(CanPacketParser::new());

    // Register IEC60870 parser
    // registry.register_parser(Iec60870PacketParser::new());

    log::info!(
        "Protocol parsers initialized: {:?}",
        registry.registered_protocols()
    );
}
