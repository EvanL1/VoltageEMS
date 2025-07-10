//! # Rules Service (rulesrv)
//!
//! A rule execution engine for VoltageEMS that processes complex business rules
//! based on real-time data from other services.
//!
//! ## Overview
//!
//! rulesrv is designed to handle rule-based decision making, condition evaluation,
//! and action execution in response to data changes from the model service (modsrv).
//! It subscribes to model outputs via Redis pub/sub and executes configured rules.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
//! │  Redis Sub      │───►│  Rule Engine    │───►│ Action Handler  │
//! │  (Model Output) │    │  (DAG Executor) │    │  (Control/Alert)│
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

/// Redis integration for pub/sub
pub mod redis;

/// Rule definitions and management
pub mod rules;

/// Action handlers for rule execution
pub mod actions;

// Re-export commonly used types
pub use config::Config;
pub use error::{Result, RulesrvError};
