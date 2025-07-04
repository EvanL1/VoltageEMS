//! # Configuration Management for Model Service
//! 
//! This module provides comprehensive configuration management for the Model Service (ModSrv),
//! supporting multiple configuration sources:
//! 1. Local configuration files (YAML/JSON)
//! 2. Configuration center service (HTTP)
//! 3. Environment variables (override)

use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;
use crate::storage::SyncMode;
use crate::error::ModelSrvError;
use anyhow::Context;

/// Service identification and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    #[serde(default)]
    pub instance_id: String,
}

/// Redis database connection configuration
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct RedisConfig {
    /// Redis server URL (e.g., redis://localhost:6379)
    pub url: String,
    /// Redis server hostname (legacy, use url instead)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    /// Redis server port (legacy, use url instead)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    /// Redis database number
    #[serde(default)]
    pub database: u8,
    /// Prefix for all Redis keys
    #[serde(default = "default_redis_prefix")]
    pub key_prefix: String,
    /// Connection pool size
    #[serde(default = "default_pool_size")]
    pub pool_size: u32,
    /// Optional password
    #[serde(default)]
    pub password: Option<String>,
}

/// Logging system configuration
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LoggingConfig {
    /// Log level filter (trace, debug, info, warn, error)
    #[serde(default = "default_log_level")]
    pub level: String,
    /// Path to log file for persistent logging
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    /// Enable console output for logs
    #[serde(default = "default_log_console")]
    pub console: bool,
    /// Log directory
    #[serde(default = "default_log_dir")]
    pub log_dir: String,
    /// Maximum log file size in bytes
    #[serde(default = "default_max_file_size")]
    pub max_file_size: u64,
    /// Maximum number of log files to keep
    #[serde(default = "default_max_files")]
    pub max_files: u32,
}

/// Model execution and template configuration
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ModelConfig {
    /// Model execution interval in milliseconds
    pub update_interval_ms: u64,
    /// Redis key pattern for model configurations
    pub config_key_pattern: String,
    /// Redis key pattern for input data
    pub data_key_pattern: String,
    /// Redis key pattern for model outputs
    pub output_key_pattern: String,
    /// Directory containing model templates
    #[serde(default = "default_templates_dir")]
    pub templates_dir: String,
}

/// HTTP API server configuration
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ApiConfig {
    /// API server host address
    #[serde(default = "default_api_host")]
    pub host: String,
    
    /// API server port number
    #[serde(default = "default_api_port")]
    pub port: u16,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            host: default_api_host(),
            port: default_api_port(),
        }
    }
}

/// Control operations configuration
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ControlConfig {
    /// Redis key pattern for control operations
    pub operation_key_pattern: String,
    /// Whether control operations are enabled
    #[serde(default = "default_control_enabled")]
    pub enabled: bool,
}

/// System monitoring and alerting configuration
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MonitoringConfig {
    /// Whether monitoring is enabled
    #[serde(default)]
    pub enabled: bool,
    /// Performance notification threshold in milliseconds
    pub notification_threshold_ms: Option<u128>,
    /// Metrics port
    #[serde(default = "default_metrics_port")]
    pub metrics_port: u16,
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            notification_threshold_ms: None,
            metrics_port: default_metrics_port(),
        }
    }
}

/// Storage configuration
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct StorageConfig {
    /// Whether to use Redis storage backend
    #[serde(default = "default_use_redis")]
    pub use_redis: bool,
    
    /// Storage backend mode (memory, redis, hybrid)
    #[serde(default = "default_storage_mode")]
    pub storage_mode: String,
    
    /// Data synchronization interval in seconds
    #[serde(default = "default_sync_interval_secs")]
    pub sync_interval_secs: u64,
}

/// Configuration for the Model Service
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Config {
    /// Service metadata
    #[serde(default = "default_service_info")]
    pub service: ServiceInfo,
    /// Redis database configuration
    pub redis: RedisConfig,
    /// Logging system configuration
    pub logging: LoggingConfig,
    /// Model execution configuration
    pub model: ModelConfig,
    /// Control operations configuration
    pub control: ControlConfig,
    /// HTTP API server configuration
    #[serde(default)]
    pub api: ApiConfig,
    /// System monitoring configuration
    #[serde(default)]
    pub monitoring: MonitoringConfig,
    /// Storage configuration
    #[serde(default)]
    pub storage: StorageConfig,
    
    // Legacy fields for compatibility
    #[serde(skip)]
    pub templates_dir: String,
    #[serde(skip)]
    pub log_level: String,
    #[serde(skip)]
    pub use_redis: bool,
    #[serde(skip)]
    pub storage_mode: String,
    #[serde(skip)]
    pub sync_interval_secs: u64,
}

// Default value functions
fn default_service_info() -> ServiceInfo {
    ServiceInfo {
        name: "modsrv".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        description: "Model Service for VoltageEMS".to_string(),
        instance_id: format!("modsrv-{}", uuid::Uuid::new_v4()),
    }
}

fn default_redis_prefix() -> String { "voltage:modsrv:".to_string() }
fn default_pool_size() -> u32 { 10 }
fn default_log_level() -> String { "info".to_string() }
fn default_log_console() -> bool { true }
fn default_log_dir() -> String { "logs".to_string() }
fn default_max_file_size() -> u64 { 10 * 1024 * 1024 } // 10MB
fn default_max_files() -> u32 { 5 }
fn default_api_host() -> String { "0.0.0.0".to_string() }
fn default_api_port() -> u16 { 8092 }
fn default_metrics_port() -> u16 { 9092 }
fn default_templates_dir() -> String { "templates".to_string() }
fn default_control_enabled() -> bool { true }
fn default_use_redis() -> bool { true }
fn default_storage_mode() -> String { "hybrid".to_string() }
fn default_sync_interval_secs() -> u64 { 60 }

impl Config {
    /// Create configuration from file (legacy method)
    pub fn new(config_file: &str) -> Result<Self> {
        Self::from_file(config_file)
    }
    
    /// Load configuration from file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path_ref = path.as_ref();
        if !path_ref.exists() {
            return Err(ModelSrvError::ConfigError(format!(
                "Configuration file not found: {}",
                path_ref.display()
            )));
        }
        
        let content = std::fs::read_to_string(path_ref)
            .map_err(|e| ModelSrvError::ConfigError(format!("Failed to read config file: {}", e)))?;
        
        let mut config: Config = if path_ref.extension().and_then(|s| s.to_str()) == Some("json") {
            serde_json::from_str(&content)
                .map_err(|e| ModelSrvError::ConfigError(format!("Failed to parse JSON: {}", e)))?
        } else {
            serde_yaml::from_str(&content)
                .map_err(|e| ModelSrvError::ConfigError(format!("Failed to parse YAML: {}", e)))?
        };
        
        // Handle legacy Redis config
        if config.redis.url.is_empty() {
            if let (Some(host), Some(port)) = (&config.redis.host, &config.redis.port) {
                config.redis.url = format!("redis://{}:{}", host, port);
            } else {
                config.redis.url = "redis://localhost:6379".to_string();
            }
        }
        
        // Set legacy fields for compatibility
        config.templates_dir = config.model.templates_dir.clone();
        config.log_level = config.logging.level.clone();
        config.use_redis = config.storage.use_redis;
        config.storage_mode = config.storage.storage_mode.clone();
        config.sync_interval_secs = config.storage.sync_interval_secs;
        
        Ok(config)
    }

    /// Create default configuration
    pub fn default() -> Self {
        let service = default_service_info();
        let redis = RedisConfig {
            url: "redis://localhost:6379".to_string(),
            host: None,
            port: None,
            database: 0,
            key_prefix: default_redis_prefix(),
            pool_size: default_pool_size(),
            password: None,
        };
        let logging = LoggingConfig {
            level: default_log_level(),
            file: Some("modsrv.log".to_string()),
            console: default_log_console(),
            log_dir: default_log_dir(),
            max_file_size: default_max_file_size(),
            max_files: default_max_files(),
        };
        let model = ModelConfig {
            update_interval_ms: 1000,
            config_key_pattern: "voltage:modsrv:model:config:*".to_string(),
            data_key_pattern: "voltage:data:*".to_string(),
            output_key_pattern: "voltage:modsrv:model:output:*".to_string(),
            templates_dir: default_templates_dir(),
        };
        let control = ControlConfig {
            operation_key_pattern: "voltage:control:operation:*".to_string(),
            enabled: default_control_enabled(),
        };
        let api = ApiConfig::default();
        let monitoring = MonitoringConfig::default();
        let storage = StorageConfig {
            use_redis: default_use_redis(),
            storage_mode: default_storage_mode(),
            sync_interval_secs: default_sync_interval_secs(),
        };
        
        Config {
            service,
            redis,
            logging,
            model,
            control,
            api,
            monitoring,
            storage,
            templates_dir: default_templates_dir(),
            log_level: default_log_level(),
            use_redis: default_use_redis(),
            storage_mode: default_storage_mode(),
            sync_interval_secs: default_sync_interval_secs(),
        }
    }

    pub fn get_sync_mode(&self) -> SyncMode {
        match self.storage.storage_mode.as_str() {
            "write_through" => SyncMode::WriteThrough,
            "write_back" => SyncMode::WriteBack(Duration::from_secs(self.storage.sync_interval_secs)),
            "on_demand" => SyncMode::OnDemand,
            _ => SyncMode::WriteThrough,
        }
    }
}

/// Configuration loader with multiple source support
pub struct ConfigLoader {
    config_file: Option<String>,
    config_center_url: Option<String>,
    env_prefix: String,
}

impl ConfigLoader {
    pub fn new() -> Self {
        Self {
            config_file: None,
            config_center_url: None,
            env_prefix: "MODSRV_".to_string(),
        }
    }
    
    pub fn with_file(mut self, path: impl Into<String>) -> Self {
        self.config_file = Some(path.into());
        self
    }
    
    pub fn with_config_center(mut self, url: impl Into<String>) -> Self {
        self.config_center_url = Some(url.into());
        self
    }
    
    pub fn with_env_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.env_prefix = prefix.into();
        self
    }
    
    /// Load configuration from all sources
    pub async fn load(&self) -> Result<Config> {
        // 1. Start with default configuration
        let mut config = Config::default();
        
        // 2. Load from local file if specified
        if let Some(file_path) = &self.config_file {
            if Path::new(file_path).exists() {
                config = Config::from_file(file_path)?;
                tracing::info!("Loaded configuration from file: {}", file_path);
            } else {
                tracing::warn!("Configuration file not found: {}, using defaults", file_path);
            }
        }
        
        // 3. Try to load from config center
        if let Some(config_center_url) = &self.config_center_url {
            match self.load_from_config_center(config_center_url).await {
                Ok(center_config) => {
                    config = center_config;
                    tracing::info!("Loaded configuration from config center: {}", config_center_url);
                }
                Err(e) => {
                    tracing::warn!("Failed to load from config center: {}", e);
                }
            }
        }
        
        // 4. Apply environment variable overrides
        config = self.apply_env_overrides(config)?;
        
        // 5. Validate configuration
        self.validate_config(&config)?;
        
        Ok(config)
    }
    
    /// Load configuration from config center
    async fn load_from_config_center(&self, base_url: &str) -> Result<Config> {
        let url = format!("{}/api/v1/config/modsrv", base_url);
        let client = reqwest::Client::new();
        
        let response = client
            .get(&url)
            .header("Accept", "application/json")
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .map_err(|e| ModelSrvError::ConfigError(format!("Failed to contact config center: {}", e)))?;
        
        if !response.status().is_success() {
            return Err(ModelSrvError::ConfigError(format!(
                "Config center returned error: {}",
                response.status()
            )));
        }
        
        let mut config: Config = response
            .json()
            .await
            .map_err(|e| ModelSrvError::ConfigError(format!("Failed to parse config center response: {}", e)))?;
        
        // Set legacy fields
        config.templates_dir = config.model.templates_dir.clone();
        config.log_level = config.logging.level.clone();
        config.use_redis = config.storage.use_redis;
        config.storage_mode = config.storage.storage_mode.clone();
        config.sync_interval_secs = config.storage.sync_interval_secs;
        
        Ok(config)
    }
    
    /// Apply environment variable overrides
    fn apply_env_overrides(&self, mut config: Config) -> Result<Config> {
        // Redis URL
        if let Ok(url) = std::env::var(format!("{}REDIS_URL", self.env_prefix)) {
            config.redis.url = url;
        }
        
        // API host and port
        if let Ok(host) = std::env::var(format!("{}API_HOST", self.env_prefix)) {
            config.api.host = host;
        }
        if let Ok(port) = std::env::var(format!("{}API_PORT", self.env_prefix)) {
            config.api.port = port.parse()
                .map_err(|_| ModelSrvError::ConfigError("Invalid API port".to_string()))?;
        }
        
        // Log level
        if let Ok(level) = std::env::var(format!("{}LOG_LEVEL", self.env_prefix)) {
            config.logging.level = level.clone();
            config.log_level = level;
        }
        
        // Model update interval
        if let Ok(interval) = std::env::var(format!("{}MODEL_UPDATE_INTERVAL_MS", self.env_prefix)) {
            config.model.update_interval_ms = interval.parse()
                .map_err(|_| ModelSrvError::ConfigError("Invalid update interval".to_string()))?;
        }
        
        Ok(config)
    }
    
    /// Validate configuration
    fn validate_config(&self, config: &Config) -> Result<()> {
        // Validate service info
        if config.service.name.is_empty() {
            return Err(ModelSrvError::ConfigError("Service name cannot be empty".to_string()));
        }
        
        // Validate Redis URL
        if !config.redis.url.starts_with("redis://") {
            return Err(ModelSrvError::ConfigError("Redis URL must start with redis://".to_string()));
        }
        
        // Validate API port
        if config.api.port == 0 {
            return Err(ModelSrvError::ConfigError("API port cannot be 0".to_string()));
        }
        
        // Validate log level
        match config.logging.level.as_str() {
            "trace" | "debug" | "info" | "warn" | "error" => {}
            _ => return Err(ModelSrvError::ConfigError(format!(
                "Invalid log level: {}",
                config.logging.level
            ))),
        }
        
        // Validate model update interval
        if config.model.update_interval_ms == 0 {
            return Err(ModelSrvError::ConfigError(
                "Model update interval must be greater than 0".to_string()
            ));
        }
        
        Ok(())
    }
}

/// Helper function to load configuration
pub async fn load_config() -> Result<Config> {
    // Load .env file if exists
    dotenv::dotenv().ok();
    
    let loader = ConfigLoader::new()
        .with_file(
            std::env::var("MODSRV_CONFIG_FILE")
                .unwrap_or_else(|_| "config/modsrv.yaml".to_string())
        )
        .with_config_center(
            std::env::var("CONFIG_CENTER_URL").ok()
        )
        .with_env_prefix("MODSRV_");
    
    loader.load().await
}

/// Generate default configuration file
pub fn generate_default_config() -> String {
    let config = Config::default();
    serde_yaml::to_string(&config)
        .unwrap_or_else(|_| "# Failed to generate config".to_string())
}