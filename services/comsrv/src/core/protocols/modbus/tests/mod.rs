//! Modbus protocol test suite
//! 
//! This module contains comprehensive tests for the Modbus protocol implementation,
//! including unit tests, integration tests, and performance benchmarks.

// Test modules
pub mod pdu_tests;
pub mod frame_tests;
pub mod client_tests;
pub mod polling_tests;
pub mod integration_tests;
pub mod mock_transport;
pub mod test_helpers;
// pub mod simple_integration_test; // File removed
// pub mod logging_test;
// pub mod simple_logging_test;

// Re-export commonly used test utilities
pub use mock_transport::MockTransport;
pub use test_helpers::*;