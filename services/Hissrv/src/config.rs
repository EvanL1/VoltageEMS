use crate::error::{HisSrvError, Result};
use clap::{Parser, ArgAction};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::time::SystemTime;
use tracing::info;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub service: ServiceConfig,
    pub redis: RedisConfig,
    pub storage: StorageConfig,
    pub data: DataConfig,
    pub api: ApiConfig,
    pub monitoring: MonitoringConfig,
    pub logging: LoggingConfig,
    pub performance: PerformanceConfig,
    
    // Internal fields
    #[serde(skip)]
    pub config_file: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServiceConfig {
    pub name: String,
    pub version: String,
    pub port: u16,
    pub host: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RedisConfig {
    pub connection: RedisConnectionConfig,
    pub subscription: RedisSubscriptionConfig,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RedisConnectionConfig {
    pub host: String,
    pub port: u16,
    pub password: String,
    pub socket: String,
    pub database: u8,
    pub pool_size: u32,
    pub timeout: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RedisSubscriptionConfig {
    pub channels: Vec<String>,
    pub key_patterns: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StorageConfig {
    pub default: String,
    pub backends: StorageBackends,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StorageBackends {
    pub influxdb: InfluxDBConfig,
    pub postgresql: PostgreSQLConfig,
    pub mongodb: MongoDBConfig,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InfluxDBConfig {
    pub enabled: bool,
    pub url: String,
    pub database: String,
    pub username: String,
    pub password: String,
    pub retention_days: u32,
    pub batch_size: u32,
    pub flush_interval: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PostgreSQLConfig {
    pub enabled: bool,
    pub host: String,
    pub port: u16,
    pub database: String,
    pub username: String,
    pub password: String,
    pub pool_size: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MongoDBConfig {
    pub enabled: bool,
    pub uri: String,
    pub database: String,
    pub collection: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DataConfig {
    pub filters: DataFilters,
    pub transformations: Vec<DataTransformation>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DataFilters {
    pub default_policy: String,
    pub rules: Vec<FilterRule>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FilterRule {
    pub pattern: String,
    pub action: String,
    pub storage: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DataTransformation {
    pub from_pattern: String,
    pub to_format: String,
    pub tags: HashMap<String, String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApiConfig {
    pub enabled: bool,
    pub prefix: String,
    pub swagger_ui: bool,
    pub cors: CorsConfig,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CorsConfig {
    pub enabled: bool,
    pub origins: Vec<String>,
    pub methods: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MonitoringConfig {
    pub enabled: bool,
    pub metrics_port: u16,
    pub health_check: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String,
    pub format: String,
    pub file: String,
    pub max_size: String,
    pub max_files: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PerformanceConfig {
    pub worker_threads: u32,
    pub max_concurrent_requests: u32,
    pub queue_size: u32,
    pub batch_processing: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            service: ServiceConfig {
                name: "hissrv".to_string(),
                version: "0.2.0".to_string(),
                port: 8080,
                host: "0.0.0.0".to_string(),
            },
            redis: RedisConfig {
                connection: RedisConnectionConfig {
                    host: "127.0.0.1".to_string(),
                    port: 6379,
                    password: String::new(),
                    socket: String::new(),
                    database: 0,
                    pool_size: 10,
                    timeout: 5,
                },
                subscription: RedisSubscriptionConfig {
                    channels: vec!["data:*".to_string(), "events:*".to_string()],
                    key_patterns: vec!["*".to_string()],
                },
            },
            storage: StorageConfig {
                default: "influxdb".to_string(),
                backends: StorageBackends {
                    influxdb: InfluxDBConfig {
                        enabled: true,
                        url: "http://localhost:8086".to_string(),
                        database: "hissrv_data".to_string(),
                        username: String::new(),
                        password: String::new(),
                        retention_days: 30,
                        batch_size: 1000,
                        flush_interval: 10,
                    },
                    postgresql: PostgreSQLConfig {
                        enabled: false,
                        host: "localhost".to_string(),
                        port: 5432,
                        database: "hissrv".to_string(),
                        username: "postgres".to_string(),
                        password: String::new(),
                        pool_size: 10,
                    },
                    mongodb: MongoDBConfig {
                        enabled: false,
                        uri: "mongodb://localhost:27017".to_string(),
                        database: "hissrv".to_string(),
                        collection: "data".to_string(),
                    },
                },
            },
            data: DataConfig {
                filters: DataFilters {
                    default_policy: "store".to_string(),
                    rules: Vec::new(),
                },
                transformations: Vec::new(),
            },
            api: ApiConfig {
                enabled: true,
                prefix: "/api/v1".to_string(),
                swagger_ui: true,
                cors: CorsConfig {
                    enabled: true,
                    origins: vec!["*".to_string()],
                    methods: vec!["GET".to_string(), "POST".to_string(), "PUT".to_string(), "DELETE".to_string()],
                },
            },
            monitoring: MonitoringConfig {
                enabled: true,
                metrics_port: 9090,
                health_check: true,
            },
            logging: LoggingConfig {
                level: "info".to_string(),
                format: "json".to_string(),
                file: "logs/hissrv.log".to_string(),
                max_size: "100MB".to_string(),
                max_files: 10,
            },
            performance: PerformanceConfig {
                worker_threads: 4,
                max_concurrent_requests: 1000,
                queue_size: 10000,
                batch_processing: true,
            },
            config_file: "hissrv.yaml".to_string(),
        }
    }
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    #[arg(long, short, help = "Configuration file path", default_value = "hissrv.yaml")]
    config: String,

    #[arg(long, help = "Service host to bind to")]
    host: Option<String>,

    #[arg(long, short, help = "Service port to bind to")]
    port: Option<u16>,

    #[arg(long, help = "Redis host")]
    redis_host: Option<String>,

    #[arg(long, help = "Redis port")]
    redis_port: Option<u16>,

    #[arg(long, help = "Redis password")]
    redis_password: Option<String>,

    #[arg(long, help = "Log level (trace, debug, info, warn, error)")]
    log_level: Option<String>,

    #[arg(long, action = ArgAction::SetTrue, help = "Enable verbose logging")]
    verbose: bool,

    #[arg(long, action = ArgAction::SetTrue, help = "Enable API server")]
    enable_api: bool,

    #[arg(long, action = ArgAction::SetTrue, help = "Disable API server")]
    disable_api: bool,
}

impl Config {
    pub fn new() -> Self {
        Config::default()
    }
    
    /// Load configuration from config center or fall back to local file
    pub async fn load() -> Result<Self> {
        // Check if we should use config center
        if let Ok(_) = std::env::var("CONFIG_CENTER_URL") {
            info!("CONFIG_CENTER_URL found, attempting to load from config center");
            
            match crate::config_center::ConfigBuilder::new().build() {
                Ok(client) => {
                    match client.get_config().await {
                        Ok(service_config) => {
                            info!("Successfully loaded configuration from config center");
                            return Ok(service_config.into());
                        }
                        Err(e) => {
                            tracing::warn!("Failed to load from config center: {}, falling back to local config", e);
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to build config client: {}, falling back to local config", e);
                }
            }
        }
        
        // Fall back to loading from args/file
        Self::from_args()
    }

    pub fn from_args() -> Result<Self> {
        let args = Args::parse();
        let mut config = Config::default();

        // Set config file path
        config.config_file = args.config.clone();

        // Load from config file
        if Path::new(&args.config).exists() {
            config = Self::load_from_file(&args.config)?;
            config.config_file = args.config.clone();
        }

        // Override with command line arguments
        if let Some(host) = args.host {
            config.service.host = host;
        }
        if let Some(port) = args.port {
            config.service.port = port;
        }
        if let Some(host) = args.redis_host {
            config.redis.connection.host = host;
        }
        if let Some(port) = args.redis_port {
            config.redis.connection.port = port;
        }
        if let Some(password) = args.redis_password {
            config.redis.connection.password = password;
        }
        if let Some(level) = args.log_level {
            config.logging.level = level;
        }
        if args.verbose {
            config.logging.level = "debug".to_string();
        }
        if args.enable_api {
            config.api.enabled = true;
        }
        if args.disable_api {
            config.api.enabled = false;
        }

        Ok(config)
    }

    pub fn load_from_file(path: &str) -> Result<Self> {
        let content = fs::read_to_string(path)
            .map_err(|e| HisSrvError::ConfigError(format!("Failed to read config file {}: {}", path, e)))?;
        
        let config: Config = serde_yaml::from_str(&content)
            .map_err(|e| HisSrvError::ConfigError(format!("Failed to parse config file {}: {}", path, e)))?;
        
        Ok(config)
    }

    pub fn save_to_file(&self, path: &str) -> Result<()> {
        let content = serde_yaml::to_string(self)
            .map_err(|e| HisSrvError::ConfigError(format!("Failed to serialize config: {}", e)))?;
        
        fs::write(path, content)
            .map_err(|e| HisSrvError::ConfigError(format!("Failed to write config file {}: {}", path, e)))?;
        
        Ok(())
    }

    pub fn should_store_key(&self, key: &str) -> bool {
        // Check against specific filter rules
        for rule in &self.data.filters.rules {
            // Convert glob pattern to regex
            let regex_pattern = rule.pattern
                .replace("*", ".*")
                .replace("?", ".");

            // Check if key matches pattern
            if let Ok(regex) = Regex::new(&regex_pattern) {
                if regex.is_match(key) {
                    return rule.action == "store";
                }
            }
        }

        // If no rule matched, use default policy
        self.data.filters.default_policy == "store"
    }

    pub fn get_storage_backend(&self, key: &str) -> String {
        // Check if any rule specifies a storage backend
        for rule in &self.data.filters.rules {
            let regex_pattern = rule.pattern
                .replace("*", ".*")
                .replace("?", ".");

            if let Ok(regex) = Regex::new(&regex_pattern) {
                if regex.is_match(key) {
                    if let Some(storage) = &rule.storage {
                        return storage.clone();
                    }
                }
            }
        }

        // Return default storage backend
        self.storage.default.clone()
    }

    pub fn config_file_changed(&self, last_mod_time: &mut SystemTime) -> Result<bool> {
        let metadata = fs::metadata(&self.config_file)?;
        
        if let Ok(modified) = metadata.modified() {
            if modified > *last_mod_time {
                *last_mod_time = modified;
                return Ok(true);
            }
        }
        
        Ok(false)
    }
} 