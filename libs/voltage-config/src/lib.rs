//! Shared configuration structures for VoltageEMS services
//!
//! This crate provides unified configuration types used across all VoltageEMS services,
//! ensuring type safety and consistency in configuration management.

pub mod common;
pub mod comsrv;
pub mod modsrv;
pub mod products;
pub mod rules;
pub mod serde_defaults;

// New shared type modules
pub mod api;
pub mod error;
pub mod keyspace;
pub mod protocols;
pub mod routing_cache;
pub mod validation;

// Re-export commonly used types
pub use common::{
    parse_four_remote, ApiConfig, BaseServiceConfig, FourRemote, LoggingConfig, RedisConfig,
    DEFAULT_COMSRV_URL, DEFAULT_MODSRV_URL, DEFAULT_RULES_URL, ENV_COMSRV_URL, ENV_MODSRV_URL,
    ENV_RULES_URL,
};

// Re-export hot reload infrastructure
pub use common::{
    ChannelReloadResult, InstanceReloadResult, ReloadResult, ReloadableService, RuleReloadResult,
};
pub use comsrv::{ChannelConfig, ComsrvConfig, ComsrvValidator, SqlInsertablePoint};
pub use modsrv::{
    ActionPoint, CreateInstanceRequest, Instance, MeasurementPoint, ModsrvConfig, ModsrvValidator,
    Product, ProductHierarchy, PropertyTemplate, SqlInsertableProduct,
};
// Note: modsrv::PointType (alias for PointRole) is kept internal to avoid confusion with protocols::PointType
pub use rules::{RulesConfig, RulesValidator};

// Re-export database schema definitions from each module
pub use comsrv::{
    ADJUSTMENT_POINTS_TABLE as COMSRV_ADJUSTMENT_POINTS_TABLE,
    CHANNELS_TABLE as COMSRV_CHANNELS_TABLE, CONTROL_POINTS_TABLE as COMSRV_CONTROL_POINTS_TABLE,
    SERVICE_CONFIG_TABLE as COMSRV_SERVICE_CONFIG_TABLE,
    SIGNAL_POINTS_TABLE as COMSRV_SIGNAL_POINTS_TABLE,
    SYNC_METADATA_TABLE as COMSRV_SYNC_METADATA_TABLE,
    TELEMETRY_POINTS_TABLE as COMSRV_TELEMETRY_POINTS_TABLE,
};

pub use modsrv::{
    ACTION_POINTS_TABLE as MODSRV_ACTION_POINTS_TABLE,
    ACTION_ROUTING_TABLE as MODSRV_ACTION_ROUTING_TABLE, INSTANCES_TABLE as MODSRV_INSTANCES_TABLE,
    MEASUREMENT_POINTS_TABLE as MODSRV_MEASUREMENT_POINTS_TABLE,
    MEASUREMENT_ROUTING_TABLE as MODSRV_MEASUREMENT_ROUTING_TABLE,
    PRODUCTS_TABLE as MODSRV_PRODUCTS_TABLE,
    PROPERTY_TEMPLATES_TABLE as MODSRV_PROPERTY_TEMPLATES_TABLE,
    SERVICE_CONFIG_TABLE as MODSRV_SERVICE_CONFIG_TABLE,
    SYNC_METADATA_TABLE as MODSRV_SYNC_METADATA_TABLE,
};

pub use rules::{
    RULES_TABLE, RULE_HISTORY_TABLE, SERVICE_CONFIG_TABLE as RULES_SERVICE_CONFIG_TABLE,
    SYNC_METADATA_TABLE as RULES_SYNC_METADATA_TABLE,
};

// Re-export validation framework from common
pub use common::{ConfigValidator, GenericValidator, ValidationLevel, ValidationResult};

// Re-export CSV validation
pub use validation::{CsvFields, CsvHeaderValidator};

// Validators are already exported through the modules above

// Configuration error type
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Invalid configuration: {0}")]
    Invalid(String),

    #[error("Missing required field: {0}")]
    MissingField(String),

    #[error("Serialization error: {0}")]
    SerializationError(String),
}

// Re-export protocol types
pub use protocols::{
    ByteOrder, CommunicationMode, ParameterType, PointType, ProtocolType, QualityCode,
    SignalDataType,
};

// Re-export API types
pub use api::{
    BatchRequest, BatchResponse, ErrorInfo, ErrorResponse, HealthStatus, PaginatedResponse,
    PaginationParams, ServiceStatus, SuccessResponse, TimeRange, WebSocketMessage,
};

// Re-export AppError only when axum-support feature is enabled
#[cfg(feature = "axum-support")]
pub use api::AppError;
// Note: ValidationResult is already exported from common module

// Re-export error types
pub use error::{VoltageError, VoltageResult};

// Re-export key space configuration
pub use keyspace::KeySpaceConfig;

// Re-export routing cache
pub use routing_cache::{RoutingCache, RoutingCacheStats};

// Re-export product types
pub use products::{PointDef, ProductDef, ProductType};
