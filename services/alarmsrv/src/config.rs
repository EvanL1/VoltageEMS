use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::services::rules::AlarmRule;

/// Alarm service configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlarmConfig {
    /// Redis configuration
    pub redis: RedisConfig,
    /// API configuration
    pub api: ApiConfig,
    /// Storage configuration
    pub storage: StorageConfig,
    /// Monitoring configuration
    #[serde(default)]
    pub monitoring: MonitoringConfig,
    /// Alarm rules
    #[serde(default)]
    pub alarm_rules: Vec<AlarmRule>,
}

/// Redis connection type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RedisConnectionType {
    /// TCP connection
    Tcp,
    /// Unix socket connection  
    Unix,
}

impl Default for RedisConnectionType {
    fn default() -> Self {
        RedisConnectionType::Tcp
    }
}

/// Redis configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    /// Connection type (TCP or Unix socket)
    #[serde(default)]
    pub connection_type: RedisConnectionType,
    /// Redis host address (for TCP connections)
    #[serde(default = "default_redis_host")]
    pub host: String,
    /// Redis port (for TCP connections)
    #[serde(default = "default_redis_port")]
    pub port: u16,
    /// Unix socket path (for Unix socket connections)
    #[serde(default)]
    pub socket_path: Option<String>,
    /// Redis password
    pub password: Option<String>,
    /// Database number
    #[serde(default)]
    pub database: u8,
}

impl RedisConfig {
    /// Get Redis connection URL based on connection type
    pub fn get_connection_url(&self) -> String {
        match self.connection_type {
            RedisConnectionType::Tcp => {
                let auth = if let Some(ref password) = self.password {
                    format!(":{}@", password)
                } else {
                    String::new()
                };
                format!(
                    "redis://{}{}/{}",
                    auth,
                    format!("{}:{}", self.host, self.port),
                    self.database
                )
            }
            RedisConnectionType::Unix => {
                if let Some(ref path) = self.socket_path {
                    format!("unix://{}?db={}", path, self.database)
                } else {
                    // Fallback to TCP if socket path is not provided
                    let auth = if let Some(ref password) = self.password {
                        format!(":{}@", password)
                    } else {
                        String::new()
                    };
                    format!(
                        "redis://{}{}/{}",
                        auth,
                        format!("{}:{}", self.host, self.port),
                        self.database
                    )
                }
            }
        }
    }
}

fn default_redis_host() -> String {
    "localhost".to_string()
}

fn default_redis_port() -> u16 {
    6379
}

/// API configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    /// Listen address
    pub host: String,
    /// Listen port
    pub port: u16,
}

impl ApiConfig {
    /// Build a path with API prefix
    pub fn build_path(&self, path: &str) -> String {
        format!("/api/v1/{}", path.trim_start_matches('/'))
    }
}

/// Storage configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Retention period for resolved alarms (in days)
    pub retention_days: u32,
    /// Enable automatic cleanup
    pub auto_cleanup: bool,
    /// Cleanup interval (in hours)
    pub cleanup_interval_hours: u32,
}

/// Monitoring configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonitoringConfig {
    /// Enable data scanning
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Channels to monitor
    #[serde(default)]
    pub channels: Vec<u16>,
    /// Point types to monitor (m/s/c/a)
    #[serde(default = "default_point_types")]
    pub point_types: Vec<String>,
    /// Scan interval in seconds
    #[serde(default = "default_scan_interval")]
    pub scan_interval: u64,
}

impl Default for MonitoringConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            channels: vec![],
            point_types: default_point_types(),
            scan_interval: default_scan_interval(),
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_point_types() -> Vec<String> {
    vec!["m".to_string(), "s".to_string()]
}

fn default_scan_interval() -> u64 {
    10
}

impl Default for AlarmConfig {
    fn default() -> Self {
        Self {
            redis: RedisConfig {
                connection_type: RedisConnectionType::Tcp,
                host: "localhost".to_string(),
                port: 6379,
                socket_path: None,
                password: None,
                database: 0,
            },
            api: ApiConfig {
                host: "0.0.0.0".to_string(),
                port: 8080,
            },
            storage: StorageConfig {
                retention_days: 30,
                auto_cleanup: true,
                cleanup_interval_hours: 24,
            },
            monitoring: MonitoringConfig::default(),
            alarm_rules: Vec::new(),
        }
    }
}

impl AlarmConfig {
    /// Load configuration
    pub async fn load() -> Result<Self> {
        // Try to load from configuration file first
        let config_path = "alarmsrv.yaml";
        if std::path::Path::new(config_path).exists() {
            let config_str = std::fs::read_to_string(config_path)?;
            let mut config: AlarmConfig = serde_yaml::from_str(&config_str)?;

            // Override with environment variables if set
            if let Ok(host) = std::env::var("REDIS_HOST") {
                config.redis.host = host;
            }
            if let Ok(port) = std::env::var("REDIS_PORT") {
                if let Ok(p) = port.parse() {
                    config.redis.port = p;
                }
            }
            if let Ok(api_port) = std::env::var("API_PORT") {
                if let Ok(p) = api_port.parse() {
                    config.api.port = p;
                }
            }

            Ok(config)
        } else {
            // Fallback to environment variables only
            let config = Self {
                redis: RedisConfig {
                    connection_type: match std::env::var("REDIS_CONNECTION_TYPE")
                        .unwrap_or_else(|_| "tcp".to_string())
                        .to_lowercase()
                        .as_str()
                    {
                        "unix" => RedisConnectionType::Unix,
                        _ => RedisConnectionType::Tcp,
                    },
                    host: std::env::var("REDIS_HOST").unwrap_or_else(|_| "localhost".to_string()),
                    port: std::env::var("REDIS_PORT")
                        .unwrap_or_else(|_| "6379".to_string())
                        .parse()
                        .unwrap_or(6379),
                    socket_path: std::env::var("REDIS_SOCKET_PATH").ok(),
                    password: std::env::var("REDIS_PASSWORD").ok(),
                    database: std::env::var("REDIS_DB")
                        .unwrap_or_else(|_| "0".to_string())
                        .parse()
                        .unwrap_or(0),
                },
                api: ApiConfig {
                    host: std::env::var("API_HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
                    port: std::env::var("API_PORT")
                        .unwrap_or_else(|_| "8087".to_string())
                        .parse()
                        .unwrap_or(8087),
                },
                storage: StorageConfig {
                    retention_days: std::env::var("STORAGE_RETENTION_DAYS")
                        .unwrap_or_else(|_| "30".to_string())
                        .parse()
                        .unwrap_or(30),
                    auto_cleanup: std::env::var("STORAGE_AUTO_CLEANUP")
                        .unwrap_or_else(|_| "true".to_string())
                        .parse()
                        .unwrap_or(true),
                    cleanup_interval_hours: std::env::var("STORAGE_CLEANUP_INTERVAL_HOURS")
                        .unwrap_or_else(|_| "24".to_string())
                        .parse()
                        .unwrap_or(24),
                },
                monitoring: MonitoringConfig::default(),
                alarm_rules: Vec::new(),
            };

            Ok(config)
        }
    }

    /// Generate default configuration file
    pub fn generate_default_config() -> String {
        let config = Self::default();
        serde_yaml::to_string(&config)
            .unwrap_or_else(|_| "# Failed to generate config file".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AlarmConfig::default();
        assert_eq!(config.redis.host, "localhost");
        assert_eq!(config.redis.port, 6379);
        assert_eq!(config.api.host, "0.0.0.0");
        assert_eq!(config.api.port, 8080);
        assert_eq!(config.storage.retention_days, 30);
        assert!(config.storage.auto_cleanup);
    }

    #[tokio::test]
    async fn test_config_load() {
        let config = AlarmConfig::load().await.unwrap();
        assert!(!config.redis.host.is_empty());
        assert!(config.redis.port > 0);
        assert!(config.api.port > 0);
        assert!(config.storage.retention_days > 0);
    }

    #[test]
    fn test_generate_default_config() {
        let yaml = AlarmConfig::generate_default_config();
        assert!(yaml.contains("redis"));
        assert!(yaml.contains("api"));
        assert!(yaml.contains("storage"));
    }

    #[test]
    fn test_redis_connection_url_tcp() {
        let config = RedisConfig {
            connection_type: RedisConnectionType::Tcp,
            host: "127.0.0.1".to_string(),
            port: 6379,
            socket_path: None,
            password: None,
            database: 0,
        };

        let url = config.get_connection_url();
        assert_eq!(url, "redis://127.0.0.1:6379/0");
    }

    #[test]
    fn test_redis_connection_url_tcp_with_password() {
        let config = RedisConfig {
            connection_type: RedisConnectionType::Tcp,
            host: "127.0.0.1".to_string(),
            port: 6379,
            socket_path: None,
            password: Some("mypassword".to_string()),
            database: 1,
        };

        let url = config.get_connection_url();
        assert_eq!(url, "redis://:mypassword@127.0.0.1:6379/1");
    }

    #[test]
    fn test_redis_connection_url_unix() {
        let config = RedisConfig {
            connection_type: RedisConnectionType::Unix,
            host: "localhost".to_string(),
            port: 6379,
            socket_path: Some("/tmp/redis.sock".to_string()),
            password: None,
            database: 2,
        };

        let url = config.get_connection_url();
        assert_eq!(url, "unix:///tmp/redis.sock?db=2");
    }

    #[test]
    fn test_redis_connection_url_unix_fallback() {
        let config = RedisConfig {
            connection_type: RedisConnectionType::Unix,
            host: "localhost".to_string(),
            port: 6379,
            socket_path: None, // No socket path provided
            password: None,
            database: 0,
        };

        let url = config.get_connection_url();
        assert_eq!(url, "redis://localhost:6379/0");
    }
}
