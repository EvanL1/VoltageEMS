pub mod common;
pub mod modbus;
// pub mod iec60870;
// pub mod can;

// TODO: Implement protocol parser registry
// use crate::core::protocols::common::combase::get_global_parser_registry;
// use crate::core::protocols::modbus::ModbusPacketParser;
// use crate::core::protocols::can::CanPacketParser;
// use crate::core::protocols::iec60870::Iec60870PacketParser;

/// Initialize all protocol parsers
///
/// Placeholder - to be implemented when parser registry is ready
pub fn init_protocol_parsers() {
    // TODO: Implement protocol parser initialization
    tracing::info!("Protocol parsers initialization placeholder");
}