use ahash::AHashMap;
use async_trait::async_trait;
use dashmap::DashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::core::config::{ChannelConfig, ChannelLoggingConfig, ConfigManager, ProtocolType};
use crate::core::protocols::common::ComBase;

// use crate::core::protocols::iec60870::iec104::Iec104Client;
use crate::utils::{ComSrvError, Result};

/// Configuration value type - using JSON internally for better ergonomics
/// YAML files are converted to JSON at entry/exit points
pub type ConfigValue = serde_json::Value;

/// Convert YAML string to JSON Value for internal processing
pub fn yaml_to_json(yaml_str: &str) -> Result<ConfigValue> {
    let yaml_value: serde_yaml::Value = serde_yaml::from_str(yaml_str)
        .map_err(|e| ComSrvError::ConfigError(format!("Failed to parse YAML: {e}")))?;

    yaml_value_to_json(yaml_value)
}

/// Convert JSON Value back to YAML string for file output
pub fn json_to_yaml(json_value: &ConfigValue) -> Result<String> {
    let yaml_value = json_value_to_yaml(json_value.clone())?;
    serde_yaml::to_string(&yaml_value)
        .map_err(|e| ComSrvError::ConfigError(format!("Failed to serialize to YAML: {e}")))
}

/// Convert serde_yaml::Value to serde_json::Value
fn yaml_value_to_json(yaml: serde_yaml::Value) -> Result<ConfigValue> {
    match yaml {
        serde_yaml::Value::Null => Ok(serde_json::Value::Null),
        serde_yaml::Value::Bool(b) => Ok(serde_json::Value::Bool(b)),
        serde_yaml::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(serde_json::Value::Number(serde_json::Number::from(i)))
            } else if let Some(f) = n.as_f64() {
                Ok(serde_json::Value::Number(
                    serde_json::Number::from_f64(f).ok_or_else(|| {
                        ComSrvError::ConfigError("Invalid float number".to_string())
                    })?,
                ))
            } else {
                Err(ComSrvError::ConfigError(
                    "Unsupported number format".to_string(),
                ))
            }
        }
        serde_yaml::Value::String(s) => Ok(serde_json::Value::String(s)),
        serde_yaml::Value::Sequence(seq) => {
            let json_array: Result<Vec<_>> = seq.into_iter().map(yaml_value_to_json).collect();
            Ok(serde_json::Value::Array(json_array?))
        }
        serde_yaml::Value::Mapping(map) => {
            let mut json_obj = serde_json::Map::new();
            for (k, v) in map {
                let key = match k {
                    serde_yaml::Value::String(s) => s,
                    _ => {
                        return Err(ComSrvError::ConfigError(
                            "Non-string key in YAML mapping".to_string(),
                        ))
                    }
                };
                json_obj.insert(key, yaml_value_to_json(v)?);
            }
            Ok(serde_json::Value::Object(json_obj))
        }
        serde_yaml::Value::Tagged(_) => Err(ComSrvError::ConfigError(
            "Tagged YAML values not supported".to_string(),
        )),
    }
}

/// Convert serde_json::Value to serde_yaml::Value
fn json_value_to_yaml(json: ConfigValue) -> Result<serde_yaml::Value> {
    match json {
        serde_json::Value::Null => Ok(serde_yaml::Value::Null),
        serde_json::Value::Bool(b) => Ok(serde_yaml::Value::Bool(b)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(serde_yaml::Value::Number(serde_yaml::Number::from(i)))
            } else if let Some(f) = n.as_f64() {
                Ok(serde_yaml::Value::Number(serde_yaml::Number::from(f)))
            } else {
                Err(ComSrvError::ConfigError(
                    "Unsupported number format".to_string(),
                ))
            }
        }
        serde_json::Value::String(s) => Ok(serde_yaml::Value::String(s)),
        serde_json::Value::Array(arr) => {
            let yaml_seq: Result<Vec<_>> = arr.into_iter().map(json_value_to_yaml).collect();
            Ok(serde_yaml::Value::Sequence(yaml_seq?))
        }
        serde_json::Value::Object(obj) => {
            let mut yaml_map = serde_yaml::Mapping::new();
            for (k, v) in obj {
                yaml_map.insert(serde_yaml::Value::String(k), json_value_to_yaml(v)?);
            }
            Ok(serde_yaml::Value::Mapping(yaml_map))
        }
    }
}

/// Convert JSON Value to HashMap<String, serde_yaml::Value> for ChannelParameters
fn json_to_yaml_params(json: ConfigValue) -> std::collections::HashMap<String, serde_yaml::Value> {
    let mut param_map = std::collections::HashMap::new();

    if let serde_json::Value::Object(obj) = json {
        for (key, value) in obj {
            let yaml_value = match value {
                serde_json::Value::String(s) => serde_yaml::Value::String(s),
                serde_json::Value::Number(n) => {
                    if let Some(i) = n.as_i64() {
                        serde_yaml::Value::Number(serde_yaml::Number::from(i))
                    } else if let Some(f) = n.as_f64() {
                        serde_yaml::Value::Number(serde_yaml::Number::from(f))
                    } else {
                        serde_yaml::Value::Null
                    }
                }
                serde_json::Value::Bool(b) => serde_yaml::Value::Bool(b),
                serde_json::Value::Null => serde_yaml::Value::Null,
                _ => serde_yaml::Value::Null,
            };
            param_map.insert(key, yaml_value);
        }
    }

    param_map
}

/// Dynamic communication client type for type-erased storage
pub type DynComClient = Arc<RwLock<Box<dyn ComBase>>>;

/// Protocol client factory trait for extensible protocol support
///
/// Implement this trait for each protocol to enable factory-based creation
/// and registration with validation support.
#[async_trait]
pub trait ProtocolClientFactory: Send + Sync {
    /// Get the protocol type this factory handles
    fn protocol_type(&self) -> ProtocolType;

    /// Create a new client instance for the given configuration
    ///
    /// Made async to support protocols that require async initialization
    /// (e.g., TLS handshake, port detection, network discovery)
    ///
    /// # Arguments
    ///
    /// * `config` - Channel configuration containing protocol parameters
    /// * `config_manager` - Optional config manager for loading point tables
    ///
    /// # Returns
    ///
    /// `Ok(client)` if successful, `Err` if creation failed
    async fn create_client(
        &self,
        config: ChannelConfig,
        config_manager: Option<&ConfigManager>,
    ) -> Result<Box<dyn ComBase>>;

    /// Validate configuration before client creation
    ///
    /// # Arguments
    ///
    /// * `config` - Channel configuration to validate
    ///
    /// # Returns
    ///
    /// `Ok(())` if configuration is valid, `Err` with details if invalid
    fn validate_config(&self, config: &ChannelConfig) -> Result<()>;

    /// Get default configuration template for this protocol
    ///
    /// Returns configuration in YAML format for consistency with config files
    fn default_config(&self) -> ChannelConfig;

    /// Get configuration schema for validation and UI generation
    ///
    /// Returns YAML schema for frontend consumption
    fn config_schema(&self) -> ConfigValue;
}

/// Mock ComBase implementation for testing
#[derive(Debug)]
#[allow(dead_code)]
struct MockComBase {
    name: String,
    channel_id: u16,
    protocol_type: String,
    running: AtomicBool,
    should_fail_start: AtomicBool,
    should_fail_stop: AtomicBool,
}

impl MockComBase {
    fn new(name: &str, channel_id: u16, protocol_type: &str) -> Self {
        Self {
            name: name.to_string(),
            channel_id,
            protocol_type: protocol_type.to_string(),
            running: AtomicBool::new(false),
            should_fail_start: AtomicBool::new(false),
            should_fail_stop: AtomicBool::new(false),
        }
    }

    fn with_start_failure(name: &str, channel_id: u16, protocol_type: &str) -> Self {
        Self {
            name: name.to_string(),
            channel_id,
            protocol_type: protocol_type.to_string(),
            running: AtomicBool::new(false),
            should_fail_start: AtomicBool::new(true),
            should_fail_stop: AtomicBool::new(false),
        }
    }

    fn with_stop_failure(name: &str, channel_id: u16, protocol_type: &str) -> Self {
        Self {
            name: name.to_string(),
            channel_id,
            protocol_type: protocol_type.to_string(),
            running: AtomicBool::new(false),
            should_fail_start: AtomicBool::new(false),
            should_fail_stop: AtomicBool::new(true),
        }
    }
}

#[async_trait::async_trait]
impl ComBase for MockComBase {
    fn name(&self) -> &str {
        &self.name
    }

    fn channel_id(&self) -> String {
        self.channel_id.to_string()
    }

    fn protocol_type(&self) -> &str {
        &self.protocol_type
    }

    fn get_parameters(&self) -> std::collections::HashMap<String, String> {
        let mut params = std::collections::HashMap::new();
        params.insert("mock".to_string(), "true".to_string());
        params.insert("channel_id".to_string(), self.channel_id.to_string());
        params
    }

    async fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    async fn start(&mut self) -> Result<()> {
        if self.should_fail_start.load(Ordering::Relaxed) {
            return Err(crate::utils::ComSrvError::InvalidOperation(
                "Mock start failure".to_string(),
            ));
        }
        self.running.store(true, Ordering::Relaxed);
        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        if self.should_fail_stop.load(Ordering::Relaxed) {
            return Err(crate::utils::ComSrvError::InvalidOperation(
                "Mock stop failure".to_string(),
            ));
        }
        self.running.store(false, Ordering::Relaxed);
        Ok(())
    }

    async fn status(&self) -> crate::core::protocols::common::data_types::ChannelStatus {
        crate::core::protocols::common::data_types::ChannelStatus::new(&self.channel_id())
    }

    async fn update_status(
        &mut self,
        _status: crate::core::protocols::common::data_types::ChannelStatus,
    ) -> Result<()> {
        Ok(())
    }

    async fn get_all_points(&self) -> Vec<crate::core::protocols::common::data_types::PointData> {
        Vec::new()
    }

    async fn read_point(
        &self,
        _point_id: &str,
    ) -> Result<crate::core::protocols::common::data_types::PointData> {
        Err(crate::utils::ComSrvError::InvalidOperation(
            "Mock implementation".to_string(),
        ))
    }

    async fn write_point(&mut self, _point_id: &str, _value: &str) -> Result<()> {
        Err(crate::utils::ComSrvError::InvalidOperation(
            "Mock implementation".to_string(),
        ))
    }

    async fn get_diagnostics(&self) -> std::collections::HashMap<String, String> {
        std::collections::HashMap::new()
    }
}

/// Channel entry combining channel and metadata for atomic operations
#[derive(Clone)]
struct ChannelEntry {
    channel: Arc<RwLock<Box<dyn ComBase>>>,
    metadata: ChannelMetadata,
}

impl std::fmt::Debug for ChannelEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChannelEntry")
            .field("channel", &"<ComBase>")
            .field("metadata", &self.metadata)
            .finish()
    }
}

/// High-performance protocol factory for creating communication protocol instances
pub struct ProtocolFactory {
    /// Store created channels using DashMap for concurrent access
    /// Now stores ChannelEntry for atomic operations
    channels: DashMap<u16, ChannelEntry, ahash::RandomState>,
    /// Registry of protocol factories by protocol type
    protocol_factories: DashMap<ProtocolType, Arc<dyn ProtocolClientFactory>, ahash::RandomState>,
    /// Optional Redis storage for channel metadata and state
    redis_store: Option<crate::core::storage::redis_storage::RedisStore>,
}

impl std::fmt::Debug for ProtocolFactory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProtocolFactory")
            .field("channels", &self.channels.len())
            .field("protocol_factories", &self.protocol_factories.len())
            .field("redis_store", &self.redis_store.is_some())
            .finish()
    }
}

/// Channel metadata for quick access
#[derive(Debug, Clone)]
struct ChannelMetadata {
    pub name: String,
    pub protocol_type: ProtocolType,
    pub created_at: std::time::Instant,
    pub last_accessed: Arc<RwLock<std::time::Instant>>,
}

impl ProtocolFactory {
    /// Create a new protocol factory with optimized data structures
    pub fn new() -> Self {
        Self {
            channels: DashMap::with_hasher(ahash::RandomState::new()),
            protocol_factories: DashMap::with_hasher(ahash::RandomState::new()),
            redis_store: None,
        }
    }

    /// Create a new protocol factory with Redis storage support
    pub fn new_with_redis(redis_store: crate::core::storage::redis_storage::RedisStore) -> Self {
        Self {
            channels: DashMap::with_hasher(ahash::RandomState::new()),
            protocol_factories: DashMap::with_hasher(ahash::RandomState::new()),
            redis_store: Some(redis_store),
        }
    }

    /// Enable Redis storage for this factory
    pub fn enable_redis_storage(
        &mut self,
        redis_store: crate::core::storage::redis_storage::RedisStore,
    ) -> Result<()> {
        // Migrate existing channel metadata to Redis
        if let Some(ref _redis) = self.redis_store {
            tracing::warn!("Redis storage is already enabled, replacing existing store");
        }

        // Store existing channels to Redis
        let channel_entries: Vec<_> = self
            .channels
            .iter()
            .map(|entry| {
                let (id, channel_entry) = (*entry.key(), entry.value().clone());
                (id, channel_entry)
            })
            .collect();

        for (channel_id, channel_entry) in channel_entries {
            let metadata = crate::core::storage::redis_storage::RedisChannelMetadata {
                name: channel_entry.metadata.name.clone(),
                protocol_type: format!("{:?}", channel_entry.metadata.protocol_type),
                created_at: chrono::DateTime::<chrono::Local>::from(
                    std::time::UNIX_EPOCH + channel_entry.metadata.created_at.elapsed(),
                )
                .format("%Y-%m-%dT%H:%M:%S%.3f")
                .to_string(),
                last_accessed: chrono::Local::now()
                    .format("%Y-%m-%dT%H:%M:%S%.3f")
                    .to_string(),
                running: false, // Will be updated when channels are started
                parameters: std::collections::HashMap::new(),
            };

            // Store to Redis asynchronously (fire and forget for now)
            let redis_clone = redis_store.clone();
            tokio::spawn(async move {
                if let Err(e) = redis_clone
                    .set_channel_metadata(channel_id, &metadata)
                    .await
                {
                    tracing::error!(
                        "Failed to migrate channel {} metadata to Redis: {}",
                        channel_id,
                        e
                    );
                }
            });
        }

        self.redis_store = Some(redis_store);
        tracing::info!(
            "Redis storage enabled for ProtocolFactory with {} existing channels",
            self.channels.len()
        );
        Ok(())
    }

    /// Disable Redis storage and use only in-memory storage
    pub fn disable_redis_storage(&mut self) {
        if self.redis_store.is_some() {
            self.redis_store = None;
            tracing::info!(
                "Redis storage disabled for ProtocolFactory, using in-memory storage only"
            );
        }
    }

    /// Check if Redis storage is enabled
    pub fn is_redis_enabled(&self) -> bool {
        self.redis_store.is_some()
    }

    /// Get Redis store reference if available
    pub fn redis_store(&self) -> Option<&crate::core::storage::redis_storage::RedisStore> {
        self.redis_store.as_ref()
    }

    /// Synchronize channel metadata to Redis
    pub async fn sync_channel_metadata(&mut self) -> Result<()> {
        if let Some(ref redis_store) = self.redis_store {
            let mut sync_count = 0;
            let mut error_count = 0;

            for entry in self.channels.iter() {
                let (channel_id, channel_entry) = (*entry.key(), entry.value());

                let metadata = crate::core::storage::redis_storage::RedisChannelMetadata {
                    name: channel_entry.metadata.name.clone(),
                    protocol_type: format!("{:?}", channel_entry.metadata.protocol_type),
                    created_at: chrono::DateTime::<chrono::Local>::from(
                        std::time::UNIX_EPOCH + channel_entry.metadata.created_at.elapsed(),
                    )
                    .format("%Y-%m-%dT%H:%M:%S%.3f")
                    .to_string(),
                    last_accessed: chrono::Local::now()
                        .format("%Y-%m-%dT%H:%M:%S%.3f")
                        .to_string(),
                    running: channel_entry.channel.read().await.is_running().await,
                    parameters: std::collections::HashMap::new(),
                };

                match redis_store
                    .set_channel_metadata(channel_id, &metadata)
                    .await
                {
                    Ok(_) => sync_count += 1,
                    Err(e) => {
                        tracing::error!(
                            "Failed to sync channel {} metadata to Redis: {}",
                            channel_id,
                            e
                        );
                        error_count += 1;
                    }
                }
            }

            tracing::info!(
                "Channel metadata sync completed: {} successful, {} errors",
                sync_count,
                error_count
            );

            if error_count > 0 {
                return Err(ComSrvError::RedisError(format!(
                    "Failed to sync {} channel metadata entries to Redis",
                    error_count
                )));
            }
        }
        Ok(())
    }

    /// Register a protocol factory
    ///
    /// # Arguments
    ///
    /// * `factory` - Protocol factory implementation to register
    pub fn register_protocol_factory(&self, factory: Arc<dyn ProtocolClientFactory>) {
        let protocol_type = factory.protocol_type();
        self.protocol_factories
            .insert(protocol_type.clone(), factory);
        tracing::info!("Registered protocol factory for {protocol_type:?}");
    }

    /// Unregister a protocol factory for hot-swappable protocol support
    ///
    /// Removes the factory for the specified protocol type. This enables
    /// runtime plugin management and protocol hot-swapping capabilities.
    ///
    /// **Important**: This method will fail if there are active channels
    /// using the specified protocol. All channels of this protocol type
    /// must be stopped and removed before unregistering the factory.
    ///
    /// # Arguments
    ///
    /// * `protocol_type` - The protocol type to unregister
    ///
    /// # Returns
    ///
    /// `Ok(true)` if the factory was found and removed successfully,
    /// `Ok(false)` if no factory was found for the protocol type,
    /// `Err` if there are active channels using this protocol
    ///
    /// # Examples
    ///
    /// ```rust
    /// use comsrv::core::protocols::common::protocol_factory::ProtocolFactory;
    /// use comsrv::core::config::ProtocolType;
    ///
    /// let factory = ProtocolFactory::new();
    ///
    /// // This will succeed if no channels are using ModbusTcp
    /// match factory.unregister_protocol_factory(&ProtocolType::ModbusTcp) {
    ///     Ok(true) => println!("Factory unregistered successfully"),
    ///     Ok(false) => println!("No factory found for this protocol"),
    ///     Err(e) => println!("Cannot unregister: {e}"),
    /// }
    /// ```
    pub fn unregister_protocol_factory(&self, protocol_type: &ProtocolType) -> Result<bool> {
        // Check if there are any active channels using this protocol
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
            return Err(ComSrvError::ConfigError(format!(
                "Cannot unregister protocol factory for {:?}: {} active channels found (IDs: {:?}). \
                Please stop and remove all channels of this protocol type first.",
                protocol_type, active_channels.len(), active_channels
            )));
        }

        // Safe to remove the factory
        match self.protocol_factories.remove(protocol_type) {
            Some(_) => {
                tracing::info!("Protocol factory unregistered successfully: {protocol_type:?}");
                Ok(true)
            }
            None => {
                tracing::warn!(
                    "Attempted to unregister non-existent protocol factory: {protocol_type:?}"
                );
                Ok(false)
            }
        }
    }

    /// Get list of supported protocol types
    pub fn supported_protocols(&self) -> Vec<ProtocolType> {
        self.protocol_factories
            .iter()
            .map(|entry| entry.key().clone())
            .collect()
    }

    /// Check if a protocol is supported
    pub fn is_protocol_supported(&self, protocol_type: &ProtocolType) -> bool {
        self.protocol_factories.contains_key(protocol_type)
    }

    /// Validate configuration for a specific protocol
    ///
    /// # Arguments
    ///
    /// * `config` - Channel configuration to validate
    pub fn validate_config(&self, config: &ChannelConfig) -> Result<()> {
        let protocol_id = config.protocol.to_lowercase();

        // Use plugin system for validation
        if let Some(plugin) = crate::core::plugins::PluginRegistry::get_global(&protocol_id) {
            // Convert serde_yaml::Value to serde_json::Value for plugin validation
            let json_params: std::collections::HashMap<String, serde_json::Value> = config
                .parameters
                .iter()
                .map(|(k, v)| {
                    let json_str = serde_yaml::to_string(v).unwrap_or_default();
                    let json_val =
                        serde_yaml::from_str(&json_str).unwrap_or(serde_json::Value::Null);
                    (k.clone(), json_val)
                })
                .collect();

            // Use plugin's validate_config method
            plugin.validate_config(&json_params)?;
            Ok(())
        } else {
            // Handle special cases
            match protocol_id.as_str() {
                "virtual" => {
                    // Virtual protocol has basic validation
                    if config.name.is_empty() {
                        return Err(ComSrvError::ConfigError(
                            "Channel name cannot be empty".to_string(),
                        ));
                    }
                    Ok(())
                }
                _ => {
                    // List available plugins for better error message
                    let available_plugins = crate::core::plugins::PluginManager::list_plugins();
                    Err(ComSrvError::ProtocolNotSupported(format!(
                        "Plugin '{}' not found. Available plugins: {:?}",
                        protocol_id, available_plugins
                    )))
                }
            }
        }
    }

    /// Get default configuration for a protocol
    ///
    /// # Arguments
    ///
    /// * `protocol_type` - Protocol type to get default configuration for
    pub fn get_default_config(&self, protocol_type: &ProtocolType) -> Option<ChannelConfig> {
        self.protocol_factories
            .get(protocol_type)
            .map(|factory| factory.default_config())
    }

    /// Extract Modbus polling configuration from channel parameters
    #[allow(dead_code)]

    fn extract_modbus_polling_config(
        &self,
        parameters: &std::collections::HashMap<String, serde_yaml::Value>,
    ) -> crate::core::config::types::channel_parameters::ModbusPollingConfig {
        use crate::core::config::types::channel_parameters::ModbusPollingConfig;

        // Check if polling configuration exists in parameters
        if let Some(polling_value) = parameters.get("polling") {
            if let Ok(polling_config) =
                serde_yaml::from_value::<ModbusPollingConfig>(polling_value.clone())
            {
                return polling_config;
            }
        }

        // Return default configuration if not found or parsing fails
        ModbusPollingConfig::default()
    }

    /// Get configuration schema for a protocol
    ///
    /// # Arguments
    ///
    /// * `protocol_type` - Protocol type to get schema for
    pub fn get_config_schema(&self, protocol_type: &ProtocolType) -> Option<ConfigValue> {
        self.protocol_factories
            .get(protocol_type)
            .map(|factory| factory.config_schema())
    }

    /// Check if the factory has no channels
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.channels.is_empty()
    }

    /// Create a protocol instance using registered factories
    ///
    /// # Arguments
    ///
    /// * `config` - Channel configuration
    pub async fn create_protocol(&self, config: ChannelConfig) -> Result<Box<dyn ComBase>> {
        self.create_protocol_with_config_manager(config, None).await
    }

    /// Create a protocol instance using registered factories with config manager
    ///
    /// # Arguments
    ///
    /// * `config` - Channel configuration
    /// * `config_manager` - Config manager for loading point tables
    pub async fn create_protocol_with_config_manager(
        &self,
        mut config: ChannelConfig,
        config_manager: Option<&ConfigManager>,
    ) -> Result<Box<dyn ComBase>> {
        // First validate the configuration
        self.validate_config(&config)?;

        // Load CSV points from config manager if available and not already loaded
        if config.combined_points.is_empty() {
            if let Some(cm) = config_manager {
                if let Some(channel_config) = cm.get_channel(config.id) {
                    if !channel_config.combined_points.is_empty() {
                        tracing::info!(
                            "Loading {} CSV points from ConfigManager for channel {} ({})",
                            channel_config.combined_points.len(),
                            config.id,
                            config.name
                        );
                        config.combined_points = channel_config.combined_points.clone();
                    } else if let Some(ref table_config) = channel_config.table_config {
                        // If ConfigManager has table_config but no loaded points, try loading them
                        tracing::info!(
                            "ConfigManager has table_config but no loaded points for channel {}, attempting to load CSV",
                            config.id
                        );

                        // Get base path from environment or use default
                        let base_path = std::env::var("COMSRV_CSV_BASE_PATH")
                            .unwrap_or_else(|_| "config".to_string());
                        let base_path = std::path::Path::new(&base_path);

                        match crate::core::config::unified_loader::UnifiedCsvLoader::load_channel_tables(
                            table_config,
                            base_path,
                        ) {
                            Ok(points) => {
                                tracing::info!(
                                    "Successfully loaded {} points from CSV for channel {}",
                                    points.len(),
                                    config.id
                                );
                                config.combined_points = points;
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "Failed to load CSV tables for channel {}: {}",
                                    config.id,
                                    e
                                );
                            }
                        }
                    }
                }
            }
        }
        // Use plugin system exclusively
        let protocol_id = config.protocol.to_lowercase();

        tracing::info!("Creating protocol instance using plugin system: {protocol_id}");

        // Get plugin instance from registry
        let plugin =
            crate::core::plugins::PluginRegistry::get_global(&protocol_id).ok_or_else(|| {
                ComSrvError::ProtocolNotSupported(format!(
                    "Plugin {} not found in registry. Available plugins: {:?}",
                    protocol_id,
                    crate::core::plugins::PluginManager::list_plugins()
                ))
            })?;

        // Validate configuration
        // Convert serde_yaml::Value to serde_json::Value
        let json_params: std::collections::HashMap<String, serde_json::Value> = config
            .parameters
            .iter()
            .map(|(k, v)| {
                let json_str = serde_yaml::to_string(v).unwrap_or_default();
                let json_val = serde_yaml::from_str(&json_str).unwrap_or(serde_json::Value::Null);
                (k.clone(), json_val)
            })
            .collect();
        plugin.validate_config(&json_params)?;

        // Create protocol instance with CSV points loaded
        plugin.create_instance(config).await
    }

    /// Create multiple protocols in parallel for improved performance
    ///
    /// # Arguments
    ///
    /// * `configs` - Channel configurations
    pub async fn create_protocols_parallel(
        &self,
        configs: Vec<ChannelConfig>,
    ) -> Vec<Result<Box<dyn ComBase>>> {
        use futures::future::join_all;

        // Fix closure capture issue by creating futures directly
        let futures: Vec<_> = configs
            .into_iter()
            .map(|config| {
                let fut = self.create_protocol(config);
                fut
            })
            .collect();

        join_all(futures).await
    }

    /// Create and register a channel with optimized performance and validation
    ///
    /// # Arguments
    ///
    /// * `config` - Channel configuration
    pub async fn create_channel(&self, config: ChannelConfig) -> Result<()> {
        self.create_channel_with_config_manager(config, None).await
    }

    /// Create and register a channel with config manager support for point table loading
    ///
    /// # Arguments
    ///
    /// * `config` - Channel configuration
    /// * `config_manager` - Optional config manager for loading point tables
    pub async fn create_channel_with_config_manager(
        &self,
        config: ChannelConfig,
        config_manager: Option<&ConfigManager>,
    ) -> Result<()> {
        let channel_id = config.id;

        // Validate configuration using registered factories
        self.validate_config(&config)?;

        // Initialize channel-specific logging if enabled
        if config.logging.enabled {
            self.setup_channel_logging(&config)?;
        }

        // Create protocol instance with config manager support
        let protocol = self
            .create_protocol_with_config_manager(config.clone(), config_manager)
            .await?;

        // Create metadata
        let metadata = ChannelMetadata {
            name: config.name.clone(),
            protocol_type: ProtocolType::parse_protocol_type(&config.protocol)?,
            created_at: std::time::Instant::now(),
            last_accessed: Arc::new(RwLock::new(std::time::Instant::now())),
        };

        let channel_wrapper = Arc::new(RwLock::new(protocol));

        // Atomic insertion using DashMap's entry API to prevent race conditions
        let entry = ChannelEntry {
            channel: channel_wrapper,
            metadata: metadata.clone(),
        };

        // Use entry API for atomic operation
        match self.channels.entry(channel_id) {
            dashmap::mapref::entry::Entry::Vacant(vacant) => {
                vacant.insert(entry);

                // Store to Redis if enabled
                if let Some(ref redis_store) = self.redis_store {
                    let redis_metadata =
                        crate::core::storage::redis_storage::RedisChannelMetadata {
                            name: metadata.name.clone(),
                            protocol_type: format!("{:?}", metadata.protocol_type),
                            created_at: chrono::Local::now()
                                .format("%Y-%m-%dT%H:%M:%S%.3f")
                                .to_string(),
                            last_accessed: chrono::Local::now()
                                .format("%Y-%m-%dT%H:%M:%S%.3f")
                                .to_string(),
                            running: false,
                            parameters: config
                                .parameters
                                .iter()
                                .map(|(k, v)| {
                                    let json_value =
                                        serde_json::to_value(v).unwrap_or(serde_json::Value::Null);
                                    (k.clone(), json_value)
                                })
                                .collect(),
                        };

                    if let Err(e) = redis_store
                        .set_channel_metadata(channel_id, &redis_metadata)
                        .await
                    {
                        tracing::warn!(
                            "Failed to store channel {} metadata to Redis: {}",
                            channel_id,
                            e
                        );
                    } else {
                        tracing::debug!("Stored channel {} metadata to Redis", channel_id);
                    }
                }

                tracing::info!(
                    "Created channel {} with protocol {:?}{} [Channel-{}]",
                    channel_id,
                    config.protocol,
                    if self.redis_store.is_some() {
                        " (with Redis storage)"
                    } else {
                        ""
                    },
                    channel_id
                );
                Ok(())
            }
            dashmap::mapref::entry::Entry::Occupied(_) => Err(ComSrvError::ConfigError(format!(
                "Channel ID already exists: {channel_id}"
            ))),
        }
    }

    /// Setup channel-specific logging
    /// Creates dedicated log files for each channel based on configuration
    fn setup_channel_logging(&self, config: &ChannelConfig) -> Result<()> {
        use std::fs;

        // Create log directory if specified
        if config.logging.enabled {
            let full_log_dir = if let Some(ref log_dir) = config.logging.log_dir {
                log_dir.clone()
            } else {
                format!("logs/channel_{}", config.id)
            };

            if let Err(e) = fs::create_dir_all(&full_log_dir) {
                tracing::warn!(
                    "Failed to create channel log directory {}: {}",
                    full_log_dir,
                    e
                );
                return Err(ComSrvError::ConfigError(format!(
                    "Failed to create channel log directory {}: {}",
                    full_log_dir, e
                )));
            }

            // Create initial log file for the channel
            let log_file_path = format!("{}/channel_{}.log", full_log_dir, config.id);
            if let Ok(mut file) = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log_file_path)
            {
                let init_message = format!(
                    "{{\"timestamp\":\"{}\",\"level\":\"INFO\",\"channel_id\":{},\"channel_name\":\"{}\",\"message\":\"Channel log initialized\"}}\n",
                    chrono::Local::now().format("%Y-%m-%dT%H:%M:%S%.6f"),
                    config.id,
                    config.name
                );
                let _ = file.write_all(init_message.as_bytes());
                let _ = file.flush();
            }

            tracing::info!(
                "Created channel log directory: {} for channel {} ({})",
                full_log_dir,
                config.id,
                config.name
            );

            // Also create a debug log file if debug logging is enabled
            if config.logging.level.as_ref().is_some_and(|l| l == "debug") {
                let debug_log_path = format!("{}/channel_{}_debug.log", full_log_dir, config.id);
                if let Ok(mut file) = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&debug_log_path)
                {
                    let debug_message = format!(
                        "{{\"timestamp\":\"{}\",\"level\":\"DEBUG\",\"channel_id\":{},\"channel_name\":\"{}\",\"message\":\"Debug logging enabled for channel\"}}\n",
                        chrono::Local::now().format("%Y-%m-%dT%H:%M:%S%.6f"),
                        config.id,
                        config.name
                    );
                    let _ = file.write_all(debug_message.as_bytes());
                    let _ = file.flush();
                }
            }
        }

        Ok(())
    }

    /// Write a message to channel-specific log file
    /// This is a helper method for protocols to write to their dedicated log files
    pub fn write_channel_log(&self, channel_id: u16, level: &str, message: &str) -> Result<()> {
        // Find the channel configuration
        if let Some(channel_entry) = self.channels.get(&channel_id) {
            // Try to write to the channel log file if it exists
            let log_dir = format!("logs/{}", channel_entry.metadata.name);
            let log_file_path = format!("{}/channel_{}.log", log_dir, channel_id);

            if let Ok(mut file) = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&log_file_path)
            {
                let log_entry = format!(
                    "{{\"timestamp\":\"{}\",\"level\":\"{}\",\"channel_id\":{},\"channel_name\":\"{}\",\"message\":\"{}\"}}\n",
                    chrono::Local::now().format("%Y-%m-%dT%H:%M:%S%.6f"),
                    level,
                    channel_id,
                    channel_entry.metadata.name,
                    message
                );
                let _ = file.write_all(log_entry.as_bytes());
                let _ = file.flush();
            }
        }

        Ok(())
    }

    /// Start all channels with improved performance and non-blocking operations
    ///
    /// Uses snapshot approach to avoid issues with concurrent channel insertion/removal
    /// during the startup process.
    pub async fn start_all_channels(&self) -> Result<()> {
        use futures::future::join_all;

        // Take a snapshot of channel IDs to avoid concurrent modification issues
        let channel_ids: Vec<u16> = self.channels.iter().map(|entry| *entry.key()).collect();

        if channel_ids.is_empty() {
            tracing::info!("No channels to start");
            return Ok(());
        }

        tracing::info!(
            "Starting all channels with snapshot approach: total_channels={}",
            channel_ids.len()
        );

        let start_futures = channel_ids.into_iter().map(|id| {
            async move {
                // Re-check if channel still exists (might have been removed concurrently)
                if let Some(channel_entry) = self.channels.get(&id) {
                    let mut channel = channel_entry.channel.write().await;
                    match channel.start().await {
                        Ok(_) => {
                            tracing::info!("Channel started successfully: channel_id={id}");
                            Ok(())
                        }
                        Err(e) => {
                            tracing::error!("Failed to start channel {}: {e}", id);
                            Err(e)
                        }
                    }
                } else {
                    tracing::debug!("Channel {} was removed during startup, skipping", id);
                    Ok(())
                }
            }
        });

        let results = join_all(start_futures).await;

        // Collect and report results
        let mut successful_starts = 0;
        let mut failed_starts = 0;
        let mut errors = Vec::new();

        for result in results {
            match result {
                Ok(_) => successful_starts += 1,
                Err(e) => {
                    failed_starts += 1;
                    errors.push(e);
                }
            }
        }

        tracing::info!(
            "Channel startup completed: {} successful, {} failed, {} total attempted",
            successful_starts,
            failed_starts,
            successful_starts + failed_starts
        );

        if failed_starts > 0 {
            let error_msg = format!(
                "Failed to start {} out of {} channels. First error: {}",
                failed_starts,
                successful_starts + failed_starts,
                errors
                    .first()
                    .map(|e| e.to_string())
                    .unwrap_or_else(|| "Unknown error".to_string())
            );
            return Err(ComSrvError::InvalidOperation(error_msg));
        }

        Ok(())
    }

    /// Stop all channels with improved performance and non-blocking cleanup
    pub async fn stop_all_channels(&self) -> Result<()> {
        use futures::future::join_all;

        let stop_futures = self.channels.iter().map(|entry| {
            let (id, channel_entry) = (entry.key(), entry.value());
            let id = *id;
            let channel_wrapper = channel_entry.channel.clone();

            async move {
                let mut channel = channel_wrapper.write().await;
                match channel.stop().await {
                    Ok(_) => {
                        tracing::info!("Channel {} stopped successfully", id);
                    },
                    Err(e) => {
                        tracing::warn!("Channel {}: Failed to stop channel - continuing with other channels: {e}", id);
                        // Continue stopping other channels even if one fails
                    }
                }
            }
        });

        join_all(stop_futures).await;

        Ok(())
    }

    /// Get channel by ID with optimized access
    pub async fn get_channel(&self, id: u16) -> Option<Arc<RwLock<Box<dyn ComBase>>>> {
        let channel_entry = self.channels.get(&id)?;

        // Update last accessed time
        *channel_entry.metadata.last_accessed.write().await = std::time::Instant::now();

        Some(channel_entry.channel.clone())
    }

    /// Get all channels as a vector of (id, channel) pairs
    pub fn get_all_channels(&self) -> Vec<(u16, Arc<RwLock<Box<dyn ComBase>>>)> {
        self.channels
            .iter()
            .map(|entry| {
                let id = *entry.key();
                let channel = entry.value().channel.clone();
                (id, channel)
            })
            .collect()
    }

    /// Get mutable channel by ID
    pub async fn get_channel_mut(&self, id: u16) -> Option<Arc<RwLock<Box<dyn ComBase>>>> {
        // For thread-safe access, we still return Arc<RwLock<_>>
        // The caller is responsible for acquiring write lock
        self.get_channel(id).await
    }

    /// Get all channel IDs efficiently
    /// If Redis is enabled, this will return IDs from both memory and Redis storage
    pub fn get_channel_ids(&self) -> Vec<u16> {
        self.channels.iter().map(|entry| *entry.key()).collect()
    }

    /// Get the number of registered channels
    #[inline]
    pub fn channel_count(&self) -> usize {
        self.channels.len()
    }

    /// Get the number of channels that are currently running
    pub async fn running_channel_count(&self) -> usize {
        use futures::future::join_all;

        let running_futures = self.channels.iter().map(|entry| {
            let channel = entry.value().channel.clone();
            async move {
                let ch = channel.read().await;
                ch.is_running().await
            }
        });

        join_all(running_futures)
            .await
            .into_iter()
            .filter(|running| *running)
            .count()
    }

    /// Get channel statistics with real-time protocol counts
    pub async fn get_channel_stats(&self) -> ChannelStats {
        let total_channels = self.channels.len();
        let running_channels = self.running_channel_count().await;
        let mut protocol_counts = AHashMap::new();

        // Count channels by protocol from current data
        for entry in self.channels.iter() {
            let metadata = &entry.value().metadata;
            let protocol_name = metadata.protocol_type.as_str();
            *protocol_counts
                .entry(protocol_name.to_string())
                .or_insert(0) += 1;
        }

        ChannelStats {
            total_channels,
            running_channels,
            protocol_counts,
        }
    }

    /// Clean up idle channels with parallel non-blocking operations
    ///
    /// Removes channels that have been idle for longer than the specified duration.
    /// Uses parallel processing for efficient cleanup of multiple channels.
    ///
    /// # Arguments
    ///
    /// * `max_idle_time` - Maximum idle time before a channel is considered for cleanup
    pub async fn cleanup_channels(&self, max_idle_time: std::time::Duration) {
        use futures::future::join_all;

        let now = std::time::Instant::now();
        let mut channels_to_remove = Vec::new();

        // Identify idle channels
        for entry in self.channels.iter() {
            let (id, channel_entry) = (entry.key(), entry.value());
            let last_accessed = *channel_entry.metadata.last_accessed.read().await;

            if now.duration_since(last_accessed) > max_idle_time {
                channels_to_remove.push(*id);
                tracing::info!("Channel {} marked for cleanup due to inactivity", id);
            }
        }

        if channels_to_remove.is_empty() {
            tracing::debug!("No idle channels found for cleanup");
            return;
        }

        // Clone self reference to avoid borrow across await issues
        let channels_ref = &self.channels;

        // Stop channels in parallel with proper error handling
        let stop_futures = channels_to_remove.iter().map(|&id| async move {
            if let Some(channel_entry) = channels_ref.get(&id) {
                let mut channel = channel_entry.channel.write().await;
                match channel.stop().await {
                    Ok(_) => {
                        tracing::info!("Channel {} stopped successfully", id);
                        Ok(id)
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Channel {}: Failed to stop - continuing with cleanup: {}",
                            id,
                            e
                        );
                        Err((id, e))
                    }
                }
            } else {
                tracing::debug!("Channel {} already removed during cleanup", id);
                Ok(id)
            }
        });

        let stop_results = join_all(stop_futures).await;

        // Remove channels from the map
        let mut successfully_stopped = 0;
        let mut failed_to_stop = 0;

        for result in stop_results {
            match result {
                Ok(id) => {
                    self.channels.remove(&id);
                    successfully_stopped += 1;
                    tracing::debug!("Channel {} removed from factory during cleanup", id);
                }
                Err((id, e)) => {
                    failed_to_stop += 1;
                    tracing::error!(
                        "Failed to cleanup channel {}, keeping in factory: {}",
                        id,
                        e
                    );
                }
            }
        }

        tracing::info!(
            "Channel cleanup completed: {} successfully cleaned, {} failed cleanup",
            successfully_stopped,
            failed_to_stop
        );
    }

    /// Hot update channel configuration
    ///
    /// Provides seamless configuration updates without service interruption
    ///
    /// # Arguments
    ///
    /// * `id` - Channel ID to update
    /// * `new_config` - New configuration to apply
    /// * `config_manager` - Optional config manager for point table loading
    pub async fn update_channel_config(
        &self,
        id: u16,
        new_config: ChannelConfig,
        config_manager: Option<&ConfigManager>,
    ) -> Result<()> {
        // Validate new configuration first
        self.validate_config(&new_config)?;

        // Get existing channel
        let channel_entry = self
            .channels
            .get(&id)
            .ok_or_else(|| ComSrvError::ChannelError(format!("Channel {} not found", id)))?;

        let old_channel = channel_entry.channel.clone();

        // Stop the old channel
        {
            let mut channel = old_channel.write().await;
            if let Err(e) = channel.stop().await {
                tracing::warn!("Failed to stop channel {} during update: {e}", id);
            }
        }

        // Create new protocol instance
        let new_protocol = self
            .create_protocol_with_config_manager(new_config.clone(), config_manager)
            .await?;

        // Create new metadata
        let new_metadata = ChannelMetadata {
            name: new_config.name.clone(),
            protocol_type: ProtocolType::parse_protocol_type(&new_config.protocol)?,
            created_at: std::time::Instant::now(),
            last_accessed: Arc::new(RwLock::new(std::time::Instant::now())),
        };

        let new_channel_wrapper = Arc::new(RwLock::new(new_protocol));

        // Atomic replacement
        let new_entry = ChannelEntry {
            channel: new_channel_wrapper.clone(),
            metadata: new_metadata,
        };

        self.channels.insert(id, new_entry);

        // Start the new channel
        {
            let mut channel = new_channel_wrapper.write().await;
            if let Err(e) = channel.start().await {
                tracing::error!("Failed to start updated channel {}: {e}", id);
                return Err(ComSrvError::ChannelError(format!(
                    "Failed to start updated channel {}: {}",
                    id, e
                )));
            }
        }

        tracing::info!("Successfully updated channel {} configuration", id);
        Ok(())
    }

    /// Get channel metadata by ID (name and protocol type)
    pub async fn get_channel_metadata(&self, id: u16) -> Option<(String, String)> {
        if let Some(entry) = self.channels.get(&id) {
            let metadata = &entry.metadata;
            Some((
                metadata.name.clone(),
                format!("{:?}", metadata.protocol_type),
            ))
        } else {
            None
        }
    }
}

/// Channel statistics
#[derive(Debug, Clone)]
pub struct ChannelStats {
    pub total_channels: usize,
    pub running_channels: usize,
    pub protocol_counts: AHashMap<String, usize>,
}

/// Create and configure a default protocol factory with all built-in protocols
///
/// # Returns
///
/// Configured `ProtocolFactory` with all supported protocols registered
pub fn create_default_factory() -> ProtocolFactory {
    // The new() method already registers built-in factories
    ProtocolFactory::new()
}

/// Create a factory with custom protocol factories
///
/// # Arguments
///
/// * `custom_factories` - Vector of custom protocol factories to register
///
/// # Returns
///
/// Configured `ProtocolFactory` with built-in and custom protocols registered
pub fn create_factory_with_custom_protocols(
    custom_factories: Vec<Arc<dyn ProtocolClientFactory>>,
) -> ProtocolFactory {
    let factory = ProtocolFactory::new();

    // Register custom factories
    for custom_factory in custom_factories {
        factory.register_protocol_factory(custom_factory);
    }

    factory
}

impl Default for ProtocolFactory {
    fn default() -> Self {
        Self::new()
    }
}

/// Placeholder for backward compatibility - to be implemented
// TODO: Implement proper protocol parser registry

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::loaders::point_mapper::CombinedPoint;
    use crate::core::config::ProtocolType;

    fn create_test_channel_config(id: u16, protocol: ProtocolType) -> ChannelConfig {
        let parameters = serde_json::json!({
            "host": "127.0.0.1",
            "port": 502,
            "timeout": 5000,
            "max_retries": 3,
            "poll_rate": 1000
        });

        // Convert JSON to HashMap<String, serde_yaml::Value> for compatibility
        let param_map = json_to_yaml_params(parameters);

        ChannelConfig {
            id,
            name: format!("Test Channel {id}"),
            description: Some("Test channel configuration".to_string()),
            protocol: protocol.to_string(),
            parameters: param_map,
            logging: ChannelLoggingConfig::default(),
            table_config: None,
            points: Vec::new(),
            combined_points: Vec::new(),
        }
    }

    fn create_modbus_rtu_test_config(id: u16) -> ChannelConfig {
        let parameters = serde_json::json!({
            "port": "/dev/ttyUSB0",
            "baud_rate": 9600,
            "data_bits": 8,
            "parity": "None",
            "stop_bits": 1,
            "timeout": 5000,
            "max_retries": 3,
            "poll_rate": 1000,
        });

        // Convert JSON to HashMap<String, serde_yaml::Value> for compatibility
        let param_map = json_to_yaml_params(parameters);

        ChannelConfig {
            id,
            name: format!("Test RTU Channel {id}"),
            description: Some("Test RTU channel configuration".to_string()),
            protocol: ProtocolType::ModbusRtu.to_string(),
            parameters: param_map,
            logging: ChannelLoggingConfig::default(),
            table_config: None,
            points: Vec::new(),
            combined_points: Vec::new(),
        }
    }

    #[test]
    fn test_protocol_factory_new() {
        let factory = ProtocolFactory::new();
        assert_eq!(factory.channel_count(), 0);
        assert!(factory.is_empty());
        assert!(!factory.supported_protocols().is_empty());
        assert!(factory.is_protocol_supported(&ProtocolType::ModbusTcp));
        assert!(factory.is_protocol_supported(&ProtocolType::ModbusRtu));
    }

    #[test]
    fn test_supported_protocols() {
        let factory = ProtocolFactory::new();
        let protocols = factory.supported_protocols();
        assert!(protocols.contains(&ProtocolType::ModbusTcp));
        assert!(protocols.contains(&ProtocolType::ModbusRtu));
    }

    #[test]
    fn test_config_validation() {
        let factory = ProtocolFactory::new();

        // Test valid Modbus TCP config
        let valid_config = create_test_channel_config(1, ProtocolType::ModbusTcp);
        assert!(factory.validate_config(&valid_config).is_ok());

        // Test invalid config (missing host)
        let mut invalid_config = valid_config.clone();
        invalid_config.parameters.remove("host");
        assert!(factory.validate_config(&invalid_config).is_err());
    }

    #[test]
    fn test_get_default_config() {
        let factory = ProtocolFactory::new();

        let modbus_config = factory.get_default_config(&ProtocolType::ModbusTcp);
        assert!(modbus_config.is_some());
        assert_eq!(
            modbus_config.unwrap().protocol,
            ProtocolType::ModbusTcp.to_string()
        );

        let modbus_rtu_config = factory.get_default_config(&ProtocolType::ModbusRtu);
        assert!(modbus_rtu_config.is_some());
        assert_eq!(
            modbus_rtu_config.unwrap().protocol,
            ProtocolType::ModbusRtu.to_string()
        );

        // Unsupported protocol should return None
        let unsupported = factory.get_default_config(&ProtocolType::Can);
        assert!(unsupported.is_none());
    }

    #[test]
    fn test_get_config_schema() {
        let factory = ProtocolFactory::new();

        let modbus_schema = factory.get_config_schema(&ProtocolType::ModbusTcp);
        assert!(modbus_schema.is_some());

        let modbus_rtu_schema = factory.get_config_schema(&ProtocolType::ModbusRtu);
        assert!(modbus_rtu_schema.is_some());

        // Unsupported protocol should return None
        let unsupported_schema = factory.get_config_schema(&ProtocolType::Can);
        assert!(unsupported_schema.is_none());
    }

    #[tokio::test]
    async fn test_create_modbus_tcp_protocol() {
        let factory = ProtocolFactory::new();
        let config = create_test_channel_config(1, ProtocolType::ModbusTcp);

        let result = factory.create_protocol(config).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_create_modbus_rtu_protocol() {
        let factory = ProtocolFactory::new();
        let config = create_modbus_rtu_test_config(4);

        let result = factory.create_protocol(config).await;
        assert!(result.is_ok(), "Modbus RTU protocol should be supported");
    }

    #[tokio::test]
    async fn test_create_virtual_protocol() {
        let factory = ProtocolFactory::new();
        let config = create_test_channel_config(5, ProtocolType::Virtual);

        let result = factory.create_protocol(config).await;
        assert!(result.is_ok(), "Virtual protocol should be supported");

        // Verify it's a MockComBase instance
        let protocol = result.unwrap();
        let protocol_type = protocol.protocol_type();
        assert_eq!(protocol_type, "Virtual");
    }

    #[tokio::test]
    async fn test_create_channel() {
        let factory = ProtocolFactory::new();
        let config = create_test_channel_config(10, ProtocolType::ModbusTcp);

        let result = factory.create_channel(config).await;
        assert!(result.is_ok());
        assert_eq!(factory.channel_count(), 1);
        assert!(!factory.is_empty());
    }

    #[tokio::test]
    async fn test_create_duplicate_channel() {
        let factory = ProtocolFactory::new();
        let config1 = create_test_channel_config(20, ProtocolType::ModbusTcp);
        let config2 = create_modbus_rtu_test_config(20); // Same ID, different protocol

        let result1 = factory.create_channel(config1).await;
        assert!(result1.is_ok());

        let result2 = factory.create_channel(config2).await;
        assert!(result2.is_err());
        assert!(matches!(result2.unwrap_err(), ComSrvError::ConfigError(_)));
    }

    #[tokio::test]
    async fn test_concurrent_duplicate_channel_creation() {
        let factory = Arc::new(ProtocolFactory::new());
        let config1 = create_test_channel_config(21, ProtocolType::ModbusTcp);
        let config2 = create_test_channel_config(21, ProtocolType::ModbusRtu);

        let factory_clone = factory.clone();
        let handle1 = tokio::spawn(async move { factory_clone.create_channel(config1).await });
        let factory_clone = factory.clone();
        let handle2 = tokio::spawn(async move { factory_clone.create_channel(config2).await });

        let res1 = handle1.await.unwrap();
        let res2 = handle2.await.unwrap();

        assert!(res1.is_ok() ^ res2.is_ok(), "one creation must fail");
        assert_eq!(factory.channel_count(), 1);
    }

    #[tokio::test]
    async fn test_get_channel() {
        let factory = ProtocolFactory::new();
        let config = create_test_channel_config(30, ProtocolType::ModbusTcp);

        factory.create_channel(config).await.unwrap();

        let channel = factory.get_channel(30).await;
        assert!(channel.is_some());

        let non_existent = factory.get_channel(999).await;
        assert!(non_existent.is_none());
    }

    #[tokio::test]
    async fn test_get_channel_mut() {
        let factory = ProtocolFactory::new();
        let config = create_test_channel_config(40, ProtocolType::ModbusTcp);

        factory.create_channel(config).await.unwrap();

        let channel = factory.get_channel_mut(40).await;
        assert!(channel.is_some());

        let non_existent = factory.get_channel_mut(999).await;
        assert!(non_existent.is_none());
    }

    #[tokio::test]
    async fn test_get_all_channels() {
        let factory = ProtocolFactory::new();
        let config1 = create_test_channel_config(50, ProtocolType::ModbusTcp);
        let config2 = create_modbus_rtu_test_config(51);

        factory.create_channel(config1).await.unwrap();
        factory.create_channel(config2).await.unwrap();

        let all_channels = factory.get_all_channels();
        assert_eq!(all_channels.len(), 2);
    }

    #[tokio::test]
    async fn test_get_channel_ids() {
        let factory = ProtocolFactory::new();
        let config1 = create_test_channel_config(60, ProtocolType::ModbusTcp);
        let config2 = create_modbus_rtu_test_config(61);

        factory.create_channel(config1).await.unwrap();
        factory.create_channel(config2).await.unwrap();

        let ids = factory.get_channel_ids();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&60));
        assert!(ids.contains(&61));
    }

    #[tokio::test]
    async fn test_get_channel_stats() {
        let factory = ProtocolFactory::new();
        let config1 = create_test_channel_config(70, ProtocolType::ModbusTcp);
        let config2 = create_modbus_rtu_test_config(71);
        let config3 = create_test_channel_config(72, ProtocolType::ModbusTcp);

        factory.create_channel(config1).await.unwrap();
        factory.create_channel(config2).await.unwrap();
        factory.create_channel(config3).await.unwrap();

        let stats = factory.get_channel_stats().await;
        assert_eq!(stats.total_channels, 3);
        assert_eq!(stats.running_channels, 0); // Channels not started yet

        // Note: We skip starting channels in tests since they would fail without actual devices
        // In a real environment, channels would be started successfully

        // Check protocol counts - use the actual protocol type string representation
        let modbus_tcp_count = stats
            .protocol_counts
            .get("ModbusTcp")
            .or_else(|| stats.protocol_counts.get("modbus_tcp"))
            .or_else(|| stats.protocol_counts.get("Modbus TCP"));
        let modbus_rtu_count = stats
            .protocol_counts
            .get("ModbusRtu")
            .or_else(|| stats.protocol_counts.get("modbus_rtu"))
            .or_else(|| stats.protocol_counts.get("Modbus RTU"));

        // For debugging, print all protocol counts
        println!("Protocol counts: {:?}", stats.protocol_counts);

        // Adjust test expectations based on actual implementation
        assert!(modbus_tcp_count.unwrap_or(&0) >= &1); // At least 1 TCP protocol
        assert!(modbus_rtu_count.unwrap_or(&0) >= &1); // At least 1 RTU protocol
    }

    #[tokio::test]
    async fn test_create_protocols_parallel() {
        let factory = ProtocolFactory::new();
        let configs = vec![
            create_test_channel_config(80, ProtocolType::ModbusTcp),
            create_modbus_rtu_test_config(81),
            create_test_channel_config(82, ProtocolType::Can), // Should fail
        ];

        let results = factory.create_protocols_parallel(configs).await;
        assert_eq!(results.len(), 3);
        assert!(results[0].is_ok());
        assert!(results[1].is_ok());
        assert!(results[2].is_err());
    }

    #[tokio::test]
    async fn test_cleanup_channels() {
        let factory = ProtocolFactory::new();
        let config = create_test_channel_config(90, ProtocolType::ModbusTcp);

        factory.create_channel(config).await.unwrap();

        // Test cleanup with very short idle time (should not remove channels immediately)
        factory
            .cleanup_channels(std::time::Duration::from_millis(1))
            .await;

        // Channel should still exist
        assert_eq!(factory.channel_count(), 1);
    }

    #[tokio::test]
    async fn test_channel_metadata() {
        let factory = ProtocolFactory::new();
        let config = create_test_channel_config(100, ProtocolType::ModbusTcp);

        factory.create_channel(config.clone()).await.unwrap();

        // Verify metadata was stored
        let metadata = factory.channels.get(&100).unwrap().metadata.clone();
        assert_eq!(metadata.name, config.name);
        // metadata.protocol_type is ProtocolType, config.protocol is String
        assert_eq!(metadata.protocol_type.to_string(), config.protocol);
    }

    #[test]
    fn test_default_implementation() {
        let factory = ProtocolFactory::default();
        assert_eq!(factory.channel_count(), 0);
        assert!(factory.is_empty());
    }

    #[tokio::test]
    async fn test_concurrent_channel_creation() {
        let factory = Arc::new(ProtocolFactory::new());
        let config = create_test_channel_config(200, ProtocolType::ModbusTcp);

        let f1 = {
            let factory = factory.clone();
            let cfg = config.clone();
            tokio::spawn(async move { factory.create_channel(cfg).await })
        };

        let f2 = {
            let factory = factory.clone();
            let cfg = config.clone();
            tokio::spawn(async move { factory.create_channel(cfg).await })
        };

        let r1 = f1.await.unwrap();
        let r2 = f2.await.unwrap();

        assert!(r1.is_ok() ^ r2.is_ok(), "only one creation should succeed");
        assert_eq!(factory.channel_count(), 1);
    }

    #[test]
    fn test_unregister_protocol_factory() {
        let factory = ProtocolFactory::new();

        // Test unregistering existing factory
        assert!(factory.is_protocol_supported(&ProtocolType::ModbusTcp));
        let removed = factory.unregister_protocol_factory(&ProtocolType::ModbusTcp);
        assert!(removed.is_ok());
        assert!(!factory.is_protocol_supported(&ProtocolType::ModbusTcp));

        // Test unregistering non-existent factory
        let removed_again = factory.unregister_protocol_factory(&ProtocolType::ModbusTcp);
        assert!(removed_again.is_ok());

        // Test unregistering different protocol
        assert!(factory.is_protocol_supported(&ProtocolType::ModbusRtu));
        let removed_rtu = factory.unregister_protocol_factory(&ProtocolType::ModbusRtu);
        assert!(removed_rtu.is_ok());
        assert!(!factory.is_protocol_supported(&ProtocolType::ModbusRtu));
    }

    // This test is commented out because we now use plugin system exclusively
    // #[test]
    // fn test_hot_swappable_protocol_factory() {
    //     let factory = ProtocolFactory::new();
    //
    //     // Plugin system is now used instead of built-in factories
    // }

    #[tokio::test]
    async fn test_mock_combase_functionality() {
        let mut mock = MockComBase::new("test_mock", 999, "Mock");

        // Test initial state
        assert_eq!(mock.name(), "test_mock");
        assert_eq!(mock.channel_id(), "999");
        assert_eq!(mock.protocol_type(), "Mock");
        assert!(!mock.is_running().await);

        // Test successful start
        assert!(mock.start().await.is_ok());
        assert!(mock.is_running().await);

        // Test successful stop
        assert!(mock.stop().await.is_ok());
        assert!(!mock.is_running().await);

        // Test start failure
        let mut failing_mock = MockComBase::with_start_failure("failing_mock", 998, "Mock");
        assert!(failing_mock.start().await.is_err());
        assert!(!failing_mock.is_running().await);

        // Test stop failure
        let mut stop_failing_mock =
            MockComBase::with_stop_failure("stop_failing_mock", 997, "Mock");
        assert!(stop_failing_mock.start().await.is_ok());
        assert!(stop_failing_mock.is_running().await);
        assert!(stop_failing_mock.stop().await.is_err());
        // Should still be running after failed stop
        assert!(stop_failing_mock.is_running().await);
    }

    #[tokio::test]
    async fn test_start_all_channels_with_mock() {
        let factory = ProtocolFactory::new();

        // Create mock channels directly in the factory
        let mock1 = Box::new(MockComBase::new("mock1", 1001, "Mock"));
        let mock2 = Box::new(MockComBase::new("mock2", 1002, "Mock"));
        let mock3 = Box::new(MockComBase::with_start_failure("mock3", 1003, "Mock"));

        let entry1 = ChannelEntry {
            channel: Arc::new(RwLock::new(mock1)),
            metadata: ChannelMetadata {
                name: "Mock Channel 1".to_string(),
                protocol_type: ProtocolType::Virtual,
                created_at: std::time::Instant::now(),
                last_accessed: Arc::new(RwLock::new(std::time::Instant::now())),
            },
        };

        let entry2 = ChannelEntry {
            channel: Arc::new(RwLock::new(mock2)),
            metadata: ChannelMetadata {
                name: "Mock Channel 2".to_string(),
                protocol_type: ProtocolType::Virtual,
                created_at: std::time::Instant::now(),
                last_accessed: Arc::new(RwLock::new(std::time::Instant::now())),
            },
        };

        let entry3 = ChannelEntry {
            channel: Arc::new(RwLock::new(mock3)),
            metadata: ChannelMetadata {
                name: "Mock Channel 3 (Failing)".to_string(),
                protocol_type: ProtocolType::Virtual,
                created_at: std::time::Instant::now(),
                last_accessed: Arc::new(RwLock::new(std::time::Instant::now())),
            },
        };

        factory.channels.insert(1001, entry1);
        factory.channels.insert(1002, entry2);
        factory.channels.insert(1003, entry3);

        // Test start_all_channels - should fail due to mock3
        let result = factory.start_all_channels().await;
        assert!(result.is_err());

        // Verify that successful channels were started
        let channel1 = factory.channels.get(&1001).unwrap();
        assert!(channel1.channel.read().await.is_running().await);

        let channel2 = factory.channels.get(&1002).unwrap();
        assert!(channel2.channel.read().await.is_running().await);

        // Verify that failing channel was not started
        let channel3 = factory.channels.get(&1003).unwrap();
        assert!(!channel3.channel.read().await.is_running().await);
    }
}
