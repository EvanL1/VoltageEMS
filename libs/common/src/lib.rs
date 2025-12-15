//! `VoltageEMS` basic library (basic library)
//!
//! Provides basic functions shared by all services, including:
//! - Redis client
//! - monitoring and health checking
//! - logging functions
//! - service configuration types

#[cfg(feature = "redis")]
pub mod redis;

#[cfg(feature = "sqlite")]
pub mod sqlite;

pub mod service_config;

// Common modules
pub mod admin_api;
pub mod api_types;
pub mod config_loader;
pub mod logging;
pub mod serde_helpers;
pub mod service_bootstrap;
pub mod shutdown;
pub mod system_metrics;
pub mod validation;
pub mod warning_monitor;

// Re-export commonly used csv types (previously in csv.rs module)
pub use csv::{Reader, ReaderBuilder, StringRecord, Writer, WriterBuilder};

// Re-export commonly used service_config types at crate root for convenience
pub use service_config::{
    // Helpers
    helpers,
    parse_four_remote,
    timeouts,
    // Config types
    ApiConfig,
    BaseServiceConfig,
    // Reload
    ChannelReloadResult,
    // Enums
    ComparisonOperator,
    // Validation
    ConfigValidator,
    FourRemote,
    GenericValidator,
    InstanceReloadResult,
    InstanceStatus,
    LogRotationConfig,
    LoggingConfig,
    PointType,
    RedisConfig,
    // Redis keys
    RedisRoutingKeys,
    ReloadResult,
    ReloadableService,
    ResponseStatus,
    RuleReloadResult,
    // Database types
    ServiceConfigRecord,
    SyncMetadataRecord,
    ValidationLevel,
    ValidationResult,
    // Constants
    DEFAULT_API_HOST,
    DEFAULT_COMSRV_URL,
    DEFAULT_MODSRV_URL,
    DEFAULT_REDIS_HOST,
    DEFAULT_REDIS_PORT,
    DEFAULT_REDIS_URL,
    DEFAULT_RULES_URL,
    ENV_COMSRV_URL,
    ENV_MODSRV_URL,
    ENV_RULES_URL,
    LOCALHOST_HOST,
    SERVICE_CONFIG_TABLE,
    SYNC_METADATA_TABLE,
};

// Re-export commonly used API types
pub use api_types::{
    // Response types
    BatchRequest,
    BatchResponse,
    BatchResult,
    ComponentHealth,
    ControlAction,
    ErrorInfo,
    ErrorResponse,
    HealthStatus,
    PaginatedResponse,
    PaginationParams,
    ServiceStatus,
    SortOrder,
    SuccessResponse,
    TimeRange,
    WebSocketMessage,
};

// Re-export AppError when axum feature is enabled
#[cfg(feature = "axum")]
pub use api_types::AppError;

// Re-export PointRole from voltage-model (canonical location)
pub use voltage_model::PointRole;

// Bootstrap modules
pub mod bootstrap_args;
pub mod bootstrap_database;
pub mod bootstrap_system;
pub mod bootstrap_validation;

// Test utilities (for use in test code only)
pub mod test_utils;

// Re-export common dependencies
pub use anyhow;
pub use serde;
pub use serde_json;
pub use tokio;

// Re-export CLI dependencies when cli feature is enabled
#[cfg(feature = "cli")]
pub use clap;

// Re-export clap derive macros separately for proper macro resolution
#[cfg(feature = "cli")]
pub use clap::{Args, Parser, Subcommand, ValueEnum};

#[cfg(feature = "cli")]
pub use reqwest;

// Pre-import common types
pub mod prelude {
    #[cfg(feature = "redis")]
    pub use crate::redis::RedisClient;
}
