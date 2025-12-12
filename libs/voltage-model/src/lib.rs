//! Voltage Model Library
//!
//! Core domain model for VoltageEMS.
//! This library provides fundamental types and business logic.
//!
//! # Modules
//!
//! - `types`: Core domain types (PointType, etc.)
//! - `products`: Product type definitions (ProductType, ProductDef, PointDef)
//! - `validation`: Input validation utilities for instance names, product names, etc.
//!
//! # Note
//!
//! For expression evaluation, use `voltage-calc` crate which provides
//! a more powerful calculation engine with stateful functions.

pub mod error;
pub mod products;
pub mod types;
pub mod validation;

// Re-exports for convenience
pub use error::{ModelError, Result};
pub use products::{PointDef, ProductDef, ProductType};
pub use types::PointType;
pub use validation::{validate_calculation_id, validate_instance_name, validate_product_name};
