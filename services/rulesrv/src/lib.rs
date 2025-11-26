//! RuleSrv Library
//!
//! Rule engine business logic for VoltageEMS.
//!
//! ## Architecture
//!
//! This service uses Vue Flow-based rule chains:
//! - `chain_executor`: Executes Vue Flow rule chains stored as `RuleChain`
//! - `parser`: Parses Vue Flow JSON into executable `FlowNode` structures
//! - `rules_repository`: SQLite persistence for `rules` table

// Core modules
pub mod app;
pub mod chain_executor;
pub mod error;
pub mod parser;
pub mod reload;
pub mod routes;
pub mod rules_repository;

// Re-export commonly used types
pub use chain_executor::{ActionExecuted, ChainExecutionResult, ChainExecutor};
pub use error::{Result, RuleSrvError};
pub use parser::parse_flow_json;

// Re-export app state and initialization
pub use app::{create_app_state, AppState};

// Re-export route creation
pub use routes::create_routes;
