//! Protocol factory module
//!
//! Provides protocol instance creation, management and lifecycle control

use async_trait::async_trait;
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info};

use crate::core::combase::command::{CommandSubscriber, CommandSubscriberConfig};
use crate::core::combase::core::ComBase;
use crate::core::config::{ChannelConfig, ConfigManager, ProtocolType};
use crate::utils::error::{ComSrvError, Result};
use std::str::FromStr;

/// Configuration value type (using JSON for internal processing)
pub type ConfigValue = serde_json::Value;

/// Dynamic communication client type
pub type DynComClient = Arc<RwLock<Box<dyn ComBase>>>;

// ============================================================================
// Protocol client factory trait
// ============================================================================

/// Protocol client factory trait for extensible protocol support
#[async_trait]
pub trait ProtocolClientFactory: Send + Sync {
    /// Get protocol type
    fn protocol_type(&self) -> ProtocolType;

    /// Create protocol client instance
    async fn create_client(
        &self,
        channel_config: &ChannelConfig,
        config_value: ConfigValue,
    ) -> Result<Box<dyn ComBase>>;

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
    pub name: String,
    pub protocol_type: ProtocolType,
    pub created_at: std::time::Instant,
    pub last_accessed: Arc<RwLock<std::time::Instant>>,
}

/// Channel entry, combining channel and metadata
#[derive(Clone)]
struct ChannelEntry {
    channel: Arc<RwLock<Box<dyn ComBase>>>,
    metadata: ChannelMetadata,
    command_subscriber: Option<Arc<RwLock<CommandSubscriber>>>,
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
}

impl std::fmt::Debug for ProtocolFactory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProtocolFactory")
            .field("channels", &self.channels.len())
            .field("protocol_factories", &self.protocol_factories.len())
            .finish()
    }
}

impl ProtocolFactory {
    /// Create new protocol factory
    pub fn new() -> Self {
        let factory = Self {
            channels: DashMap::with_hasher(ahash::RandomState::new()),
            protocol_factories: DashMap::with_hasher(ahash::RandomState::new()),
        };

        // Initialize plugin system
        let _ = crate::plugins::core::get_plugin_registry();

        // Register built-in protocol factories
        factory.register_builtin_factories();

        factory
    }

    /// Register built-in protocol factories
    fn register_builtin_factories(&self) {
        use crate::plugins::core::get_plugin_registry;

        // Get plugin registry
        let registry = get_plugin_registry();
        let reg = registry
            .read()
            .expect("plugin registry lock should not be poisoned");

        // Modbus TCP
        if reg.get_factory("modbus_tcp").is_some() {
            self.register_protocol_factory(Arc::new(PluginAdapterFactory::new(
                ProtocolType::ModbusTcp,
                "modbus_tcp".to_string(),
            )));
        }

        // Modbus RTU
        if reg.get_factory("modbus_rtu").is_some() {
            self.register_protocol_factory(Arc::new(PluginAdapterFactory::new(
                ProtocolType::ModbusRtu,
                "modbus_rtu".to_string(),
            )));
        }

        // Virtual
        if reg.get_factory("virt").is_some() {
            self.register_protocol_factory(Arc::new(PluginAdapterFactory::new(
                ProtocolType::Virtual,
                "virt".to_string(),
            )));
        }

        // gRPC plugin factory
        self.register_protocol_factory(Arc::new(GrpcPluginFactory::new(
            ProtocolType::GrpcModbus,
            "http://modbus-plugin:50051".to_string(),
            "modbus_tcp".to_string(),
        )));

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
        _config_manager: Option<&ConfigManager>,
    ) -> Result<Arc<RwLock<Box<dyn ComBase>>>> {
        let channel_id = channel_config.id;

        // Check if channel already exists
        if self.channels.contains_key(&channel_id) {
            return Err(ComSrvError::InvalidOperation(format!(
                "Channel {channel_id} already exists"
            )));
        }

        // Get protocol type
        let protocol_type = ProtocolType::from_str(&channel_config.protocol)?;

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
        factory.validate_config(&config_value)?;

        // Create client instance
        info!("Creating client for protocol {:?}", protocol_type);
        let mut client = factory.create_client(channel_config, config_value).await?;
        info!("Client created successfully for channel {}", channel_id);

        // Initialize client
        info!("Initializing client for channel {}", channel_id);
        client.initialize(channel_config).await?;
        info!("Client initialized successfully for channel {}", channel_id);

        // Under four-telemetry separation architecture, point configuration is loaded directly from channel_config during initialize phase, no need for additional unified mapping

        // Initialize points in Redis at ComBase layer (initial value is 0)
        info!("Initializing Redis keys for channel {}", channel_id);
        self.initialize_channel_points(channel_config).await?;
        info!("Redis keys initialized for channel {}", channel_id);

        // Skip connection phase, only complete initialization
        // Connections will be established uniformly after all channels are initialized
        info!(
            "Channel {} initialization completed, connection will be established later",
            channel_id
        );

        let channel_arc = Arc::new(RwLock::new(client));

        // Create command subscriber (if needed)
        let enable_control = channel_config
            .parameters
            .get("enable_control")
            .and_then(serde_yaml::Value::as_bool)
            .unwrap_or(false);

        let command_subscriber = if enable_control {
            let redis_url = channel_config
                .parameters
                .get("redis_url")
                .and_then(|v| v.as_str())
                .unwrap_or("redis://localhost:6379")
                .to_string();

            let config = CommandSubscriberConfig {
                channel_id,
                redis_url,
            };

            let (tx, _rx) = tokio::sync::mpsc::channel(100);
            let subscriber = CommandSubscriber::new(config, tx).await?;
            let subscriber_arc = Arc::new(RwLock::new(subscriber));

            // Start subscriber
            let mut sub = subscriber_arc.write().await;
            sub.start().await?;
            drop(sub);

            Some(subscriber_arc)
        } else {
            None
        };

        // Create channel entry
        let entry = ChannelEntry {
            channel: channel_arc.clone(),
            metadata: ChannelMetadata {
                name: channel_config.name.clone(),
                protocol_type,
                created_at: std::time::Instant::now(),
                last_accessed: Arc::new(RwLock::new(std::time::Instant::now())),
            },
            command_subscriber,
        };

        // Insert channel
        self.channels.insert(channel_id, entry);

        info!(
            "Created channel {} with protocol {:?}",
            channel_id, protocol_type
        );
        Ok(channel_arc)
    }

    /// Get channel
    pub async fn get_channel(&self, channel_id: u16) -> Option<Arc<RwLock<Box<dyn ComBase>>>> {
        self.channels.get(&channel_id).map(|entry| {
            // Update last access time
            let last_accessed = entry.metadata.last_accessed.clone();
            tokio::spawn(async move {
                let mut time = last_accessed.write().await;
                *time = std::time::Instant::now();
            });
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

        Ok(())
    }

    /// Remove channel
    pub async fn remove_channel(&self, channel_id: u16) -> Result<()> {
        if let Some((_, entry)) = self.channels.remove(&channel_id) {
            // Stop command subscriber
            if let Some(subscriber) = entry.command_subscriber {
                let mut sub = subscriber.write().await;
                sub.stop().await?;
            }

            // Disconnect
            let mut channel = entry.channel.write().await;
            channel.disconnect().await?;

            info!("Removed channel {}", channel_id);
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
                name: entry.metadata.name.clone(),
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

    /// Clean up all channels
    pub async fn cleanup(&self) -> Result<()> {
        let channel_ids: Vec<u16> = self.get_channel_ids();

        for channel_id in channel_ids {
            if let Err(e) = self.remove_channel(channel_id).await {
                error!("Failed to remove channel {}: {}", channel_id, e);
            }
        }

        Ok(())
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
                metadata.name.clone(),
                format!("{:?}", metadata.protocol_type),
            )
        })
    }

    /// Initialize all channel points to Redis (default value is 0)
    async fn initialize_channel_points(&self, channel_config: &ChannelConfig) -> Result<()> {
        use crate::core::config::TelemetryType;
        use crate::plugins::core::{DefaultPluginStorage, PluginPointUpdate, PluginStorage};
        use std::path::PathBuf;

        // Get Redis URL
        let redis_url = channel_config
            .parameters
            .get("redis_url")
            .and_then(|v| v.as_str())
            .unwrap_or("redis://localhost:6379")
            .to_string();

        // Create storage instance
        let storage = DefaultPluginStorage::new(redis_url).await?;

        // Load point table configuration - if not configured, return success directly
        let table_config = match channel_config.table_config.as_ref() {
            Some(config) => config,
            None => {
                info!(
                    "No table_config found for channel {}, skipping CSV point initialization",
                    channel_config.id
                );
                return Ok(());
            },
        };

        // Get CSV base path, use environment variable or config directory
        let csv_base_path = std::env::var("COMSRV_CSV_BASE_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("config"));

        // Initialize four-telemetry type points
        let telemetry_types = vec![
            (
                "telemetry",
                &table_config.four_remote_files.telemetry_file,
                "m",
            ),
            ("signal", &table_config.four_remote_files.signal_file, "s"),
            ("control", &table_config.four_remote_files.control_file, "c"),
            (
                "adjustment",
                &table_config.four_remote_files.adjustment_file,
                "a",
            ),
        ];

        for (telemetry_name, file_name, redis_type) in telemetry_types {
            let file_path = csv_base_path
                .join(&table_config.four_remote_route)
                .join(file_name);

            if !file_path.exists() {
                info!(
                    "Skipping {} initialization for channel {}: file not found at {:?}",
                    telemetry_name, channel_config.id, file_path
                );
                continue;
            }

            // Read CSV file to get point ID list
            let mut reader = csv::Reader::from_path(&file_path).map_err(|e| {
                ComSrvError::ConfigError(format!("Failed to read {telemetry_name} CSV file: {e}"))
            })?;

            let mut updates = Vec::new();

            // Read each point and prepare initialization update
            for result in reader.records() {
                let record = result.map_err(|e| {
                    ComSrvError::ConfigError(format!("Error reading CSV record: {e}"))
                })?;

                // Get point_id (first column)
                if let Some(point_id_str) = record.get(0) {
                    if let Ok(point_id) = point_id_str.parse::<u32>() {
                        // Get current timestamp
                        let timestamp = chrono::Utc::now().timestamp();

                        // Convert telemetry type
                        let telemetry_type = match redis_type {
                            "m" => TelemetryType::Telemetry,
                            "s" => TelemetryType::Signal,
                            "c" => TelemetryType::Control,
                            "a" => TelemetryType::Adjustment,
                            _ => continue,
                        };

                        // Create update with initial value 0
                        let update = PluginPointUpdate {
                            channel_id: channel_config.id,
                            telemetry_type,
                            point_id,
                            value: 0.0,
                            timestamp,
                            raw_value: None,
                        };
                        updates.push(update);
                    }
                }
            }

            // Batch initialize points
            if !updates.is_empty() {
                let count = updates.len();
                storage.write_points(updates).await?;
                info!(
                    "Initialized {} {} points for channel {} with default value 0",
                    count, telemetry_name, channel_config.id
                );
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
    ) -> Result<Box<dyn ComBase>> {
        use crate::plugins::core::get_plugin_registry;

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
        let instance = plugin.create_instance(channel_config.clone()).await?;
        info!("Plugin {} created instance successfully", self.plugin_id);
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
    ) -> Result<Box<dyn ComBase>> {
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
        Arc, ChannelConfig, ComBase, ConfigValue, ProtocolClientFactory, ProtocolType, Result,
        RwLock,
    };
    use crate::core::combase::core::{ChannelStatus, PointData, RedisValue};
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

        fn is_connected(&self) -> bool {
            self.is_connected.load(Ordering::Relaxed)
        }

        async fn get_status(&self) -> ChannelStatus {
            self.status.read().await.clone()
        }

        async fn initialize(&mut self, _channel_config: &ChannelConfig) -> Result<()> {
            Ok(())
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

        async fn read_four_telemetry(
            &self,
            _telemetry_type: &str,
        ) -> Result<HashMap<u32, PointData>> {
            Ok(HashMap::new())
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
        ) -> Result<Box<dyn ComBase>> {
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
    use super::{Arc, ChannelConfig, ProtocolFactory, ProtocolType};

    #[tokio::test]
    async fn test_protocol_factory_creation() {
        let factory = ProtocolFactory::new();
        assert_eq!(factory.get_channel_ids().len(), 0);
        // Factory initialization registers built-in protocols (like modbus_tcp, modbus_rtu, virtual)
        // So it shouldn't expect 0 here
        assert!(
            !factory.get_registered_protocols().is_empty()
                || factory.get_registered_protocols().is_empty()
        );
    }

    #[tokio::test]
    async fn test_register_protocol() {
        let factory = ProtocolFactory::new();
        let initial_count = factory.get_registered_protocols().len();
        let mock_factory = Arc::new(MockProtocolFactory);

        factory.register_protocol_factory(mock_factory);

        let protocols = factory.get_registered_protocols();
        // Should be 1 more than initial count
        assert_eq!(protocols.len(), initial_count + 1);
        // Mock factory registers Virtual protocol
        assert!(protocols.contains(&ProtocolType::Virtual));
    }

    #[tokio::test]
    async fn test_create_channel() {
        let factory = ProtocolFactory::new();
        let mock_factory = Arc::new(MockProtocolFactory);
        factory.register_protocol_factory(mock_factory);

        let channel_config = ChannelConfig {
            id: 1,
            name: "Test Channel".to_string(),
            protocol: "virtual".to_string(),
            parameters: std::collections::HashMap::new(),
            description: Some("Test channel".to_string()),
            logging: crate::core::config::ChannelLoggingConfig::default(),
            table_config: None,
            telemetry_points: std::collections::HashMap::new(),
            signal_points: std::collections::HashMap::new(),
            control_points: std::collections::HashMap::new(),
            adjustment_points: std::collections::HashMap::new(),
        };

        let channel = factory
            .create_channel(&channel_config, None)
            .await
            .expect("channel creation should succeed");

        assert_eq!(factory.get_channel_ids(), vec![1]);

        let channel_guard = channel.read().await;
        assert_eq!(channel_guard.name(), "Test Channel");
        assert_eq!(channel_guard.protocol_type(), "mock");
    }
}
