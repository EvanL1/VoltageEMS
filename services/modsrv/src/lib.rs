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
//!     storage::memory_store::MemoryStore,
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
/// including Redis settings, model parameters, and control configurations.
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
/// and data serialization for integration with Redis-based storage.
pub mod redis_handler;

/// Control operations and automation
/// 
/// Automated control system with condition-based actions, remote control
/// capabilities, and integration with model outputs for decision making.
pub mod control;

/// Template management system
/// 
/// Template-based model creation system allowing reusable model definitions
/// with configurable parameters and standardized interfaces.
pub mod template;

/// Data storage abstraction layer
/// 
/// Multi-backend storage system supporting Redis, memory, and hybrid
/// storage modes with consistent interfaces and synchronization options.
pub mod storage;

/// Storage agent for backend management
/// 
/// High-level storage management with automatic backend selection,
/// connection pooling, and performance optimization.
pub mod storage_agent;

/// RESTful API endpoints
/// 
/// Complete HTTP API for model management, template operations,
/// system monitoring, and real-time data access.
pub mod api;

/// Rules and conditions system
/// 
/// Rule-based logic system for complex decision making, condition
/// evaluation, and automated responses to system state changes.
pub mod rules;

/// Rules execution engine
/// 
/// Runtime engine for executing rules, managing rule lifecycles,
/// and coordinating rule-based control actions.
pub mod rules_engine;

/// System monitoring and metrics
/// 
/// Performance monitoring, system health checks, and metrics collection
/// for operational visibility and troubleshooting.
pub mod monitoring;

// Re-export commonly used types for convenience
pub use storage_agent::StorageAgent;
pub use storage::DataStore;
pub use storage::SyncMode;
pub use error::{Result, ModelSrvError}; 