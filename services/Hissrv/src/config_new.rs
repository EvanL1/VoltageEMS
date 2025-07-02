use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::collections::HashMap;
use voltage_config::prelude::*;

/// Historical service configuration using the unified config framework
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HisServiceConfig {
    /// Base service configuration (flattened)
    #[serde(flatten)]
    pub base: BaseServiceConfig,
    
    /// API server configuration
    pub api: ApiConfig,
    
    /// Storage configuration
    pub storage: StorageConfig,
    
    /// Data processing configuration
    pub data: DataConfig,
    
    /// Performance configuration
    pub performance: PerformanceConfig,
}

/// API configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    /// Enable API server
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// API host
    #[serde(default = "default_api_host")]
    pub host: String,
    /// API port
    #[serde(default = "default_api_port")]
    pub port: u16,
    /// API prefix
    #[serde(default = "default_api_prefix")]
    pub prefix: String,
    /// Enable Swagger UI
    #[serde(default = "default_true")]
    pub swagger_ui: bool,
    /// CORS configuration
    pub cors: CorsConfig,
}

/// CORS configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorsConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_cors_origins")]
    pub origins: Vec<String>,
    #[serde(default = "default_cors_methods")]
    pub methods: Vec<String>,
}

/// Storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Default storage backend
    #[serde(default = "default_storage_backend")]
    pub default: String,
    /// Storage backends
    pub backends: StorageBackends,
}

/// Storage backends configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageBackends {
    pub influxdb: InfluxDBConfig,
    pub postgresql: PostgreSQLConfig,
    pub mongodb: MongoDBConfig,
}

/// InfluxDB configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfluxDBConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_influxdb_url")]
    pub url: String,
    #[serde(default = "default_influxdb_database")]
    pub database: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: String,
    #[serde(default = "default_retention_days")]
    pub retention_days: u32,
    #[serde(default = "default_batch_size")]
    pub batch_size: u32,
    #[serde(default = "default_flush_interval")]
    pub flush_interval: u64,
}

/// PostgreSQL configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostgreSQLConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_postgres_host")]
    pub host: String,
    #[serde(default = "default_postgres_port")]
    pub port: u16,
    #[serde(default = "default_postgres_database")]
    pub database: String,
    #[serde(default = "default_postgres_username")]
    pub username: String,
    #[serde(default)]
    pub password: String,
    #[serde(default = "default_pool_size")]
    pub pool_size: u32,
}

/// MongoDB configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MongoDBConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_mongodb_uri")]
    pub uri: String,
    #[serde(default = "default_mongodb_database")]
    pub database: String,
    #[serde(default = "default_mongodb_collection")]
    pub collection: String,
}

/// Data processing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataConfig {
    /// Data filters
    pub filters: DataFilters,
    /// Data transformations
    #[serde(default)]
    pub transformations: Vec<DataTransformation>,
}

/// Data filter configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataFilters {
    /// Default policy (store or ignore)
    #[serde(default = "default_filter_policy")]
    pub default_policy: String,
    /// Filter rules
    #[serde(default)]
    pub rules: Vec<FilterRule>,
}

/// Filter rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterRule {
    pub pattern: String,
    pub action: String,
    pub storage: Option<String>,
}

/// Data transformation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataTransformation {
    pub from_pattern: String,
    pub to_format: String,
    #[serde(default)]
    pub tags: HashMap<String, String>,
}

/// Performance configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    /// Worker threads
    #[serde(default = "default_worker_threads")]
    pub worker_threads: u32,
    /// Maximum concurrent requests
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent_requests: u32,
    /// Queue size
    #[serde(default = "default_queue_size")]
    pub queue_size: u32,
    /// Enable batch processing
    #[serde(default = "default_true")]
    pub batch_processing: bool,
}

// Default value functions
fn default_true() -> bool {
    true
}

fn default_api_host() -> String {
    "0.0.0.0".to_string()
}

fn default_api_port() -> u16 {
    8092
}

fn default_api_prefix() -> String {
    "/api/v1".to_string()
}

fn default_cors_origins() -> Vec<String> {
    vec!["*".to_string()]
}

fn default_cors_methods() -> Vec<String> {
    vec!["GET".to_string(), "POST".to_string(), "PUT".to_string(), "DELETE".to_string()]
}

fn default_storage_backend() -> String {
    "influxdb".to_string()
}

fn default_influxdb_url() -> String {
    "http://localhost:8086".to_string()
}

fn default_influxdb_database() -> String {
    "hissrv_data".to_string()
}

fn default_retention_days() -> u32 {
    30
}

fn default_batch_size() -> u32 {
    1000
}

fn default_flush_interval() -> u64 {
    10
}

fn default_postgres_host() -> String {
    "localhost".to_string()
}

fn default_postgres_port() -> u16 {
    5432
}

fn default_postgres_database() -> String {
    "hissrv".to_string()
}

fn default_postgres_username() -> String {
    "postgres".to_string()
}

fn default_pool_size() -> u32 {
    10
}

fn default_mongodb_uri() -> String {
    "mongodb://localhost:27017".to_string()
}

fn default_mongodb_database() -> String {
    "hissrv".to_string()
}

fn default_mongodb_collection() -> String {
    "data".to_string()
}

fn default_filter_policy() -> String {
    "store".to_string()
}

fn default_worker_threads() -> u32 {
    4
}

fn default_max_concurrent() -> u32 {
    1000
}

fn default_queue_size() -> u32 {
    10000
}

impl Configurable for HisServiceConfig {
    fn validate(&self) -> voltage_config::Result<()> {
        // Validate API configuration
        if self.api.port == 0 {
            return Err(ConfigError::Validation("API port cannot be 0".into()));
        }
        
        // Validate storage configuration
        if !self.storage.backends.influxdb.enabled && 
           !self.storage.backends.postgresql.enabled && 
           !self.storage.backends.mongodb.enabled {
            return Err(ConfigError::Validation(
                "At least one storage backend must be enabled".into()
            ));
        }
        
        // Validate InfluxDB config if enabled
        if self.storage.backends.influxdb.enabled {
            if self.storage.backends.influxdb.retention_days == 0 {
                return Err(ConfigError::Validation(
                    "InfluxDB retention days must be greater than 0".into()
                ));
            }
            if self.storage.backends.influxdb.batch_size == 0 {
                return Err(ConfigError::Validation(
                    "InfluxDB batch size must be greater than 0".into()
                ));
            }
        }
        
        // Validate filter rules
        for rule in &self.data.filters.rules {
            match rule.action.as_str() {
                "store" | "ignore" => {},
                _ => return Err(ConfigError::Validation(
                    format!("Invalid filter action: {}. Must be 'store' or 'ignore'", rule.action)
                )),
            }
        }
        
        // Validate performance settings
        if self.performance.worker_threads == 0 {
            return Err(ConfigError::Validation(
                "Worker threads must be greater than 0".into()
            ));
        }
        
        Ok(())
    }
    
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl ServiceConfig for HisServiceConfig {
    fn base(&self) -> &BaseServiceConfig {
        &self.base
    }
    
    fn base_mut(&mut self) -> &mut BaseServiceConfig {
        &mut self.base
    }
}

impl HisServiceConfig {
    /// Load configuration using the unified framework
    pub async fn load() -> Result<Self> {
        let loader = ConfigLoaderBuilder::new()
            .base_path("config")
            .add_file("hissrv.yml")
            .environment(Environment::from_env())
            .env_prefix("HIS")
            .defaults(serde_json::json!({
                "service": {
                    "name": "hissrv",
                    "version": env!("CARGO_PKG_VERSION"),
                    "description": "Historical Data Service"
                },
                "redis": {
                    "url": "redis://localhost:6379",
                    "prefix": "voltage:his:",
                    "pool_size": 30
                },
                "logging": {
                    "level": "info",
                    "console": true,
                    "file": {
                        "path": "logs/hissrv.log",
                        "rotation": "daily",
                        "max_size": "100MB",
                        "max_files": 10
                    }
                },
                "monitoring": {
                    "metrics_enabled": true,
                    "metrics_port": 9092,
                    "health_check_enabled": true,
                    "health_check_port": 8093
                },
                "api": {
                    "enabled": true,
                    "host": "0.0.0.0",
                    "port": 8092,
                    "prefix": "/api/v1",
                    "swagger_ui": true,
                    "cors": {
                        "enabled": true,
                        "origins": ["*"],
                        "methods": ["GET", "POST", "PUT", "DELETE"]
                    }
                },
                "storage": {
                    "default": "influxdb",
                    "backends": {
                        "influxdb": {
                            "enabled": true,
                            "url": "http://localhost:8086",
                            "database": "hissrv_data",
                            "username": "",
                            "password": "",
                            "retention_days": 30,
                            "batch_size": 1000,
                            "flush_interval": 10
                        },
                        "postgresql": {
                            "enabled": false,
                            "host": "localhost",
                            "port": 5432,
                            "database": "hissrv",
                            "username": "postgres",
                            "password": "",
                            "pool_size": 10
                        },
                        "mongodb": {
                            "enabled": false,
                            "uri": "mongodb://localhost:27017",
                            "database": "hissrv",
                            "collection": "data"
                        }
                    }
                },
                "data": {
                    "filters": {
                        "default_policy": "store",
                        "rules": []
                    },
                    "transformations": []
                },
                "performance": {
                    "worker_threads": 4,
                    "max_concurrent_requests": 1000,
                    "queue_size": 10000,
                    "batch_processing": true
                }
            }))?
            .build()?;
        
        let config: HisServiceConfig = loader.load_async().await
            .map_err(|e| anyhow::anyhow!("Failed to load configuration: {}", e))?;
        
        // Validate complete configuration
        config.validate_all()
            .map_err(|e| anyhow::anyhow!("Configuration validation failed: {}", e))?;
        
        Ok(config)
    }
    
    /// Generate default configuration file
    pub fn generate_default_config() -> String {
        let config = HisServiceConfig {
            base: BaseServiceConfig {
                service: ServiceInfo {
                    name: "hissrv".to_string(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                    description: "Historical Data Service".to_string(),
                    instance_id: String::new(),
                },
                redis: RedisConfig {
                    url: "redis://localhost:6379".to_string(),
                    prefix: "voltage:his:".to_string(),
                    pool_size: 30,
                    database: 0,
                    password: None,
                },
                logging: LoggingConfig {
                    level: "info".to_string(),
                    console: true,
                    file: Some(voltage_config::base::LogFileConfig {
                        path: "logs/hissrv.log".to_string(),
                        rotation: "daily".to_string(),
                        max_size: "100MB".to_string(),
                        max_files: 10,
                    }),
                    json_format: false,
                },
                monitoring: MonitoringConfig {
                    metrics_enabled: true,
                    metrics_port: 9092,
                    health_check_enabled: true,
                    health_check_port: 8093,
                    health_check_interval: 30,
                },
            },
            api: ApiConfig {
                enabled: true,
                host: "0.0.0.0".to_string(),
                port: 8092,
                prefix: "/api/v1".to_string(),
                swagger_ui: true,
                cors: CorsConfig {
                    enabled: true,
                    origins: vec!["*".to_string()],
                    methods: vec!["GET".to_string(), "POST".to_string(), "PUT".to_string(), "DELETE".to_string()],
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
                    rules: vec![],
                },
                transformations: vec![],
            },
            performance: PerformanceConfig {
                worker_threads: 4,
                max_concurrent_requests: 1000,
                queue_size: 10000,
                batch_processing: true,
            },
        };
        
        serde_yaml::to_string(&config).unwrap_or_else(|_| "# Failed to generate config file".to_string())
    }
    
    /// Check if a key should be stored based on filter rules
    pub fn should_store_key(&self, key: &str) -> bool {
        use regex::Regex;
        
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

    /// Get storage backend for a specific key
    pub fn get_storage_backend(&self, key: &str) -> String {
        use regex::Regex;
        
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
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_config_validation() {
        let mut config = HisServiceConfig {
            base: BaseServiceConfig {
                service: ServiceInfo {
                    name: "hissrv".to_string(),
                    version: "1.0.0".to_string(),
                    description: "Test".to_string(),
                    instance_id: String::new(),
                },
                redis: RedisConfig {
                    url: "redis://localhost:6379".to_string(),
                    prefix: "test:".to_string(),
                    pool_size: 10,
                    database: 0,
                    password: None,
                },
                logging: LoggingConfig {
                    level: "info".to_string(),
                    console: true,
                    file: None,
                    json_format: false,
                },
                monitoring: MonitoringConfig {
                    metrics_enabled: true,
                    metrics_port: 9090,
                    health_check_enabled: true,
                    health_check_port: 8080,
                    health_check_interval: 30,
                },
            },
            api: ApiConfig {
                enabled: true,
                host: "0.0.0.0".to_string(),
                port: 8092,
                prefix: "/api/v1".to_string(),
                swagger_ui: true,
                cors: CorsConfig {
                    enabled: true,
                    origins: vec!["*".to_string()],
                    methods: vec!["GET".to_string()],
                },
            },
            storage: StorageConfig {
                default: "influxdb".to_string(),
                backends: StorageBackends {
                    influxdb: InfluxDBConfig {
                        enabled: true,
                        url: "http://localhost:8086".to_string(),
                        database: "test".to_string(),
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
                        database: "test".to_string(),
                        username: "postgres".to_string(),
                        password: String::new(),
                        pool_size: 10,
                    },
                    mongodb: MongoDBConfig {
                        enabled: false,
                        uri: "mongodb://localhost:27017".to_string(),
                        database: "test".to_string(),
                        collection: "data".to_string(),
                    },
                },
            },
            data: DataConfig {
                filters: DataFilters {
                    default_policy: "store".to_string(),
                    rules: vec![],
                },
                transformations: vec![],
            },
            performance: PerformanceConfig {
                worker_threads: 4,
                max_concurrent_requests: 1000,
                queue_size: 10000,
                batch_processing: true,
            },
        };
        
        // Valid configuration should pass
        assert!(config.validate_all().is_ok());
        
        // No storage backend enabled should fail
        config.storage.backends.influxdb.enabled = false;
        assert!(config.validate_all().is_err());
        config.storage.backends.influxdb.enabled = true;
        
        // Invalid worker threads should fail
        config.performance.worker_threads = 0;
        assert!(config.validate_all().is_err());
    }
    
    #[test]
    fn test_filter_rules() {
        let config = HisServiceConfig {
            base: Default::default(),
            api: ApiConfig {
                enabled: true,
                host: "0.0.0.0".to_string(),
                port: 8092,
                prefix: "/api/v1".to_string(),
                swagger_ui: true,
                cors: CorsConfig {
                    enabled: true,
                    origins: vec!["*".to_string()],
                    methods: vec!["GET".to_string()],
                },
            },
            storage: StorageConfig {
                default: "influxdb".to_string(),
                backends: StorageBackends {
                    influxdb: InfluxDBConfig {
                        enabled: true,
                        url: "http://localhost:8086".to_string(),
                        database: "test".to_string(),
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
                        database: "test".to_string(),
                        username: "postgres".to_string(),
                        password: String::new(),
                        pool_size: 10,
                    },
                    mongodb: MongoDBConfig {
                        enabled: false,
                        uri: "mongodb://localhost:27017".to_string(),
                        database: "test".to_string(),
                        collection: "data".to_string(),
                    },
                },
            },
            data: DataConfig {
                filters: DataFilters {
                    default_policy: "store".to_string(),
                    rules: vec![
                        FilterRule {
                            pattern: "temp_*".to_string(),
                            action: "ignore".to_string(),
                            storage: None,
                        },
                        FilterRule {
                            pattern: "important_*".to_string(),
                            action: "store".to_string(),
                            storage: Some("postgresql".to_string()),
                        },
                    ],
                },
                transformations: vec![],
            },
            performance: PerformanceConfig {
                worker_threads: 4,
                max_concurrent_requests: 1000,
                queue_size: 10000,
                batch_processing: true,
            },
        };
        
        // Test filter matching
        assert!(!config.should_store_key("temp_sensor_1"));
        assert!(config.should_store_key("important_data"));
        assert!(config.should_store_key("other_data"));
        
        // Test storage backend selection
        assert_eq!(config.get_storage_backend("important_data"), "postgresql");
        assert_eq!(config.get_storage_backend("other_data"), "influxdb");
    }
}