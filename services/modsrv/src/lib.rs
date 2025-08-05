//! # ModSrv - Model Service
//!
//! Concise and efficient industrial IoT model service providing device model management, data subscription and control interfaces.
//!
//! ## Core Features
//!
//! 1. **Configuration Loading**: Load model definitions from configuration files and initialize in Redis
//! 2. **Data Synchronization**: Implement bidirectional data synchronization with ComsRv through Lua scripts
//! 3. **Control Interface**: Provide HTTP API interface to handle external control commands
//!
//! ## Architecture Design
//!
//! ```text
//! Config Loading → Model Initialization → Lua Sync → API Interface
//!       ↓                  ↓                ↓           ↓
//!  config.rs → model.rs → EdgeRedis → api.rs
//! ```
//!
//! ## Basic Usage
//!
//! ### Configure Model
//!
//! ```json
//! {
//!   "id": "power_meter",
//!   "name": "Power Meter",
//!   "description": "Smart power meter monitoring",
//!   "monitoring": {
//!     "voltage": {
//!       "description": "Voltage",
//!       "unit": "V"
//!     },
//!     "current": {
//!       "description": "Current",
//!       "unit": "A"
//!     }
//!   },
//!   "control": {
//!     "switch": {
//!       "description": "Main switch"
//!     },
//!     "limit": {
//!       "description": "Power limit",
//!       "unit": "kW"
//!     }
//!   }
//! }
//! ```
//!
//! ### Start Service
//!
//! ```bash
//! # Run service
//! modsrv service
//!
//! # View model information
//! modsrv info
//!
//! # Check configuration
//! modsrv check-config
//! ```
//!
//! ### API Endpoints
//!
//! ```bash
//! # Health check
//! GET /health
//!
//! # Get model list
//! GET /models
//!
//! # Get model real-time data
//! GET /models/{model_id}/values
//!
//! # Execute control command
//! POST /models/{model_id}/control/{control_name}
//! {"value": 1.0}
//!
//! ```

#![allow(dead_code)]
#![allow(unused_imports)]

/// Configuration management module
///
/// Provides configuration file loading, environment variable processing and configuration validation
pub mod config;

/// Error handling module
///
/// Defines unified error types and result handling
pub mod error;

/// Core model module
///
/// Contains model definitions, data reading, control command processing and other core functions
pub mod model;

/// REST API module
///
/// Provides HTTP interfaces for model management and control operations
pub mod api;

/// Template management module
///
/// Provides device model template functionality for quick model creation
pub mod template;

// Re-export commonly used types
pub use api::ApiServer;
pub use config::Config;
pub use error::{ModelSrvError, Result};
pub use model::{ModelInstance, ModelManager, ModelMapping};
pub use template::{Template, TemplateManager};

/// Service version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Service name
pub const SERVICE_NAME: &str = "modsrv";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_info() {
        // VERSION is a compile-time constant and always has a value
        assert_eq!(VERSION, "0.0.1");
        assert_eq!(SERVICE_NAME, "modsrv");
    }
}
