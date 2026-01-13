//! Channel logging infrastructure for the Industrial Gateway.
//!
//! This module provides a unified, protocol-agnostic logging system for all channel operations.
//!
//! # Design Philosophy
//!
//! - **Unified abstraction**: One `ChannelLogHandler` trait for all protocols
//! - **Protocol-agnostic**: External applications only need one implementation
//! - **Configurable**: Fine-grained control over what events to log
//! - **Raw packet support**: Capture protocol-level frames with metadata
//!
//! # Example
//!
//! ```ignore
//! use crate::protocols::core::logging::{ChannelLogConfig, ChannelLogHandler, ChannelLogEvent};
//!
//! struct MyLogHandler;
//!
//! #[async_trait]
//! impl ChannelLogHandler for MyLogHandler {
//!     async fn on_log(&self, channel_id: u32, event: ChannelLogEvent) {
//!         println!("[CH:{}] {:?}", channel_id, event);
//!     }
//! }
//!
//! let mut channel = ModbusChannel::new(config, 1);
//! channel.set_log_handler(Arc::new(MyLogHandler));
//! channel.set_log_config(ChannelLogConfig::all());
//! ```

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::SystemTime;

use crate::protocols::core::{
    AdjustmentCommand, ConnectionState, ControlCommand, ReadRequest, ReadResponse, WriteResult,
};

// ============================================================================
// Packet Direction and Metadata
// ============================================================================

/// Direction of a raw packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PacketDirection {
    /// Packet sent to device/server.
    Send,
    /// Packet received from device/server.
    Receive,
}

impl std::fmt::Display for PacketDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Send => write!(f, ">>>"),
            Self::Receive => write!(f, "<<<"),
        }
    }
}

/// Modbus transport type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModbusTransportType {
    /// Modbus TCP.
    Tcp,
    /// Modbus RTU (serial).
    Rtu,
    /// Modbus ASCII (serial).
    Ascii,
}

/// Protocol-specific packet metadata.
///
/// This enum dispatches different metadata structures for each protocol,
/// allowing type-safe handling while maintaining a unified interface.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum PacketMetadata {
    /// Modbus TCP/RTU/ASCII packet metadata.
    Modbus {
        /// Transport type (TCP, RTU, ASCII).
        transport: ModbusTransportType,
        /// Slave/Unit ID.
        slave_id: u8,
        /// Function code.
        function_code: u8,
    },

    /// IEC 60870-5-104 packet metadata.
    Iec104 {
        /// ASDU type identifier.
        asdu_type: u8,
        /// Cause of transmission.
        cause_of_tx: u8,
        /// Common address.
        common_addr: u16,
    },

    /// OPC UA packet metadata.
    OpcUa {
        /// Message type (e.g., "OpenSecureChannel", "Publish").
        message_type: String,
        /// Request ID.
        request_id: u32,
    },

    /// J1939 CAN bus packet metadata.
    J1939 {
        /// Parameter Group Number.
        pgn: u32,
        /// Source address.
        source: u8,
        /// Destination address.
        destination: u8,
    },

    /// GPIO (no raw packets, included for completeness).
    Gpio,

    /// Virtual channel (no raw packets).
    Virtual,

    /// Other/unknown protocol.
    Other {
        /// Protocol name.
        protocol: String,
    },
}

impl PacketMetadata {
    /// Create Modbus TCP metadata.
    pub fn modbus_tcp(slave_id: u8, function_code: u8) -> Self {
        Self::Modbus {
            transport: ModbusTransportType::Tcp,
            slave_id,
            function_code,
        }
    }

    /// Create Modbus RTU metadata.
    pub fn modbus_rtu(slave_id: u8, function_code: u8) -> Self {
        Self::Modbus {
            transport: ModbusTransportType::Rtu,
            slave_id,
            function_code,
        }
    }

    /// Create IEC 104 metadata.
    pub fn iec104(asdu_type: u8, cause_of_tx: u8, common_addr: u16) -> Self {
        Self::Iec104 {
            asdu_type,
            cause_of_tx,
            common_addr,
        }
    }

    /// Create J1939 metadata.
    pub fn j1939(pgn: u32, source: u8, destination: u8) -> Self {
        Self::J1939 {
            pgn,
            source,
            destination,
        }
    }

    /// Get protocol name as string.
    pub fn protocol_name(&self) -> &str {
        match self {
            Self::Modbus { transport, .. } => match transport {
                ModbusTransportType::Tcp => "modbus-tcp",
                ModbusTransportType::Rtu => "modbus-rtu",
                ModbusTransportType::Ascii => "modbus-ascii",
            },
            Self::Iec104 { .. } => "iec104",
            Self::OpcUa { .. } => "opcua",
            Self::J1939 { .. } => "j1939",
            Self::Gpio => "gpio",
            Self::Virtual => "virtual",
            Self::Other { protocol } => protocol,
        }
    }
}

// ============================================================================
// Error Context
// ============================================================================

/// Context in which an error occurred.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorContext {
    /// Error during connection.
    Connection,
    /// Error during read operation.
    Read,
    /// Error during control write.
    WriteControl,
    /// Error during adjustment write.
    WriteAdjustment,
    /// Error during polling.
    Polling,
    /// Protocol-level error.
    Protocol,
    /// Unknown error context.
    Unknown,
}

impl std::fmt::Display for ErrorContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Connection => write!(f, "connection"),
            Self::Read => write!(f, "read"),
            Self::WriteControl => write!(f, "write_control"),
            Self::WriteAdjustment => write!(f, "write_adjustment"),
            Self::Polling => write!(f, "polling"),
            Self::Protocol => write!(f, "protocol"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

// ============================================================================
// Channel Log Event
// ============================================================================

/// Channel log event.
///
/// This enum represents all possible events that can be logged from a channel.
/// External applications implement `ChannelLogHandler` to receive these events.
#[derive(Debug, Clone)]
pub enum ChannelLogEvent {
    /// Channel connected successfully.
    Connected {
        /// Event timestamp.
        timestamp: SystemTime,
        /// Connection endpoint (e.g., "192.168.1.100:502").
        endpoint: String,
        /// Connection duration in milliseconds.
        duration_ms: u64,
    },

    /// Channel disconnected.
    Disconnected {
        /// Event timestamp.
        timestamp: SystemTime,
        /// Disconnect reason (None = intentional disconnect).
        reason: Option<String>,
    },

    /// Read operation completed.
    ReadOperation {
        /// Event timestamp.
        timestamp: SystemTime,
        /// Read request.
        request: ReadRequest,
        /// Read result (Ok = success, Err = error message).
        result: Result<ReadResponse, String>,
        /// Operation duration in milliseconds.
        duration_ms: u64,
    },

    /// Poll cycle completed.
    PollCycleCompleted {
        /// Event timestamp.
        timestamp: SystemTime,
        /// Number of points in the batch (avoids cloning entire DataBatch for logging).
        points_count: usize,
        /// Cycle duration in milliseconds.
        duration_ms: u64,
        /// Number of successfully read points.
        success_count: usize,
        /// Number of failed points.
        failed_count: usize,
    },

    /// Control command write completed.
    ControlWrite {
        /// Event timestamp.
        timestamp: SystemTime,
        /// Control commands sent.
        commands: Vec<ControlCommand>,
        /// Write result.
        result: Result<WriteResult, String>,
        /// Operation duration in milliseconds.
        duration_ms: u64,
    },

    /// Adjustment command write completed.
    AdjustmentWrite {
        /// Event timestamp.
        timestamp: SystemTime,
        /// Adjustment commands sent.
        commands: Vec<AdjustmentCommand>,
        /// Write result.
        result: Result<WriteResult, String>,
        /// Operation duration in milliseconds.
        duration_ms: u64,
    },

    /// Error occurred.
    Error {
        /// Event timestamp.
        timestamp: SystemTime,
        /// Error message.
        error: String,
        /// Error context.
        context: ErrorContext,
    },

    /// Reconnect attempt.
    ReconnectAttempt {
        /// Event timestamp.
        timestamp: SystemTime,
        /// Current attempt number.
        attempt: u32,
        /// Maximum attempts (None = unlimited).
        max_attempts: Option<u32>,
        /// Time until next retry in milliseconds.
        next_retry_ms: Option<u64>,
    },

    /// Reconnect succeeded.
    ReconnectSuccess {
        /// Event timestamp.
        timestamp: SystemTime,
        /// Total attempts made.
        total_attempts: u32,
        /// Total duration in milliseconds.
        total_duration_ms: u64,
    },

    /// Connection state changed.
    StateChanged {
        /// Event timestamp.
        timestamp: SystemTime,
        /// Previous state.
        old_state: ConnectionState,
        /// New state.
        new_state: ConnectionState,
    },

    /// Raw packet sent/received.
    RawPacket {
        /// Event timestamp.
        timestamp: SystemTime,
        /// Packet direction.
        direction: PacketDirection,
        /// Raw packet data.
        data: Vec<u8>,
        /// Protocol-specific metadata.
        metadata: PacketMetadata,
    },
}

impl ChannelLogEvent {
    /// Get the event timestamp.
    pub fn timestamp(&self) -> SystemTime {
        match self {
            Self::Connected { timestamp, .. } => *timestamp,
            Self::Disconnected { timestamp, .. } => *timestamp,
            Self::ReadOperation { timestamp, .. } => *timestamp,
            Self::PollCycleCompleted { timestamp, .. } => *timestamp,
            Self::ControlWrite { timestamp, .. } => *timestamp,
            Self::AdjustmentWrite { timestamp, .. } => *timestamp,
            Self::Error { timestamp, .. } => *timestamp,
            Self::ReconnectAttempt { timestamp, .. } => *timestamp,
            Self::ReconnectSuccess { timestamp, .. } => *timestamp,
            Self::StateChanged { timestamp, .. } => *timestamp,
            Self::RawPacket { timestamp, .. } => *timestamp,
        }
    }

    /// Get the event type as a string.
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::Connected { .. } => "connected",
            Self::Disconnected { .. } => "disconnected",
            Self::ReadOperation { .. } => "read_operation",
            Self::PollCycleCompleted { .. } => "poll_cycle",
            Self::ControlWrite { .. } => "control_write",
            Self::AdjustmentWrite { .. } => "adjustment_write",
            Self::Error { .. } => "error",
            Self::ReconnectAttempt { .. } => "reconnect_attempt",
            Self::ReconnectSuccess { .. } => "reconnect_success",
            Self::StateChanged { .. } => "state_changed",
            Self::RawPacket { .. } => "raw_packet",
        }
    }
}

// ============================================================================
// Log Event Type (for filtering)
// ============================================================================

/// Log event type for filtering configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogEventType {
    /// Connection events.
    Connected,
    /// Disconnection events.
    Disconnected,
    /// Read operation events.
    ReadOperation,
    /// Poll cycle events.
    PollCycle,
    /// Control write events.
    ControlWrite,
    /// Adjustment write events.
    AdjustmentWrite,
    /// Error events.
    Error,
    /// Reconnect attempt events.
    ReconnectAttempt,
    /// Reconnect success events.
    ReconnectSuccess,
    /// State change events.
    StateChanged,
    /// Raw packet events.
    RawPacket,
}

impl LogEventType {
    /// Get all event types.
    pub fn all() -> HashSet<LogEventType> {
        use LogEventType::*;
        [
            Connected,
            Disconnected,
            ReadOperation,
            PollCycle,
            ControlWrite,
            AdjustmentWrite,
            Error,
            ReconnectAttempt,
            ReconnectSuccess,
            StateChanged,
            RawPacket,
        ]
        .into_iter()
        .collect()
    }

    /// Get default event types (excludes high-frequency events).
    pub fn default_set() -> HashSet<LogEventType> {
        use LogEventType::*;
        [
            Connected,
            Disconnected,
            ControlWrite,
            AdjustmentWrite,
            Error,
            ReconnectAttempt,
            ReconnectSuccess,
            StateChanged,
        ]
        .into_iter()
        .collect()
    }

    /// Get only error and connection events.
    pub fn errors_and_connections() -> HashSet<LogEventType> {
        use LogEventType::*;
        [
            Connected,
            Disconnected,
            Error,
            ReconnectAttempt,
            ReconnectSuccess,
            StateChanged,
        ]
        .into_iter()
        .collect()
    }
}

// ============================================================================
// Log Verbosity
// ============================================================================

/// Log verbosity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogVerbosity {
    /// Minimal logging: only event occurrence.
    Minimal,
    /// Standard logging: event with basic info.
    #[default]
    Standard,
    /// Verbose logging: full data included.
    Verbose,
}

// ============================================================================
// Channel Log Config
// ============================================================================

/// Channel logging configuration.
///
/// Controls which events are logged and with what level of detail.
#[derive(Debug, Clone)]
pub struct ChannelLogConfig {
    /// Enabled event types.
    enabled_events: HashSet<LogEventType>,
    /// Verbosity level.
    verbosity: LogVerbosity,
    /// Log successful read operations.
    log_successful_reads: bool,
    /// Log successful write operations.
    log_successful_writes: bool,
    /// Poll cycle sample rate (1 = every cycle, 10 = every 10th).
    poll_cycle_sample_rate: u32,
    /// Enable raw packet logging.
    log_raw_packets: bool,
    /// Maximum packet size to log (0 = no limit).
    max_packet_size: usize,
}

impl Default for ChannelLogConfig {
    fn default() -> Self {
        Self {
            enabled_events: LogEventType::default_set(),
            verbosity: LogVerbosity::Standard,
            log_successful_reads: false,
            log_successful_writes: true,
            poll_cycle_sample_rate: 1,
            log_raw_packets: false,
            max_packet_size: 0,
        }
    }
}

impl ChannelLogConfig {
    /// Create a new default configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a configuration that logs everything.
    pub fn all() -> Self {
        Self {
            enabled_events: LogEventType::all(),
            verbosity: LogVerbosity::Verbose,
            log_successful_reads: true,
            log_successful_writes: true,
            poll_cycle_sample_rate: 1,
            log_raw_packets: true,
            max_packet_size: 0,
        }
    }

    /// Create a configuration that logs only errors and connections.
    pub fn errors_only() -> Self {
        Self {
            enabled_events: LogEventType::errors_and_connections(),
            verbosity: LogVerbosity::Standard,
            log_successful_reads: false,
            log_successful_writes: false,
            poll_cycle_sample_rate: 0,
            log_raw_packets: false,
            max_packet_size: 0,
        }
    }

    /// Create a disabled configuration.
    pub fn disabled() -> Self {
        Self {
            enabled_events: HashSet::new(),
            verbosity: LogVerbosity::Standard,
            log_successful_reads: false,
            log_successful_writes: false,
            poll_cycle_sample_rate: 0,
            log_raw_packets: false,
            max_packet_size: 0,
        }
    }

    /// Enable a specific event type.
    #[must_use]
    pub fn enable_event(mut self, event_type: LogEventType) -> Self {
        self.enabled_events.insert(event_type);
        self
    }

    /// Disable a specific event type.
    #[must_use]
    pub fn disable_event(mut self, event_type: LogEventType) -> Self {
        self.enabled_events.remove(&event_type);
        self
    }

    /// Set enabled event types.
    #[must_use]
    pub fn with_events(mut self, events: HashSet<LogEventType>) -> Self {
        self.enabled_events = events;
        self
    }

    /// Set verbosity level.
    #[must_use]
    pub fn with_verbosity(mut self, verbosity: LogVerbosity) -> Self {
        self.verbosity = verbosity;
        self
    }

    /// Set whether to log successful reads.
    #[must_use]
    pub fn with_successful_reads(mut self, enable: bool) -> Self {
        self.log_successful_reads = enable;
        self
    }

    /// Set whether to log successful writes.
    #[must_use]
    pub fn with_successful_writes(mut self, enable: bool) -> Self {
        self.log_successful_writes = enable;
        self
    }

    /// Set poll cycle sample rate.
    #[must_use]
    pub fn with_poll_sample_rate(mut self, rate: u32) -> Self {
        self.poll_cycle_sample_rate = rate;
        self
    }

    /// Enable/disable raw packet logging.
    #[must_use]
    pub fn with_raw_packets(mut self, enable: bool) -> Self {
        self.log_raw_packets = enable;
        if enable && !self.enabled_events.contains(&LogEventType::RawPacket) {
            self.enabled_events.insert(LogEventType::RawPacket);
        }
        self
    }

    /// Set maximum packet size to log.
    #[must_use]
    pub fn with_max_packet_size(mut self, size: usize) -> Self {
        self.max_packet_size = size;
        self
    }

    /// Check if an event type is enabled.
    pub fn is_enabled(&self, event_type: LogEventType) -> bool {
        self.enabled_events.contains(&event_type)
    }

    /// Get verbosity level.
    pub fn verbosity(&self) -> LogVerbosity {
        self.verbosity
    }

    /// Check if raw packets should be logged.
    pub fn should_log_raw_packets(&self) -> bool {
        self.log_raw_packets && self.enabled_events.contains(&LogEventType::RawPacket)
    }

    /// Check if an event should be logged based on configuration.
    pub fn should_log(&self, event: &ChannelLogEvent) -> bool {
        let event_type = match event {
            ChannelLogEvent::Connected { .. } => LogEventType::Connected,
            ChannelLogEvent::Disconnected { .. } => LogEventType::Disconnected,
            ChannelLogEvent::ReadOperation { result, .. } => {
                if !self.enabled_events.contains(&LogEventType::ReadOperation) {
                    return false;
                }
                if result.is_ok() && !self.log_successful_reads {
                    return false;
                }
                LogEventType::ReadOperation
            },
            ChannelLogEvent::PollCycleCompleted { .. } => LogEventType::PollCycle,
            ChannelLogEvent::ControlWrite { result, .. } => {
                if !self.enabled_events.contains(&LogEventType::ControlWrite) {
                    return false;
                }
                if result.is_ok() && !self.log_successful_writes {
                    return false;
                }
                LogEventType::ControlWrite
            },
            ChannelLogEvent::AdjustmentWrite { result, .. } => {
                if !self.enabled_events.contains(&LogEventType::AdjustmentWrite) {
                    return false;
                }
                if result.is_ok() && !self.log_successful_writes {
                    return false;
                }
                LogEventType::AdjustmentWrite
            },
            ChannelLogEvent::Error { .. } => LogEventType::Error,
            ChannelLogEvent::ReconnectAttempt { .. } => LogEventType::ReconnectAttempt,
            ChannelLogEvent::ReconnectSuccess { .. } => LogEventType::ReconnectSuccess,
            ChannelLogEvent::StateChanged { .. } => LogEventType::StateChanged,
            ChannelLogEvent::RawPacket { .. } => {
                if !self.log_raw_packets {
                    return false;
                }
                LogEventType::RawPacket
            },
        };

        self.enabled_events.contains(&event_type)
    }
}

// ============================================================================
// Channel Log Handler Trait
// ============================================================================

/// Channel log handler trait.
///
/// Implement this trait to receive log events from channels.
/// The handler is protocol-agnostic - the same implementation
/// works with Modbus, IEC 104, OPC UA, and all other protocols.
#[async_trait]
pub trait ChannelLogHandler: Send + Sync {
    /// Handle a log event.
    ///
    /// # Arguments
    ///
    /// * `channel_id` - The channel identifier
    /// * `event` - The log event
    async fn on_log(&self, channel_id: u32, event: ChannelLogEvent);
}

// ============================================================================
// Built-in Log Handlers
// ============================================================================

/// No-op log handler that discards all events.
pub struct NoopLogHandler;

#[async_trait]
impl ChannelLogHandler for NoopLogHandler {
    async fn on_log(&self, _channel_id: u32, _event: ChannelLogEvent) {
        // Intentionally empty
    }
}

/// Print log handler that outputs to stdout.
pub struct PrintLogHandler {
    /// Optional prefix for log messages.
    prefix: String,
}

impl PrintLogHandler {
    /// Create a new print handler.
    pub fn new() -> Self {
        Self {
            prefix: String::new(),
        }
    }

    /// Create a print handler with a prefix.
    pub fn with_prefix(prefix: impl Into<String>) -> Self {
        Self {
            prefix: prefix.into(),
        }
    }

    /// Format bytes as hex string.
    ///
    /// Optimized to use a single allocation with `String::with_capacity`.
    fn format_hex(data: &[u8], max_len: usize) -> String {
        use std::fmt::Write;

        let truncated = if max_len > 0 && data.len() > max_len {
            &data[..max_len]
        } else {
            data
        };

        // Pre-allocate: "XX " per byte (3 chars), plus suffix if truncated
        let is_truncated = max_len > 0 && data.len() > max_len;
        let capacity = truncated.len() * 3 + if is_truncated { 30 } else { 0 };
        let mut hex = String::with_capacity(capacity);

        for (i, b) in truncated.iter().enumerate() {
            if i > 0 {
                hex.push(' ');
            }
            // write! to String is infallible
            let _ = write!(&mut hex, "{:02X}", b);
        }

        if is_truncated {
            let _ = write!(&mut hex, " ... ({} bytes total)", data.len());
        }

        hex
    }
}

impl Default for PrintLogHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ChannelLogHandler for PrintLogHandler {
    async fn on_log(&self, channel_id: u32, event: ChannelLogEvent) {
        let event_type = event.event_type();

        match event {
            ChannelLogEvent::Connected {
                endpoint,
                duration_ms,
                ..
            } => {
                println!(
                    "{}[CH:{}] {} -> {} ({}ms)",
                    self.prefix, channel_id, event_type, endpoint, duration_ms
                );
            },
            ChannelLogEvent::Disconnected { reason, .. } => {
                let reason_str = reason.as_deref().unwrap_or("intentional");
                println!(
                    "{}[CH:{}] {} reason={}",
                    self.prefix, channel_id, event_type, reason_str
                );
            },
            ChannelLogEvent::Error { error, context, .. } => {
                println!(
                    "{}[CH:{}] {} [{}] {}",
                    self.prefix, channel_id, event_type, context, error
                );
            },
            ChannelLogEvent::RawPacket {
                direction,
                data,
                metadata,
                ..
            } => {
                let hex = Self::format_hex(&data, 64);
                println!(
                    "{}[CH:{}] {} {} {} {}",
                    self.prefix,
                    channel_id,
                    metadata.protocol_name(),
                    direction,
                    data.len(),
                    hex
                );
            },
            ChannelLogEvent::StateChanged {
                old_state,
                new_state,
                ..
            } => {
                println!(
                    "{}[CH:{}] {} {} -> {}",
                    self.prefix, channel_id, event_type, old_state, new_state
                );
            },
            ChannelLogEvent::PollCycleCompleted {
                points_count,
                duration_ms,
                success_count,
                failed_count,
                ..
            } => {
                println!(
                    "{}[CH:{}] {} points={} ok={} fail={} ({}ms)",
                    self.prefix,
                    channel_id,
                    event_type,
                    points_count,
                    success_count,
                    failed_count,
                    duration_ms
                );
            },
            _ => {
                println!("{}[CH:{}] {}", self.prefix, channel_id, event_type);
            },
        }
    }
}

/// Composite log handler that forwards events to multiple handlers.
pub struct CompositeLogHandler {
    handlers: Vec<Arc<dyn ChannelLogHandler>>,
}

impl CompositeLogHandler {
    /// Create a new composite handler.
    pub fn new() -> Self {
        Self {
            handlers: Vec::new(),
        }
    }

    /// Add a handler (builder pattern).
    #[must_use]
    pub fn with_handler(mut self, handler: Arc<dyn ChannelLogHandler>) -> Self {
        self.handlers.push(handler);
        self
    }

    /// Add a handler (mutable).
    pub fn add_handler(&mut self, handler: Arc<dyn ChannelLogHandler>) {
        self.handlers.push(handler);
    }
}

impl Default for CompositeLogHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ChannelLogHandler for CompositeLogHandler {
    async fn on_log(&self, channel_id: u32, event: ChannelLogEvent) {
        let len = self.handlers.len();
        if len == 0 {
            return;
        }

        // Optimization: clone for first N-1 handlers, move for the last one
        for (i, handler) in self.handlers.iter().enumerate() {
            if i == len - 1 {
                // Last handler: move the event instead of cloning
                handler.on_log(channel_id, event).await;
                return;
            } else {
                handler.on_log(channel_id, event.clone()).await;
            }
        }
    }
}

/// Tracing log handler that integrates with the `tracing` crate.
pub struct TracingLogHandler;

#[async_trait]
impl ChannelLogHandler for TracingLogHandler {
    async fn on_log(&self, channel_id: u32, event: ChannelLogEvent) {
        use tracing::{debug, error, info, trace, warn};

        match &event {
            ChannelLogEvent::Connected {
                endpoint,
                duration_ms,
                ..
            } => {
                info!(
                    channel_id = channel_id,
                    endpoint = %endpoint,
                    duration_ms = duration_ms,
                    "Channel connected"
                );
            },
            ChannelLogEvent::Disconnected { reason, .. } => {
                if let Some(reason) = reason {
                    warn!(
                        channel_id = channel_id,
                        reason = %reason,
                        "Channel disconnected"
                    );
                } else {
                    info!(channel_id = channel_id, "Channel disconnected");
                }
            },
            ChannelLogEvent::Error { error, context, .. } => {
                error!(
                    channel_id = channel_id,
                    error = %error,
                    context = %context,
                    "Channel error"
                );
            },
            ChannelLogEvent::ControlWrite {
                commands,
                result,
                duration_ms,
                ..
            } => match result {
                Ok(write_result) => {
                    debug!(
                        channel_id = channel_id,
                        commands_count = commands.len(),
                        success_count = write_result.success_count,
                        duration_ms = duration_ms,
                        "Control write completed"
                    );
                },
                Err(e) => {
                    warn!(
                        channel_id = channel_id,
                        commands_count = commands.len(),
                        error = %e,
                        "Control write failed"
                    );
                },
            },
            ChannelLogEvent::PollCycleCompleted {
                points_count,
                duration_ms,
                success_count,
                failed_count,
                ..
            } => {
                trace!(
                    channel_id = channel_id,
                    points_count = points_count,
                    success_count = success_count,
                    failed_count = failed_count,
                    duration_ms = duration_ms,
                    "Poll cycle completed"
                );
            },
            ChannelLogEvent::RawPacket {
                direction,
                data,
                metadata,
                ..
            } => {
                let hex = data
                    .iter()
                    .take(32)
                    .fold(String::with_capacity(64), |mut s, b| {
                        use std::fmt::Write;
                        let _ = write!(s, "{:02X}", b);
                        s
                    });
                trace!(
                    channel_id = channel_id,
                    protocol = metadata.protocol_name(),
                    direction = ?direction,
                    size = data.len(),
                    data = %hex,
                    "Raw packet"
                );
            },
            ChannelLogEvent::StateChanged {
                old_state,
                new_state,
                ..
            } => {
                info!(
                    channel_id = channel_id,
                    old_state = %old_state,
                    new_state = %new_state,
                    "Connection state changed"
                );
            },
            _ => {
                debug!(channel_id = channel_id, event = ?event, "Channel event");
            },
        }
    }
}

// ============================================================================
// Log Context
// ============================================================================

/// Logging context for use within protocol implementations.
///
/// This struct encapsulates the logging configuration and handler,
/// providing convenient methods for logging various events.
pub struct LogContext {
    /// Channel ID.
    channel_id: u32,
    /// Log handler.
    handler: Option<Arc<dyn ChannelLogHandler>>,
    /// Log configuration.
    config: ChannelLogConfig,
    /// Poll cycle counter for sampling.
    poll_counter: AtomicU64,
}

impl LogContext {
    /// Create a new log context.
    pub fn new(channel_id: u32) -> Self {
        Self {
            channel_id,
            handler: None,
            config: ChannelLogConfig::default(),
            poll_counter: AtomicU64::new(0),
        }
    }

    /// Create with a handler.
    #[must_use]
    pub fn with_handler(mut self, handler: Arc<dyn ChannelLogHandler>) -> Self {
        self.handler = Some(handler);
        self
    }

    /// Create with a configuration.
    #[must_use]
    pub fn with_config(mut self, config: ChannelLogConfig) -> Self {
        self.config = config;
        self
    }

    /// Set the log handler.
    pub fn set_handler(&mut self, handler: Arc<dyn ChannelLogHandler>) {
        self.handler = Some(handler);
    }

    /// Set the log configuration.
    pub fn set_config(&mut self, config: ChannelLogConfig) {
        self.config = config;
    }

    /// Get the current configuration.
    pub fn config(&self) -> &ChannelLogConfig {
        &self.config
    }

    /// Get the channel ID.
    pub fn channel_id(&self) -> u32 {
        self.channel_id
    }

    /// Log an event (async).
    pub async fn log(&self, event: ChannelLogEvent) {
        if let Some(handler) = &self.handler {
            if self.config.should_log(&event) {
                handler.on_log(self.channel_id, event).await;
            }
        }
    }

    /// Log an event (spawns async task).
    pub fn log_spawn(&self, event: ChannelLogEvent) {
        if let Some(handler) = &self.handler {
            if self.config.should_log(&event) {
                let handler = handler.clone();
                let channel_id = self.channel_id;
                tokio::spawn(async move {
                    handler.on_log(channel_id, event).await;
                });
            }
        }
    }

    /// Check if poll cycle should be logged (based on sample rate).
    pub fn should_log_poll_cycle(&self) -> bool {
        if !self.config.is_enabled(LogEventType::PollCycle) {
            return false;
        }
        let rate = self.config.poll_cycle_sample_rate;
        if rate == 0 {
            return false;
        }
        if rate == 1 {
            return true;
        }
        let count = self.poll_counter.fetch_add(1, Ordering::Relaxed);
        count.is_multiple_of(rate as u64)
    }

    // === Convenience methods ===

    /// Log a connected event.
    pub async fn log_connected(&self, endpoint: impl Into<String>, duration_ms: u64) {
        self.log(ChannelLogEvent::Connected {
            timestamp: SystemTime::now(),
            endpoint: endpoint.into(),
            duration_ms,
        })
        .await;
    }

    /// Log a disconnected event.
    pub async fn log_disconnected(&self, reason: Option<String>) {
        self.log(ChannelLogEvent::Disconnected {
            timestamp: SystemTime::now(),
            reason,
        })
        .await;
    }

    /// Log an error event.
    pub async fn log_error(&self, error: impl Into<String>, context: ErrorContext) {
        self.log(ChannelLogEvent::Error {
            timestamp: SystemTime::now(),
            error: error.into(),
            context,
        })
        .await;
    }

    /// Log a state change event.
    pub async fn log_state_changed(&self, old_state: ConnectionState, new_state: ConnectionState) {
        self.log(ChannelLogEvent::StateChanged {
            timestamp: SystemTime::now(),
            old_state,
            new_state,
        })
        .await;
    }

    /// Log a control write event.
    ///
    /// Takes `&[ControlCommand]` to avoid forcing callers to allocate.
    /// The slice is only cloned if the event will actually be logged.
    pub async fn log_control_write(
        &self,
        commands: &[ControlCommand],
        result: Result<WriteResult, String>,
        duration_ms: u64,
    ) {
        // Fast path: skip allocation if logging is disabled
        if self.handler.is_none() || !self.config.is_enabled(LogEventType::ControlWrite) {
            return;
        }

        self.log(ChannelLogEvent::ControlWrite {
            timestamp: SystemTime::now(),
            commands: commands.to_vec(),
            result,
            duration_ms,
        })
        .await;
    }

    /// Log an adjustment write event.
    ///
    /// Takes `&[AdjustmentCommand]` to avoid forcing callers to allocate.
    /// The slice is only cloned if the event will actually be logged.
    pub async fn log_adjustment_write(
        &self,
        commands: &[AdjustmentCommand],
        result: Result<WriteResult, String>,
        duration_ms: u64,
    ) {
        // Fast path: skip allocation if logging is disabled
        if self.handler.is_none() || !self.config.is_enabled(LogEventType::AdjustmentWrite) {
            return;
        }

        self.log(ChannelLogEvent::AdjustmentWrite {
            timestamp: SystemTime::now(),
            commands: commands.to_vec(),
            result,
            duration_ms,
        })
        .await;
    }

    /// Log a poll cycle event.
    ///
    /// Takes `points_count` instead of the full `DataBatch` to avoid cloning.
    pub async fn log_poll_cycle(
        &self,
        points_count: usize,
        duration_ms: u64,
        success_count: usize,
        failed_count: usize,
    ) {
        if self.should_log_poll_cycle() {
            self.log(ChannelLogEvent::PollCycleCompleted {
                timestamp: SystemTime::now(),
                points_count,
                duration_ms,
                success_count,
                failed_count,
            })
            .await;
        }
    }

    /// Log a reconnect attempt event.
    pub async fn log_reconnect_attempt(
        &self,
        attempt: u32,
        max_attempts: Option<u32>,
        next_retry_ms: Option<u64>,
    ) {
        self.log(ChannelLogEvent::ReconnectAttempt {
            timestamp: SystemTime::now(),
            attempt,
            max_attempts,
            next_retry_ms,
        })
        .await;
    }

    /// Log a reconnect success event.
    pub async fn log_reconnect_success(&self, total_attempts: u32, total_duration_ms: u64) {
        self.log(ChannelLogEvent::ReconnectSuccess {
            timestamp: SystemTime::now(),
            total_attempts,
            total_duration_ms,
        })
        .await;
    }

    /// Log a raw packet event.
    pub async fn log_raw_packet(
        &self,
        direction: PacketDirection,
        mut data: Vec<u8>,
        metadata: PacketMetadata,
    ) {
        if !self.config.should_log_raw_packets() {
            return;
        }

        // Apply max packet size truncation (in-place, no reallocation)
        if self.config.max_packet_size > 0 && data.len() > self.config.max_packet_size {
            data.truncate(self.config.max_packet_size);
        }

        self.log(ChannelLogEvent::RawPacket {
            timestamp: SystemTime::now(),
            direction,
            data,
            metadata,
        })
        .await;
    }
}

impl Clone for LogContext {
    fn clone(&self) -> Self {
        Self {
            channel_id: self.channel_id,
            handler: self.handler.clone(),
            config: self.config.clone(),
            poll_counter: AtomicU64::new(self.poll_counter.load(Ordering::Relaxed)),
        }
    }
}

// ============================================================================
// Loggable Protocol Trait
// ============================================================================

/// Trait for protocols that support logging.
///
/// Implement this trait to enable logging on a channel.
pub trait LoggableProtocol {
    /// Set the log handler.
    fn set_log_handler(&mut self, handler: Arc<dyn ChannelLogHandler>);

    /// Set the log configuration.
    fn set_log_config(&mut self, config: ChannelLogConfig);

    /// Get the current log configuration.
    fn log_config(&self) -> &ChannelLogConfig;
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_packet_metadata() {
        let meta = PacketMetadata::modbus_tcp(1, 0x03);
        assert_eq!(meta.protocol_name(), "modbus-tcp");

        let meta = PacketMetadata::iec104(36, 3, 1);
        assert_eq!(meta.protocol_name(), "iec104");
    }

    #[test]
    fn test_log_config() {
        let config = ChannelLogConfig::new();
        assert!(config.is_enabled(LogEventType::Connected));
        assert!(!config.is_enabled(LogEventType::PollCycle));

        let config = ChannelLogConfig::all();
        assert!(config.is_enabled(LogEventType::PollCycle));
        assert!(config.should_log_raw_packets());

        let config = ChannelLogConfig::disabled();
        assert!(!config.is_enabled(LogEventType::Connected));
    }

    #[test]
    fn test_log_event_type_sets() {
        let all = LogEventType::all();
        assert_eq!(all.len(), 11);

        let default_set = LogEventType::default_set();
        assert!(!default_set.contains(&LogEventType::PollCycle));
        assert!(default_set.contains(&LogEventType::Error));
    }

    #[tokio::test]
    async fn test_log_context() {
        use std::sync::atomic::AtomicUsize;

        struct CountingHandler {
            count: AtomicUsize,
        }

        #[async_trait]
        impl ChannelLogHandler for CountingHandler {
            async fn on_log(&self, _channel_id: u32, _event: ChannelLogEvent) {
                self.count.fetch_add(1, Ordering::SeqCst);
            }
        }

        let handler = Arc::new(CountingHandler {
            count: AtomicUsize::new(0),
        });

        let ctx = LogContext::new(1)
            .with_handler(handler.clone())
            .with_config(ChannelLogConfig::all());

        ctx.log_connected("localhost:502", 100).await;
        ctx.log_error("test error", ErrorContext::Connection).await;

        assert_eq!(handler.count.load(Ordering::SeqCst), 2);
    }
}
