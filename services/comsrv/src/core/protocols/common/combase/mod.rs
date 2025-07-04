//! Communication Base Module
//!
//! This module provides the foundational traits and types for implementing
//! communication protocols in the Voltage EMS Communication Service.

// Core data types and structures
pub mod data_types;
pub mod telemetry;
pub mod traits;
pub mod default_protocol;
pub mod stats;
pub mod defaults;

// Specialized functionality
pub mod command_manager;
pub mod point_manager;
pub mod optimized_point_manager;
pub mod redis_batch_sync;
pub mod protocol_factory;
pub mod simplified_mapping;
pub mod smart_mapping;

// Implementation modules
pub mod impl_base;
pub mod transport_bridge;
pub mod enhanced_transport_bridge;
pub mod monitoring;

// Re-export commonly used types
pub use data_types::*;
pub use telemetry::*;
pub use traits::{ComBase, FourTelemetryOperations, ConnectionManager, ConfigValidator, ProtocolPacketParser};
pub use default_protocol::{DefaultProtocol, PacketParseResult};
pub use command_manager::UniversalCommandManager;
pub use point_manager::{UniversalPointManager, UniversalPointConfig, PointManagerStats};
pub use protocol_factory::ProtocolFactory;
pub use impl_base::ComBaseImpl;
pub use transport_bridge::{UniversalTransportBridge, ProtocolBridgeConfig, BridgeStats};
pub use enhanced_transport_bridge::{EnhancedTransportBridge, ConnectionPoolConfig, RetryConfig, RequestPriority};
pub use monitoring::{BasicMonitoring, HealthChecker, HealthLevel, PerformanceMetrics, AlertManager, AlertRule};