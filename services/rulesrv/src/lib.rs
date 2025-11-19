//! RuleSrv Library
//!
//! Rule engine business logic for VoltageEMS

// Core modules (existing)
pub mod action_executor;
pub mod condition_evaluator;
pub mod error;
pub mod rule_engine;
pub mod rule_logger; // Rule execution logger
pub mod rules_repository;

// New extracted modules
pub mod app;
pub mod reload;
pub mod routes;

// Re-export commonly used types
pub use action_executor::ActionExecutor;
pub use condition_evaluator::ConditionEvaluator;
pub use error::{Result, RuleSrvError};
pub use rule_engine::{
    Action, ActionResult, Condition, ConditionGroup, ExecutionContext, ExecutionResult, Rule,
    RuleConfig,
};

// Re-export app state and initialization
pub use app::{create_app_state, AppState};

// Re-export route creation
pub use routes::create_routes;
