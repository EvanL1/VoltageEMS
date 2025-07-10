//! Domain module for alarm service
//!
//! This module contains the core domain logic, entities, and business rules
//! for the alarm service.

pub mod alarm;
pub mod classification;
pub mod types;

pub use classification::*;
pub use types::*;
