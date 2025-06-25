//! Stress test module
//!
//! Provides various scales of Modbus + Redis stress tests

pub mod comsrv_pressure_test; // Stress test using comsrv implementation
pub mod multi_protocol_pressure_test; // Multi-protocol stress test
pub mod utils; // Test utility functions

// Re-export main functions
pub use comsrv_pressure_test::*;
pub use multi_protocol_pressure_test::*;
