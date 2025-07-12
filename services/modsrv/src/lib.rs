//! # Model Service (ModSrv)
//!
//! A comprehensive model execution engine and control system for energy management systems.
//! This library provides real-time data processing, model execution, and automated control
//! operations with Redis-based data storage and multi-backend support.
//!
//! ## Overview
//!
//! ModSrv is designed to handle complex data processing workflows in industrial environments,
//! particularly for energy management and power system automation. It provides a template-based
//! model system, rule-based control logic, and comprehensive monitoring capabilities.
//!
//! ## Key Features
//!
//! - **Template-based Model System**: Define reusable models with configurable parameters
//! - **Real-time Data Processing**: Process incoming data streams with configurable intervals
//! - **Rule-based Control**: Execute automated control actions based on model outputs
//! - **Multi-backend Storage**: Support for Redis, memory, and hybrid storage backends
//! - **RESTful API**: Complete HTTP API for management and monitoring
//! - **Performance Monitoring**: Built-in metrics and performance tracking
//! - **Configuration Management**: Flexible YAML-based configuration system
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
//! │   Data Input    │───►│  Model Engine   │───►│ Control Manager │
//! │   (Redis/API)   │    │   (Templates)   │    │   (Actions)     │
//! └─────────────────┘    └─────────────────┘    └─────────────────┘
//!          │                       │                       │
//!          ▼                       ▼                       ▼
//! ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
//! │  Storage Agent  │    │ Rules Engine    │    │  Monitoring     │
//! │   (Backends)    │    │  (Conditions)   │    │  (Metrics)      │
//! └─────────────────┘    └─────────────────┘    └─────────────────┘
//! ```
//!
//! ## Basic Usage
//!
//! ### Creating a Simple Model Template
//!
//! ```rust
//! use modsrv::{
//!     template::{TemplateDefinition, TemplateDataMapping},
//!     model::{ModelDefinition, DataMapping},
//!     storage::redis_store::RedisStore,
//!     Result
//! };
//! use std::collections::HashMap;
//! use serde_json::Value;
//!
//! // Create a template for power calculation
//! let template = TemplateDefinition {
//!     id: "power_calc".to_string(),
//!     name: "Power Calculation".to_string(),
//!     description: "Calculate total power from voltage and current".to_string(),
//!     input_mappings: vec![
//!         TemplateDataMapping {
//!             source: "voltage".to_string(),
//!             target: "v".to_string(),
//!             transform: None,
//!         },
//!         TemplateDataMapping {
//!             source: "current".to_string(),
//!             target: "i".to_string(),
//!             transform: None,
//!         },
//!     ],
//!     output_key: "calculated_power".to_string(),
//!     config: Value::Object(serde_json::Map::new()),
//!     version: "1.0".to_string(),
//! };
//!
//! // Create a model instance from the template
//! let model = ModelDefinition {
//!     id: "main_power_calc".to_string(),
//!     name: "Main Power Calculator".to_string(),
//!     description: "Calculate power for main electrical feed".to_string(),
//!     input_mappings: vec![
//!         DataMapping {
//!             source_key: "device:001:voltage".to_string(),
//!             source_field: "value".to_string(),
//!             target_field: "voltage".to_string(),
//!             transform: None,
//!         },
//!         DataMapping {
//!             source_key: "device:001:current".to_string(),
//!             source_field: "value".to_string(),
//!             target_field: "current".to_string(),
//!             transform: None,
//!         },
//!     ],
//!     output_key: "main_power".to_string(),
//!     enabled: true,
//!     template_id: "power_calc".to_string(),
//!     config: HashMap::new(),
//! };
//! ```
//!
//! ### Running the Model Engine
//!
//! ```rust,no_run
//! use modsrv::{
//!     config::Config,
//!     model::ModelEngine,
//!     storage::redis_store::RedisStore,
//!     Result
//! };
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!     // Load configuration
//!     let config = Config::from_file("config.yaml")?;
//!     
//!     // Create storage backend
//!     let store = RedisStore::new(&config.redis)?;
//!     
//!     // Create and configure model engine
//!     let mut engine = ModelEngine::new(&config.redis.key_prefix);
//!     
//!     // Load models from storage
//!     engine.load_models(&store, "modsrv:model:config:*")?;
//!     
//!     // Execute all loaded models
//!     engine.execute_models(&store)?;
//!     
//!     Ok(())
//! }
//! ```
//!
//! ## Module Organization
//!
//! The library is organized into several key modules:

/// Configuration management and validation
///
/// Provides configuration structures and file loading for the model service,
/// including Redis settings and model parameters.
pub mod config;

/// Error types and result handling
///
/// Comprehensive error handling with specific error types for different
/// failure scenarios, supporting both internal errors and integration
/// with external systems.
pub mod error;

/// Model definitions and execution engine
///
/// Core model system including model definitions, data mappings,
/// template-based instantiation, and the main model execution engine.
pub mod model;

/// Redis connection and data handling
///
/// Redis client wrapper with connection management, error handling,
/// and data serialization for integration with Redis.
pub mod redis_handler;

/// Template management system
///
/// Template-based model creation system allowing reusable model definitions
/// with configurable parameters and standardized interfaces.
pub mod template;

/// RESTful API endpoints
///
/// Complete HTTP API for model management, template operations,
/// system monitoring, and real-time data access.
pub mod api;

/// System monitoring and metrics
///
/// Performance monitoring, system health checks, and metrics collection
/// for operational visibility and troubleshooting.
pub mod monitoring;

/// Storage layer for model data
///
/// Unified storage interface supporting monitor values and control commands,
/// with optimized Redis-based implementation following comsrv patterns.
pub mod storage;

/// ComSrv interface for point data access
///
/// High-performance interface for reading comsrv point data and sending
/// control commands through Redis pub/sub channels.
pub mod comsrv_interface;

/// Optimized data reading utilities
///
/// Provides efficient batch reading, caching strategies, and data aggregation
/// for high-throughput point data access from comsrv.
pub mod data_reader;

/// Control command sending utilities
///
/// Reliable control command sending with retry mechanisms, status tracking,
/// and priority-based command queuing for comsrv integration.
pub mod control_sender;

/// Rule engine for conditional logic
///
/// Provides rule-based conditional execution, supporting complex conditions
/// and actions for automated control operations.
pub mod rule_engine;

/// Cache management for model data
///
/// High-performance in-memory caching with TTL support, providing fast access
/// to frequently used point data and model outputs.
pub mod cache;

/// Optimized async model execution engine
///
/// Provides concurrent model execution with configurable parallelism,
/// supporting async operations and efficient resource utilization.
pub mod engine;

/// Device model system for physical device abstraction
///
/// Provides comprehensive device modeling with properties, telemetry,
/// commands, and real-time data flow processing.
pub mod device_model;

// Re-export commonly used types for convenience
pub use error::{ModelSrvError, Result};
