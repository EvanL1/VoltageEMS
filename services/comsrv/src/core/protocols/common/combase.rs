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
use tracing::{info, warn, error, debug};

use crate::core::config::config_manager::ChannelConfig;
use crate::utils::error::{ComSrvError, Result};

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
    
    /// Measure execution time of an asynchronous operation
    /// 
    /// Executes the provided async function and measures its execution time.
    /// Updates the channel status based on the operation result.
    /// 
    /// # Arguments
    /// 
    /// * `f` - Async function to execute and measure
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
    /// let result = service.measure_execution_async(|| {
    ///     // Simulate async work
    ///     async_operation()
    /// }).await?;
    /// # Ok(result)
    /// # }
    /// # fn async_operation() -> std::result::Result<String, String> { Ok("Done".to_string()) }
    /// ```
    pub async fn measure_execution_async<F, T, E>(&self, f: F) -> std::result::Result<T, E>
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

/// Global protocol parser registry
static mut GLOBAL_PARSER_REGISTRY: Option<ProtocolParserRegistry> = None;
static INIT_REGISTRY: std::sync::Once = std::sync::Once::new();

/// Get the global protocol parser registry
pub fn get_global_parser_registry() -> &'static mut ProtocolParserRegistry {
    unsafe {
        INIT_REGISTRY.call_once(|| {
            GLOBAL_PARSER_REGISTRY = Some(ProtocolParserRegistry::new());
        });
        GLOBAL_PARSER_REGISTRY.as_mut().unwrap()
    }
}

/// Parse a protocol packet using the global registry
pub fn parse_protocol_packet(protocol: &str, data: &[u8], direction: &str) -> PacketParseResult {
    let registry = get_global_parser_registry();
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
        self.start_polling_task().await;
        
        Ok(())
    }
    
    async fn stop_polling(&self) -> Result<()> {
        {
            let mut running = self.is_running.write().await;
            *running = false;
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
    async fn start_polling_task(&self) {
        let config = self.config.clone();
        let points = self.points.clone();
        let stats = self.stats.clone();
        let is_running = self.is_running.clone();
        let point_reader = self.point_reader.clone();
        let data_callback = self.data_callback.clone();
        let protocol_name = self.protocol_name.clone();
        
        tokio::spawn(async move {
            let mut cycle_counter = 0u64;
            
            while *is_running.read().await {
                let config_snapshot = config.read().await.clone();
                
                if !config_snapshot.enabled {
                    tokio::time::sleep(Duration::from_millis(1000)).await;
                    continue;
                }
                
                let mut poll_interval = interval(Duration::from_millis(config_snapshot.interval_ms));
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
        if stats_guard.total_cycles > 1 {
            stats_guard.current_polling_rate = 1000.0 / stats_guard.avg_cycle_time_ms;
        }
    }
} 