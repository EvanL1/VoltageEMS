//! Communication Base Module
//!
//! This module provides the foundational traits and types for implementing
//! communication protocols in the Voltage EMS Communication Service.

// Core data types and structures
pub mod data_types;
pub mod telemetry;
pub mod traits;
pub mod default_protocol;
pub mod stats;

// Specialized functionality
pub mod polling;
pub mod command_manager;
pub mod point_manager;
pub mod forward_calc;
pub mod protocol_factory;

// Implementation modules
pub mod impl_base;

// Re-export commonly used types
pub use data_types::*;
pub use telemetry::*;
pub use traits::{ComBase, FourTelemetryOperations, ConnectionManager, ConfigValidator, ProtocolPacketParser};
pub use default_protocol::{DefaultProtocol, PacketParseResult};
pub use polling::{PollingEngine, UniversalPollingEngine, PointReader};
pub use command_manager::UniversalCommandManager;
pub use point_manager::{UniversalPointManager, UniversalPointConfig, PointManagerStats};
pub use protocol_factory::ProtocolFactory;
pub use impl_base::ComBaseImpl;