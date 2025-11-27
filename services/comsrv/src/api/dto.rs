#![allow(clippy::disallowed_methods)] // json! macro used in multiple functions

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
#[allow(unused_imports)] // Used in #[schema] macro expansion
use serde_json::json;
use std::collections::HashMap;
use utoipa::ToSchema;

// Import shared API types from voltage-config
pub use voltage_config::api::{
    AppError, ComponentHealth, ErrorInfo, ErrorResponse, HealthStatus, PaginatedResponse,
    ServiceStatus as SharedServiceStatus, SuccessResponse,
};
// Import Core types for zero-duplication architecture
pub use voltage_config::comsrv::{ChannelConfig, ChannelCore};

// ============================================================================
// New simplified API models
// ============================================================================

/// Control command (remote control)
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ControlRequest {
    #[schema(example = 101)]
    pub point_id: u32,
    #[schema(example = 1)]
    pub value: u8, // 0 or 1
}

/// Adjustment command (setpoint)
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AdjustmentRequest {
    #[schema(example = 201)]
    pub point_id: u32,
    #[schema(example = 5000.0)]
    pub value: f64,
}

/// Batch control commands
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BatchControlRequest {
    #[schema(example = json!([{"point_id": 101, "value": 1}, {"point_id": 102, "value": 0}]))]
    pub commands: Vec<ControlRequest>,
}

/// Batch adjustment commands
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BatchAdjustmentRequest {
    #[schema(example = json!([{"point_id": 201, "value": 5000.0}, {"point_id": 202, "value": 380.0}]))]
    pub commands: Vec<AdjustmentRequest>,
}

/// Batch command execution result
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BatchCommandResult {
    pub total: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub errors: Vec<BatchCommandError>,
}

/// Unified write response - supports both single and batch operations
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(untagged)]
pub enum WriteResponse {
    /// Single point write response
    Single(WritePointResponse),
    /// Batch write response
    Batch(BatchCommandResult),
}

/// Individual command error in batch
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct BatchCommandError {
    #[schema(example = 101)]
    pub point_id: u32,
    #[schema(example = "Invalid control value")]
    pub error: String,
}

/// Control value request for RESTful endpoints
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ControlValueRequest {
    #[schema(example = 1)]
    pub value: u8, // 0 or 1
}

/// Adjustment value request for RESTful endpoints
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AdjustmentValueRequest {
    #[schema(example = 50.0)]
    pub value: f64,
}

/// Telemetry value request for manual data injection (testing purpose)
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TelemetryValueRequest {
    #[schema(example = 380.5)]
    pub value: f64,
}

/// Signal value request for manual data injection (testing purpose)
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SignalValueRequest {
    #[schema(example = 1.0)]
    pub value: f64, // 0.0 or 1.0
}

// ============================================================================
// Unified Write Point API (New Design)
// ============================================================================

/// Unified write point request (supports all point types: T/S/C/A, single and batch)
///
/// This is the unified endpoint for writing values to channel points.
/// Supports both single point writes and batch operations.
///
/// ## Point Types (supports short names and full names)
/// - **T** / **Telemetry**: Measurement values (normally read-only, write for testing)
/// - **S** / **Signal**: Status signals (normally read-only, write for testing)
/// - **C** / **Control**: Remote control commands (0/1 for on/off)
/// - **A** / **Adjustment**: Setpoint adjustments (floating point values)
///
/// ## Example Requests
///
/// **Single Point Write**:
/// ```json
/// {
///   "type": "A",
///   "id": "1",
///   "value": 50.0
/// }
/// ```
///
/// **Batch Write (same type)**:
/// ```json
/// {
///   "type": "Control",
///   "points": [
///     {"id": "1", "value": 1.0},
///     {"id": "2", "value": 0.0}
///   ]
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct WritePointRequest {
    /// Point type: T/Telemetry, S/Signal, C/Control, or A/Adjustment
    #[serde(alias = "point_type", alias = "t")]
    #[schema(example = "A")]
    pub r#type: String,

    /// Single point or batch points (automatically detected)
    #[serde(flatten)]
    pub data: WritePointData,
}

/// Write point data - supports single or batch writes
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(untagged)]
pub enum WritePointData {
    /// Single point write: {"id": "1", "value": 50.0}
    Single {
        #[serde(alias = "point_id")]
        #[schema(example = "1")]
        id: String,
        #[schema(example = 50.0)]
        value: f64,
    },
    /// Batch write: {"points": [{"id": "1", "value": 50.0}, ...]}
    Batch { points: Vec<PointValue> },
}

/// Point value for batch operations
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PointValue {
    #[serde(alias = "point_id")]
    #[schema(example = "1")]
    pub id: String,
    #[schema(example = 50.0)]
    pub value: f64,
}

/// Write point response with operation details
///
/// This response provides information about the write operation:
/// what was written, when it was written, without queue management details.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct WritePointResponse {
    /// Channel ID
    #[schema(example = 1001)]
    pub channel_id: u16,

    /// Point type that was written (T/S/C/A)
    #[schema(example = "A")]
    pub point_type: String,

    /// Point ID that was written
    #[schema(example = "1")]
    pub point_id: String,

    /// Value that was written
    #[schema(example = 50.0)]
    pub value: f64,

    /// Timestamp when the write occurred (milliseconds since Unix epoch)
    #[schema(example = 1699876543210_i64)]
    pub timestamp_ms: i64,
}

/// service status response
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ServiceStatus {
    pub name: String,
    pub version: String,
    pub uptime: u64,
    #[schema(value_type = String, format = "date-time")]
    pub start_time: DateTime<Utc>,
    pub channels: u32,
    pub active_channels: u32,
}

/// channel status response for list endpoint
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ChannelStatusResponse {
    pub id: u16,
    pub name: String,
    pub description: Option<String>, // Channel description
    pub protocol: String,
    pub enabled: bool, // Enabled state
    pub connected: bool,
    #[schema(value_type = String, format = "date-time")]
    pub last_update: DateTime<Utc>,
}

/// channel status response - Enhanced version combining API and `ComBase` requirements
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ChannelStatus {
    pub id: u16,
    pub name: String,
    pub protocol: String,
    pub connected: bool,
    pub running: bool,
    #[schema(value_type = String, format = "date-time")]
    pub last_update: DateTime<Utc>,
    pub statistics: HashMap<String, serde_json::Value>,
}

impl From<crate::core::channels::ChannelStatus> for ChannelStatus {
    /// Convert from `ComBase` `ChannelStatus` to API `ChannelStatus`
    fn from(status: crate::core::channels::ChannelStatus) -> Self {
        Self {
            id: 0,                           // Will be filled by handler
            name: "Unknown".to_string(),     // Will be filled by handler
            protocol: "Unknown".to_string(), // Will be filled by handler
            connected: status.is_connected,
            running: status.is_connected, // Use is_connected as running status
            last_update: DateTime::<Utc>::from_timestamp(status.last_update, 0)
                .unwrap_or_else(Utc::now),
            statistics: HashMap::new(), // Will be filled by handler
        }
    }
}

// Helper function to create a simple health status
pub fn create_health_status(
    status: &str,
    uptime: u64,
    memory_usage: u64,
    cpu_usage: f64,
) -> HealthStatus {
    let service_status = match status {
        "healthy" | "ok" | "OK" => SharedServiceStatus::Healthy,
        "degraded" => SharedServiceStatus::Degraded,
        _ => SharedServiceStatus::Unhealthy,
    };

    let mut checks = HashMap::new();
    checks.insert(
        "memory".to_string(),
        ComponentHealth {
            status: if memory_usage < 1024 * 1024 * 1024 {
                // < 1GB
                SharedServiceStatus::Healthy
            } else {
                SharedServiceStatus::Degraded
            },
            message: Some(format!("Memory usage: {} bytes", memory_usage)),
            duration_ms: None,
        },
    );
    checks.insert(
        "cpu".to_string(),
        ComponentHealth {
            status: if cpu_usage < 80.0 {
                SharedServiceStatus::Healthy
            } else {
                SharedServiceStatus::Degraded
            },
            message: Some(format!("CPU usage: {:.2}%", cpu_usage)),
            duration_ms: None,
        },
    );

    HealthStatus {
        status: service_status,
        service: "comsrv".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime_seconds: uptime,
        timestamp: chrono::Utc::now(),
        checks,
    }
}

/// channel operation request
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ChannelOperation {
    pub operation: String, // "start", "stop", "restart"
}

/// Channel creation request
///
/// ## ID Assignment Strategy
/// - `channel_id` is optional and auto-assigned if not provided
/// - Auto-assignment uses MAX(channel_id) + 1 strategy
/// - Manual ID specification supported for configuration imports
/// - Both runtime and database ID conflicts are validated
///
/// ## Name Uniqueness
/// - Channel `name` must be unique across all channels
/// - Uniqueness is enforced at API level with clear error messages
///
/// ## Protocol-Specific Parameters
///
/// ### Modbus TCP
/// ```json
/// {
///   "host": "192.168.1.100",
///   "port": 502,
///   "timeout_ms": 5000,
///   "retry_count": 3
/// }
/// ```
///
/// ### Modbus RTU
/// ```json
/// {
///   "device": "/dev/ttyUSB0",
///   "baud_rate": 9600,
///   "data_bits": 8,
///   "stop_bits": 1,
///   "parity": "None",
///   "timeout_ms": 1000
/// }
/// ```
///
/// ### CAN Bus
/// ```json
/// {
///   "interface": "can0",
///   "bitrate": 500000,
///   "timeout_ms": 100
/// }
/// ```
///
/// ### Virtual Protocol (for testing)
/// ```json
/// {
///   "update_interval_ms": 1000
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ChannelCreateRequest {
    /// Channel ID (optional, auto-assigned if not provided)
    ///
    /// - Leave as `null` for automatic ID assignment (recommended)
    /// - Specify for configuration imports or ID preservation
    /// - Auto-assigned IDs start from MAX(existing_id) + 1
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(
        example = json!(null),
        nullable = true
    )]
    pub channel_id: Option<u16>,

    /// Channel name (must be unique across all channels)
    ///
    /// - Uniqueness enforced at API level
    /// - Used for channel identification in UI and logs
    /// - Recommended format: descriptive names like "PV Inverter 01"
    #[schema(example = "PV Inverter Channel")]
    pub name: String,

    /// Channel description (optional)
    #[schema(example = "Primary PV inverter communication channel")]
    pub description: Option<String>,

    /// Protocol type: modbus_tcp, modbus_rtu, can, virtual
    ///
    /// Available values: modbus_tcp, modbus_rtu, can, virtual
    #[schema(
        example = "modbus_tcp",
        value_type = String,
        pattern = "^(modbus_tcp|modbus_rtu|can|virtual)$"
    )]
    pub protocol: String,

    /// Enable channel immediately after creation (default: true)
    ///
    /// - true: Channel created and started automatically
    /// - false: Channel created in disabled state, requires manual enable
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = true, nullable = true)]
    pub enabled: Option<bool>,

    /// Protocol-specific parameters
    ///
    /// **Modbus TCP**:
    /// - host (string, required): Server IP address
    /// - port (number, optional, default: 502): Modbus TCP port
    /// - timeout_ms (number, optional, default: 5000): Timeout in milliseconds
    /// - retry_count (number, optional, default: 3): Number of retries
    ///
    /// **Modbus RTU**:
    /// - device (string, required): Serial port path (e.g., "/dev/ttyUSB0")
    /// - baud_rate (number, required): Baud rate (9600, 19200, 38400, 57600, 115200)
    /// - data_bits (number, required): Data bits (7 or 8)
    /// - stop_bits (number, required): Stop bits (1 or 2)
    /// - parity (string, required): Parity ("None", "Odd", "Even")
    /// - timeout_ms (number, optional, default: 1000): Timeout in milliseconds
    /// - retry_count (number, optional, default: 3): Number of retries
    /// - poll_interval_ms (number, optional): Polling interval in milliseconds
    ///
    /// **CAN**:
    /// - interface (string, required): CAN interface name (e.g., "can0")
    /// - bitrate (number, required): Bitrate (125000, 250000, 500000, 1000000)
    /// - timeout_ms (number, optional, default: 100): Timeout in milliseconds
    /// - loopback (boolean, optional, default: false): Loopback mode for testing
    /// - listen_only (boolean, optional, default: false): Listen-only mode
    /// - fd_mode (boolean, optional, default: false): CAN FD mode
    /// - data_bitrate (number, optional): CAN FD data segment bitrate
    ///
    /// **Virtual**:
    /// - update_interval_ms (number, optional, default: 1000): Update interval in milliseconds
    #[schema(
        value_type = Object,
        example = json!({"host": "192.168.1.100", "port": 502, "timeout_ms": 5000})
    )]
    pub parameters: HashMap<String, serde_json::Value>,
}

/// Channel configuration update request
/// Note: Use PUT /api/channels/{id}/enabled to change enabled state
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ChannelConfigUpdateRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub protocol: Option<String>,
    pub parameters: Option<HashMap<String, serde_json::Value>>,
}

/// Channel enabled state update request
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ChannelEnabledRequest {
    pub enabled: bool,
}

/// Channel CRUD operation result
/// Uses ChannelCore to eliminate field duplication
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ChannelCrudResult {
    /// Core channel fields (id, name, description, protocol, enabled)
    #[serde(flatten)]
    #[schema(value_type = Object)]
    pub core: ChannelCore,

    /// Runtime status
    #[schema(example = "running")]
    pub runtime_status: String, // "running", "stopped", "error", "not_started"

    /// Operation message
    #[schema(example = "Channel configuration updated successfully")]
    pub message: Option<String>,
}

impl From<(&ChannelConfig, String, Option<String>)> for ChannelCrudResult {
    fn from((config, runtime_status, message): (&ChannelConfig, String, Option<String>)) -> Self {
        Self {
            core: config.core.clone(),
            runtime_status,
            message,
        }
    }
}

/// Reload configuration result
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ReloadConfigResult {
    #[schema(example = 3)]
    pub total_channels: usize,
    #[schema(example = json!([1, 2]))]
    pub channels_added: Vec<u16>,
    #[schema(example = json!([3]))]
    pub channels_updated: Vec<u16>,
    #[schema(example = json!([]))]
    pub channels_removed: Vec<u16>,
    #[schema(example = json!([]))]
    pub errors: Vec<String>,
}

/// Routing cache reload result
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RoutingReloadResult {
    /// Number of C2M routing mappings loaded
    #[schema(example = 150)]
    pub c2m_count: usize,
    /// Number of M2C routing mappings loaded
    #[schema(example = 80)]
    pub m2c_count: usize,
    /// Number of C2C routing mappings loaded
    #[schema(example = 20)]
    pub c2c_count: usize,
    /// Error messages (if any)
    #[schema(example = json!([]))]
    pub errors: Vec<String>,
    /// Reload duration in milliseconds
    #[schema(example = 25)]
    pub duration_ms: u64,
}

// API response is now directly from voltage-config
// The shared version includes metadata field for extended functionality

// ============================================================================
// Phase 1: Channel Detail and Pagination
// ============================================================================

/// Complete channel details (configuration + runtime status + statistics)
/// Uses ChannelConfig to eliminate field duplication
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ChannelDetail {
    /// Complete channel configuration (includes core fields + parameters + logging)
    #[serde(flatten)]
    #[schema(value_type = Object)]
    pub config: ChannelConfig,

    /// Runtime status information
    pub runtime_status: ChannelRuntimeStatus,

    /// Point counts by type
    pub point_counts: PointCounts,
}

/// Channel runtime status information
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ChannelRuntimeStatus {
    pub connected: bool,
    pub running: bool,
    #[schema(value_type = String, format = "date-time")]
    pub last_update: DateTime<Utc>,
    pub statistics: HashMap<String, serde_json::Value>,
}

/// Point counts by type
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PointCounts {
    pub telemetry: usize,
    pub signal: usize,
    pub control: usize,
    pub adjustment: usize,
}

/// Channel list query parameters (pagination and filtering)
#[derive(Debug, Deserialize, ToSchema)]
pub struct ChannelListQuery {
    /// Page number (starting from 1)
    #[serde(default = "default_page")]
    pub page: usize,

    /// Items per page
    #[serde(default = "default_page_size")]
    pub page_size: usize,

    /// Filter by protocol type
    pub protocol: Option<String>,

    /// Filter by enabled status
    pub enabled: Option<bool>,

    /// Filter by connection status
    pub connected: Option<bool>,
}

fn default_page() -> usize {
    1
}

fn default_page_size() -> usize {
    20
}

/// Auto-reload query parameter for CRUD operations
///
/// Controls whether the channel should be automatically reloaded after configuration changes.
/// Default is `true` for immediate effect and better user experience.
#[derive(Debug, Deserialize, ToSchema)]
pub struct AutoReloadQuery {
    /// Whether to automatically reload channel after operation
    ///
    /// - `true` (default): Changes take effect immediately (channel is hot-reloaded)
    /// - `false`: Changes are saved to database only, manual reload required via `/api/channels/reload`
    ///
    /// Use `false` for batch operations to avoid multiple reloads.
    #[serde(default = "default_auto_reload")]
    #[schema(example = true)]
    pub auto_reload: bool,
}

fn default_auto_reload() -> bool {
    true // Default: auto-reload enabled for immediate effect
}

// ============================================================================
// Phase 2: Smart Hot Reload
// ============================================================================

/// Parameter change classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, ToSchema)]
pub enum ParameterChangeType {
    /// Only metadata changed (name, description) - no reload needed
    MetadataOnly,
    /// Non-critical parameters changed (timeout, retry) - may need reload
    NonCritical,
    /// Critical parameters changed (host, port, slave_id) - must reload
    Critical,
}

// ============================================================================
// Phase 3: Point and Mapping Management
// ============================================================================

/// Point definition (from Points table)
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PointDefinition {
    #[schema(example = 101)]
    pub point_id: u32,
    #[schema(example = "DC_Voltage")]
    pub signal_name: String,
    #[schema(example = 0.1)]
    pub scale: f64,
    #[schema(example = 0.0)]
    pub offset: f64,
    #[schema(example = "V")]
    pub unit: String,
    #[schema(example = "float32")]
    pub data_type: String,
    #[schema(example = false)]
    pub reverse: bool,
    #[schema(example = "DC bus voltage")]
    pub description: String,

    /// Protocol-specific mapping data (optional)
    /// Contains protocol parameters like CAN's start_bit/bit_length, Modbus's register_address, etc.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Object, example = json!({
        "can_id": 41234,
        "start_bit": 0,
        "bit_length": 16,
        "byte_order": "AB",
        "data_type": "uint16",
        "signed": false,
        "scale": 0.1,
        "offset": 0.0
    }))]
    pub protocol_mapping: Option<serde_json::Value>,
}

/// Grouped points response for channel points API
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct GroupedPoints {
    pub telemetry: Vec<PointDefinition>,
    pub signal: Vec<PointDefinition>,
    pub control: Vec<PointDefinition>,
    pub adjustment: Vec<PointDefinition>,
}

/// Point list response
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PointListResponse {
    #[schema(example = 1)]
    pub channel_id: u16,
    #[schema(example = "T")]
    pub point_type: String, // "T", "S", "C", "A"
    pub total_points: usize,
    pub mapped_points: usize,   // Points with mapping
    pub unmapped_points: usize, // Reserve points without mapping
    pub points: Vec<PointDefinition>,
}

/// Single point mapping detail (for GET response)
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PointMappingDetail {
    #[schema(example = 101)]
    pub point_id: u32,
    #[schema(example = "DC_Voltage")]
    pub signal_name: String, // For display convenience

    /// Protocol-specific mapping data (JSON format)
    /// Example for Modbus:
    /// {
    ///   "slave_id": 1,
    ///   "function_code": 3,
    ///   "register_address": 100,
    ///   "data_type": "float32",
    ///   "byte_order": "ABCD"
    /// }
    #[schema(value_type = Object, example = json!({
        "slave_id": 1,
        "function_code": 3,
        "register_address": 100,
        "data_type": "float32",
        "byte_order": "ABCD"
    }))]
    pub protocol_data: serde_json::Value,
}

/// Grouped mappings response for channel mappings API
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct GroupedMappings {
    pub telemetry: Vec<PointMappingDetail>,
    pub signal: Vec<PointMappingDetail>,
    pub control: Vec<PointMappingDetail>,
    pub adjustment: Vec<PointMappingDetail>,
}

/// Grouped mappings update request
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct GroupedMappingsUpdateRequest {
    /// Mappings grouped by point type
    #[serde(flatten)]
    pub mappings: GroupedMappings,
    /// Validate only without writing to database
    #[serde(default)]
    #[schema(example = false)]
    pub validate_only: bool,
}

/// Single point mapping item (for PUT request)
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct PointMappingItem {
    #[schema(example = 101)]
    pub point_id: u32,

    /// Four-remote type for this point (T/S/C/A)
    ///
    /// - T: Telemetry - Read-only measurements
    /// - S: Signal - Read-only status signals
    /// - C: Control - Write control commands
    /// - A: Adjustment - Write setpoint adjustments
    #[schema(
        value_type = String,
        example = "T",
        pattern = "^(T|S|C|A)$"
    )]
    pub four_remote: String,

    /// Protocol-specific mapping data (JSON format)
    /// Example for Modbus:
    /// {
    ///   "slave_id": 1,
    ///   "function_code": 3,
    ///   "register_address": 100,
    ///   "data_type": "float32",
    ///   "byte_order": "ABCD"
    /// }
    #[schema(value_type = Object, example = json!({
        "slave_id": 1,
        "function_code": 3,
        "register_address": 100,
        "data_type": "float32",
        "byte_order": "ABCD"
    }))]
    pub protocol_data: serde_json::Value,
}

/// Request to batch update protocol mappings
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MappingBatchUpdateRequest {
    /// List of mapping items to update
    #[schema(example = json!([
        {
            "point_id": 101,
            "protocol_data": {
                "slave_id": 1,
                "function_code": 3,
                "register_address": 100,
                "data_type": "float32",
                "byte_order": "ABCD"
            }
        },
        {
            "point_id": 102,
            "protocol_data": {
                "slave_id": 1,
                "function_code": 3,
                "register_address": 102,
                "data_type": "uint16",
                "byte_order": "ABCD"
            }
        }
    ]))]
    pub mappings: Vec<PointMappingItem>,

    /// Whether to reload the channel after update
    #[serde(default)]
    #[schema(example = false)]
    pub reload_channel: bool,

    /// Validate only without writing to database
    #[serde(default)]
    #[schema(example = false)]
    pub validate_only: bool,

    /// Update mode: replace (overwrite) or merge (shallow merge protocol_data)
    #[serde(default)]
    #[schema(example = "merge")]
    pub mode: MappingUpdateMode,
}

/// Result of batch mapping update operation
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MappingBatchUpdateResult {
    #[schema(example = 2)]
    pub updated_count: usize,

    #[schema(example = false)]
    pub channel_reloaded: bool,

    #[schema(example = json!([]))]
    pub validation_errors: Vec<String>,

    #[schema(example = "Successfully updated 2 mappings")]
    pub message: String,
}

/// Mapping update mode
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum MappingUpdateMode {
    Replace,
    Merge,
}

impl Default for MappingUpdateMode {
    fn default() -> Self {
        Self::Merge
    }
}

/// Mapping list response (for batch read)
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MappingListResponse {
    #[schema(example = 1)]
    pub channel_id: u16,
    #[schema(example = "modbus_tcp")]
    pub protocol: String,
    #[schema(example = "T")]
    pub point_type: String, // "T", "S", "C", "A"
    pub total_mappings: usize,
    pub mappings: Vec<PointMappingDetail>,
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;
    use chrono::Utc;
    use serde_json::json;

    #[test]
    fn test_service_status_serialization() {
        let start_time = Utc::now();
        let status = ServiceStatus {
            name: "TestService".to_string(),
            version: "1.0.0".to_string(),
            uptime: 3600,
            start_time,
            channels: 5,
            active_channels: 3,
        };

        let serialized = serde_json::to_string(&status).unwrap();
        assert!(serialized.contains("TestService"));
        assert!(serialized.contains("1.0.0"));
        assert!(serialized.contains("3600"));
    }

    #[test]
    fn test_channel_status_serialization() {
        let now = Utc::now();
        let mut parameters = HashMap::new();
        parameters.insert("timeout".to_string(), json!(5000));
        parameters.insert("slave_id".to_string(), json!(1));

        let status = ChannelStatus {
            id: 1,
            name: "Test Channel".to_string(),
            protocol: "modbus_tcp".to_string(),
            connected: true,
            running: true,
            last_update: now,
            statistics: parameters,
        };

        let serialized = serde_json::to_string(&status).unwrap();
        assert!(serialized.contains('1'));
        assert!(serialized.contains("modbus_tcp"));
        assert!(serialized.contains("true"));
    }

    #[test]
    fn test_health_status_serialization() {
        let health = create_health_status("healthy", 7200, 1_024_000, 15.5);

        // Verify health status fields (without comparing enums)
        assert_eq!(health.service, "comsrv");
        assert_eq!(health.uptime_seconds, 7200);
        assert!(health.checks.contains_key("memory"));
        assert!(health.checks.contains_key("cpu"));

        // Verify serialization contains expected values
        let serialized = serde_json::to_string(&health).unwrap();
        assert!(serialized.contains("healthy"));
        assert!(serialized.contains("7200"));
        assert!(serialized.contains("comsrv"));
    }

    #[test]
    fn test_channel_operation_deserialization() {
        let json_data = r#"{"operation": "start"}"#;
        let operation: ChannelOperation = serde_json::from_str(json_data).unwrap();
        assert_eq!(operation.operation, "start");

        let json_data = r#"{"operation": "stop"}"#;
        let operation: ChannelOperation = serde_json::from_str(json_data).unwrap();
        assert_eq!(operation.operation, "stop");

        let json_data = r#"{"operation": "restart"}"#;
        let operation: ChannelOperation = serde_json::from_str(json_data).unwrap();
        assert_eq!(operation.operation, "restart");
    }

    #[test]
    fn test_error_response_serialization() {
        let error_info = ErrorInfo::new("Not found").with_code(404);
        let error = ErrorResponse {
            success: false,
            error: error_info,
        };

        let serialized = serde_json::to_string(&error).unwrap();
        assert!(serialized.contains("404"));
        assert!(serialized.contains("Not found"));
        assert!(serialized.contains("\"success\":false"));
    }

    #[test]
    fn test_success_response() {
        let data = "test data".to_string();
        let response = SuccessResponse::new(data);

        assert_eq!(response.data, "test data");
        assert!(response.metadata.is_empty());

        let serialized = serde_json::to_string(&response).unwrap();
        assert!(serialized.contains("test data"));
        assert!(serialized.contains("data"));
    }

    #[test]
    fn test_error_response() {
        let error = ErrorInfo::new("Something went wrong");
        let response = ErrorResponse {
            success: false,
            error,
        };

        assert_eq!(response.error.message, "Something went wrong");
        assert!(!response.success);

        let serialized = serde_json::to_string(&response).unwrap();
        assert!(serialized.contains("Something went wrong"));
        assert!(serialized.contains("error"));
        assert!(serialized.contains("\"success\":false"));
    }

    #[test]
    fn test_channel_status_with_empty_parameters() {
        let now = Utc::now();
        let status = ChannelStatus {
            id: 1,
            name: "Simple Channel".to_string(),
            protocol: "Virtual".to_string(),
            connected: false,
            running: false,
            last_update: now,
            statistics: HashMap::new(),
        };

        let serialized = serde_json::to_string(&status).unwrap();
        assert!(serialized.contains('1'));
        assert!(serialized.contains("false"));
    }

    #[test]
    fn test_combase_channel_status_conversion() {
        let combase_status = crate::core::channels::ChannelStatus {
            is_connected: true,
            last_update: 1_234_567_890,
        };
        let api_status = ChannelStatus::from(combase_status);

        assert_eq!(api_status.id, 0); // Default value
        assert_eq!(api_status.name, "Unknown");
        assert_eq!(api_status.protocol, "Unknown");
        assert!(api_status.connected);
        assert!(api_status.statistics.is_empty());
    }

    #[test]
    fn test_channel_create_request_deserialization() {
        let json_data = r#"{
            "channel_id": 1001,
            "name": "Test Channel",
            "description": "Test channel for Modbus TCP",
            "protocol": "modbus_tcp",
            "enabled": true,
            "parameters": {
                "host": "192.168.1.100",
                "port": 502,
                "slave_id": 1
            }
        }"#;

        let request: ChannelCreateRequest = serde_json::from_str(json_data).unwrap();
        assert_eq!(request.channel_id, Some(1001));
        assert_eq!(request.name, "Test Channel");
        assert_eq!(
            request.description,
            Some("Test channel for Modbus TCP".to_string())
        );
        assert_eq!(request.protocol, "modbus_tcp");
        assert_eq!(request.enabled, Some(true));
        assert_eq!(request.parameters.len(), 3);
        assert_eq!(
            request.parameters.get("host"),
            Some(&json!("192.168.1.100"))
        );
        assert_eq!(request.parameters.get("port"), Some(&json!(502)));
        assert_eq!(request.parameters.get("slave_id"), Some(&json!(1)));
    }

    #[test]
    fn test_channel_config_update_request_deserialization() {
        let json_data = r#"{
            "name": "Updated Channel Name",
            "description": "Updated description",
            "parameters": {
                "timeout": 5000
            }
        }"#;

        let request: ChannelConfigUpdateRequest = serde_json::from_str(json_data).unwrap();
        assert_eq!(request.name, Some("Updated Channel Name".to_string()));
        assert_eq!(request.description, Some("Updated description".to_string()));
        assert!(request.protocol.is_none());
        assert!(request.parameters.is_some());

        let params = request.parameters.unwrap();
        assert_eq!(params.get("timeout"), Some(&json!(5000)));
    }
}
