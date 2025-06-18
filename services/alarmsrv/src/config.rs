use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Alarm service configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlarmConfig {
    /// Redis configuration
    pub redis: RedisConfig,
    /// API configuration
    pub api: ApiConfig,
}

/// Redis configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    /// Redis host address
    pub host: String,
    /// Redis port
    pub port: u16,
    /// Redis password
    pub password: Option<String>,
    /// Database number
    pub database: u8,
}

/// API configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    /// Listen address
    pub host: String,
    /// Listen port
    pub port: u16,
}

impl Default for AlarmConfig {
    fn default() -> Self {
        Self {
            redis: RedisConfig {
                host: "localhost".to_string(),
                port: 6379,
                password: None,
                database: 0,
            },
            api: ApiConfig {
                host: "0.0.0.0".to_string(),
                port: 8080,
            },
        }
    }
}

impl AlarmConfig {
    /// Load configuration
    pub async fn load() -> Result<Self> {
        // Try to load configuration from environment variables
        let config = Self {
            redis: RedisConfig {
                host: std::env::var("REDIS_HOST").unwrap_or_else(|_| "localhost".to_string()),
                port: std::env::var("REDIS_PORT")
                    .unwrap_or_else(|_| "6379".to_string())
                    .parse()
                    .unwrap_or(6379),
                password: std::env::var("REDIS_PASSWORD").ok(),
                database: std::env::var("REDIS_DB")
                    .unwrap_or_else(|_| "0".to_string())
                    .parse()
                    .unwrap_or(0),
            },
            api: ApiConfig {
                host: std::env::var("API_HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
                port: std::env::var("API_PORT")
                    .unwrap_or_else(|_| "8080".to_string())
                    .parse()
                    .unwrap_or(8080),
            },
        };

        Ok(config)
    }

    /// Generate default configuration file
    pub fn generate_default_config() -> String {
        let config = Self::default();
        serde_yaml::to_string(&config).unwrap_or_else(|_| "# Failed to generate config file".to_string())
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
    }

    #[tokio::test]
    async fn test_config_load() {
        let config = AlarmConfig::load().await.unwrap();
        assert!(!config.redis.host.is_empty());
        assert!(config.redis.port > 0);
        assert!(config.api.port > 0);
    }

    #[test]
    fn test_generate_default_config() {
        let yaml = AlarmConfig::generate_default_config();
        assert!(yaml.contains("redis"));
        assert!(yaml.contains("api"));
    }
} 