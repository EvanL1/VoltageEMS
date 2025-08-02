//! Simplified Alarm Service Library
//!
//! This library provides a streamlined alarm service using Redis Functions directly.
//! The KISS (Keep It Simple, Stupid) principle guides this implementation.
//!
//! ## Core Features
//!
//! - **Create/Store Alarms**: Simple alarm creation and storage using Redis Functions
//! - **Query Alarms**: Basic filtering and pagination for alarm retrieval  
//! - **Update Alarm State**: Acknowledge and resolve alarms with state validation
//! - **Basic Statistics**: Simple alarm counts and statistics
//! - **HTTP API**: RESTful endpoints for external integration
//!
//! ## Architecture
//!
//! ```text
//! HTTP API → AlarmService → Redis Functions → Redis Storage
//!     ↓           ↓              ↓              ↓
//!   api.rs → alarm_service.rs → Lua Scripts → Hash/Sets
//! ```
//!
//! ## Files (8 total, ~2000 lines)
//!
//! - `alarm_service.rs` - Core alarm operations using Redis Functions
//! - `api.rs` - HTTP API endpoints and handlers
//! - `config.rs` - Simple configuration management
//! - `error.rs` - Unified error handling
//! - `main.rs` - Application entry point
//! - `lib.rs` - Library exports
//!
//! This replaces the previous 28-file, 4500+ line complex implementation.

#![allow(dead_code)]
#![allow(unused_imports)]

/// Core alarm service functionality
pub mod alarm_service;

/// HTTP API endpoints and handlers
pub mod api;

/// Configuration management
pub mod config;

/// Error handling
pub mod error;

// Re-export main types
pub use alarm_service::{
    Alarm, AlarmLevel, AlarmQuery, AlarmQueryResult, AlarmService, AlarmStatistics, AlarmStatus,
};
pub use api::{AppState, CreateAlarmRequest};
pub use config::AlarmConfig;
pub use error::{AlarmError, Result};

/// Service version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Service name
pub const SERVICE_NAME: &str = "alarmsrv";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_info() {
        assert_eq!(VERSION, "0.0.1");
        assert_eq!(SERVICE_NAME, "alarmsrv");
    }
}
