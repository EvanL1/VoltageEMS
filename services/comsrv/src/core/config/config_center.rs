//! Configuration Center Integration
//!
//! This module provides integration with centralized configuration management systems.

use crate::utils::error::{ComSrvError, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::fs;
use tracing::{debug, info, warn};

/// Configuration response from config center
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigResponse {
    pub version: String,
    pub checksum: String,
    pub last_modified: String,
    pub content: serde_json::Value,
}

/// Configuration item response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigItemResponse {
    pub key: String,
    pub value: serde_json::Value,
    #[serde(rename = "type")]
    pub value_type: String,
}

/// Configuration change event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigChangeEvent {
    pub event: String,
    pub service: String,
    pub keys: Vec<String>,
    pub version: String,
}

/// Configuration source trait for different config center implementations
#[async_trait]
pub trait ConfigSource: Send + Sync {
    /// Fetch complete configuration
    async fn fetch_config(&self, service_name: &str) -> Result<ConfigResponse>;

    /// Fetch specific configuration item
    async fn fetch_item(&self, service_name: &str, key: &str) -> Result<ConfigItemResponse>;

    /// Get source name for logging
    fn name(&self) -> &str;
}

/// HTTP-based configuration center client
pub struct HttpConfigClient {
    base_url: String,
    client: reqwest::Client,
    auth_token: Option<String>,
}

impl HttpConfigClient {
    pub fn new(base_url: String, auth_token: Option<String>) -> Self {
        Self {
            base_url,
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .unwrap(),
            auth_token,
        }
    }
}

#[async_trait]
impl ConfigSource for HttpConfigClient {
    async fn fetch_config(&self, service_name: &str) -> Result<ConfigResponse> {
        let url = format!("{}/api/v1/config/service/{service_name}", self.base_url);

        info!("Fetching configuration from: {url}");

        let mut request = self.client.get(&url);

        // Add auth header if token is provided
        if let Some(token) = &self.auth_token {
            request = request.header("Authorization", format!("Bearer {token}"));
        }

        let response = request
            .send()
            .await
            .map_err(|e| ComSrvError::ConfigError(format!("Failed to fetch config: {e}")))?;

        if response.status().is_success() {
            let config: ConfigResponse = response.json().await.map_err(|e| {
                ComSrvError::ConfigError(format!("Failed to parse config response: {e}"))
            })?;

            debug!("Successfully fetched config version: {}", config.version);
            Ok(config)
        } else {
            Err(ComSrvError::ConfigError(format!(
                "Config fetch failed with status: {}",
                response.status()
            )))
        }
    }

    async fn fetch_item(&self, service_name: &str, key: &str) -> Result<ConfigItemResponse> {
        let url = format!(
            "{}/api/v1/config/service/{}/item/{}",
            self.base_url, service_name, key
        );

        debug!("Fetching config item from: {url}");

        let mut request = self.client.get(&url);

        if let Some(token) = &self.auth_token {
            request = request.header("Authorization", format!("Bearer {token}"));
        }

        let response = request
            .send()
            .await
            .map_err(|e| ComSrvError::ConfigError(format!("Failed to fetch config item: {e}")))?;

        if response.status().is_success() {
            let item: ConfigItemResponse = response.json().await.map_err(|e| {
                ComSrvError::ConfigError(format!("Failed to parse config item response: {e}"))
            })?;

            Ok(item)
        } else {
            Err(ComSrvError::ConfigError(format!(
                "Config item fetch failed with status: {}",
                response.status()
            )))
        }
    }

    fn name(&self) -> &str {
        "HttpConfigCenter"
    }
}

/// Configuration cache manager
pub struct ConfigCache {
    cache_dir: String,
    ttl_seconds: u64,
}

impl ConfigCache {
    pub fn new(cache_dir: String, ttl_seconds: u64) -> Self {
        Self {
            cache_dir,
            ttl_seconds,
        }
    }

    /// Get cache file path for a service
    fn cache_path(&self, service_name: &str) -> std::path::PathBuf {
        Path::new(&self.cache_dir).join(format!("{}.cache.json", service_name))
    }

    /// Save configuration to cache
    pub async fn save(&self, service_name: &str, config: &ConfigResponse) -> Result<()> {
        // Ensure cache directory exists
        fs::create_dir_all(&self.cache_dir).await.map_err(|e| {
            ComSrvError::ConfigError(format!("Failed to create cache directory: {e}"))
        })?;

        let cache_path = self.cache_path(service_name);

        // Create cache entry with metadata
        let cache_entry = serde_json::json!({
            "cached_at": chrono::Utc::now().to_rfc3339(),
            "ttl_seconds": self.ttl_seconds,
            "config": config,
        });

        let content = serde_json::to_string_pretty(&cache_entry)
            .map_err(|e| ComSrvError::ConfigError(format!("Failed to serialize cache: {e}")))?;

        fs::write(&cache_path, content)
            .await
            .map_err(|e| ComSrvError::ConfigError(format!("Failed to write cache file: {e}")))?;

        debug!("Saved config cache to: {}", cache_path.display());
        Ok(())
    }

    /// Load configuration from cache
    pub async fn load(&self, service_name: &str) -> Result<ConfigResponse> {
        let cache_path = self.cache_path(service_name);

        if !cache_path.exists() {
            return Err(ComSrvError::ConfigError("Cache file not found".to_string()));
        }

        let content = fs::read_to_string(&cache_path)
            .await
            .map_err(|e| ComSrvError::ConfigError(format!("Failed to read cache file: {e}")))?;

        let cache_entry: serde_json::Value = serde_json::from_str(&content)
            .map_err(|e| ComSrvError::ConfigError(format!("Failed to parse cache file: {e}")))?;

        // Check if cache is expired
        if let Some(cached_at_str) = cache_entry["cached_at"].as_str() {
            if let Ok(cached_at) = chrono::DateTime::parse_from_rfc3339(cached_at_str) {
                let age = chrono::Utc::now() - cached_at.with_timezone(&chrono::Utc);
                if age.num_seconds() as u64 > self.ttl_seconds {
                    return Err(ComSrvError::ConfigError("Cache expired".to_string()));
                }
            }
        }

        // Extract config from cache entry
        let config: ConfigResponse = serde_json::from_value(cache_entry["config"].clone())
            .map_err(|e| {
                ComSrvError::ConfigError(format!("Failed to extract config from cache: {e}"))
            })?;

        debug!("Loaded config from cache: {}", cache_path.display());
        Ok(config)
    }

    /// Clear cache for a service
    pub async fn clear(&self, service_name: &str) -> Result<()> {
        let cache_path = self.cache_path(service_name);
        if cache_path.exists() {
            fs::remove_file(&cache_path).await.map_err(|e| {
                ComSrvError::ConfigError(format!("Failed to remove cache file: {e}"))
            })?;
        }
        Ok(())
    }
}

/// Configuration center integration manager
pub struct ConfigCenterClient {
    service_name: String,
    source: Option<Box<dyn ConfigSource>>,
    cache: ConfigCache,
    fallback_config_path: Option<String>,
}

impl ConfigCenterClient {
    /// Create a new config center client
    pub fn new(service_name: String) -> Self {
        Self {
            service_name,
            source: None,
            cache: ConfigCache::new("/var/cache/comsrv".to_string(), 3600),
            fallback_config_path: None,
        }
    }

    /// Initialize from environment variables
    pub fn from_env(service_name: String) -> Self {
        let mut client = Self::new(service_name);

        // Check for config center URL
        if let Ok(url) = std::env::var("CONFIG_CENTER_URL") {
            let auth_token = std::env::var("CONFIG_CENTER_TOKEN").ok();
            let http_client = HttpConfigClient::new(url, auth_token);
            client.source = Some(Box::new(http_client));
            info!("Initialized config center client from environment");
        }

        // Set cache directory from env
        if let Ok(cache_dir) = std::env::var("CONFIG_CACHE_DIR") {
            let ttl = std::env::var("CONFIG_CACHE_TTL")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(3600);
            client.cache = ConfigCache::new(cache_dir, ttl);
        }

        // Set fallback config path
        if let Ok(path) = std::env::var("CONFIG_FALLBACK_PATH") {
            client.fallback_config_path = Some(path);
        }

        client
    }

    /// Set configuration source
    pub fn with_source(mut self, source: Box<dyn ConfigSource>) -> Self {
        self.source = Some(source);
        self
    }

    /// Set cache configuration
    pub fn with_cache(mut self, cache_dir: String, ttl_seconds: u64) -> Self {
        self.cache = ConfigCache::new(cache_dir, ttl_seconds);
        self
    }

    /// Set fallback config path
    pub fn with_fallback(mut self, path: String) -> Self {
        self.fallback_config_path = Some(path);
        self
    }

    /// Fetch configuration with fallback strategy
    pub async fn fetch_config(&self) -> Result<serde_json::Value> {
        // Try config center first
        if let Some(source) = &self.source {
            match source.fetch_config(&self.service_name).await {
                Ok(config) => {
                    info!("Successfully fetched config from {}", source.name());

                    // Save to cache
                    if let Err(e) = self.cache.save(&self.service_name, &config).await {
                        warn!("Failed to save config to cache: {e}");
                    }

                    return Ok(config.content);
                }
                Err(e) => {
                    warn!("Failed to fetch from config center: {e}");
                }
            }
        }

        // Try cache as fallback
        match self.cache.load(&self.service_name).await {
            Ok(config) => {
                warn!("Using cached configuration");
                return Ok(config.content);
            }
            Err(e) => {
                debug!("Cache not available: {e}");
            }
        }

        // Try local file as last resort
        if let Some(path) = &self.fallback_config_path {
            warn!("Falling back to local config file: {path}");
            return Err(ComSrvError::ConfigError(
                "Config center and cache unavailable, use local file".to_string(),
            ));
        }

        Err(ComSrvError::ConfigError(
            "No configuration source available".to_string(),
        ))
    }

    /// Fetch specific configuration item
    pub async fn fetch_item(&self, key: &str) -> Result<serde_json::Value> {
        if let Some(source) = &self.source {
            match source.fetch_item(&self.service_name, key).await {
                Ok(item) => {
                    debug!("Successfully fetched config item: {key}");
                    return Ok(item.value);
                }
                Err(e) => {
                    warn!("Failed to fetch config item: {e}");
                }
            }
        }

        // Fallback to fetching complete config and extracting item
        let config = self.fetch_config().await?;

        // Parse key path (e.g., "channels.0.parameters.host")
        let parts: Vec<&str> = key.split('.').collect();
        let mut current = &config;

        for part in parts {
            if let Ok(index) = part.parse::<usize>() {
                // Array index
                current = current.get(index).ok_or_else(|| {
                    ComSrvError::ConfigError(format!("Config key not found: {key}"))
                })?;
            } else {
                // Object key
                current = current.get(part).ok_or_else(|| {
                    ComSrvError::ConfigError(format!("Config key not found: {key}"))
                })?;
            }
        }

        Ok(current.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_config_cache() {
        let temp_dir = tempfile::tempdir().unwrap();
        let cache = ConfigCache::new(temp_dir.path().to_str().unwrap().to_string(), 60);

        let config = ConfigResponse {
            version: "1.0.0".to_string(),
            checksum: "abc123".to_string(),
            last_modified: chrono::Utc::now().to_rfc3339(),
            content: serde_json::json!({
                "service": {
                    "name": "test"
                }
            }),
        };

        // Save and load
        cache.save("test-service", &config).await.unwrap();
        let loaded = cache.load("test-service").await.unwrap();

        assert_eq!(loaded.version, config.version);
        assert_eq!(loaded.checksum, config.checksum);
    }

    #[test]
    fn test_config_client_from_env() {
        std::env::set_var("CONFIG_CENTER_URL", "http://localhost:8080");
        std::env::set_var("CONFIG_CENTER_TOKEN", "test-token");
        std::env::set_var("CONFIG_CACHE_DIR", "/tmp/test-cache");
        std::env::set_var("CONFIG_CACHE_TTL", "7200");

        let client = ConfigCenterClient::from_env("test-service".to_string());

        assert!(client.source.is_some());

        // Cleanup
        std::env::remove_var("CONFIG_CENTER_URL");
        std::env::remove_var("CONFIG_CENTER_TOKEN");
        std::env::remove_var("CONFIG_CACHE_DIR");
        std::env::remove_var("CONFIG_CACHE_TTL");
    }
}
