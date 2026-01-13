//! 工业通信协议层 (Protocol Layer)
//!
//! 此模块提供统一的工业协议抽象，支持多种通信协议：
//! - Modbus TCP/RTU
//! - IEC 60870-5-104
//! - OPC UA
//! - CAN/J1939
//! - GPIO
//! - Virtual Channel
//!
//! ## 设计原则
//!
//! - **协议无关**: 统一的数据模型和点位寻址
//! - **双模式支持**: 轮询和事件驱动通信
//! - **零业务耦合**: 纯协议层，不包含 SCADA 概念

pub mod adapters;
pub mod codec;
pub mod config;
pub mod core;
pub mod gateway;

/// Prelude module for convenient imports
pub mod prelude {
    pub use crate::protocols::core::{
        data::*,
        error::{GatewayError, Result},
        logging::*,
        point::*,
        quality::*,
        traits::*,
    };
}

// Re-export core types at module root for convenience
pub use self::core::data::{DataBatch, DataPoint, Value};
pub use self::core::error::{GatewayError, Result};
pub use self::core::logging::{
    ChannelLogConfig, ChannelLogEvent, ChannelLogHandler, LogContext, LogEventType, LogVerbosity,
    LoggableProtocol, PacketDirection, PacketMetadata,
};
pub use self::core::metadata::{
    get_protocol_registry, DriverMetadata, HasMetadata, ParameterMetadata, ParameterType,
    ProtocolMetadata, ProtocolRegistry,
};
pub use self::core::quality::Quality;
pub use self::core::traits::{
    CommunicationMode, ConnectionState, Protocol, ProtocolCapabilities, ProtocolClient,
};

// Re-export config types
pub use self::config::ChannelBuildResult;

// Re-export gateway types
pub use self::gateway::{
    parse_address, ChannelConfig, ChannelMode, ChannelModeConfig, ChannelRuntime, ConfigError,
    GatewayConfig, GatewayGlobalConfig, PointDef,
};
