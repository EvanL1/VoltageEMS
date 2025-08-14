//! Protocol factory module
//!
//! Provides protocol instance creation, management and lifecycle control

use async_trait::async_trait;
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, error, info, warn};

use crate::core::combase::traits::{ComClient, TelemetryBatch};
use crate::core::combase::trigger::{CommandTrigger, CommandTriggerConfig};
use crate::core::config::{ChannelConfig, ProtocolType};
use crate::utils::error::{ComSrvError, Result};
use std::str::FromStr;

/// Configuration value type (using JSON for internal processing)
pub type ConfigValue = serde_json::Value;

/// Dynamic communication client type
pub type DynComClient = Arc<RwLock<Box<dyn ComClient>>>;

// ============================================================================
// Protocol client factory trait
// ============================================================================

/// Protocol client factory trait for extensible protocol support
#[async_trait]
pub trait ProtocolClientFactory: Send + Sync {
    /// Get protocol type
    fn protocol_type(&self) -> ProtocolType;

    /// Create protocol client instance
    /// The factory should ensure that the created client properly isolates
    /// telemetry, signal, control, and adjustment point configurations
    async fn create_client(
        &self,
        channel_config: &ChannelConfig,
        config_value: ConfigValue,
    ) -> Result<Box<dyn ComClient>>;

    /// Validate configuration
    fn validate_config(&self, config: &ConfigValue) -> Result<()>;

    /// Get configuration template
    fn get_config_template(&self) -> ConfigValue;

    /// Get protocol information
    fn get_protocol_info(&self) -> serde_json::Value {
        serde_json::json!({
            "protocol_type": self.protocol_type(),
            "supports_batch": false,
            "supports_async": true
        })
    }
}

// ============================================================================
// Channel management structures
// ============================================================================

/// Channel metadata
#[derive(Debug, Clone)]
struct ChannelMetadata {
    pub name: Arc<str>, // Use Arc<str> instead of String
    pub protocol_type: ProtocolType,
    pub created_at: std::time::Instant,
    pub last_accessed: Arc<RwLock<std::time::Instant>>,
}

/// Channel entry, combining channel and metadata
#[derive(Clone)]
struct ChannelEntry {
    channel: Arc<RwLock<Box<dyn ComClient>>>,
    metadata: ChannelMetadata,
    command_trigger: Option<Arc<RwLock<CommandTrigger>>>,
    channel_config: Arc<ChannelConfig>, // Use Arc to avoid cloning
}

impl std::fmt::Debug for ChannelEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChannelEntry")
            .field("metadata", &self.metadata)
            .finish_non_exhaustive()
    }
}

// ============================================================================
// Main protocol factory structure
// ============================================================================

/// Protocol factory, manages all protocols and channels
pub struct ProtocolFactory {
    /// Store created channels
    channels: DashMap<u16, ChannelEntry, ahash::RandomState>,
    /// Protocol factory registry
    protocol_factories: DashMap<ProtocolType, Arc<dyn ProtocolClientFactory>, ahash::RandomState>,
    /// Global Redis URL for all channels
    redis_url: Arc<str>, // Use Arc<str> to avoid cloning
    /// Telemetry sync task handle
    sync_task_handle: Arc<RwLock<Option<tokio::task::JoinHandle<()>>>>,
    /// Data receiver for batch processing
    data_receiver: Arc<Mutex<Option<tokio::sync::mpsc::Receiver<TelemetryBatch>>>>,
    /// Data sender for protocols
    data_sender: tokio::sync::mpsc::Sender<TelemetryBatch>,
}

impl std::fmt::Debug for ProtocolFactory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProtocolFactory")
            .field("channels", &self.channels.len())
            .field("protocol_factories", &self.protocol_factories.len())
            .field("redis_url", &self.redis_url)
            .finish()
    }
}

impl ProtocolFactory {
    /// Create new protocol factory
    pub fn new() -> Self {
        // Get Redis URL from environment or use default
        let redis_url =
            std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".to_string());
        let redis_url: Arc<str> = redis_url.into();

        // Create channel with larger buffer for batch processing
        let (tx, rx) = tokio::sync::mpsc::channel(1000);

        let factory = Self {
            channels: DashMap::with_hasher(ahash::RandomState::new()),
            protocol_factories: DashMap::with_hasher(ahash::RandomState::new()),
            redis_url,
            sync_task_handle: Arc::new(RwLock::new(None)),
            data_receiver: Arc::new(Mutex::new(Some(rx))),
            data_sender: tx,
        };

        // Initialize plugin system and register built-in protocols
        if let Err(e) = crate::plugins::init_plugin_system() {
            error!("Failed to initialize plugin system: {}", e);
        }

        // Register protocols from the plugin registry
        factory.register_from_plugin_registry();

        factory
    }

    /// Create new protocol factory with custom Redis URL
    pub fn with_redis_url(redis_url: String) -> Self {
        let redis_url: Arc<str> = redis_url.into();

        // Create channel with larger buffer for batch processing
        let (tx, rx) = tokio::sync::mpsc::channel(1000);

        let factory = Self {
            channels: DashMap::with_hasher(ahash::RandomState::new()),
            protocol_factories: DashMap::with_hasher(ahash::RandomState::new()),
            redis_url,
            sync_task_handle: Arc::new(RwLock::new(None)),
            data_receiver: Arc::new(Mutex::new(Some(rx))),
            data_sender: tx,
        };

        // Initialize plugin system and register built-in protocols
        if let Err(e) = crate::plugins::init_plugin_system() {
            error!("Failed to initialize plugin system: {}", e);
        }

        // Register protocols from the plugin registry
        factory.register_from_plugin_registry();

        factory
    }

    /// Register protocol factories from plugin registry
    fn register_from_plugin_registry(&self) {
        use crate::plugins::registry::get_plugin_registry;

        // Get plugin registry
        let registry = get_plugin_registry();
        let reg = registry
            .read()
            .expect("plugin registry lock should not be poisoned");

        // Register all available plugins from the registry
        for protocol_name in reg.list_protocol_factories() {
            if reg.get_factory(&protocol_name).is_some() {
                // Parse protocol type from name
                if let Ok(protocol_type) = ProtocolType::from_str(&protocol_name) {
                    self.register_protocol_factory(Arc::new(PluginAdapterFactory::new(
                        protocol_type,
                        protocol_name.clone(),
                    )));
                    info!("Registered protocol factory for: {}", protocol_name);
                }
            }
        }

        // Virtual protocol factory (for testing)
        #[cfg(any(test, feature = "test-utils"))]
        {
            use self::test_support::MockProtocolFactory;
            self.register_protocol_factory(Arc::new(MockProtocolFactory));
        }
    }

    /// Register protocol factory
    pub fn register_protocol_factory(&self, factory: Arc<dyn ProtocolClientFactory>) {
        let protocol_type = factory.protocol_type();
        self.protocol_factories.insert(protocol_type, factory);
        info!("Registered protocol factory for {protocol_type:?}");
    }

    /// Unregister protocol factory
    pub fn unregister_protocol_factory(&self, protocol_type: &ProtocolType) -> Result<bool> {
        // Check if there are active channels using this protocol
        let active_channels: Vec<u16> = self
            .channels
            .iter()
            .filter_map(|entry| {
                let (id, channel_entry) = (entry.key(), entry.value());
                if channel_entry.metadata.protocol_type == *protocol_type {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();

        if !active_channels.is_empty() {
            return Err(ComSrvError::InvalidOperation(format!(
                "Cannot unregister protocol factory: {} channels are still active",
                active_channels.len()
            )));
        }

        match self.protocol_factories.remove(protocol_type) {
            Some(_) => {
                info!("Unregistered protocol factory for {protocol_type:?}");
                Ok(true)
            },
            None => Ok(false),
        }
    }

    /// Get list of registered protocol types
    pub fn get_registered_protocols(&self) -> Vec<ProtocolType> {
        self.protocol_factories
            .iter()
            .map(|entry| *entry.key())
            .collect()
    }

    /// Create channel
    pub async fn create_channel(
        &self,
        channel_config: &ChannelConfig,
    ) -> Result<Arc<RwLock<Box<dyn ComClient>>>> {
        // Clone channel config to make it mutable for loading CSV configurations
        let mut channel_config = channel_config.clone();
        let channel_id = channel_config.id;

        // Step 1: Validate channel doesn't exist
        self.validate_channel_not_exists(channel_id).map_err(|e| {
            error!(
                "Failed to validate channel {} availability: {}",
                channel_id, e
            );
            e
        })?;

        // Step 2: Load CSV configurations FIRST (before creating protocol)
        self.load_csv_configurations(&mut channel_config).await?;
        info!(
            "Loaded CSV configurations for channel {}: {} telemetry, {} signal, {} control, {} adjustment points",
            channel_id,
            channel_config.telemetry_points.len(),
            channel_config.signal_points.len(),
            channel_config.control_points.len(),
            channel_config.adjustment_points.len()
        );

        // Step 3: Prepare protocol client (with loaded configurations)
        let (mut client, protocol_type) = self
            .prepare_protocol_client(&channel_config)
            .await
            .map_err(|e| {
                error!(
                    "Failed to prepare protocol client for channel {} ({}): {}",
                    channel_id, channel_config.protocol, e
                );
                e
            })?;

        // Step 4: Setup Redis storage
        self.setup_redis_storage(&channel_config)
            .await
            .map_err(|e| {
                error!(
                    "Failed to setup Redis storage for channel {}: {}",
                    channel_id, e
                );
                e
            })?;

        // Step 5: Initialize protocol (with loaded configurations)
        self.initialize_protocol(&mut client, &channel_config)
            .await
            .map_err(|e| {
                error!(
                    "Failed to initialize protocol {:?} for channel {}: {}",
                    protocol_type, channel_id, e
                );
                e
            })?;

        // Step 6: Setup command trigger
        let (command_trigger, rx) = self.setup_command_trigger(channel_id).await.map_err(|e| {
            error!(
                "Failed to setup command trigger for channel {}: {}",
                channel_id, e
            );
            e
        })?;
        client.set_command_receiver(rx);

        // Step 7: Register channel entry
        let channel_arc = Arc::new(RwLock::new(client));
        self.register_channel_entry(
            channel_id,
            channel_arc.clone(),
            &channel_config,
            protocol_type,
            command_trigger,
        );

        // Step 8: Create channel-specific logger
        let log_dir = std::env::var("LOG_DIR").unwrap_or_else(|_| "logs".to_string());
        if let Err(e) = voltage_libs::logging::create_channel_logger(
            std::path::Path::new(&log_dir),
            &channel_id.to_string(),
        ) {
            warn!("Failed to create channel logger for {}: {}", channel_id, e);
        }

        info!(
            "Created channel {} with protocol {:?}",
            channel_id, protocol_type
        );
        Ok(channel_arc)
    }

    /// Validate that channel doesn't already exist
    fn validate_channel_not_exists(&self, channel_id: u16) -> Result<()> {
        if self.channels.contains_key(&channel_id) {
            return Err(ComSrvError::InvalidOperation(format!(
                "Channel {channel_id} already exists"
            )));
        }
        Ok(())
    }

    /// Prepare protocol client: get factory, validate config, create client
    async fn prepare_protocol_client(
        &self,
        channel_config: &ChannelConfig,
    ) -> Result<(Box<dyn ComClient>, ProtocolType)> {
        let channel_id = channel_config.id;

        // Get protocol type
        let protocol_type = ProtocolType::from_str(&channel_config.protocol).map_err(|e| {
            ComSrvError::ConfigError(format!(
                "Failed to parse protocol type '{}': {}",
                channel_config.protocol, e
            ))
        })?;

        // Find protocol factory
        let factory = self.protocol_factories.get(&protocol_type).ok_or_else(|| {
            ComSrvError::ConfigError(format!(
                "No factory registered for protocol: {protocol_type:?}"
            ))
        })?;

        // Convert channel_config.parameters to ConfigValue
        let config_value = serde_json::to_value(&channel_config.parameters)
            .map_err(|e| ComSrvError::ConfigError(format!("Failed to convert parameters: {e}")))?;

        // Validate configuration
        factory.validate_config(&config_value).map_err(|e| {
            ComSrvError::ConfigError(format!(
                "Failed to validate {:?} configuration: {}",
                protocol_type, e
            ))
        })?;

        // Create client instance
        info!("Creating client for protocol {:?}", protocol_type);
        let client = factory
            .create_client(channel_config, config_value)
            .await
            .map_err(|e| {
                ComSrvError::ConfigError(format!(
                    "Failed to create {:?} client instance: {}",
                    protocol_type, e
                ))
            })?;
        info!("Client created successfully for channel {}", channel_id);

        Ok((client, protocol_type))
    }

    /// Initialize protocol instance
    async fn initialize_protocol(
        &self,
        client: &mut Box<dyn ComClient>,
        channel_config: &ChannelConfig,
    ) -> Result<()> {
        let channel_id = channel_config.id;

        info!("Initializing client for channel {}", channel_id);
        client.initialize(Arc::new(channel_config.clone())).await?;
        info!("Client initialized successfully for channel {}", channel_id);

        // Set data channel for protocols that support it
        client.set_data_channel(self.data_sender.clone());

        // Skip connection phase, only complete initialization
        // Connections will be established uniformly after all channels are initialized
        info!(
            "Channel {} initialization completed, connection will be established later",
            channel_id
        );

        Ok(())
    }

    /// Load CSV configurations into channel config
    async fn load_csv_configurations(&self, channel_config: &mut ChannelConfig) -> Result<()> {
        use std::path::PathBuf;

        let channel_id = channel_config.id;
        info!("Loading CSV configurations for channel {}", channel_id);

        // Get csv_base_path from environment or use default
        let csv_base_path =
            std::env::var("CSV_BASE_PATH").unwrap_or_else(|_| "/app/config".to_string());
        let csv_base_path = PathBuf::from(csv_base_path);
        let channel_dir = csv_base_path.join(channel_id.to_string());

        // Fixed file names for each telemetry type
        let telemetry_types = vec![
            ("telemetry", "telemetry.csv", "T"),
            ("signal", "signal.csv", "S"),
            ("control", "control.csv", "C"),
            ("adjustment", "adjustment.csv", "A"),
        ];

        for (telemetry_name, file_name, redis_type) in telemetry_types {
            let file_path = channel_dir.join(file_name);
            let mapping_file = channel_dir
                .join("mapping")
                .join(format!("{}_mapping.csv", telemetry_name));

            if !file_path.exists() {
                info!(
                    "Skipping {} for channel {}: file not found at {:?}",
                    telemetry_name, channel_id, file_path
                );
                continue;
            }

            // Read the mapping file first
            let mapping_data = if mapping_file.exists() {
                let mut mapping_reader = csv::Reader::from_path(&mapping_file).map_err(|e| {
                    ComSrvError::ConfigError(format!(
                        "Failed to read {} mapping file: {e}",
                        telemetry_name
                    ))
                })?;
                let mut mappings = std::collections::HashMap::new();
                for result in mapping_reader.records() {
                    let record = result.map_err(|e| {
                        ComSrvError::ConfigError(format!("Error reading mapping CSV record: {e}"))
                    })?;
                    // Expecting: point_id, slave_id, function_code, register_address, data_type, byte_order
                    if let (
                        Some(point_id_str),
                        Some(slave_id),
                        Some(function_code),
                        Some(register_address),
                    ) = (record.get(0), record.get(1), record.get(2), record.get(3))
                    {
                        if let Ok(point_id) = point_id_str.parse::<u32>() {
                            let mut params = std::collections::HashMap::new();
                            params.insert("slave_id".to_string(), slave_id.to_string());
                            params.insert("function_code".to_string(), function_code.to_string());
                            params.insert(
                                "register_address".to_string(),
                                register_address.to_string(),
                            );
                            if let Some(data_type) = record.get(4) {
                                params.insert("data_type".to_string(), data_type.to_string());
                            }
                            if let Some(byte_order) = record.get(5) {
                                params.insert("byte_order".to_string(), byte_order.to_string());
                            }
                            mappings.insert(point_id, params);
                        }
                    }
                }
                Some(mappings)
            } else {
                info!(
                    "No mapping file found for {} at {:?}",
                    telemetry_name, mapping_file
                );
                None
            };

            // Read point definitions and combine with mapping
            let mut reader = csv::Reader::from_path(&file_path).map_err(|e| {
                ComSrvError::ConfigError(format!("Failed to read {telemetry_name} CSV file: {e}"))
            })?;

            for (line_num, result) in reader.records().enumerate() {
                let record = match result {
                    Ok(record) => record,
                    Err(e) => {
                        warn!(
                            "Skipping invalid CSV record at line {}: {}",
                            line_num + 2,
                            e
                        );
                        continue;
                    },
                };

                // Get point_id (first column) and other fields
                if let Some(point_id_str) = record.get(0) {
                    if let Ok(point_id) = point_id_str.parse::<u32>() {
                        // Create CombinedPoint if we have mapping data
                        if let Some(ref mapping_data) = mapping_data {
                            if let Some(protocol_params) = mapping_data.get(&point_id) {
                                // Parse scaling info from CSV columns
                                let scale = record.get(2).and_then(|s| s.parse::<f64>().ok());
                                let offset = record.get(3).and_then(|s| s.parse::<f64>().ok());
                                let unit = record
                                    .get(4)
                                    .filter(|s| !s.is_empty())
                                    .map(|s| s.to_string());
                                let reverse_str = record.get(5);
                                let reverse = reverse_str.and_then(|s| s.parse::<bool>().ok());

                                debug!(
                                    "Parsing reverse for {} point {}: raw_str={:?}, parsed={:?}",
                                    telemetry_name, point_id, reverse_str, reverse
                                );

                                let scaling =
                                    if scale.is_some() || offset.is_some() || reverse.is_some() {
                                        Some(crate::core::config::ScalingInfo {
                                            scale: scale.unwrap_or(1.0),
                                            offset: offset.unwrap_or(0.0),
                                            unit,
                                            reverse,
                                        })
                                    } else {
                                        None
                                    };

                                info!(
                                    "Loading {} point {}: scale={:?}, offset={:?}, reverse={:?}, scaling={:?}",
                                    telemetry_name, point_id, scale, offset, reverse, scaling
                                );

                                let combined_point = crate::core::config::CombinedPoint {
                                    point_id,
                                    signal_name: record.get(1).unwrap_or("").to_string(),
                                    telemetry_type: redis_type.to_string(),
                                    data_type: protocol_params
                                        .get("data_type")
                                        .cloned()
                                        .unwrap_or_else(|| "float32".to_string()),
                                    protocol_params: protocol_params.clone(),
                                    scaling,
                                };

                                // Add to appropriate HashMap in channel_config
                                match telemetry_name {
                                    "telemetry" => {
                                        channel_config
                                            .telemetry_points
                                            .insert(point_id, combined_point);
                                    },
                                    "signal" => {
                                        channel_config
                                            .signal_points
                                            .insert(point_id, combined_point);
                                    },
                                    "control" => {
                                        channel_config
                                            .control_points
                                            .insert(point_id, combined_point);
                                    },
                                    "adjustment" => {
                                        channel_config
                                            .adjustment_points
                                            .insert(point_id, combined_point);
                                    },
                                    _ => {},
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Setup Redis storage and initialize channel points
    async fn setup_redis_storage(&self, channel_config: &ChannelConfig) -> Result<()> {
        let channel_id = channel_config.id;

        info!("Initializing Redis keys for channel {}", channel_id);

        // Create a Redis client once for all operations
        info!("Creating Redis client with URL: {}", &self.redis_url);
        let mut redis_client = voltage_libs::redis::RedisClient::new(&self.redis_url)
            .await
            .map_err(|e| ComSrvError::Storage(format!("Failed to connect to Redis: {e}")))?;
        info!("Redis client created successfully");

        self.initialize_channel_points_to_redis(channel_config, &mut redis_client)
            .await?;
        info!("Redis keys initialized for channel {}", channel_id);

        Ok(())
    }

    /// Setup command trigger for the channel
    async fn setup_command_trigger(
        &self,
        channel_id: u16,
    ) -> Result<(
        Option<Arc<RwLock<CommandTrigger>>>,
        tokio::sync::mpsc::Receiver<super::traits::ChannelCommand>,
    )> {
        info!("Creating CommandTrigger for channel {}", channel_id);

        let config = CommandTriggerConfig {
            channel_id,
            redis_url: self.redis_url.to_string(), // Convert Arc<str> to String only when needed
            ..Default::default()                   // Use default mode and timeout_seconds
        };

        let (tx, rx) = tokio::sync::mpsc::channel(100);
        let subscriber = CommandTrigger::new(config, tx).await?;
        let subscriber_arc = Arc::new(RwLock::new(subscriber));

        // Start subscriber
        info!("Starting CommandTrigger for channel {}", channel_id);
        let mut sub = subscriber_arc.write().await;
        sub.start().await?;
        drop(sub);

        info!(
            "CommandTrigger started successfully for channel {}",
            channel_id
        );

        Ok((Some(subscriber_arc), rx))
    }

    /// Register channel entry in the channels map
    fn register_channel_entry(
        &self,
        channel_id: u16,
        channel_arc: Arc<RwLock<Box<dyn ComClient>>>,
        channel_config: &ChannelConfig,
        protocol_type: ProtocolType,
        command_trigger: Option<Arc<RwLock<CommandTrigger>>>,
    ) {
        let entry = ChannelEntry {
            channel: channel_arc,
            metadata: ChannelMetadata {
                name: channel_config.name.clone().into(), // Convert String to Arc<str>
                protocol_type,
                created_at: std::time::Instant::now(),
                last_accessed: Arc::new(RwLock::new(std::time::Instant::now())),
            },
            command_trigger,
            channel_config: Arc::new(channel_config.clone()), // TODO: optimize to pass Arc
        };

        self.channels.insert(channel_id, entry);
    }

    /// Get channel
    pub async fn get_channel(&self, channel_id: u16) -> Option<Arc<RwLock<Box<dyn ComClient>>>> {
        self.channels.get(&channel_id).map(|entry| {
            // Update last access time (using try_write to avoid spawning)
            if let Ok(mut time) = entry.metadata.last_accessed.try_write() {
                *time = std::time::Instant::now();
            }
            entry.channel.clone()
        })
    }

    /// Batch connect all initialized channels
    pub async fn connect_all_channels(&self) -> Result<()> {
        info!(
            "Starting batch connection for {} channels",
            self.channels.len()
        );

        let channel_ids: Vec<u16> = self.channels.iter().map(|entry| *entry.key()).collect();
        let mut successful_connections = 0;
        let mut failed_connections = 0;

        for channel_id in channel_ids {
            if let Some(entry) = self.channels.get(&channel_id) {
                info!("Connecting channel {}", channel_id);

                let mut client = entry.channel.write().await;
                match client.connect().await {
                    Ok(()) => {
                        info!("Channel {} connected successfully", channel_id);
                        successful_connections += 1;
                    },
                    Err(e) => {
                        error!("Failed to connect channel {}: {}", channel_id, e);
                        failed_connections += 1;
                    },
                }
            }
        }

        info!(
            "Batch connection completed: {} successful, {} failed",
            successful_connections, failed_connections
        );

        // Start telemetry sync task after all channels are connected
        self.start_telemetry_sync_task().await?;

        Ok(())
    }

    /// Remove channel with proper resource cleanup
    pub async fn remove_channel(&self, channel_id: u16) -> Result<()> {
        if let Some((_, entry)) = self.channels.remove(&channel_id) {
            let mut cleanup_errors = Vec::new();

            // Stop command trigger (best effort)
            if let Some(trigger) = entry.command_trigger {
                if let Err(e) = async {
                    let mut sub = trigger.write().await;
                    sub.stop().await
                }
                .await
                {
                    error!(
                        "Failed to stop command trigger for channel {}: {}",
                        channel_id, e
                    );
                    cleanup_errors.push(format!("command trigger: {}", e));
                }
            }

            // Disconnect channel (best effort)
            if let Err(e) = async {
                let mut channel = entry.channel.write().await;
                channel.disconnect().await
            }
            .await
            {
                error!("Failed to disconnect channel {}: {}", channel_id, e);
                cleanup_errors.push(format!("disconnect: {}", e));
            }

            // If there were cleanup errors, report them but still consider the removal successful
            if !cleanup_errors.is_empty() {
                warn!(
                    "Channel {} removed with cleanup errors: {}",
                    channel_id,
                    cleanup_errors.join(", ")
                );
            } else {
                info!("Channel {} removed successfully", channel_id);
            }

            Ok(())
        } else {
            Err(ComSrvError::InvalidOperation(format!(
                "Channel {channel_id} not found"
            )))
        }
    }

    /// Get all channel IDs
    pub fn get_channel_ids(&self) -> Vec<u16> {
        self.channels.iter().map(|entry| *entry.key()).collect()
    }

    /// Get channel statistics
    pub async fn get_channel_stats(&self, channel_id: u16) -> Option<ChannelStats> {
        if let Some(entry) = self.channels.get(&channel_id) {
            let channel = entry.channel.read().await;
            let status = channel.get_status().await;

            Some(ChannelStats {
                channel_id,
                name: entry.metadata.name.to_string(), // Convert Arc<str> to String only for output
                protocol_type: entry.metadata.protocol_type,
                is_connected: status.is_connected,
                created_at: entry.metadata.created_at,
                last_accessed: *entry.metadata.last_accessed.read().await,
                points_count: status.points_count,
                success_count: status.success_count,
                error_count: status.error_count,
            })
        } else {
            None
        }
    }

    /// Get all channel statistics
    pub async fn get_all_channel_stats(&self) -> Vec<ChannelStats> {
        let mut stats = Vec::new();

        for entry in &self.channels {
            let channel_id = *entry.key();
            if let Some(channel_stats) = self.get_channel_stats(channel_id).await {
                stats.push(channel_stats);
            }
        }

        stats
    }

    /// Start telemetry synchronization task
    async fn start_telemetry_sync_task(&self) -> Result<()> {
        info!("Starting telemetry sync task...");

        // Take the receiver from the factory
        let receiver = {
            let mut receiver_opt = self.data_receiver.lock().await;
            receiver_opt.take()
        };

        let Some(mut receiver) = receiver else {
            return Err(ComSrvError::InvalidOperation(
                "Data receiver already taken".to_string(),
            ));
        };

        // Clone necessary references for the task
        let channels = self.channels.clone();
        let redis_url = Arc::clone(&self.redis_url); // More efficient Arc clone
        let sync_handle = self.sync_task_handle.clone();

        // Create the sync task
        let sync_task = tokio::spawn(async move {
            // Create storage instance for immediate writing

            // Create and configure LuaSyncManager
            let lua_sync_config = crate::core::sync::LuaSyncConfig {
                enabled: true, // Enable Lua synchronization
                batch_size: 1000,
                retry_count: 3,
                async_sync: true,
                trigger_alarms: true,
            };

            let redis_client = match voltage_libs::redis::RedisClient::new(&redis_url).await {
                Ok(client) => client,
                Err(e) => {
                    error!("Failed to create Redis client for LuaSyncManager: {}", e);
                    return;
                },
            };

            let lua_sync_manager =
                match crate::core::sync::LuaSyncManager::new(lua_sync_config, redis_client).await {
                    Ok(manager) => Arc::new(manager),
                    Err(e) => {
                        error!("Failed to create LuaSyncManager: {}", e);
                        return;
                    },
                };

            // Create storage with Lua sync manager
            let mut storage = match crate::storage::StorageManager::new(&redis_url).await {
                Ok(s) => s,
                Err(e) => {
                    error!("Failed to create storage: {}", e);
                    return;
                },
            };
            storage.set_data_sync(lua_sync_manager);
            let storage = Arc::new(storage);

            // Event-driven processing: immediately write data upon receipt
            loop {
                // Receive data from protocols
                match receiver.recv().await {
                    Some(batch) => {
                        debug!("Received telemetry batch for channel {}", batch.channel_id);

                        // Immediately spawn async task to avoid blocking the receiver loop
                        // This allows the loop to quickly consume channel buffer
                        let channels_clone = channels.clone();
                        let storage_clone = storage.clone();

                        tokio::spawn(async move {
                            let start = std::time::Instant::now();

                            // Get channel config for scaling
                            let channel_config = channels_clone
                                .get(&batch.channel_id)
                                .map(|entry| Arc::clone(&entry.channel_config));

                            let mut updates = Vec::new();

                            // Process telemetry data
                            for (point_id, raw_value, _timestamp) in batch.telemetry {
                                let processed_value = if let Some(ref config) = channel_config {
                                    // Apply scaling from channel config
                                    if let Some(point_config) =
                                        config.telemetry_points.get(&point_id)
                                    {
                                        debug!(
                                            "Processing telemetry point {}: raw={}, scaling={:?}",
                                            point_id, raw_value, point_config.scaling
                                        );
                                        let processed =
                                            crate::core::data_processor::process_point_value(
                                                raw_value,
                                                &crate::core::config::TelemetryType::Telemetry,
                                                point_config.scaling.as_ref(),
                                            );
                                        debug!(
                                            "Telemetry point {} processed: raw={} -> processed={}",
                                            point_id, raw_value, processed
                                        );
                                        processed
                                    } else {
                                        warn!(
                                            "Telemetry point {} not found in config, using raw value {}",
                                            point_id, raw_value
                                        );
                                        raw_value
                                    }
                                } else {
                                    warn!(
                                        "No channel config for channel {}, using raw value {} for point {}",
                                        batch.channel_id, raw_value, point_id
                                    );
                                    raw_value
                                };

                                let update = crate::plugins::registry::PluginPointUpdate {
                                    telemetry_type: crate::core::config::TelemetryType::Telemetry,
                                    point_id,
                                    value: processed_value,
                                    raw_value: Some(raw_value),
                                };
                                updates.push(update);
                            }

                            // Process signal data
                            for (point_id, raw_value, _timestamp) in batch.signal {
                                let processed_value = if let Some(ref config) = channel_config {
                                    // Apply scaling/reverse from channel config
                                    if let Some(point_config) = config.signal_points.get(&point_id)
                                    {
                                        debug!(
                                            "Channel {} processing signal point {}: raw={}, scaling={:?}",
                                            batch.channel_id, point_id, raw_value, point_config.scaling
                                        );
                                        let processed =
                                            crate::core::data_processor::process_point_value(
                                                raw_value,
                                                &crate::core::config::TelemetryType::Signal,
                                                point_config.scaling.as_ref(),
                                            );
                                        debug!(
                                            "Signal point {} processed: raw={} -> processed={}",
                                            point_id, raw_value, processed
                                        );
                                        processed
                                    } else {
                                        debug!(
                                            "Signal point {} not found in config, using raw value",
                                            point_id
                                        );
                                        raw_value
                                    }
                                } else {
                                    raw_value
                                };

                                let update = crate::plugins::registry::PluginPointUpdate {
                                    telemetry_type: crate::core::config::TelemetryType::Signal,
                                    point_id,
                                    value: processed_value,
                                    raw_value: Some(raw_value),
                                };
                                updates.push(update);
                            }

                            // Write updates to Redis
                            if !updates.is_empty() {
                                debug!(
                                    "Writing {} updates for channel {}",
                                    updates.len(),
                                    batch.channel_id
                                );

                                if let Err(e) = storage_clone
                                    .batch_update_and_publish(batch.channel_id, updates)
                                    .await
                                {
                                    error!(
                                        "Failed to sync telemetry for channel {}: {}",
                                        batch.channel_id, e
                                    );
                                } else {
                                    let elapsed = start.elapsed();
                                    debug!(
                                        "Successfully synced telemetry for channel {} in {:?}",
                                        batch.channel_id, elapsed
                                    );
                                }
                            }
                        });
                    },
                    None => {
                        warn!("Telemetry sync receiver closed, exiting sync task");
                        break;
                    },
                }
            }
        });

        // Store the task handle
        *sync_handle.write().await = Some(sync_task);

        info!("Telemetry sync task started successfully (event-driven mode)");
        Ok(())
    }

    /// Clean up all channels with graceful shutdown
    pub async fn cleanup(&self) -> Result<()> {
        info!("Starting factory cleanup...");
        let mut cleanup_errors = Vec::new();

        // Stop sync task first (gracefully if possible)
        if let Some(handle) = self.sync_task_handle.write().await.take() {
            // Give the task a chance to complete gracefully
            let timeout = tokio::time::Duration::from_secs(5);

            // Use select to race between the handle and timeout
            tokio::select! {
                result = handle => {
                    match result {
                        Ok(()) => info!("Telemetry sync task stopped gracefully"),
                        Err(e) => {
                            error!("Telemetry sync task failed: {:?}", e);
                            cleanup_errors.push(format!("sync task: {:?}", e));
                        }
                    }
                },
                _ = tokio::time::sleep(timeout) => {
                    warn!("Telemetry sync task did not stop within timeout");
                    // Task handle has been consumed, but it will be dropped and canceled
                }
            }
        }

        // Get all channel IDs before starting cleanup
        let channel_ids: Vec<u16> = self.get_channel_ids();
        info!("Cleaning up {} channels", channel_ids.len());

        // Remove all channels (best effort)
        for channel_id in channel_ids {
            if let Err(e) = self.remove_channel(channel_id).await {
                error!("Failed to remove channel {}: {}", channel_id, e);
                cleanup_errors.push(format!("channel {}: {}", channel_id, e));
            }
        }

        // Report overall cleanup status
        if cleanup_errors.is_empty() {
            info!("Factory cleanup completed successfully");
            Ok(())
        } else {
            let error_msg = format!(
                "Factory cleanup completed with {} errors: {}",
                cleanup_errors.len(),
                cleanup_errors.join(", ")
            );
            warn!("{}", error_msg);
            // Still return Ok as cleanup is best-effort
            Ok(())
        }
    }

    /// Get channel count
    pub fn channel_count(&self) -> usize {
        self.channels.len()
    }

    /// Get running channel count
    pub async fn running_channel_count(&self) -> usize {
        let mut count = 0;
        for entry in &self.channels {
            let channel = entry.value();
            let channel_guard = channel.channel.read().await;
            if channel_guard.is_connected() {
                count += 1;
            }
        }
        count
    }

    /// Get channel metadata
    pub async fn get_channel_metadata(&self, channel_id: u16) -> Option<(String, String)> {
        self.channels.get(&channel_id).map(|entry| {
            let metadata = &entry.metadata;
            (
                metadata.name.to_string(), // Convert Arc<str> to String only for output
                format!("{:?}", metadata.protocol_type),
            )
        })
    }

    /// Initialize all channel points to Redis using batch Lua function
    async fn initialize_channel_points_to_redis(
        &self,
        channel_config: &ChannelConfig,
        redis_client: &mut voltage_libs::redis::RedisClient,
    ) -> Result<()> {
        info!(
            "Initializing channel {} points to Redis: {} telemetry, {} signal, {} control, {} adjustment",
            channel_config.id,
            channel_config.telemetry_points.len(),
            channel_config.signal_points.len(),
            channel_config.control_points.len(),
            channel_config.adjustment_points.len()
        );

        // Initialize each telemetry type from channel_config
        let telemetry_types = vec![
            ("telemetry", "T", &channel_config.telemetry_points),
            ("signal", "S", &channel_config.signal_points),
            ("control", "C", &channel_config.control_points),
            ("adjustment", "A", &channel_config.adjustment_points),
        ];

        for (telemetry_name, redis_type, points) in telemetry_types {
            if points.is_empty() {
                debug!(
                    "No {} points configured for channel {}",
                    telemetry_name, channel_config.id
                );
                continue;
            }

            // Collect point IDs from the HashMap
            let point_ids: Vec<u32> = points.keys().copied().collect();

            // Use Lua function for batch initialization
            let start_time = std::time::Instant::now();

            // Prepare parameters for Lua function
            let channel_id = channel_config.id.to_string();
            let points_json = serde_json::to_string(&point_ids).map_err(|e| {
                ComSrvError::ConfigError(format!("Failed to serialize point IDs: {e}"))
            })?;

            let options = serde_json::json!({
                "default_value": 0.0,
                "force_overwrite": true
            });
            let options_json = options.to_string();

            // Call Redis Lua function for batch initialization
            info!(
                "Calling generic_batch_init_points for channel {} {} with {} points",
                channel_config.id,
                telemetry_name,
                point_ids.len()
            );

            let result: String = redis_client
                .fcall(
                    "generic_batch_init_points",
                    &[],
                    &[&channel_id, redis_type, &points_json, &options_json],
                )
                .await
                .map_err(|e| {
                    ComSrvError::RedisError(format!(
                        "Failed to call batch init function for {}: {}",
                        telemetry_name, e
                    ))
                })?;

            let elapsed = start_time.elapsed();

            // Parse result
            match serde_json::from_str::<serde_json::Value>(&result) {
                Ok(result_json) => {
                    let total_points = result_json["total_points"].as_u64().unwrap_or(0);
                    let new_points = result_json["new_points"].as_u64().unwrap_or(0);
                    let existing_points = result_json["existing_points"].as_u64().unwrap_or(0);

                    info!(
                        "Batch initialized {} points for channel {} {}: {} new, {} existing (took {:?})",
                        total_points, channel_config.id, telemetry_name, new_points, existing_points, elapsed
                    );
                },
                Err(e) => {
                    warn!(
                        "Failed to parse batch init result for channel {} {}: {} (raw: {})",
                        channel_config.id, telemetry_name, e, result
                    );
                },
            }
        }

        Ok(())
    }
}

impl Default for ProtocolFactory {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Helper structures and functions
// ============================================================================

/// Channel statistics
#[derive(Debug, Clone)]
pub struct ChannelStats {
    pub channel_id: u16,
    pub name: String,
    pub protocol_type: ProtocolType,
    pub is_connected: bool,
    pub created_at: std::time::Instant,
    pub last_accessed: std::time::Instant,
    pub points_count: usize,
    pub success_count: u64,
    pub error_count: u64,
}

/// Create default factory
pub fn create_default_factory() -> ProtocolFactory {
    ProtocolFactory::new()
}

/// Create factory with custom protocols
pub fn create_factory_with_custom_protocols(
    protocols: Vec<Arc<dyn ProtocolClientFactory>>,
) -> ProtocolFactory {
    let factory = ProtocolFactory::new();

    for protocol in protocols {
        factory.register_protocol_factory(protocol);
    }

    factory
}

// ============================================================================
// Plugin adapter factory
// ============================================================================

/// Plugin system adapter factory
/// Adapts plugin system's `ProtocolPlugin` to `ProtocolClientFactory`
struct PluginAdapterFactory {
    protocol_type: ProtocolType,
    plugin_id: String,
}

impl PluginAdapterFactory {
    fn new(protocol_type: ProtocolType, plugin_id: String) -> Self {
        Self {
            protocol_type,
            plugin_id,
        }
    }
}

#[async_trait]
impl ProtocolClientFactory for PluginAdapterFactory {
    fn protocol_type(&self) -> ProtocolType {
        self.protocol_type
    }

    async fn create_client(
        &self,
        channel_config: &ChannelConfig,
        _config_value: ConfigValue,
    ) -> Result<Box<dyn ComClient>> {
        use crate::plugins::registry::get_plugin_registry;

        // Get plugin in a small scope
        let plugin = {
            let registry = get_plugin_registry();
            let reg = registry
                .read()
                .expect("plugin registry lock should not be poisoned");

            let factory = reg.get_factory(&self.plugin_id).ok_or_else(|| {
                ComSrvError::ConfigError(format!("Plugin factory not found: {}", self.plugin_id))
            })?;

            // Create plugin instance
            factory()
        }; // RwLockReadGuard is released here

        // Use plugin to create protocol instance
        info!(
            "Using plugin {} to create protocol instance",
            self.plugin_id
        );
        // Use the new create_client method
        let instance = plugin
            .create_client(Arc::new(channel_config.clone()))
            .await?;
        info!(
            "Plugin {} created client instance successfully",
            self.plugin_id
        );
        Ok(instance)
    }

    fn validate_config(&self, _config: &ConfigValue) -> Result<()> {
        // TODO: Call plugin's configuration validation
        Ok(())
    }

    fn get_config_template(&self) -> ConfigValue {
        // TODO: Get configuration template from plugin
        serde_json::json!({})
    }
}

// ============================================================================
// gRPC plugin factory
// ============================================================================

/// gRPC plugin factory
/// Used to create remote plugin clients connected via gRPC
#[derive(Debug)]
pub struct GrpcPluginFactory {
    protocol_type: ProtocolType,
    endpoint: String,
    plugin_protocol: String,
}

impl GrpcPluginFactory {
    pub fn new(protocol_type: ProtocolType, endpoint: String, plugin_protocol: String) -> Self {
        Self {
            protocol_type,
            endpoint,
            plugin_protocol,
        }
    }
}

#[async_trait]
impl ProtocolClientFactory for GrpcPluginFactory {
    fn protocol_type(&self) -> ProtocolType {
        self.protocol_type
    }

    async fn create_client(
        &self,
        _channel_config: &ChannelConfig,
        _config_value: ConfigValue,
    ) -> Result<Box<dyn ComClient>> {
        use crate::plugins::grpc::adapter::GrpcPluginAdapter;

        info!(
            "Creating gRPC plugin client for protocol {} at {}",
            self.plugin_protocol, self.endpoint
        );

        let adapter = GrpcPluginAdapter::new(&self.endpoint, &self.plugin_protocol).await?;
        Ok(Box::new(adapter))
    }

    fn validate_config(&self, _config: &ConfigValue) -> Result<()> {
        // TODO: Validate gRPC endpoint format
        Ok(())
    }

    fn get_config_template(&self) -> ConfigValue {
        serde_json::json!({
            "endpoint": "http://localhost:50051",
            "protocol": self.plugin_protocol,
            "timeout": 30,
            "retry_count": 3
        })
    }
}

// ============================================================================
// Test support
// ============================================================================

#[cfg(any(test, feature = "test-utils"))]
pub mod test_support {
    use super::{
        Arc, ChannelConfig, ComClient, ConfigValue, ProtocolClientFactory, ProtocolType, Result,
        RwLock,
    };
    use crate::core::combase::traits::{ChannelStatus, PointData, RedisValue};
    use crate::core::combase::ComBase;
    use crate::core::config::types::TelemetryType;
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicBool, Ordering};

    /// Mock communication base implementation for testing
    #[derive(Debug)]
    pub struct MockComBase {
        name: String,
        #[allow(dead_code)]
        channel_id: u16,
        protocol_type: String,
        is_connected: AtomicBool,
        status: Arc<RwLock<ChannelStatus>>,
    }

    impl MockComBase {
        pub fn new(name: &str, channel_id: u16, protocol_type: &str) -> Self {
            Self {
                name: name.to_string(),
                channel_id,
                protocol_type: protocol_type.to_string(),
                is_connected: AtomicBool::new(false),
                status: Arc::new(RwLock::new(ChannelStatus::default())),
            }
        }
    }

    #[async_trait]
    impl ComBase for MockComBase {
        fn name(&self) -> &str {
            &self.name
        }

        fn protocol_type(&self) -> &str {
            &self.protocol_type
        }

        fn get_channel_id(&self) -> u16 {
            0 // Mock implementation returns 0
        }

        async fn get_status(&self) -> ChannelStatus {
            self.status.read().await.clone()
        }

        async fn initialize(&mut self, _channel_config: Arc<ChannelConfig>) -> Result<()> {
            Ok(())
        }

        async fn read_four_telemetry(
            &self,
            _telemetry_type: TelemetryType,
        ) -> Result<HashMap<u32, PointData>> {
            Ok(HashMap::new())
        }
    }

    #[async_trait]
    impl ComClient for MockComBase {
        fn is_connected(&self) -> bool {
            self.is_connected.load(Ordering::Relaxed)
        }

        async fn connect(&mut self) -> Result<()> {
            self.is_connected.store(true, Ordering::Relaxed);
            let mut status = self.status.write().await;
            status.is_connected = true;
            Ok(())
        }

        async fn disconnect(&mut self) -> Result<()> {
            self.is_connected.store(false, Ordering::Relaxed);
            let mut status = self.status.write().await;
            status.is_connected = false;
            Ok(())
        }

        async fn control(&mut self, commands: Vec<(u32, RedisValue)>) -> Result<Vec<(u32, bool)>> {
            Ok(commands.into_iter().map(|(id, _)| (id, true)).collect())
        }

        async fn adjustment(
            &mut self,
            adjustments: Vec<(u32, RedisValue)>,
        ) -> Result<Vec<(u32, bool)>> {
            Ok(adjustments.into_iter().map(|(id, _)| (id, true)).collect())
        }
    }

    /// Mock protocol factory
    #[derive(Debug)]
    pub struct MockProtocolFactory;

    #[async_trait]
    impl ProtocolClientFactory for MockProtocolFactory {
        fn protocol_type(&self) -> ProtocolType {
            // Use Virtual protocol type to avoid overwriting real Modbus
            ProtocolType::Virtual
        }

        async fn create_client(
            &self,
            channel_config: &ChannelConfig,
            _config_value: ConfigValue,
        ) -> Result<Box<dyn ComClient>> {
            Ok(Box::new(MockComBase::new(
                &channel_config.name,
                channel_config.id,
                "mock",
            )))
        }

        fn validate_config(&self, _config: &ConfigValue) -> Result<()> {
            Ok(())
        }

        fn get_config_template(&self) -> ConfigValue {
            serde_json::json!({
                "type": "mock",
                "parameters": {}
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::*;
    use super::{Arc, ProtocolFactory, ProtocolType};

    #[tokio::test]
    async fn test_protocol_factory_creation() {
        let factory = ProtocolFactory::new();
        assert_eq!(factory.get_channel_ids().len(), 0);
        // Factory initialization registers built-in protocols (like modbus_tcp, modbus_rtu, virtual)
        // So it shouldn't expect 0 here
        // Check that we can get registered protocols without error
        let protocols = factory.get_registered_protocols();
        // Factory should have some built-in protocols registered
        assert!(!protocols.is_empty()); // Should have at least one protocol
    }

    #[tokio::test]
    async fn test_register_protocol() {
        let factory = ProtocolFactory::new();
        let mock_factory = Arc::new(MockProtocolFactory);

        factory.register_protocol_factory(mock_factory);

        let protocols = factory.get_registered_protocols();
        // After registering mock factory, we should have at least Virtual
        assert!(!protocols.is_empty());
        // Mock factory registers Virtual protocol
        assert!(protocols.contains(&ProtocolType::Virtual));
    }

    #[tokio::test]
    async fn test_create_channel() {
        // This test now only tests the factory pattern without Redis
        let factory = ProtocolFactory::new();

        // Just test that we can register protocol factories
        let mock_factory = Arc::new(MockProtocolFactory);
        factory.register_protocol_factory(mock_factory);

        // We can verify the factory is registered by checking it doesn't panic
        // when we try to use it. Full channel creation requires Redis,
        // so we keep this test minimal
    }
}
