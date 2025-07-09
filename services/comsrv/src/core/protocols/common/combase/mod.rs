//! Communication Base Module
//!
//! This module provides the foundational traits and types for implementing
//! communication protocols in the Voltage EMS Communication Service.

// Core data types and structures
pub mod data_types;
pub mod default_protocol;
pub mod defaults;
pub mod stats;
pub mod telemetry;
pub mod traits;

// Specialized functionality
pub mod command_manager;
pub mod optimized_point_manager;
pub mod point_manager;
pub mod protocol_factory;
pub mod redis_batch_sync;
pub mod simplified_mapping;
pub mod smart_mapping;

// Implementation modules
pub mod enhanced_transport_bridge;
pub mod impl_base;
pub mod monitoring;
pub mod transport_bridge;

// Re-export commonly used types
pub use command_manager::UniversalCommandManager;
pub use data_types::*;
pub use default_protocol::{DefaultProtocol, PacketParseResult};
pub use enhanced_transport_bridge::{
    ConnectionPoolConfig, EnhancedTransportBridge, RequestPriority, RetryConfig,
};
pub use impl_base::ComBaseImpl;
pub use monitoring::{
    AlertManager, AlertRule, BasicMonitoring, HealthChecker, HealthLevel, PerformanceMetrics,
};
pub use point_manager::{PointManagerStats, UniversalPointConfig, UniversalPointManager};
pub use protocol_factory::ProtocolFactory;
pub use telemetry::*;
pub use traits::{
    ComBase, ConfigValidator, ConnectionManager, FourTelemetryOperations, ProtocolPacketParser,
};
pub use transport_bridge::{BridgeStats, ProtocolBridgeConfig, UniversalTransportBridge};
