use serde::{Deserialize, Serialize};
use std::path::Path;
use voltage_libs::config::utils::get_global_redis_url;
use voltage_libs::config::ConfigLoader;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub server: ServerConfig,
    #[serde(default)]
    pub redis: RedisConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    #[serde(default = "default_redis_url")]
    pub url: String,
    #[serde(default = "default_pool_size")]
    pub pool_size: u32,
}

impl Config {
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        // Try multiple configuration file paths
        let config_paths = [
            "config/apigateway/apigateway.yaml",
            "config/apigateway.yaml",
            "apigateway.yaml",
        ];

        let mut yaml_path = None;
        for path in &config_paths {
            if Path::new(path).exists() {
                yaml_path = Some(path.to_string());
                break;
            }
        }

        // Use the new ConfigLoader
        let loader = ConfigLoader::new()
            .with_defaults(Config::default())
            .with_env_prefix("APIGATEWAY");

        let config = if let Some(path) = yaml_path {
            loader.with_yaml_file(&path).build()
        } else {
            loader.build()
        }?;

        Ok(config)
    }
}

// Default value functions
fn default_host() -> String {
    "0.0.0.0".to_string()
}

fn default_redis_url() -> String {
    get_global_redis_url("APIGATEWAY")
}

fn default_pool_size() -> u32 {
    10
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
        }
    }
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            url: default_redis_url(),
            pool_size: default_pool_size(),
        }
    }
}
