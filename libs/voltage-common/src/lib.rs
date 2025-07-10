//! VoltageEMS Common Library
//!
//! This library provides shared utilities and common functionality used across
//! all VoltageEMS microservices.

#![allow(clippy::approx_constant)]

pub mod config;
pub mod error;
pub mod logging;
pub mod redis;
pub mod types;
pub mod utils;

// #[cfg(feature = "http")]
// pub mod http;  // Removed - not compatible with current dependencies

#[cfg(feature = "metrics")]
pub mod metrics;

#[cfg(feature = "test-utils")]
pub mod test_utils;

// Re-exports for convenience
pub use error::{Error, Result};
pub use logging::init_logging;

/// Common prelude for VoltageEMS services
pub mod prelude {
    pub use crate::error::{Error, Result};
    pub use crate::logging::init_logging;
    pub use crate::types::*;
    pub use tracing::{debug, error, info, trace, warn};
}
