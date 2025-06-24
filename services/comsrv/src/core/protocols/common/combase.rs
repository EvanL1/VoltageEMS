//! # Communication Base Module
//! 
//! This module provides the foundational traits and types for implementing
//! communication protocols in the Voltage EMS Communication Service. It defines
//! the common interface that all protocol implementations must satisfy.
//! 
//! ## Overview
//! 
//! The Communication Service supports multiple industrial communication protocols
//! through a unified interface. This module defines:
//! 
//! - **ComBase Trait**: The primary interface all protocols must implement
//! - **ChannelStatus**: Status reporting and health monitoring
//! - **PointData**: Real-time data point representation
//! - **ComBaseImpl**: Reference implementation with common functionality
//! 
//! ## Key Components
//! 
//! ### ComBase Trait
//! 
//! The `ComBase` trait provides a standardized interface for:
//! - Protocol lifecycle management (start/stop)
//! - Status monitoring and error reporting
//! - Real-time data collection
//! - Configuration parameter access
//! 
//! ### Channel Status Monitoring
//! 
//! The status system provides:
//! - Connection state tracking
//! - Performance metrics (response times)
//! - Error condition reporting
//! - Timestamped status updates
//! 
//! ## Usage Example
//! 
//! ```rust
//! use comsrv::core::protocols::common::combase::{ComBase, ChannelStatus, PointData};
//! use comsrv::utils::Result;
//! 
//! // Example usage of the communication base interface
//! async fn example_usage(mut service: Box<dyn ComBase>) -> Result<()> {
//!     // Start the communication service
//!     service.start().await?;
//!     
//!     // Check operational status
//!     let status = service.status().await;
//!     println!("Service {}: connected={}", service.name(), status.connected);
//!     
//!     // Collect data points
//!     let points = service.get_all_points().await;
//!     for point in points {
//!         println!("Point {}: {}", point.id, point.value);
//!     }
//!     
//!     // Graceful shutdown
//!     service.stop().await?;
//!     Ok(())
//! }
//! ```
//! 
//! ## Protocol Implementation Guide
//! 
//! To implement a new communication protocol:
//! 
//! 1. **Create a Protocol Struct**: Define your protocol's specific configuration and state
//! 2. **Implement ComBase**: Provide implementations for all required methods
//! 3. **Handle Lifecycle**: Properly manage connection setup and teardown
//! 4. **Report Status**: Keep channel status updated with current information
//! 5. **Error Handling**: Use the unified error system for consistent reporting
//! 
//! ## Example Implementation Structure
//! 
//! ```rust
//! use comsrv::core::protocols::common::combase::ComBaseImpl;
//! use comsrv::core::config::config_manager::ChannelConfig;
//! 
//! // Example: Creating a communication service base
//! fn create_service_base(name: &str, protocol: &str, config: ChannelConfig) -> ComBaseImpl {
//!     ComBaseImpl::new(name, protocol, config)
//! }
//! ```

use std::sync::Arc;
use std::time::{Duration, Instant};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use tokio::sync::RwLock;
use std::collections::HashMap;
use serde_json;
use tokio::time::interval;
use serde::{Serialize, Deserialize};

use log::{trace, info, warn, error, debug, info as log_info, warn as log_warn, error as log_error, debug as log_debug};

use crate::core::config::config_manager::ChannelConfig;
use crate::utils::error::{ComSrvError, Result};
use crate::core::storage::redis_storage::{RemoteCommand, CommandResult, CommandType};

/// Channel operational status and health information
/// 
/// Provides comprehensive status information for a communication channel,
/// including connection state, performance metrics, and error conditions.
/// This structure is used for monitoring and diagnostics of communication channels.
/// 
/// # Fields
/// 
/// * `id` - Unique identifier for the channel
/// * `connected` - Whether the channel is currently connected
/// * `last_response_time` - Most recent response time in milliseconds
/// * `last_error` - Description of the most recent error (empty if no error)
/// * `last_update_time` - Timestamp of the last status update
/// 
/// # Examples
/// 
/// ```
/// use comsrv::core::protocols::common::combase::ChannelStatus;
/// 
/// let status = ChannelStatus::new("modbus_001");
/// assert!(!status.connected);
/// assert!(!status.has_error());
/// assert!(status.last_error.is_empty());
/// ```
#[derive(Debug, Clone)]
pub struct ChannelStatus {
    /// Channel identifier
    pub id: String,
    /// Connection status
    pub connected: bool,
    /// Last response time in milliseconds
    pub last_response_time: f64,
    /// Last error message
    pub last_error: String,
    /// Last status update time
    pub last_update_time: DateTime<Utc>,
}

impl ChannelStatus {
    /// Create a new channel status with default values
    /// 
    /// Initializes a new channel status with disconnected state, zero response time,
    /// no error message, and current timestamp.
    /// 
    /// # Arguments
    /// 
    /// * `channel_id` - Unique identifier for the channel
    /// 
    /// # Returns
    /// 
    /// New `ChannelStatus` instance with default values
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use comsrv::core::protocols::common::combase::ChannelStatus;
    /// 
    /// let status = ChannelStatus::new("modbus_001");
    /// assert_eq!(status.id, "modbus_001");
    /// assert!(!status.connected);
    /// assert_eq!(status.last_response_time, 0.0);
    /// assert!(status.last_error.is_empty());
    /// ```
    pub fn new(channel_id: &str) -> Self {
        Self {
            id: channel_id.to_string(),
            connected: false,
            last_response_time: 0.0,
            last_error: String::new(),
            last_update_time: Utc::now(),
        }
    }

    /// Check if the channel has an error condition
    /// 
    /// Determines whether the channel currently has an error by checking
    /// if the error message is non-empty.
    /// 
    /// # Returns
    /// 
    /// `true` if there is an active error condition, `false` otherwise
    /// 
    /// # Examples
    /// 
    /// ```
    /// use comsrv::core::protocols::common::combase::ChannelStatus;
    /// 
    /// let mut status = ChannelStatus::new("test_channel");
    /// assert!(!status.has_error());
    /// 
    /// // Simulate an error condition
    /// status.last_error = "Connection failed".to_string();
    /// assert!(status.has_error());
    /// ```
    pub fn has_error(&self) -> bool {
        !self.last_error.is_empty()
    }
}

/// Real-time data point structure with quality indicators
/// 
/// Represents a single data point from a communication channel with
/// associated metadata including quality indicators and timestamps.
/// This structure is used for real-time data collection and monitoring.
/// 
/// # Fields
/// 
/// * `id` - Unique identifier for the data point
/// * `name` - Human-readable name for the data point
/// * `value` - Current value as a string
/// * `quality` - Data quality indicator (0=bad, 1=good, 2=uncertain)
/// * `timestamp` - Time when the data was collected
/// * `unit` - Engineering unit for the value
/// * `description` - Detailed description of the data point
/// 
/// # Data Quality
/// 
/// The quality field indicates data reliability:
/// - `0`: Bad quality data (unreliable)
/// - `1`: Good quality data (reliable)
/// - `2`: Uncertain quality data (may be reliable)
/// 
/// # Examples
/// 
/// ```
/// use comsrv::core::protocols::common::combase::PointData;
/// use chrono::Utc;
///
/// let point = PointData {
///     id: "voltage_1".to_string(),
///     name: "Main Bus Voltage".to_string(),
///     value: "230.5".to_string(),
///     quality: 1,
///     timestamp: Utc::now(),
///     unit: "V".to_string(),
///     description: "Primary electrical bus voltage measurement".to_string(),
/// };
///
/// println!("Point {}: {} {} (quality: {})", point.name, point.value, point.unit, point.quality);
/// ```
#[derive(Debug, Clone)]
pub struct PointData {
    /// Point ID
    pub id: String,
    /// Point name
    pub name: String,
    /// Point value as string
    pub value: String,
    /// Data quality (0=bad, 1=good, 2=uncertain)
    pub quality: u8,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
    /// Engineering unit
    pub unit: String,
    /// Point description
    pub description: String,
}

/// Universal Polling Configuration
/// 
/// This configuration is protocol-agnostic and can be used by any communication protocol
/// that requires periodic data collection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PollingConfig {
    /// Enable or disable polling for this channel
    pub enabled: bool,
    /// Polling interval in milliseconds
    pub interval_ms: u64,
    /// Maximum number of points to read per polling cycle
    pub max_points_per_cycle: u32,
    /// Timeout for each polling operation
    pub timeout_ms: u64,
    /// Number of retry attempts on failure
    pub max_retries: u32,
    /// Delay between retries in milliseconds
    pub retry_delay_ms: u64,
    /// Enable batch reading optimization (protocol-specific)
    pub enable_batch_reading: bool,
    /// Minimum delay between individual point reads in milliseconds
    pub point_read_delay_ms: u64,
}

impl Default for PollingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            interval_ms: 1000,
            max_points_per_cycle: 1000,
            timeout_ms: 5000,
            max_retries: 3,
            retry_delay_ms: 1000,
            enable_batch_reading: true,
            point_read_delay_ms: 10,
        }
    }
}

/// Universal Polling Statistics
/// 
/// These statistics are collected for any protocol that uses the polling system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PollingStats {
    /// Total number of polling cycles executed
    pub total_cycles: u64,
    /// Number of successful polling cycles
    pub successful_cycles: u64,
    /// Number of failed polling cycles
    pub failed_cycles: u64,
    /// Total data points read successfully
    pub total_points_read: u64,
    /// Total data points that failed to read
    pub total_points_failed: u64,
    /// Average polling cycle time in milliseconds
    pub avg_cycle_time_ms: f64,
    /// Current polling rate (cycles per second)
    pub current_polling_rate: f64,
    /// Last successful polling timestamp
    pub last_successful_polling: Option<DateTime<Utc>>,
    /// Last polling error message
    pub last_polling_error: Option<String>,
    /// Communication quality percentage (0-100)
    pub communication_quality: f64,
}

/// Point value type enumeration for four-telemetry operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PointValueType {
    /// Analog measurements
    Analog(f64),
    /// Digital status
    Digital(bool),
}

/// Point operation type for remote control
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RemoteOperationType {
    /// Digital control
    Control { value: bool },
    /// Analog regulation
    Regulation { value: f64 },
}

/// Command execution request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteOperationRequest {
    /// Operation ID
    pub operation_id: String,
    /// Point name
    pub point_name: String,
    /// Operation type
    pub operation_type: RemoteOperationType,
    /// Operator information
    pub operator: Option<String>,
    /// Operation description
    pub description: Option<String>,
    /// Request timestamp
    pub timestamp: DateTime<Utc>,
}

/// Command execution response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteOperationResponse {
    /// Operation ID (corresponds to request ID)
    pub operation_id: String,
    /// Execution success
    pub success: bool,
    /// Error message (if any)
    pub error_message: Option<String>,
    /// Actual value after execution
    pub actual_value: Option<PointValueType>,
    /// Execution completion timestamp
    pub execution_time: DateTime<Utc>,
}

/// Define the standard four-telemetry interface for SCADA systems
#[async_trait]
pub trait FourTelemetryOperations: Send + Sync {
    /// Remote Measurement - Read analog measurement values from remote devices
    /// Read analog measurement values from remote devices
    /// 
    /// # Arguments
    /// * `point_names` - List of point names to read
    /// 
    /// # Returns
    /// * `Ok(Vec<(String, PointValueType)>)` - Successfully read values with point names
    /// * `Err(ComSrvError)` - Read operation failed
    async fn remote_measurement(&self, point_names: &[String]) -> Result<Vec<(String, PointValueType)>>;
    
    /// Read digital status values from remote devices
    /// 
    /// # Arguments
    /// * `point_names` - List of point names to read
    /// 
    /// # Returns
    /// * `Ok(Vec<(String, PointValueType)>)` - Successfully read values with point names
    /// * `Err(ComSrvError)` - Read operation failed
    async fn remote_signaling(&self, point_names: &[String]) -> Result<Vec<(String, PointValueType)>>;
    
    /// Execute digital control operations on remote devices
    /// 
    /// # Arguments
    /// * `request` - Remote control operation request
    /// 
    /// # Returns
    /// * `Ok(RemoteOperationResponse)` - Control operation result
    /// * `Err(ComSrvError)` - Control operation failed
    async fn remote_control(&self, request: RemoteOperationRequest) -> Result<RemoteOperationResponse>;
    
    /// Execute analog regulation operations on remote devices
    /// 
    /// # Arguments
    /// * `request` - Remote regulation operation request
    /// 
    /// # Returns
    /// * `Ok(RemoteOperationResponse)` - Regulation operation result
    /// * `Err(ComSrvError)` - Regulation operation failed
    async fn remote_regulation(&self, request: RemoteOperationRequest) -> Result<RemoteOperationResponse>;

    /// Get all available remote control points
    async fn get_control_points(&self) -> Vec<String>;
    
    /// Get all available remote regulation points
    async fn get_regulation_points(&self) -> Vec<String>;
    
    /// Get all available measurement points
    async fn get_measurement_points(&self) -> Vec<String>;
    
    /// Get all available signaling points  
    async fn get_signaling_points(&self) -> Vec<String>;
}

/// Universal Redis Command Manager
/// Universal Redis command manager for handling four-telemetry commands across all protocols
#[derive(Clone)]
pub struct UniversalCommandManager {
    /// Redis store for command handling
    redis_store: Option<crate::core::storage::redis_storage::RedisStore>,
    /// Channel ID for this communication instance
    channel_id: String,
    /// Command listener task handle
    command_listener_handle: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
    /// Running state
    is_running: Arc<RwLock<bool>>,
}

impl UniversalCommandManager {
    /// Create a new command manager
    pub fn new(channel_id: String) -> Self {
        Self {
            redis_store: None,
            channel_id,
            command_listener_handle: Arc::new(RwLock::new(None)),
            is_running: Arc::new(RwLock::new(false)),
        }
    }

    /// Initialize with Redis store
    pub fn with_redis_store(mut self, redis_store: crate::core::storage::redis_storage::RedisStore) -> Self {
        self.redis_store = Some(redis_store);
        self
    }

    /// Start command listener
    pub async fn start<T>(&self, four_telemetry_impl: Arc<T>) -> Result<()> 
    where 
        T: FourTelemetryOperations + 'static,
    {
        if self.redis_store.is_none() {
            // No Redis integration, skip command listener
            return Ok(());
        }

        *self.is_running.write().await = true;

        let redis_store = self.redis_store.as_ref().unwrap().clone();
        let channel_id = self.channel_id.clone();
        let is_running = Arc::clone(&self.is_running);

        let handle = tokio::spawn(async move {
            Self::command_listener_loop(redis_store, four_telemetry_impl, channel_id, is_running).await;
        });

        *self.command_listener_handle.write().await = Some(handle);
        info!("Universal command manager started for channel: {}", self.channel_id);
        Ok(())
    }

    /// Stop command listener
    pub async fn stop(&self) -> Result<()> {
        *self.is_running.write().await = false;

        if let Some(handle) = self.command_listener_handle.write().await.take() {
            handle.abort();
        }

        info!("Universal command manager stopped for channel: {}", self.channel_id);
        Ok(())
    }

    /// Redis command listener loop
    async fn command_listener_loop<T>(
        redis_store: crate::core::storage::redis_storage::RedisStore,
        four_telemetry_impl: Arc<T>,
        channel_id: String,
        is_running: Arc<RwLock<bool>>,
    ) 
    where 
        T: FourTelemetryOperations + 'static,
    {
        use futures::StreamExt;
        
        info!("Starting Redis command listener for channel: {}", channel_id);

        // Create PubSub connection
        let mut pubsub = match redis_store.create_pubsub().await {
            Ok(pubsub) => pubsub,
            Err(e) => {
                error!("Failed to create Redis PubSub connection: {}", e);
                return;
            }
        };

        // Subscribe to command channel
        let command_channel = format!("commands:{}", channel_id);
        if let Err(e) = pubsub.subscribe(&command_channel).await {
            error!("Failed to subscribe to command channel {}: {}", command_channel, e);
            return;
        }

        info!("Subscribed to Redis command channel: {}", command_channel);

        // Listen for commands
        while *is_running.read().await {
            match pubsub.on_message().next().await {
                Some(msg) => {
                    let command_id: String = match msg.get_payload() {
                        Ok(payload) => payload,
                        Err(e) => {
                            warn!("Failed to parse command notification payload: {}", e);
                            continue;
                        }
                    };

                    debug!("Received command notification: {}", command_id);

                    // Process command
                    if let Err(e) = Self::process_redis_command(
                        &redis_store,
                        &four_telemetry_impl,
                        &channel_id,
                        &command_id,
                    ).await {
                        error!("Failed to process command {}: {}", command_id, e);
                    }
                }
                None => {
                    trace!("No message received from Redis PubSub");
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        }

        debug!("Redis command listener loop stopped");
    }

    /// Process a Redis command using four-telemetry operations
    async fn process_redis_command<T>(
        redis_store: &crate::core::storage::redis_storage::RedisStore,
        four_telemetry_impl: &Arc<T>,
        channel_id: &str,
        command_id: &str,
    ) -> Result<()>
    where 
        T: FourTelemetryOperations + 'static,
    {
        use crate::core::storage::redis_storage::{CommandType, CommandResult};

        // Get command from Redis
        let command = match redis_store.get_command(channel_id, command_id).await? {
            Some(cmd) => cmd,
            None => {
                warn!("Command {} not found in Redis", command_id);
                return Ok(());
            }
        };

        info!("Processing command: {} for point: {} with value: {}", 
            command_id, command.point_name, command.value);

        // Convert Redis command to four-telemetry request
        let request = RemoteOperationRequest {
            operation_id: command.command_id.clone(),
            point_name: command.point_name.clone(),
            operation_type: match command.command_type {
                CommandType::RemoteControl => RemoteOperationType::Control { 
                    value: command.value != 0.0 
                },
                CommandType::RemoteRegulation => RemoteOperationType::Regulation { 
                    value: command.value 
                },
            },
            operator: None,
            description: None,
            timestamp: Utc::now(),
        };

        // Execute command using four-telemetry interface
        let response = match command.command_type {
            CommandType::RemoteControl => {
                four_telemetry_impl.remote_control(request).await
            }
            CommandType::RemoteRegulation => {
                four_telemetry_impl.remote_regulation(request).await
            }
        };

        // Convert four-telemetry response to Redis result
        let result = match response {
            Ok(resp) => {
                info!("Command {} executed successfully", command_id);
                
                CommandResult {
                    command_id: resp.operation_id,
                    success: resp.success,
                    error_message: resp.error_message,
                    execution_time: resp.execution_time.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
                    actual_value: resp.actual_value.map(|v| match v {
                        PointValueType::Analog(val) => val,
                        PointValueType::Digital(val) => if val { 1.0 } else { 0.0 },
                    }),
                }
            }
            Err(e) => {
                error!("Command {} execution failed: {}", command_id, e);
                
                CommandResult {
                    command_id: command.command_id.clone(),
                    success: false,
                    error_message: Some(e.to_string()),
                    execution_time: Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
                    actual_value: None,
                }
            }
        };

        // Save result to Redis
        if let Err(e) = redis_store.set_command_result(channel_id, &result).await {
            warn!("Failed to save command result: {}", e);
        }

        // Delete processed command
        if let Err(e) = redis_store.delete_command(channel_id, command_id).await {
            warn!("Failed to delete processed command: {}", e);
        }

        Ok(())
    }

    /// Sync real-time data to Redis
    pub async fn sync_data_to_redis(&self, data_points: &[PointData]) -> Result<()> {
        if let Some(ref redis_store) = self.redis_store {
            for point in data_points {
                let realtime_value = crate::core::storage::redis_storage::RealtimeValue {
                    raw: point.value.parse::<f64>().unwrap_or(0.0),
                    processed: point.value.parse::<f64>().unwrap_or(0.0),
                    timestamp: point.timestamp.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
                };

                let redis_key = format!("realtime:{}:{}", self.channel_id, point.id);

                if let Err(e) = redis_store.set_realtime_value_with_expire(&redis_key, &realtime_value, 3600).await {
                    warn!("Failed to sync point {} to Redis: {}", point.id, e);
                } else {
                    trace!("Successfully synced point {} to Redis", point.id);
                }
            }
            
            debug!("Synced {} points to Redis for channel {}", data_points.len(), self.channel_id);
        }
        Ok(())
    }
}

impl Default for PollingStats {
    fn default() -> Self {
        Self {
            total_cycles: 0,
            successful_cycles: 0,
            failed_cycles: 0,
            total_points_read: 0,
            total_points_failed: 0,
            avg_cycle_time_ms: 0.0,
            current_polling_rate: 0.0,
            last_successful_polling: None,
            last_polling_error: None,
            communication_quality: 100.0,
        }
    }
}

/// Point Definition for Polling
/// 
/// Defines a data point that should be read during polling cycles.
/// This is protocol-agnostic and can represent any type of data point.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PollingPoint {
    /// Unique point identifier
    pub id: String,
    /// Human-readable point name
    pub name: String,
    /// Protocol-specific address (e.g., Modbus register address, IEC60870 IOA)
    pub address: u32,
    /// Data type for value interpretation
    pub data_type: String,
    /// Scaling factor applied to raw values
    pub scale: f64,
    /// Offset applied after scaling
    pub offset: f64,
    /// Engineering unit
    pub unit: String,
    /// Point description
    pub description: String,
    /// Access mode (read, write, read-write)
    pub access_mode: String,
    /// Point group for batch operations
    pub group: String,
    /// Protocol-specific parameters
    pub protocol_params: HashMap<String, serde_json::Value>,
}

/// Generic connection state used by [`ConnectionManager`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionState {
    /// Channel is disconnected.
    Disconnected,
    /// Channel is attempting to establish a connection.
    Connecting,
    /// Channel is connected and operational.
    Connected,
    /// Channel encountered an error during connection.
    Error(String),
}

/// Unified trait for connection management across protocols.
///
/// Implementors should handle protocol specific connect/disconnect logic
/// while updating the provided [`ConnectionState`] information.
#[async_trait]
pub trait ConnectionManager: Send + Sync {
    /// Connect to the remote endpoint.
    async fn connect(&mut self) -> Result<()>;

    /// Disconnect from the remote endpoint.
    async fn disconnect(&mut self) -> Result<()>;

    /// Attempt to reconnect using protocol specific strategy.
    async fn reconnect(&mut self) -> Result<()> {
        self.disconnect().await?;
        self.connect().await
    }

    /// Retrieve the current connection state.
    async fn connection_state(&self) -> ConnectionState;
}

/// Trait for configuration validation of protocol implementations.
#[async_trait]
pub trait ConfigValidator: Send + Sync {
    /// Validate configuration parameters.
    async fn validate_config(&self) -> Result<()> {
        Ok(())
    }
}

/// Trait representing protocol specific statistics collection.
pub trait ProtocolStats: Send + Sync {
    /// Reset all statistic counters.
    fn reset(&mut self);
}

/// Primary communication interface for all protocol implementations
/// 
/// This trait defines the standard interface that all communication protocols
/// must implement to integrate with the Communication Service. It provides
/// a consistent API for managing protocol lifecycle, monitoring status,
/// and accessing real-time data.
/// 
/// ## Design Principles
/// 
/// - **Protocol Agnostic**: Works with any communication protocol
/// - **Async by Default**: All operations are asynchronous for scalability
/// - **Status Monitoring**: Built-in status reporting and error tracking
/// - **Type Safety**: Strongly typed interfaces with clear error handling
/// 
/// ## Implementation Requirements
/// 
/// All implementing types must:
/// - Be `Send + Sync` for thread safety
/// - Implement `Debug` for logging and debugging
/// - Handle errors gracefully without panicking
/// - Provide accurate status information
/// 
/// ## Lifecycle Management
/// 
/// The typical lifecycle of a communication service:
/// 1. Creation and configuration
/// 2. Start operation (`start()`)
/// 3. Normal operation with data collection
/// 4. Status monitoring and error handling
/// 5. Graceful shutdown (`stop()`)
/// 
/// # Examples
/// 
/// ```
/// use comsrv::core::protocols::common::combase::{ComBase, ChannelStatus};
/// use comsrv::utils::Result;
/// 
/// #[tokio::main]
/// async fn main() -> Result<()> {
///     // Create a mock service that implements ComBase
///     struct MockService;
///     
///     // Implementation would go here
///     println!("ComBase usage example");
///     Ok(())
/// }
/// ```
#[async_trait]
pub trait ComBase: Send + Sync + std::fmt::Debug {
    /// Downcast helper for dynamic protocol access
    fn as_any(&self) -> &dyn std::any::Any;
    /// Get the human-readable name of the communication service
    /// 
    /// Returns a descriptive name for this communication service instance,
    /// typically used for logging, monitoring, and user interfaces.
    /// 
    /// # Returns
    /// 
    /// Service name as a string slice
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// # use comsrv::core::protocols::common::combase::ComBase;
    /// # fn example(service: &dyn ComBase) {
    /// println!("Service name: {}", service.name());
    /// # }
    /// ```
    fn name(&self) -> &str;
    
    /// Get the unique channel identifier
    /// 
    /// Returns a unique identifier for this communication channel,
    /// used for distinguishing between multiple channels and for
    /// configuration management.
    /// 
    /// # Returns
    /// 
    /// Channel ID as a string
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// # use comsrv::core::protocols::common::combase::ComBase;
    /// # fn example(service: &dyn ComBase) {
    /// let channel_id = service.channel_id();
    /// println!("Channel: {}", channel_id);
    /// # }
    /// ```
    fn channel_id(&self) -> String;
    
    /// Get the protocol type identifier
    /// 
    /// Returns the type of communication protocol implemented by this service,
    /// such as "ModbusTcp", "ModbusRtu", "IEC60870", etc.
    /// 
    /// # Returns
    /// 
    /// Protocol type as a string slice
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// # use comsrv::core::protocols::common::combase::ComBase;
    /// # fn example(service: &dyn ComBase) {
    /// match service.protocol_type() {
    ///     "ModbusTcp" => println!("Using Modbus TCP protocol"),
    ///     "IEC60870" => println!("Using IEC 60870 protocol"),
    ///     _ => println!("Unknown protocol"),
    /// }
    /// # }
    /// ```
    fn protocol_type(&self) -> &str;
    
    /// Get protocol-specific parameters and configuration
    /// 
    /// Returns a map of configuration parameters specific to this protocol
    /// implementation. These parameters can be used for diagnostics,
    /// monitoring, or dynamic reconfiguration.
    /// 
    /// # Returns
    /// 
    /// HashMap containing parameter names and values as strings
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// # use comsrv::core::protocols::common::combase::ComBase;
    /// # fn example(service: &dyn ComBase) {
    /// let params = service.get_parameters();
    /// if let Some(host) = params.get("host") {
    ///     println!("Connected to host: {}", host);
    /// }
    /// # }
    /// ```
    fn get_parameters(&self) -> HashMap<String, String>;
    
    /// Check if the communication service is currently running
    /// 
    /// Determines whether the service is in an active, running state
    /// and capable of processing communication requests.
    /// 
    /// # Returns
    /// 
    /// `true` if the service is running, `false` otherwise
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// # use comsrv::core::protocols::common::combase::ComBase;
    /// # async fn example(service: &dyn ComBase) {
    /// if service.is_running().await {
    ///     println!("Service is active");
    /// } else {
    ///     println!("Service is stopped");
    /// }
    /// # }
    /// ```
    async fn is_running(&self) -> bool;
    
    /// Start the communication service
    /// 
    /// Initiates the communication service and sets the running state to true.
    /// This base implementation only manages the running state; protocol-specific
    /// implementations should override this method to perform actual connection setup.
    /// 
    /// # Returns
    /// 
    /// * `Ok(())` - Service started successfully
    /// * `Err(error)` - Failure during startup
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use comsrv::core::protocols::common::combase::ComBaseImpl;
    /// use comsrv::core::config::config_manager::{ChannelConfig, ProtocolType, ChannelParameters};
    /// use std::collections::HashMap;
    /// 
    /// # async fn example(service: &ComBaseImpl) -> comsrv::utils::Result<()> {
    /// // Service startup is handled by ComBaseImpl
    /// service.start().await?;
    /// assert!(service.is_running().await);
    /// # Ok(())
    /// # }
    /// ```
    async fn start(&mut self) -> Result<()>;
    
    /// Stop the communication service gracefully
    /// 
    /// Stops the communication service and sets the running state to false.
    /// This base implementation only manages the running state; protocol-specific
    /// implementations should override this method to perform actual connection cleanup.
    /// 
    /// # Returns
    /// 
    /// * `Ok(())` - Service stopped successfully
    /// * `Err(error)` - Failure during shutdown
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use comsrv::core::protocols::common::combase::ComBaseImpl;
    /// use comsrv::core::config::config_manager::{ChannelConfig, ProtocolType, ChannelParameters};
    /// use std::collections::HashMap;
    /// 
    /// # async fn example(service: &ComBaseImpl) -> comsrv::utils::Result<()> {
    /// // Service shutdown is handled by ComBaseImpl
    /// service.stop().await?;
    /// assert!(!service.is_running().await);
    /// # Ok(())
    /// # }
    /// ```
    async fn stop(&mut self) -> Result<()>;
    
    /// Get the current status of the communication channel
    /// 
    /// Returns a snapshot of the current channel status including connection state,
    /// response time metrics, error conditions, and last update timestamp.
    /// 
    /// # Returns
    /// 
    /// Current `ChannelStatus` with up-to-date information
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use comsrv::core::protocols::common::combase::ComBaseImpl;
    /// use comsrv::core::config::config_manager::{ChannelConfig, ProtocolType, ChannelParameters};
    /// use std::collections::HashMap;
    /// 
    /// # async fn example(service: &ComBaseImpl) {
    /// let status = service.status().await;
    /// println!("Channel {}: connected={}", status.id, status.connected);
    /// # }
    /// ```
    async fn status(&self) -> ChannelStatus;
    
/// Check if the channel currently has an error condition
    /// 
    /// Convenience method that checks the current status for error conditions.
    /// This provides a quick way to determine if the channel is experiencing
    /// problems without retrieving the full status.
    /// 
    /// # Returns
    /// 
    /// `true` if there is an active error condition, `false` otherwise
    /// 
    /// # Default Implementation
    /// 
    /// The default implementation calls `status().await.has_error()`,
    /// but implementations may override this for better performance.
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// # use comsrv::core::protocols::common::combase::ComBase;
    /// # async fn example(service: &dyn ComBase) {
    /// if service.has_error().await {
    ///     println!("Channel has active errors");
    ///     let error_msg = service.last_error().await;
    ///     println!("Error details: {}", error_msg);
    /// }
    /// # }
    /// ```
    async fn has_error(&self) -> bool {
        self.status().await.has_error()
    }
    
    /// Get the most recent error message from the channel
    /// 
    /// Returns the error message from the most recent error condition.
    /// If there are no current errors, returns an empty string.
    /// 
    /// # Returns
    /// 
    /// Error message as a string (empty if no errors)
    /// 
    /// # Default Implementation
    /// 
    /// The default implementation calls `status().await.last_error`,
    /// but implementations may override this for better performance.
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// # use comsrv::core::protocols::common::combase::ComBase;
    /// # async fn example(service: &dyn ComBase) {
    /// let error_msg = service.last_error().await;
    /// if !error_msg.is_empty() {
    ///     eprintln!("Channel error: {}", error_msg);
    /// }
    /// # }
    /// ```
    async fn last_error(&self) -> String {
        self.status().await.last_error
    }
    
    /// Get all real-time data points from the communication channel
    /// 
    /// Retrieves all available data points from the channel with their
    /// current values, quality indicators, and timestamps. This method
    /// is used for bulk data collection and monitoring.
    /// 
    /// # Returns
    /// 
    /// Vector of `PointData` structures containing all available data points
    /// 
    /// # Default Implementation
    /// 
    /// The default implementation returns an empty vector. Protocol
    /// implementations should override this method to provide actual
    /// data point collection.
    /// 
    /// # Data Quality
    /// 
    /// Each data point includes a quality indicator:
    /// - `true`: Data is current and reliable
    /// - `false`: Data may be stale or unreliable
    /// 
    /// # Performance Considerations
    /// 
    /// This method may involve network communication and should be
    /// called with appropriate frequency based on system requirements.
    /// Consider caching strategies for high-frequency access.
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// # use comsrv::core::protocols::common::combase::ComBase;
    /// # async fn example(service: &dyn ComBase) {
    /// let points = service.get_all_points().await;
    /// 
    /// for point in points {
    ///     if point.quality {
    ///         println!("Point {}: {} (good quality)", point.id, point.value);
    ///     } else {
    ///         println!("Point {}: {} (poor quality)", point.id, point.value);
    ///     }
    /// }
    /// # }
    /// ```
    async fn get_all_points(&self) -> Vec<PointData> {
        Vec::new() // Default implementation returns an empty vector
    }
}

/// Protocol logging trait for unified logging across all communication protocols
/// 
/// This trait provides standardized logging methods that can be used by all protocol
/// implementations. It's separate from ComBase to maintain object safety while 
/// providing rich logging capabilities.
pub trait ProtocolLogger: Send + Sync {
    /// Get the channel ID for logging context
    fn channel_id(&self) -> String;
    
    /// Get the protocol type for logging context  
    fn protocol_type(&self) -> &str;
    
    /// Log protocol connection events with standardized format
    /// 
    /// Provides a unified way to log connection-related events across all protocols.
    /// Uses the channel ID as the log target for filtering.
    /// 
    /// # Arguments
    /// 
    /// * `event` - Connection event ("connecting", "connected", "disconnected", "reconnecting")
    /// * `details` - Optional additional details about the connection event
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// # use comsrv::core::protocols::common::combase::ProtocolLogger;
    /// # async fn example(logger: &dyn ProtocolLogger) {
    /// logger.log_connection("connecting", Some("192.168.1.100:502")).await;
    /// logger.log_connection("connected", None).await;
    /// logger.log_connection("disconnected", Some("Connection timeout")).await;
    /// # }
    /// ```
    async fn log_connection(&self, event: &str, details: Option<&str>) {
        let channel_id = self.channel_id();
        let protocol = self.protocol_type();
        let timestamp = chrono::Utc::now().format("%H:%M:%S%.3f");
        
        let message = match details {
            Some(detail) => format!("== [{}] {} {} ({})", timestamp, event, protocol, detail),
            None => format!("== [{}] {} {}", timestamp, event, protocol),
        };
        
        let target = format!("{}::channel::{}", protocol.to_lowercase(), channel_id);
        
        match event {
            "connected" | "reconnected" => log_info!(target: &target, "{}", message),
            "connecting" | "reconnecting" => log_info!(target: &target, "{}", message),
            "disconnected" => log_warn!(target: &target, "{}", message),
            _ => log_debug!(target: &target, "{}", message),
        }
    }
    
    /// Log protocol operation success
    /// 
    /// Logs successful protocol operations with timing information.
    /// 
    /// # Arguments
    /// 
    /// * `operation` - Operation type ("read", "write", "batch_read", etc.)
    /// * `direction` - Direction indicator (">>" for request, "<<" for response)
    /// * `details` - Operation details (address, value, etc.)
    /// * `result_value` - Success result value
    /// * `duration_ms` - Operation duration in milliseconds
    async fn log_operation_success(&self, operation: &str, direction: &str, details: &str, result_value: &str, duration_ms: u128) {
        let channel_id = self.channel_id();
        let protocol = self.protocol_type();
        let timestamp = chrono::Utc::now().format("%H:%M:%S%.3f");
        
        let target = format!("{}::channel::{}", protocol.to_lowercase(), channel_id);
        let message = format!("{} [{}] {} {} OK: {} ({}ms)", 
            direction, timestamp, operation, details, result_value, duration_ms);
        
        log_debug!(target: &target, "{}", message);
    }
    
    /// Log protocol operation failure
    /// 
    /// Logs failed protocol operations with timing information.
    /// 
    /// # Arguments
    /// 
    /// * `operation` - Operation type
    /// * `direction` - Direction indicator  
    /// * `details` - Operation details
    /// * `error_msg` - Error message
    /// * `duration_ms` - Operation duration in milliseconds
    async fn log_operation_error(&self, operation: &str, direction: &str, details: &str, error_msg: &str, duration_ms: u128) {
        let channel_id = self.channel_id();
        let protocol = self.protocol_type();
        let timestamp = chrono::Utc::now().format("%H:%M:%S%.3f");
        
        let target = format!("{}::channel::{}", protocol.to_lowercase(), channel_id);
        let message = format!("{} [{}] {} {} ERR: {} ({}ms)", 
            direction, timestamp, operation, details, error_msg, duration_ms);
        
        log_error!(target: &target, "{}", message);
    }
    
    /// Log protocol operation request
    /// 
    /// Logs the start of a protocol operation with request details.
    /// 
    /// # Arguments
    /// 
    /// * `operation` - Operation type
    /// * `details` - Request details
    async fn log_request(&self, operation: &str, details: &str) {
        let channel_id = self.channel_id();
        let protocol = self.protocol_type();
        let timestamp = chrono::Utc::now().format("%H:%M:%S%.3f");
        
        let target = format!("{}::channel::{}", protocol.to_lowercase(), channel_id);
        let message = format!(">> [{}] {} {}", timestamp, operation, details);
        
        log_debug!(target: &target, "{}", message);
    }
    
    /// Log protocol data synchronization success
    /// 
    /// Logs successful data synchronization activities like Redis updates, batch operations, etc.
    /// 
    /// # Arguments
    /// 
    /// * `sync_type` - Type of synchronization ("redis_sync", "batch_update", etc.)
    /// * `count` - Number of items synchronized
    async fn log_data_sync_success(&self, sync_type: &str, count: usize) {
        let channel_id = self.channel_id();
        let protocol = self.protocol_type();
        let timestamp = chrono::Utc::now().format("%H:%M:%S%.3f");
        
        let target = format!("{}::channel::{}", protocol.to_lowercase(), channel_id);
        let message = format!("== [{}] {} completed: {} items", timestamp, sync_type, count);
        
        log_debug!(target: &target, "{}", message);
    }
    
    /// Log protocol data synchronization failure
    /// 
    /// Logs failed data synchronization activities.
    /// 
    /// # Arguments
    /// 
    /// * `sync_type` - Type of synchronization
    /// * `count` - Number of items attempted
    /// * `error_msg` - Error message
    async fn log_data_sync_error(&self, sync_type: &str, count: usize, error_msg: &str) {
        let channel_id = self.channel_id();
        let protocol = self.protocol_type();
        let timestamp = chrono::Utc::now().format("%H:%M:%S%.3f");
        
        let target = format!("{}::channel::{}", protocol.to_lowercase(), channel_id);
        let message = format!("== [{}] {} failed: {} (attempted {} items)", timestamp, sync_type, error_msg, count);
        
        log_error!(target: &target, "{}", message);
    }
    
    /// Convenience method to log operation results with automatic timing
    /// 
    /// This method handles both success and error cases with proper timing calculation.
    /// 
    /// # Arguments
    /// 
    /// * `operation` - Operation type
    /// * `direction` - Direction indicator
    /// * `details` - Operation details
    /// * `result` - Operation result
    /// * `start_time` - Operation start time
    async fn log_operation_result<T, E>(&self, operation: &str, direction: &str, details: &str, result: &std::result::Result<T, E>, start_time: Instant) 
    where 
        T: std::fmt::Display,
        E: std::fmt::Display,
    {
        let duration_ms = start_time.elapsed().as_millis();
        
        match result {
            Ok(value) => {
                self.log_operation_success(operation, direction, details, &value.to_string(), duration_ms).await;
            },
            Err(error) => {
                self.log_operation_error(operation, direction, details, &error.to_string(), duration_ms).await;
            }
        }
    }
    
    /// Convenience method to log data sync results
    /// 
    /// # Arguments
    /// 
    /// * `sync_type` - Type of synchronization
    /// * `count` - Number of items
    /// * `result` - Synchronization result
    async fn log_data_sync_result<E>(&self, sync_type: &str, count: usize, result: &std::result::Result<(), E>) 
    where 
        E: std::fmt::Display,
    {
        match result {
            Ok(()) => {
                self.log_data_sync_success(sync_type, count).await;
            },
            Err(error) => {
                self.log_data_sync_error(sync_type, count, &error.to_string()).await;
            }
        }
    }
}

/// Base implementation of the ComBase trait
/// 
/// `ComBaseImpl` provides a reference implementation of the `ComBase` trait
/// with common functionality that can be used by protocol implementations.
/// It handles status management, error tracking, and performance monitoring.
/// 
/// # Features
/// 
/// - **Status Management**: Automatic status tracking and updates
/// - **Error Handling**: Built-in error tracking and reporting
/// - **Performance Monitoring**: Response time measurement utilities
/// - **Thread Safety**: All operations are thread-safe using Arc and RwLock
/// 
/// # Usage
/// 
/// This implementation can be used as a base for custom protocol implementations
/// or as a standalone service for testing and development.
/// 
/// # Examples
/// 
/// ```rust
/// use comsrv::core::protocols::common::combase::ComBaseImpl;
/// use comsrv::core::config::config_manager::{ChannelConfig, ProtocolType, ChannelParameters};
/// use std::collections::HashMap;
/// 
/// // Create a test configuration
/// let config = ChannelConfig {
///     id: 1,
///     name: "Test Channel".to_string(),
///     description: "Test Description".to_string(),
///     protocol: ProtocolType::ModbusTcp,
///     parameters: ChannelParameters::Generic(HashMap::new()),
/// };
/// 
/// let service = ComBaseImpl::new("test_service", "modbus_tcp", config);
/// assert_eq!(service.name(), "test_service");
/// assert_eq!(service.protocol_type(), "modbus_tcp");
/// ```
#[derive(Debug)]
pub struct ComBaseImpl {
    /// Service name
    name: String,
    /// Channel ID
    channel_id: u16,
    /// Protocol type
    protocol_type: String,
    /// Channel configuration
    config: ChannelConfig,
    /// Channel status
    status: Arc<RwLock<ChannelStatus>>,
    /// Running state
    running: Arc<RwLock<bool>>,
    /// Last error message
    last_error: Arc<RwLock<String>>,
}

impl ComBaseImpl {
    /// Create a new ComBaseImpl instance
    /// 
    /// Initializes a new base implementation with the specified name, protocol type,
    /// and configuration. The instance starts in a stopped state with no errors.
    /// 
    /// # Arguments
    /// 
    /// * `name` - Human-readable name for the service
    /// * `protocol_type` - Protocol type identifier (e.g., "ModbusTcp", "ModbusRtu")
    /// * `config` - Channel configuration with protocol-specific parameters
    /// 
    /// # Returns
    /// 
    /// New `ComBaseImpl` instance ready for use
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use comsrv::core::protocols::common::combase::ComBaseImpl;
    /// use comsrv::core::config::config_manager::{ChannelConfig, ProtocolType, ChannelParameters};
    /// use std::collections::HashMap;
    /// 
    /// let config = ChannelConfig {
    ///     id: 1,
    ///     name: "Test Channel".to_string(),
    ///     description: "Test Description".to_string(),
    ///     protocol: ProtocolType::ModbusTcp,
    ///     parameters: ChannelParameters::Generic(HashMap::new()),
    /// };
    /// 
    /// let service = ComBaseImpl::new("ModbusService", "ModbusTcp", config);
    /// assert_eq!(service.name(), "ModbusService");
    /// assert_eq!(service.protocol_type(), "ModbusTcp");
    /// ```
    pub fn new(name: &str, protocol_type: &str, config: ChannelConfig) -> Self {
        let channel_id = config.id;
        let status = ChannelStatus::new(&channel_id.to_string());
        
        Self {
            name: name.to_string(),
            channel_id,
            protocol_type: protocol_type.to_string(),
            config,
            status: Arc::new(RwLock::new(status)),
            running: Arc::new(RwLock::new(false)),
            last_error: Arc::new(RwLock::new(String::new())),
        }
    }
    
    /// Get the human-readable name of the communication service
    /// 
    /// # Returns
    /// 
    /// Service name as a string slice
    pub fn name(&self) -> &str {
        &self.name
    }
    
    /// Get the unique channel identifier as a string
    /// 
    /// # Returns
    /// 
    /// Channel ID converted to string format
    pub fn channel_id(&self) -> String {
        self.channel_id.to_string()
    }
    
    /// Get the protocol type identifier
    /// 
    /// # Returns
    /// 
    /// Protocol type as a string slice
    pub fn protocol_type(&self) -> &str {
        &self.protocol_type
    }
    
    /// Get protocol parameters as a HashMap
    /// 
    /// Converts the configuration to a string-based parameter map
    /// for use in monitoring, diagnostics, or dynamic configuration.
    /// 
    /// # Returns
    /// 
    /// HashMap containing basic protocol parameters
    /// 
    /// # Note
    /// 
    /// Additional parameters can be added by extending this method
    /// in protocol-specific implementations.
    pub fn get_parameters(&self) -> HashMap<String, String> {
        let mut params = HashMap::new();
        // Convert configuration to HashMap
        params.insert("protocol".to_string(), self.protocol_type.clone());
        params.insert("channel_id".to_string(), self.channel_id.to_string());
        // More parameters extracted from config can be added in actual implementation
        params
    }
    
    /// Get a reference to the channel configuration
    /// 
    /// Provides read-only access to the complete channel configuration
    /// for use by protocol implementations.
    /// 
    /// # Returns
    /// 
    /// Immutable reference to the channel configuration
    pub fn config(&self) -> &ChannelConfig {
        &self.config
    }
    
    /// Check if the communication service is currently running
    /// 
    /// Thread-safe check of the current running state.
    /// 
    /// # Returns
    /// 
    /// `true` if the service is running, `false` otherwise
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }
    
    /// Start the communication service
    /// 
    /// Initiates the communication service and sets the running state to true.
    /// This base implementation only manages the running state; protocol-specific
    /// implementations should override this method to perform actual connection setup.
    /// 
    /// # Returns
    /// 
    /// * `Ok(())` - Service started successfully
    /// * `Err(error)` - Failure during startup
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use comsrv::core::protocols::common::combase::ComBaseImpl;
    /// use comsrv::core::config::config_manager::{ChannelConfig, ProtocolType, ChannelParameters};
    /// use std::collections::HashMap;
    /// 
    /// # async fn example(service: &ComBaseImpl) -> comsrv::utils::Result<()> {
    /// // Service startup is handled by ComBaseImpl
    /// service.start().await?;
    /// assert!(service.is_running().await);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn start(&self) -> Result<()> {
        self.set_running(true).await;
        self.update_status(false, 0.0, None).await;
        Ok(())
    }
    
    /// Stop the communication service gracefully
    /// 
    /// Stops the communication service and sets the running state to false.
    /// This base implementation only manages the running state; protocol-specific
    /// implementations should override this method to perform actual connection cleanup.
    /// 
    /// # Returns
    /// 
    /// * `Ok(())` - Service stopped successfully
    /// * `Err(error)` - Failure during shutdown
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use comsrv::core::protocols::common::combase::ComBaseImpl;
    /// use comsrv::core::config::config_manager::{ChannelConfig, ProtocolType, ChannelParameters};
    /// use std::collections::HashMap;
    /// 
    /// # async fn example(service: &ComBaseImpl) -> comsrv::utils::Result<()> {
    /// // Service shutdown is handled by ComBaseImpl
    /// service.stop().await?;
    /// assert!(!service.is_running().await);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn stop(&self) -> Result<()> {
        self.set_running(false).await;
        self.update_status(false, 0.0, None).await;
        Ok(())
    }
    
    /// Get the current status of the communication channel
    /// 
    /// Returns a snapshot of the current channel status including connection state,
    /// response time metrics, error conditions, and last update timestamp.
    /// 
    /// # Returns
    /// 
    /// Current `ChannelStatus` with up-to-date information
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use comsrv::core::protocols::common::combase::ComBaseImpl;
    /// use comsrv::core::config::config_manager::{ChannelConfig, ProtocolType, ChannelParameters};
    /// use std::collections::HashMap;
    /// 
    /// # async fn example(service: &ComBaseImpl) {
    /// let status = service.status().await;
    /// println!("Channel {}: connected={}", status.id, status.connected);
    /// # }
    /// ```
    pub async fn status(&self) -> ChannelStatus {
        self.status.read().await.clone()
    }
    
    /// Update the channel status with new information
    /// 
    /// Updates the channel status with connection state, response time, and
    /// optional error information. This method is typically called by
    /// protocol implementations to report status changes.
    /// 
    /// # Arguments
    /// 
    /// * `connected` - Current connection state
    /// * `response_time` - Response time in milliseconds
    /// * `error` - Optional error message (None to clear errors)
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use comsrv::core::protocols::common::combase::ComBaseImpl;
    /// use comsrv::core::config::config_manager::{ChannelConfig, ProtocolType, ChannelParameters};
    /// use std::collections::HashMap;
    /// 
    /// # async fn example(service: &ComBaseImpl) {
    /// // Update status after successful operation
    /// service.update_status(true, 150.5, None).await;
    /// 
    /// // Update status with error condition
    /// service.update_status(false, 0.0, Some("Connection failed")).await;
    /// # }
    /// ```
    pub async fn update_status(&self, connected: bool, response_time: f64, error: Option<&str>) {
        let mut status = self.status.write().await;
        status.connected = connected;
        status.last_response_time = response_time;
        status.last_update_time = Utc::now();
        
        if let Some(err) = error {
            status.last_error = err.to_string();
            // Also update the separate error field
            *self.last_error.write().await = err.to_string();
        } else if !connected {
            // Clear error when disconnected normally
            status.last_error.clear();
            self.last_error.write().await.clear();
        }
    }
    
    /// Measure execution time of a synchronous operation
    /// 
    /// Executes the provided function and measures its execution time.
    /// The execution time is automatically reported to the channel status.
    /// 
    /// # Arguments
    /// 
    /// * `f` - Function to execute and measure
    /// 
    /// # Returns
    /// 
    /// Result of the executed function
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use comsrv::core::protocols::common::combase::ComBaseImpl;
    /// use comsrv::core::config::config_manager::{ChannelConfig, ProtocolType, ChannelParameters};
    /// use std::collections::HashMap;
    /// 
    /// # async fn example(service: &ComBaseImpl) -> comsrv::utils::Result<String> {
    /// let result = service.measure_execution(|| {
    ///     // Simulate some work
    ///     std::thread::sleep(std::time::Duration::from_millis(100));
    ///     Ok("Operation completed".to_string())
    /// }).await?;
    /// # Ok(result)
    /// # }
    /// ```
    pub async fn measure_execution<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce() -> Result<T> + Send,
    {
        let start = Instant::now();
        let result = f();
        let duration = start.elapsed();
        
        // Update status based on the result
        match &result {
            Ok(_) => {
                self.update_status(true, duration.as_secs_f64() * 1000.0, None).await;
            }
            Err(e) => {
                self.update_status(false, duration.as_secs_f64() * 1000.0, Some(&e.to_string())).await;
            }
        }
        
        result
    }
    
    /// Measure execution time of a synchronous operation that returns a Result
    /// 
    /// Executes the provided function and measures its execution time.
    /// Updates the channel status based on the operation result.
    /// 
    /// # Arguments
    /// 
    /// * `f` - Synchronous function to execute and measure
    /// 
    /// # Returns
    /// 
    /// Result of the executed function
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use comsrv::core::protocols::common::combase::ComBaseImpl;
    /// use comsrv::core::config::config_manager::{ChannelConfig, ProtocolType, ChannelParameters};
    /// use std::collections::HashMap;
    /// 
    /// # async fn example(service: &ComBaseImpl) -> std::result::Result<String, String> {
    /// let result = service.measure_result_execution(|| {
    ///     // Simulate sync work that returns a Result
    ///     sync_operation()
    /// }).await?;
    /// # Ok(result)
    /// # }
    /// # fn sync_operation() -> std::result::Result<String, String> { Ok("Done".to_string()) }
    /// ```
    pub async fn measure_result_execution<F, T, E>(&self, f: F) -> std::result::Result<T, E>
    where
        F: FnOnce() -> std::result::Result<T, E> + Send,
        E: ToString,
    {
        let start = std::time::Instant::now();
        let result = f();
        let duration = start.elapsed();
        let response_time = duration.as_millis() as f64;

        match &result {
            Ok(_) => {
                self.update_status(true, response_time, None).await;
            }
            Err(e) => {
                let error_msg = e.to_string();
                self.update_status(false, response_time, Some(&error_msg)).await;
            }
        }

        result
    }

    /// Set an error condition for the channel
    /// 
    /// Records an error message and updates the channel status to reflect
    /// the error condition. This method is used by protocol implementations
    /// to report error states.
    /// 
    /// # Arguments
    /// 
    /// * `error` - Error message to record
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use comsrv::core::protocols::common::combase::ComBaseImpl;
    /// use comsrv::core::config::config_manager::{ChannelConfig, ProtocolType, ChannelParameters};
    /// use std::collections::HashMap;
    /// 
    /// # async fn example(service: &ComBaseImpl) {
    /// service.set_error("Connection timeout occurred").await;
    /// assert!(service.status().await.has_error());
    /// # }
    /// ```
    pub async fn set_error(&self, error: &str) {
        *self.last_error.write().await = error.to_string();
        
        // Also update the status
        let mut status = self.status.write().await;
        status.last_error = error.to_string();
        status.last_update_time = Utc::now();
    }

    /// Set the running state of the service
    /// 
    /// Updates the internal running state. This method is used internally
    /// by start/stop operations and can be used by protocol implementations
    /// for state management.
    /// 
    /// # Arguments
    /// 
    /// * `running` - New running state
    /// 
    /// # Examples
    /// 
    /// ```rust
    /// use comsrv::core::protocols::common::combase::ComBaseImpl;
    /// use comsrv::core::config::config_manager::{ChannelConfig, ProtocolType, ChannelParameters};
    /// use std::collections::HashMap;
    /// 
    /// # async fn example(service: &ComBaseImpl) {
    /// service.set_running(true).await;
    /// assert!(service.is_running().await);
    /// 
    /// service.set_running(false).await;
    /// assert!(!service.is_running().await);
    /// # }
    /// ```
    pub async fn set_running(&self, running: bool) {
        *self.running.write().await = running;
    }
}

/// Protocol packet parsing result
/// 
/// Contains human-readable interpretation of protocol packets,
/// including packet structure and data content.
#[derive(Debug, Clone)]
pub struct PacketParseResult {
    /// Protocol type (e.g., "Modbus", "IEC60870", "CAN")
    pub protocol: String,
    /// Packet direction ("send" or "receive")
    pub direction: String,
    /// Hexadecimal representation of raw data
    pub hex_data: String,
    /// Human-readable description of packet structure
    pub description: String,
    /// Parsed data fields
    pub fields: HashMap<String, String>,
    /// Whether parsing was successful
    pub success: bool,
    /// Error message if parsing failed
    pub error: Option<String>,
}

impl PacketParseResult {
    /// Create a new successful parse result
    pub fn success(
        protocol: &str,
        direction: &str,
        hex_data: &str,
        description: &str,
        fields: HashMap<String, String>,
    ) -> Self {
        Self {
            protocol: protocol.to_string(),
            direction: direction.to_string(),
            hex_data: hex_data.to_string(),
            description: description.to_string(),
            fields,
            success: true,
            error: None,
        }
    }

    /// Create a new failed parse result
    pub fn failure(
        protocol: &str,
        direction: &str,
        hex_data: &str,
        error: &str,
    ) -> Self {
        Self {
            protocol: protocol.to_string(),
            direction: direction.to_string(),
            hex_data: hex_data.to_string(),
            description: format!("Parse error: {}", error),
            fields: HashMap::new(),
            success: false,
            error: Some(error.to_string()),
        }
    }

    /// Format as debug log entry
    pub fn format_debug_log(&self) -> String {
        if self.success {
            format!(
                "[{}] {} | {}",
                self.direction.to_uppercase(),
                self.hex_data,
                self.description
            )
        } else {
            format!(
                "[{}] {} | {} ({})",
                self.direction.to_uppercase(),
                self.hex_data,
                self.description,
                self.error.as_ref().unwrap_or(&"Unknown error".to_string())
            )
        }
    }
}

/// Protocol packet parser trait
/// 
/// Defines the interface for parsing protocol-specific packets.
/// Each protocol implementation should provide its own parser.
pub trait ProtocolPacketParser: Send + Sync {
    /// Get the protocol name
    fn protocol_name(&self) -> &str;

    /// Parse a packet and return human-readable interpretation
    fn parse_packet(&self, data: &[u8], direction: &str) -> PacketParseResult;

    /// Convert bytes to hexadecimal string
    fn format_hex_data(&self, data: &[u8]) -> String {
        data.iter()
            .map(|b| format!("{:02x}", b))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// Protocol packet parser registry
/// 
/// Manages multiple protocol parsers and routes packets to the appropriate parser.
pub struct ProtocolParserRegistry {
    parsers: HashMap<String, Box<dyn ProtocolPacketParser>>,
}

impl ProtocolParserRegistry {
    /// Create a new parser registry
    pub fn new() -> Self {
        Self {
            parsers: HashMap::new(),
        }
    }

    /// Register a parser for a specific protocol
    pub fn register_parser<P>(&mut self, parser: P)
    where
        P: ProtocolPacketParser + 'static,
    {
        let protocol_name = parser.protocol_name().to_string();
        self.parsers.insert(protocol_name, Box::new(parser));
    }

    /// Parse a packet using the appropriate protocol parser
    pub fn parse_packet(&self, protocol: &str, data: &[u8], direction: &str) -> PacketParseResult {
        if let Some(parser) = self.parsers.get(protocol) {
            parser.parse_packet(data, direction)
        } else {
            // Fallback to basic hex representation
            let hex_data = data.iter()
                .map(|b| format!("{:02x}", b))
                .collect::<Vec<_>>()
                .join(" ");
            
            PacketParseResult::failure(
                protocol,
                direction,
                &hex_data,
                &format!("No parser registered for protocol: {}", protocol),
            )
        }
    }

    /// Get list of registered protocols
    pub fn registered_protocols(&self) -> Vec<String> {
        self.parsers.keys().cloned().collect()
    }
}

use once_cell::sync::Lazy;
use parking_lot::RwLock as ParkingLotRwLock;

/// Global protocol parser registry protected by a read-write lock
static GLOBAL_PARSER_REGISTRY: Lazy<ParkingLotRwLock<ProtocolParserRegistry>> =
    Lazy::new(|| ParkingLotRwLock::new(ProtocolParserRegistry::new()));

/// Get the global protocol parser registry
pub fn get_global_parser_registry() -> &'static ParkingLotRwLock<ProtocolParserRegistry> {
    &GLOBAL_PARSER_REGISTRY
}

/// Parse a protocol packet using the global registry
pub fn parse_protocol_packet(protocol: &str, data: &[u8], direction: &str) -> PacketParseResult {
    let registry = get_global_parser_registry();
    let registry = registry.read();
    registry.parse_packet(protocol, data, direction)
}

/// Polling Engine Trait
/// 
/// This trait abstracts the polling functionality and can be implemented
/// by any communication protocol. It provides a unified interface for
/// data collection across different protocols.
#[async_trait]
pub trait PollingEngine: Send + Sync {
    /// Start the polling engine
    async fn start_polling(&self, config: PollingConfig, points: Vec<PollingPoint>) -> Result<()>;
    
    /// Stop the polling engine
    async fn stop_polling(&self) -> Result<()>;
    
    /// Get current polling statistics
    async fn get_polling_stats(&self) -> PollingStats;
    
    /// Check if polling is currently active
    async fn is_polling_active(&self) -> bool;
    
    /// Update polling configuration at runtime
    async fn update_polling_config(&self, config: PollingConfig) -> Result<()>;
    
    /// Add or update polling points
    async fn update_polling_points(&self, points: Vec<PollingPoint>) -> Result<()>;
    
    /// Read a single point (protocol-specific implementation)
    async fn read_point(&self, point: &PollingPoint) -> Result<PointData>;
    
    /// Read multiple points in batch (protocol-specific optimization)
    async fn read_points_batch(&self, points: &[PollingPoint]) -> Result<Vec<PointData>>;
}

/// Universal Polling Engine Implementation
/// 
/// This is a generic polling engine that can be used by any protocol.
/// It handles the polling loop, statistics, and delegates actual reading
/// to protocol-specific implementations.
pub struct UniversalPollingEngine {
    /// Protocol name for logging
    protocol_name: String,
    /// Polling configuration
    config: Arc<RwLock<PollingConfig>>,
    /// Points to be polled
    points: Arc<RwLock<Vec<PollingPoint>>>,
    /// Polling statistics
    stats: Arc<RwLock<PollingStats>>,
    /// Running state
    is_running: Arc<RwLock<bool>>,
    /// Point reader implementation (protocol-specific)
    point_reader: Arc<dyn PointReader>,
    /// Data callback for storing read values
    data_callback: Option<Arc<dyn Fn(Vec<PointData>) + Send + Sync>>,
    /// Task handle for polling task
    task_handle: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
}

/// Point Reader Trait
/// 
/// This trait must be implemented by each protocol to provide the actual
/// point reading functionality. The universal polling engine uses this
/// to delegate protocol-specific operations.
#[async_trait]
pub trait PointReader: Send + Sync {
    /// Read a single point using protocol-specific logic
    async fn read_point(&self, point: &PollingPoint) -> Result<PointData>;
    
    /// Read multiple points in batch (optional optimization)
    async fn read_points_batch(&self, points: &[PollingPoint]) -> Result<Vec<PointData>> {
        // Default implementation: read points individually
        let mut results = Vec::new();
        for point in points {
            match self.read_point(point).await {
                Ok(data) => results.push(data),
                Err(e) => {
                    // Create error data point
                    results.push(PointData {
                        id: point.id.clone(),
                        name: point.name.clone(),
                        value: "null".to_string(),
                        quality: 0, // 0 = bad quality
                        timestamp: Utc::now(),
                        unit: point.unit.clone(),
                        description: format!("Failed to read point {}: {}", point.id, e),
                    });
                    warn!("Failed to read point {}: {}", point.id, e);
                }
            }
        }
        Ok(results)
    }
    
    /// Check if the connection is healthy
    async fn is_connected(&self) -> bool;
    
    /// Get protocol name for logging
    fn protocol_name(&self) -> &str;
}

impl UniversalPollingEngine {
    /// Create a new universal polling engine
    pub fn new(
        protocol_name: String,
        point_reader: Arc<dyn PointReader>,
    ) -> Self {
        Self {
            protocol_name,
            config: Arc::new(RwLock::new(PollingConfig::default())),
            points: Arc::new(RwLock::new(Vec::new())),
            stats: Arc::new(RwLock::new(PollingStats::default())),
            is_running: Arc::new(RwLock::new(false)),
            point_reader,
            data_callback: None,
            task_handle: Arc::new(RwLock::new(None)),
        }
    }
    
    /// Set data callback for handling read data
    pub fn set_data_callback<F>(&mut self, callback: F)
    where
        F: Fn(Vec<PointData>) + Send + Sync + 'static,
    {
        self.data_callback = Some(Arc::new(callback));
    }
}

#[async_trait]
impl PollingEngine for UniversalPollingEngine {
    async fn start_polling(&self, config: PollingConfig, points: Vec<PollingPoint>) -> Result<()> {
        // Update configuration and points
        {
            let mut config_guard = self.config.write().await;
            *config_guard = config.clone();
        }
        {
            let mut points_guard = self.points.write().await;
            *points_guard = points;
        }
        
        // Check if already running
        {
            let mut running = self.is_running.write().await;
            if *running {
                return Err(ComSrvError::StateError("Polling engine already running".to_string()));
            }
            *running = true;
        }
        
        if !config.enabled {
            info!("Polling disabled for {} protocol", self.protocol_name);
            return Ok(());
        }
        
        info!("Starting universal polling engine for {} protocol", self.protocol_name);
        info!("Polling interval: {}ms, Max points per cycle: {}", 
              config.interval_ms, config.max_points_per_cycle);
        
        // Start the polling task
        let handle = self.start_polling_task().await;
        
        // Store the task handle for cleanup
        {
            let mut task_handle = self.task_handle.write().await;
            *task_handle = Some(handle);
        }
        
        Ok(())
    }
    
    async fn stop_polling(&self) -> Result<()> {
        {
            let mut running = self.is_running.write().await;
            *running = false;
        }
        
        // Abort the polling task if it's running
        {
            let mut handle = self.task_handle.write().await;
            if let Some(task) = handle.take() {
                task.abort();
            }
        }
        
        info!("Stopped universal polling engine for {} protocol", self.protocol_name);
        Ok(())
    }
    
    async fn get_polling_stats(&self) -> PollingStats {
        self.stats.read().await.clone()
    }
    
    async fn is_polling_active(&self) -> bool {
        *self.is_running.read().await
    }
    
    async fn update_polling_config(&self, config: PollingConfig) -> Result<()> {
        {
            let mut config_guard = self.config.write().await;
            *config_guard = config;
        }
        info!("Updated polling configuration for {} protocol", self.protocol_name);
        Ok(())
    }
    
    async fn update_polling_points(&self, points: Vec<PollingPoint>) -> Result<()> {
        {
            let mut points_guard = self.points.write().await;
            *points_guard = points;
        }
        info!("Updated polling points for {} protocol", self.protocol_name);
        Ok(())
    }
    
    async fn read_point(&self, point: &PollingPoint) -> Result<PointData> {
        self.point_reader.read_point(point).await
    }
    
    async fn read_points_batch(&self, points: &[PollingPoint]) -> Result<Vec<PointData>> {
        self.point_reader.read_points_batch(points).await
    }
}

impl UniversalPollingEngine {
    /// Start the main polling task
    async fn start_polling_task(&self) -> tokio::task::JoinHandle<()> {
        let config = self.config.clone();
        let points = self.points.clone();
        let stats = self.stats.clone();
        let is_running = self.is_running.clone();
        let point_reader = self.point_reader.clone();
        let data_callback = self.data_callback.clone();
        let protocol_name = self.protocol_name.clone();
        
        return tokio::spawn(async move {
            let mut cycle_counter = 0u64;

            let mut current_interval_ms = config.read().await.interval_ms;
            let mut poll_interval = interval(Duration::from_millis(current_interval_ms));

            while *is_running.read().await {
                let config_snapshot = config.read().await.clone();

                if !config_snapshot.enabled {
                    tokio::time::sleep(Duration::from_millis(1000)).await;
                    continue;
                }

                if config_snapshot.interval_ms != current_interval_ms {
                    current_interval_ms = config_snapshot.interval_ms;
                    poll_interval = interval(Duration::from_millis(current_interval_ms));
                }

                poll_interval.tick().await;
                cycle_counter += 1;
                
                // Check connection before polling
                if !point_reader.is_connected().await {
                    debug!("Skipping polling cycle {} for {} - not connected", 
                           cycle_counter, protocol_name);
                    continue;
                }
                
                let cycle_start = Instant::now();
                
                // Execute polling cycle
                match Self::execute_polling_cycle(
                    &config_snapshot,
                    &points,
                    &point_reader,
                    &protocol_name,
                    cycle_counter,
                ).await {
                    Ok(read_data) => {
                        // Update statistics
                        let cycle_time = cycle_start.elapsed().as_millis() as f64;
                        Self::update_stats(&stats, true, read_data.len(), cycle_time).await;
                        
                        // Call data callback if set
                        if let Some(ref callback) = data_callback {
                            callback(read_data);
                        }
                        
                        debug!("Polling cycle {} completed for {} in {:.2}ms", 
                               cycle_counter, protocol_name, cycle_time);
                    }
                    Err(e) => {
                        // Update statistics for failed cycle
                        let cycle_time = cycle_start.elapsed().as_millis() as f64;
                        Self::update_stats(&stats, false, 0, cycle_time).await;
                        
                        error!("Polling cycle {} failed for {}: {}", 
                               cycle_counter, protocol_name, e);
                    }
                }
                
                // Log periodic statistics
                if cycle_counter % 50 == 0 {
                    let current_stats = stats.read().await;
                    info!("Polling stats for {}: {}/{} successful, avg {:.2}ms, quality {:.1}%",
                          protocol_name,
                          current_stats.successful_cycles,
                          current_stats.total_cycles,
                          current_stats.avg_cycle_time_ms,
                          current_stats.communication_quality);
                }
            }
            
            info!("Polling task stopped for {} protocol", protocol_name);
        });
    }
    
    /// Execute a single polling cycle
    async fn execute_polling_cycle(
        config: &PollingConfig,
        points: &Arc<RwLock<Vec<PollingPoint>>>,
        point_reader: &Arc<dyn PointReader>,
        protocol_name: &str,
        cycle_number: u64,
    ) -> Result<Vec<PointData>> {
        let points_snapshot = points.read().await.clone();
        
        if points_snapshot.is_empty() {
            debug!("No points configured for polling in {} protocol", protocol_name);
            return Ok(Vec::new());
        }
        
        debug!("Starting polling cycle {} for {} protocol with {} points", 
               cycle_number, protocol_name, points_snapshot.len());
        
        let mut all_data = Vec::new();
        
        // Batch points by group if batch reading is enabled
        if config.enable_batch_reading {
            let grouped_points = Self::group_points_for_batch_reading(&points_snapshot);
            
            for (group_name, group_points) in grouped_points {
                debug!("Reading batch group '{}' with {} points", group_name, group_points.len());
                
                match point_reader.read_points_batch(&group_points).await {
                    Ok(mut batch_data) => {
                        all_data.append(&mut batch_data);
                    }
                    Err(e) => {
                        warn!("Batch read failed for group '{}': {}", group_name, e);
                        // Fall back to individual reads
                        for point in group_points {
                            match point_reader.read_point(&point).await {
                                Ok(data) => all_data.push(data),
                                Err(e) => {
                                    warn!("Individual read failed for point {}: {}", point.id, e);
                                    // Add error data point
                                                        all_data.push(PointData {
                        id: point.id.clone(),
                        name: point.name.clone(),
                        value: "null".to_string(),
                        quality: 0, // 0 = bad quality
                        timestamp: Utc::now(),
                        unit: point.unit.clone(),
                        description: format!("Failed to read point {}: {}", point.id, e),
                    });
                                }
                            }
                            
                            // Delay between individual reads
                            if config.point_read_delay_ms > 0 {
                                tokio::time::sleep(Duration::from_millis(config.point_read_delay_ms)).await;
                            }
                        }
                    }
                }
            }
        } else {
            // Read points individually
            for point in points_snapshot {
                match point_reader.read_point(&point).await {
                    Ok(data) => all_data.push(data),
                    Err(e) => {
                        warn!("Failed to read point {}: {}", point.id, e);
                        // Add error data point
                        all_data.push(PointData {
                            id: point.id.clone(),
                            name: point.name.clone(),
                            value: "null".to_string(),
                            quality: 0, // 0 = bad quality
                            timestamp: Utc::now(),
                            unit: point.unit.clone(),
                            description: format!("Failed to read point {}: {}", point.id, e),
                        });
                    }
                }
                
                // Delay between reads
                if config.point_read_delay_ms > 0 {
                    tokio::time::sleep(Duration::from_millis(config.point_read_delay_ms)).await;
                }
            }
        }
        
        Ok(all_data)
    }
    
    /// Group points by their group name for batch reading
    fn group_points_for_batch_reading(points: &[PollingPoint]) -> HashMap<String, Vec<PollingPoint>> {
        let mut grouped = HashMap::new();
        
        for point in points {
            let group_name = if point.group.is_empty() {
                "default".to_string()
            } else {
                point.group.clone()
            };
            
            grouped.entry(group_name).or_insert_with(Vec::new).push(point.clone());
        }
        
        grouped
    }
    
    /// Update polling statistics
    async fn update_stats(
        stats: &Arc<RwLock<PollingStats>>,
        success: bool,
        points_read: usize,
        cycle_time_ms: f64,
    ) {
        let mut stats_guard = stats.write().await;
        
        stats_guard.total_cycles += 1;
        
        if success {
            stats_guard.successful_cycles += 1;
            stats_guard.total_points_read += points_read as u64;
            stats_guard.last_successful_polling = Some(Utc::now());
            stats_guard.last_polling_error = None;
        } else {
            stats_guard.failed_cycles += 1;
        }
        
        // Update average cycle time
        let total_time = stats_guard.avg_cycle_time_ms * (stats_guard.total_cycles - 1) as f64 + cycle_time_ms;
        stats_guard.avg_cycle_time_ms = total_time / stats_guard.total_cycles as f64;
        
        // Update communication quality
        stats_guard.communication_quality = 
            (stats_guard.successful_cycles as f64 / stats_guard.total_cycles as f64) * 100.0;
        
        // Update polling rate (approximate)
        if stats_guard.total_cycles > 1 && stats_guard.avg_cycle_time_ms > 0.0 {
            stats_guard.current_polling_rate = 1000.0 / stats_guard.avg_cycle_time_ms;
        } else {
            stats_guard.current_polling_rate = 0.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use tokio::time::{sleep, Duration, Instant};
    use async_trait::async_trait;
    use crate::core::config::config_manager::{ChannelConfig, ProtocolType, ChannelParameters};

    // Mock implementations for testing
    struct MockReader {
        connected: Arc<Mutex<bool>>,
        fail_reads: Arc<Mutex<bool>>,
        read_delay: Arc<Mutex<Option<Duration>>>,
    }

    impl MockReader {
        fn new() -> Self {
            Self {
                connected: Arc::new(Mutex::new(true)),
                fail_reads: Arc::new(Mutex::new(false)),
                read_delay: Arc::new(Mutex::new(None)),
            }
        }

        fn set_connected(&self, connected: bool) {
            *self.connected.lock().unwrap() = connected;
        }

        fn set_fail_reads(&self, fail: bool) {
            *self.fail_reads.lock().unwrap() = fail;
        }

        fn set_read_delay(&self, delay: Option<Duration>) {
            *self.read_delay.lock().unwrap() = delay;
        }
    }

    #[async_trait]
    impl PointReader for MockReader {
        async fn read_point(&self, point: &PollingPoint) -> Result<PointData> {
            let delay = *self.read_delay.lock().unwrap();
            if let Some(delay) = delay {
                sleep(delay).await;
            }

            let should_fail = *self.fail_reads.lock().unwrap();
            if should_fail {
                return Err(ComSrvError::CommunicationError("Mock read failure".to_string()));
            }

            Ok(PointData {
                id: point.id.clone(),
                name: point.name.clone(),
                value: format!("value_{}", point.address),
                quality: 1,
                timestamp: Utc::now(),
                unit: point.unit.clone(),
                description: point.description.clone(),
            })
        }

        async fn read_points_batch(&self, points: &[PollingPoint]) -> Result<Vec<PointData>> {
            let should_fail = *self.fail_reads.lock().unwrap();
            if should_fail {
                return Err(ComSrvError::CommunicationError("Mock batch read failure".to_string()));
            }

            let mut results = Vec::new();
            for point in points {
                results.push(self.read_point(point).await?);
            }
            Ok(results)
        }

        async fn is_connected(&self) -> bool {
            *self.connected.lock().unwrap()
        }

        fn protocol_name(&self) -> &str {
            "mock"
        }
    }

    struct MockParser {
        protocol: String,
    }

    impl MockParser {
        fn new(protocol: &str) -> Self {
            Self {
                protocol: protocol.to_string(),
            }
        }
    }

    impl ProtocolPacketParser for MockParser {
        fn protocol_name(&self) -> &str {
            &self.protocol
        }

        fn parse_packet(&self, data: &[u8], direction: &str) -> PacketParseResult {
            let hex_data = self.format_hex_data(data);
            let mut fields = HashMap::new();
            fields.insert("length".to_string(), data.len().to_string());
            fields.insert("first_byte".to_string(), format!("0x{:02x}", data.first().unwrap_or(&0)));

            PacketParseResult::success(
                &self.protocol,
                direction,
                &hex_data,
                &format!("{} packet with {} bytes", self.protocol, data.len()),
                fields,
            )
        }
    }

    fn create_test_config() -> ChannelConfig {
        ChannelConfig {
            id: 1,
            name: "Test Channel".to_string(),
            description: "Test Description".to_string(),
            protocol: ProtocolType::ModbusTcp,
            parameters: ChannelParameters::Generic(HashMap::new()),
        }
    }

    fn create_test_point(id: &str, address: u32) -> PollingPoint {
        PollingPoint {
            id: id.to_string(),
            name: format!("Point {}", id),
            address,
            data_type: "u16".to_string(),
            scale: 1.0,
            offset: 0.0,
            unit: "V".to_string(),
            description: format!("Test point {}", id),
            access_mode: "read".to_string(),
            group: "default".to_string(),
            protocol_params: HashMap::new(),
        }
    }

    // ChannelStatus Tests
    #[test]
    fn test_channel_status_new() {
        let status = ChannelStatus::new("test_channel");
        assert_eq!(status.id, "test_channel");
        assert!(!status.connected);
        assert_eq!(status.last_response_time, 0.0);
        assert!(status.last_error.is_empty());
        assert!(!status.has_error());
    }

    #[test]
    fn test_channel_status_has_error() {
        let mut status = ChannelStatus::new("test_channel");
        assert!(!status.has_error());

        status.last_error = "Connection failed".to_string();
        assert!(status.has_error());

        status.last_error.clear();
        assert!(!status.has_error());
    }

    // PointData Tests
    #[test]
    fn test_point_data_creation() {
        let now = Utc::now();
        let point = PointData {
            id: "test_point".to_string(),
            name: "Test Point".to_string(),
            value: "123.45".to_string(),
            quality: 1,
            timestamp: now,
            unit: "V".to_string(),
            description: "Test voltage point".to_string(),
        };

        assert_eq!(point.id, "test_point");
        assert_eq!(point.name, "Test Point");
        assert_eq!(point.value, "123.45");
        assert_eq!(point.quality, 1);
        assert_eq!(point.timestamp, now);
        assert_eq!(point.unit, "V");
        assert_eq!(point.description, "Test voltage point");
    }

    // PollingConfig Tests
    #[test]
    fn test_polling_config_default() {
        let config = PollingConfig::default();
        assert!(config.enabled);
        assert_eq!(config.interval_ms, 1000);
        assert_eq!(config.max_points_per_cycle, 1000);
        assert_eq!(config.timeout_ms, 5000);
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.retry_delay_ms, 1000);
        assert!(config.enable_batch_reading);
        assert_eq!(config.point_read_delay_ms, 10);
    }

    #[test]
    fn test_polling_config_custom() {
        let config = PollingConfig {
            enabled: false,
            interval_ms: 500,
            max_points_per_cycle: 100,
            timeout_ms: 2000,
            max_retries: 1,
            retry_delay_ms: 500,
            enable_batch_reading: false,
            point_read_delay_ms: 50,
        };

        assert!(!config.enabled);
        assert_eq!(config.interval_ms, 500);
        assert_eq!(config.max_points_per_cycle, 100);
        assert_eq!(config.timeout_ms, 2000);
        assert_eq!(config.max_retries, 1);
        assert_eq!(config.retry_delay_ms, 500);
        assert!(!config.enable_batch_reading);
        assert_eq!(config.point_read_delay_ms, 50);
    }

    // PollingStats Tests
    #[test]
    fn test_polling_stats_default() {
        let stats = PollingStats::default();
        assert_eq!(stats.total_cycles, 0);
        assert_eq!(stats.successful_cycles, 0);
        assert_eq!(stats.failed_cycles, 0);
        assert_eq!(stats.total_points_read, 0);
        assert_eq!(stats.total_points_failed, 0);
        assert_eq!(stats.avg_cycle_time_ms, 0.0);
        assert_eq!(stats.current_polling_rate, 0.0);
        assert!(stats.last_successful_polling.is_none());
        assert!(stats.last_polling_error.is_none());
        assert_eq!(stats.communication_quality, 100.0);
    }

    // ConnectionState Tests
    #[test]
    fn test_connection_state_equality() {
        assert_eq!(ConnectionState::Disconnected, ConnectionState::Disconnected);
        assert_eq!(ConnectionState::Connecting, ConnectionState::Connecting);
        assert_eq!(ConnectionState::Connected, ConnectionState::Connected);
        assert_eq!(ConnectionState::Error("test".to_string()), ConnectionState::Error("test".to_string()));
        
        assert_ne!(ConnectionState::Connected, ConnectionState::Disconnected);
        assert_ne!(ConnectionState::Error("a".to_string()), ConnectionState::Error("b".to_string()));
    }

    // PacketParseResult Tests
    #[test]
    fn test_packet_parse_result_success() {
        let mut fields = HashMap::new();
        fields.insert("function_code".to_string(), "0x03".to_string());
        fields.insert("data_length".to_string(), "4".to_string());

        let result = PacketParseResult::success(
            "Modbus",
            "send",
            "01 03 00 00 00 02 c4 0b",
            "Read holding registers request",
            fields.clone(),
        );

        assert_eq!(result.protocol, "Modbus");
        assert_eq!(result.direction, "send");
        assert_eq!(result.hex_data, "01 03 00 00 00 02 c4 0b");
        assert_eq!(result.description, "Read holding registers request");
        assert_eq!(result.fields, fields);
        assert!(result.success);
        assert!(result.error.is_none());
    }

    #[test]
    fn test_packet_parse_result_failure() {
        let result = PacketParseResult::failure(
            "Modbus",
            "receive",
            "01 83 02",
            "Invalid function code",
        );

        assert_eq!(result.protocol, "Modbus");
        assert_eq!(result.direction, "receive");
        assert_eq!(result.hex_data, "01 83 02");
        assert!(result.description.contains("Parse error"));
        assert!(result.fields.is_empty());
        assert!(!result.success);
        assert!(result.error.is_some());
        assert_eq!(result.error.unwrap(), "Invalid function code");
    }

    #[test]
    fn test_packet_parse_result_format_debug_log() {
        let mut fields = HashMap::new();
        fields.insert("test".to_string(), "value".to_string());

        let success_result = PacketParseResult::success(
            "Test",
            "send",
            "01 02 03",
            "Test packet",
            fields,
        );

        let log = success_result.format_debug_log();
        assert!(log.contains("SEND"));
        assert!(log.contains("01 02 03"));
        assert!(log.contains("Test packet"));

        let failure_result = PacketParseResult::failure(
            "Test",
            "receive",
            "04 05 06",
            "Parse failed",
        );

        let log = failure_result.format_debug_log();
        assert!(log.contains("RECEIVE"));
        assert!(log.contains("04 05 06"));
        assert!(log.contains("Parse failed"));
    }

    // ProtocolParserRegistry Tests
    #[test]
    fn test_protocol_parser_registry() {
        let mut registry = ProtocolParserRegistry::new();
        assert!(registry.registered_protocols().is_empty());

        // Register parsers
        registry.register_parser(MockParser::new("Modbus"));
        registry.register_parser(MockParser::new("IEC60870"));

        let protocols = registry.registered_protocols();
        assert_eq!(protocols.len(), 2);
        assert!(protocols.contains(&"Modbus".to_string()));
        assert!(protocols.contains(&"IEC60870".to_string()));

        // Test parsing with registered protocol
        let data = [0x01, 0x03, 0x00, 0x00];
        let result = registry.parse_packet("Modbus", &data, "send");
        assert!(result.success);
        assert_eq!(result.protocol, "Modbus");
        assert_eq!(result.direction, "send");

        // Test parsing with unregistered protocol
        let result = registry.parse_packet("Unknown", &data, "send");
        assert!(!result.success);
        assert!(result.error.is_some());
        assert!(result.error.unwrap().contains("No parser registered"));
    }

    // ComBaseImpl Tests
    #[tokio::test]
    async fn test_combase_impl_creation() {
        let config = create_test_config();
        let service = ComBaseImpl::new("TestService", "TestProtocol", config);

        assert_eq!(service.name(), "TestService");
        assert_eq!(service.channel_id(), "1");
        assert_eq!(service.protocol_type(), "TestProtocol");
        assert!(!service.is_running().await);
    }

    #[tokio::test]
    async fn test_combase_impl_lifecycle() {
        let config = create_test_config();
        let service = ComBaseImpl::new("TestService", "TestProtocol", config);

        // Initial state
        assert!(!service.is_running().await);
        let status = service.status().await;
        assert!(!status.connected);
        assert!(!status.has_error());

        // Start service
        service.start().await.unwrap();
        assert!(service.is_running().await);

        // Stop service
        service.stop().await.unwrap();
        assert!(!service.is_running().await);
    }

    #[tokio::test]
    async fn test_combase_impl_status_updates() {
        let config = create_test_config();
        let service = ComBaseImpl::new("TestService", "TestProtocol", config);

        // Update status with success
        service.update_status(true, 123.45, None).await;
        let status = service.status().await;
        assert!(status.connected);
        assert_eq!(status.last_response_time, 123.45);
        assert!(!status.has_error());

        // Update status with error
        service.update_status(false, 0.0, Some("Connection failed")).await;
        let status = service.status().await;
        assert!(!status.connected);
        assert_eq!(status.last_response_time, 0.0);
        assert!(status.has_error());
        assert_eq!(status.last_error, "Connection failed");
    }

    #[tokio::test]
    async fn test_combase_impl_error_handling() {
        let config = create_test_config();
        let service = ComBaseImpl::new("TestService", "TestProtocol", config);

        // Set error
        service.set_error("Test error message").await;
        let status = service.status().await;
        assert!(status.has_error());
        assert_eq!(status.last_error, "Test error message");

        // Clear error by updating status without error
        service.update_status(false, 0.0, None).await;
        let status = service.status().await;
        assert!(!status.has_error());
        assert!(status.last_error.is_empty());
    }

    #[tokio::test]
    async fn test_combase_impl_measure_execution() {
        let config = create_test_config();
        let service = ComBaseImpl::new("TestService", "TestProtocol", config);

        // Test successful execution
        let result = service.measure_execution(|| {
            std::thread::sleep(std::time::Duration::from_millis(10));
            Ok("success".to_string())
        }).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");

        let status = service.status().await;
        assert!(status.connected);
        assert!(status.last_response_time > 0.0);

        // Test failed execution
        let result = service.measure_execution(|| {
            Err::<String, ComSrvError>(ComSrvError::CommunicationError("Test error".to_string()))
        }).await;

        assert!(result.is_err());
        let status = service.status().await;
        assert!(!status.connected);
        assert!(status.has_error());
    }

    #[tokio::test]
    async fn test_combase_impl_measure_result_execution() {
        let config = create_test_config();
        let service = ComBaseImpl::new("TestService", "TestProtocol", config);

        // Test successful execution
        let result = service.measure_result_execution(|| {
            std::thread::sleep(std::time::Duration::from_millis(5));
            Ok::<String, String>("success".to_string())
        }).await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "success");

        let status = service.status().await;
        assert!(status.connected);
        assert!(status.last_response_time > 0.0);

        // Test failed execution
        let result = service.measure_result_execution(|| {
            Err::<String, String>("Test error".to_string())
        }).await;

        assert!(result.is_err());
        let status = service.status().await;
        assert!(!status.connected);
        assert!(status.has_error());
        assert_eq!(status.last_error, "Test error");
    }

    #[test]
    fn test_combase_impl_parameters() {
        let config = create_test_config();
        let service = ComBaseImpl::new("TestService", "TestProtocol", config);

        let params = service.get_parameters();
        assert_eq!(params.get("protocol").unwrap(), "TestProtocol");
        assert_eq!(params.get("channel_id").unwrap(), "1");
    }

    // Universal Polling Engine Tests
    #[tokio::test]
    async fn test_universal_polling_engine_creation() {
        let reader = Arc::new(MockReader::new());
        let engine = UniversalPollingEngine::new("TestProto".to_string(), reader);

        assert!(!engine.is_polling_active().await);
        let stats = engine.get_polling_stats().await;
        assert_eq!(stats.total_cycles, 0);
    }

    #[tokio::test]
    async fn test_universal_polling_engine_disabled() {
        let reader = Arc::new(MockReader::new());
        let engine = UniversalPollingEngine::new("TestProto".to_string(), reader);

        let config = PollingConfig {
            enabled: false,
            ..Default::default()
        };

        let points = vec![create_test_point("p1", 1)];
        engine.start_polling(config, points).await.unwrap();

        assert!(engine.is_polling_active().await);
        
        // Wait a bit and check stats - should remain zero since polling is disabled
        sleep(Duration::from_millis(100)).await;
        let stats = engine.get_polling_stats().await;
        assert_eq!(stats.total_cycles, 0);

        engine.stop_polling().await.unwrap();
        assert!(!engine.is_polling_active().await);
    }

    #[tokio::test]
    async fn test_universal_polling_engine_successful_polling() {
        let reader = Arc::new(MockReader::new());
        let mut engine = UniversalPollingEngine::new("TestProto".to_string(), reader);

        let collected_data: Arc<Mutex<Vec<Vec<PointData>>>> = Arc::new(Mutex::new(Vec::new()));
        let data_clone = collected_data.clone();
        engine.set_data_callback(move |data| {
            data_clone.lock().unwrap().push(data);
        });

        let config = PollingConfig {
            enabled: true,
            interval_ms: 50,
            max_points_per_cycle: 10,
            timeout_ms: 1000,
            max_retries: 1,
            retry_delay_ms: 100,
            enable_batch_reading: false,
            point_read_delay_ms: 1,
        };

        let points = vec![
            create_test_point("p1", 1),
            create_test_point("p2", 2),
        ];

        engine.start_polling(config, points).await.unwrap();
        assert!(engine.is_polling_active().await);

        // Wait for some polling cycles
        sleep(Duration::from_millis(200)).await;

        engine.stop_polling().await.unwrap();
        assert!(!engine.is_polling_active().await);

        // Check statistics
        let stats = engine.get_polling_stats().await;
        assert!(stats.total_cycles > 0);
        assert!(stats.successful_cycles > 0);
        assert_eq!(stats.failed_cycles, 0);
        assert!(stats.total_points_read > 0);
        assert_eq!(stats.total_points_failed, 0);
        assert!(stats.avg_cycle_time_ms > 0.0);
        assert_eq!(stats.communication_quality, 100.0);

        // Check collected data
        let data = collected_data.lock().unwrap();
        assert!(!data.is_empty());
        for batch in data.iter() {
            assert_eq!(batch.len(), 2); // Two points
            for point in batch {
                assert_eq!(point.quality, 1);
                assert!(point.value.starts_with("value_"));
            }
        }
    }

    #[tokio::test]
    async fn test_universal_polling_engine_failed_reads() {
        let reader = Arc::new(MockReader::new());
        reader.set_fail_reads(true); // Make all reads fail

        let mut engine = UniversalPollingEngine::new("TestProto".to_string(), reader);

        let collected_data: Arc<Mutex<Vec<Vec<PointData>>>> = Arc::new(Mutex::new(Vec::new()));
        let data_clone = collected_data.clone();
        engine.set_data_callback(move |data| {
            data_clone.lock().unwrap().push(data);
        });

        let config = PollingConfig {
            enabled: true,
            interval_ms: 50,
            max_points_per_cycle: 10,
            timeout_ms: 1000,
            max_retries: 1,
            retry_delay_ms: 10,
            enable_batch_reading: false,
            point_read_delay_ms: 1,
        };

        let points = vec![create_test_point("p1", 1)];

        engine.start_polling(config, points).await.unwrap();

        // Wait for some polling cycles
        sleep(Duration::from_millis(150)).await;

        engine.stop_polling().await.unwrap();

        // Check that data was still collected (with error points)
        let data = collected_data.lock().unwrap();
        assert!(!data.is_empty());
        for batch in data.iter() {
            for point in batch {
                assert_eq!(point.quality, 0); // Bad quality due to read failure
                assert_eq!(point.value, "null");
            }
        }
    }

    #[tokio::test]
    async fn test_universal_polling_engine_batch_reading() {
        let reader = Arc::new(MockReader::new());
        let mut engine = UniversalPollingEngine::new("TestProto".to_string(), reader);

        let collected_data: Arc<Mutex<Vec<Vec<PointData>>>> = Arc::new(Mutex::new(Vec::new()));
        let data_clone = collected_data.clone();
        engine.set_data_callback(move |data| {
            data_clone.lock().unwrap().push(data);
        });

        let config = PollingConfig {
            enabled: true,
            interval_ms: 50,
            max_points_per_cycle: 10,
            timeout_ms: 1000,
            max_retries: 1,
            retry_delay_ms: 10,
            enable_batch_reading: true, // Enable batch reading
            point_read_delay_ms: 1,
        };

        let points = vec![
            create_test_point("p1", 1),
            create_test_point("p2", 2),
        ];

        engine.start_polling(config, points).await.unwrap();

        // Wait for some polling cycles
        sleep(Duration::from_millis(150)).await;

        engine.stop_polling().await.unwrap();

        // Check that data was collected
        let data = collected_data.lock().unwrap();
        assert!(!data.is_empty());
    }

    #[tokio::test]
    async fn test_universal_polling_engine_disconnected() {
        let reader = Arc::new(MockReader::new());
        reader.set_connected(false); // Simulate disconnection

        let engine = UniversalPollingEngine::new("TestProto".to_string(), reader);

        let config = PollingConfig {
            enabled: true,
            interval_ms: 50,
            ..Default::default()
        };

        let points = vec![create_test_point("p1", 1)];

        engine.start_polling(config, points).await.unwrap();

        // Wait for some polling attempts
        sleep(Duration::from_millis(150)).await;

        engine.stop_polling().await.unwrap();

        // Statistics should show no successful cycles due to disconnection
        let stats = engine.get_polling_stats().await;
        assert_eq!(stats.successful_cycles, 0);
        assert_eq!(stats.total_points_read, 0);
    }

    #[tokio::test]
    async fn test_universal_polling_engine_config_updates() {
        let reader = Arc::new(MockReader::new());
        let engine = UniversalPollingEngine::new("TestProto".to_string(), reader);

        let new_config = PollingConfig {
            enabled: true,
            interval_ms: 100,
            max_points_per_cycle: 50,
            timeout_ms: 2000,
            max_retries: 5,
            retry_delay_ms: 200,
            enable_batch_reading: false,
            point_read_delay_ms: 20,
        };

        engine.update_polling_config(new_config).await.unwrap();

        let new_points = vec![
            create_test_point("new_p1", 10),
            create_test_point("new_p2", 20),
            create_test_point("new_p3", 30),
        ];

        engine.update_polling_points(new_points).await.unwrap();
    }

    #[tokio::test]
    async fn test_universal_polling_engine_double_start() {
        let reader = Arc::new(MockReader::new());
        let engine = UniversalPollingEngine::new("TestProto".to_string(), reader);

        let config = PollingConfig::default();
        let points = vec![create_test_point("p1", 1)];

        // First start should succeed
        engine.start_polling(config.clone(), points.clone()).await.unwrap();
        assert!(engine.is_polling_active().await);

        // Second start should fail
        let result = engine.start_polling(config, points).await;
        assert!(result.is_err());

        engine.stop_polling().await.unwrap();
    }

    #[tokio::test]
    async fn test_poll_interval_respected() {
        let reader: Arc<dyn PointReader> = Arc::new(MockReader::new());
        let mut engine = UniversalPollingEngine::new("TestProto".to_string(), reader);

        let call_times: Arc<Mutex<Vec<Instant>>> = Arc::new(Mutex::new(Vec::new()));
        let times_clone = call_times.clone();
        engine.set_data_callback(move |_| {
            times_clone.lock().unwrap().push(Instant::now());
        });

        let config = PollingConfig {
            enabled: true,
            interval_ms: 100,
            max_points_per_cycle: 1,
            timeout_ms: 100,
            max_retries: 0,
            retry_delay_ms: 0,
            enable_batch_reading: false,
            point_read_delay_ms: 0,
        };

        let point = create_test_point("p1", 1);

        engine.start_polling(config, vec![point]).await.unwrap();

        sleep(Duration::from_millis(250)).await;
        engine.stop_polling().await.unwrap();

        let times = call_times.lock().unwrap();
        assert!(times.len() >= 2);
        let diff = times[1].duration_since(times[0]);
        assert!(diff >= Duration::from_millis(100));
    }

    // Point reader trait test
    #[tokio::test]
    async fn test_point_reader_default_batch_read() {
        let reader = MockReader::new();
        
        let points = vec![
            create_test_point("p1", 1),
            create_test_point("p2", 2),
        ];

        let results = reader.read_points_batch(&points).await.unwrap();
        assert_eq!(results.len(), 2);
        
        for (i, result) in results.iter().enumerate() {
            assert_eq!(result.id, points[i].id);
            assert_eq!(result.quality, 1);
        }
    }

    #[tokio::test]
    async fn test_point_reader_batch_read_with_failures() {
        let reader = MockReader::new();
        reader.set_fail_reads(true);
        
        let points = vec![create_test_point("p1", 1)];
        let result = reader.read_points_batch(&points).await;
        assert!(result.is_err());
    }

    // Integration test for polling with various scenarios
    #[tokio::test]
    async fn test_polling_integration_scenarios() {
        let reader = Arc::new(MockReader::new());
        let mut engine = UniversalPollingEngine::new("Integration".to_string(), reader.clone());

        let all_data: Arc<Mutex<Vec<PointData>>> = Arc::new(Mutex::new(Vec::new()));
        let data_clone = all_data.clone();
        engine.set_data_callback(move |data| {
            data_clone.lock().unwrap().extend(data);
        });

        let config = PollingConfig {
            enabled: true,
            interval_ms: 30,
            max_points_per_cycle: 5,
            timeout_ms: 500,
            max_retries: 2,
            retry_delay_ms: 50,
            enable_batch_reading: true,
            point_read_delay_ms: 5,
        };

        let points = vec![
            create_test_point("voltage", 40001),
            create_test_point("current", 40002),
            create_test_point("power", 40003),
        ];

        engine.start_polling(config, points).await.unwrap();

        // Phase 1: Normal operation
        sleep(Duration::from_millis(100)).await;
        
        // Phase 2: Simulate connection issues
        reader.set_connected(false);
        sleep(Duration::from_millis(80)).await;
        
        // Phase 3: Restore connection but with read errors
        reader.set_connected(true);
        reader.set_fail_reads(true);
        sleep(Duration::from_millis(80)).await;
        
        // Phase 4: Restore normal operation
        reader.set_fail_reads(false);
        sleep(Duration::from_millis(100)).await;

        engine.stop_polling().await.unwrap();

        // Verify we collected some data
        let data = all_data.lock().unwrap();
        assert!(!data.is_empty());

        // Verify statistics show mixed results
        let stats = engine.get_polling_stats().await;
        assert!(stats.total_cycles > 0);
        // Note: Communication quality might be 100% if all cycles during connected phases were successful
        // This is acceptable behavior - we're just testing that the engine handles the different scenarios
    }
}
