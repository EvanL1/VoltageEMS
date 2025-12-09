//! Voltage Model Library
//!
//! Core model logic for VoltageEMS.
//! This library provides pure business logic without service dependencies.

#![allow(clippy::disallowed_methods)] // json! macro internally uses unwrap

//! # Modules
//!
//! - `expression`: Mathematical expression evaluation using evalexpr
//! - `timeseries`: Time series analysis (moving average, rate of change)
//! - `validation`: Input validation utilities
//!
//! # Example
//!
//! ```
//! use voltage_model::ExpressionEvaluator;
//! use std::collections::HashMap;
//!
//! // Direct expression evaluation
//! let evaluator = ExpressionEvaluator::new();
//! let mut vars = HashMap::new();
//! vars.insert("a".to_string(), 10.0);
//! vars.insert("b".to_string(), 5.0);
//! let result = evaluator.evaluate("a + b * 2", &vars).unwrap();
//! assert_eq!(result, 20.0);
//! ```

pub mod error;
pub mod expression;
pub mod timeseries;
pub mod validation;

// Re-exports for convenience
pub use error::{ModelError, Result};
pub use expression::ExpressionEvaluator;
pub use timeseries::TimeSeriesProcessor;
pub use validation::{validate_calculation_id, validate_instance_name, validate_product_name};
