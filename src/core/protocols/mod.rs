pub mod common;
pub mod modbus;
pub mod iec60870;
pub mod can;
pub mod factory;

use crate::core::protocols::common::combase::get_global_parser_registry;
use crate::core::protocols::modbus::ModbusPacketParser;

/// Initialize all protocol parsers
/// 
/// Registers all available protocol parsers with the global registry.
/// This function should be called during application startup.
pub fn init_protocol_parsers() {
    let registry = get_global_parser_registry();
    
    // Register Modbus parser
    registry.register_parser(ModbusPacketParser::new());
    
    // TODO: Register other protocol parsers as they are implemented
    // registry.register_parser(CanPacketParser::new());
    // registry.register_parser(Iec60870PacketParser::new());
    
    tracing::info!("Protocol parsers initialized: {:?}", registry.registered_protocols());
} 