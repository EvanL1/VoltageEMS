//! Voltage Rules - Rule Engine Library
//!
//! A Vue Flow-based rule engine for VoltageEMS providing:
//! - Rule parsing from Vue Flow JSON format
//! - Rule execution with condition evaluation and action dispatch
//! - Rule scheduling with interval-based triggers
//! - SQLite persistence for rule storage
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────┐     ┌──────────────┐     ┌─────────────┐
//! │  Scheduler  │────▶│   Executor   │────▶│    RTDB     │
//! │  (100ms)    │     │  (evaluate)  │     │  (read/write)│
//! └─────────────┘     └──────────────┘     └─────────────┘
//!        │                   │
//!        ▼                   ▼
//! ┌─────────────┐     ┌──────────────┐
//! │ Repository  │     │RoutingCache  │
//! │  (SQLite)   │     │  (M2C route) │
//! └─────────────┘     └──────────────┘
//! ```

mod error;
mod executor;
mod parser;
mod repository;
mod scheduler;

// Re-export public API
pub use error::{Result, RuleError};
pub use executor::{ActionResult, RuleExecutionResult, RuleExecutor};
pub use parser::extract_rule_flow;
pub use repository::{
    delete_rule, get_rule, get_rule_for_execution, list_rules, load_all_rules, load_enabled_rules,
    set_rule_enabled, upsert_rule,
};
pub use scheduler::{RuleScheduler, SchedulerStatus, TriggerConfig, DEFAULT_TICK_MS};
