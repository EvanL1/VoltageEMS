//! # Rules Service (rulesrv)
//!
//! A rule execution engine for VoltageEMS that processes complex business rules
//! based on real-time data from other services.
//!
//! ## Overview
//!
//! rulesrv is designed to handle rule-based decision making, condition evaluation,
//! and action execution based on real-time data from Redis.
//! It provides a simple rule engine with condition evaluation and action execution.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
//! │  Redis Store    │───►│  Rule Engine    │───►│ Action Handler  │
//! │  (Data Source)  │    │  (Conditions)   │    │  (Control/Alert)│
//! └─────────────────┘    └─────────────────┘    └─────────────────┘
//! ```

/// API module for REST endpoints
pub mod api;

/// Configuration management
pub mod config;

/// Rule execution engine
pub mod engine;

/// Error types and result handling
pub mod error;

/// Redis integration
pub mod redis;

// Re-export commonly used types
pub use config::Config;
pub use error::{Result, RulesrvError};
