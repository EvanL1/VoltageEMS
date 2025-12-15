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
pub mod logger;
mod parser;
mod repository;
mod scheduler;
pub mod types;

// Re-export public API
pub use error::{Result, RuleError};
pub use executor::{ActionResult, RuleExecutionResult, RuleExecutor};
pub use logger::{format_conditions, RuleLogger, RuleLoggerManager};
pub use parser::extract_rule_flow;
pub use repository::{
    delete_rule, get_rule, get_rule_for_execution, list_rules, list_rules_paginated,
    load_all_rules, load_enabled_rules, set_rule_enabled, upsert_rule,
};
pub use scheduler::{RuleScheduler, SchedulerStatus, TriggerConfig, DEFAULT_TICK_MS};

// Re-export rule types for convenience
pub use types::{
    CalculationRule, FlowCondition, Rule, RuleFlow, RuleNode, RuleSwitchBranch,
    RuleValueAssignment, RuleVariable, RuleWires,
};
