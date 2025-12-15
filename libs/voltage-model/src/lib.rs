//! Voltage Model Library
//!
//! Core domain model for VoltageEMS.
//! This library provides fundamental types and business logic.
//!
//! # Modules
//!
//! - `types`: Core domain types (PointType, PointRole, etc.)
//! - `keyspace`: Redis key generation configuration
//! - `validation`: Input validation utilities for instance names, product names, etc.
//!
//! # Note
//!
//! Product definitions are now loaded from database at runtime.
//! See `modsrv::config` for runtime Product/Instance structures.

pub mod error;
pub mod keyspace;
pub mod types;
pub mod validation;

// Re-exports for convenience
pub use error::{ModelError, Result};
pub use keyspace::KeySpaceConfig;
pub use types::{PointRole, PointType};
pub use validation::{validate_calculation_id, validate_instance_name, validate_product_name};
