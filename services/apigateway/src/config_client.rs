use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

use crate::config::Config;
use crate::error::ApiGatewayError;

/// Configuration center client for API Gateway
pub struct ConfigClient {
    /// Base URL of configuration service
    config_service_url: String,
    /// Service name for this instance
    service_name: String,
    /// Cached configuration
    cached_config: Arc<RwLock<Option<Config>>>,
    /// Configuration version
    current_version: Arc<RwLock<u64>>,
    /// HTTP client
    http_client: reqwest::Client,
}

/// Configuration response from config service
#[derive(Debug, Deserialize)]
pub struct ConfigResponse {
    pub version: u64,
    pub data: ConfigData,
    pub checksum: String,
}

/// Configuration data structure
type ConfigData = Config;

/// Configuration update notification
#[derive(Debug, Deserialize)]
pub struct ConfigUpdateNotification {
    pub service: String,
    pub version: u64,
    pub update_type: String,
}

impl ConfigClient {
    /// Create a new configuration client
    pub fn new(config_service_url: String, service_name: String) -> Self {
        Self {
            config_service_url,
            service_name,
            cached_config: Arc::new(RwLock::new(None)),
            current_version: Arc::new(RwLock::new(0)),
            http_client: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .unwrap(),
        }
    }

    /// Fetch configuration from config service
    pub async fn fetch_config(&self) -> Result<Config, ApiGatewayError> {
        let url = format!(
            "{}/config/{}",
            self.config_service_url, self.service_name
        );

        let response = self
            .http_client
            .get(&url)
            .header("X-Service-Name", &self.service_name)
            .send()
            .await
            .map_err(|e| ApiGatewayError::ConfigFetchError(e.to_string()))?;

        if !response.status().is_success() {
            return Err(ApiGatewayError::ConfigFetchError(format!(
                "Config service returned: {}",
                response.status()
            )));
        }

        let config_response: ConfigResponse = response
            .json()
            .await
            .map_err(|e| ApiGatewayError::ConfigParseError(e.to_string()))?;

        // Verify checksum
        if !self.verify_checksum(&config_response) {
            return Err(ApiGatewayError::ConfigChecksumError);
        }

        // Update version
        *self.current_version.write().await = config_response.version;

        // Use the config data directly
        let config = config_response.data;

        // Update cache
        *self.cached_config.write().await = Some(config.clone());

        Ok(config)
    }

    /// Check for configuration updates
    pub async fn check_for_updates(&self) -> Result<bool, ApiGatewayError> {
        let url = format!(
            "{}/config/{}/version",
            self.config_service_url, self.service_name
        );

        let response = self
            .http_client
            .get(&url)
            .header("X-Service-Name", &self.service_name)
            .send()
            .await
            .map_err(|e| ApiGatewayError::ConfigFetchError(e.to_string()))?;

        let version_info: VersionInfo = response
            .json()
            .await
            .map_err(|e| ApiGatewayError::ConfigParseError(e.to_string()))?;

        let current = *self.current_version.read().await;
        Ok(version_info.version > current)
    }

    /// Start configuration watch loop
    pub async fn start_watch_loop(&self, update_interval: Duration) {
        let client = self.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(update_interval);

            loop {
                interval.tick().await;

                match client.check_for_updates().await {
                    Ok(true) => {
                        tracing::info!("Configuration update detected, fetching new config");

                        match client.fetch_config().await {
                            Ok(_) => {
                                tracing::info!("Configuration updated successfully");
                                // TODO: Trigger configuration reload in main application
                            }
                            Err(e) => {
                                tracing::error!("Failed to fetch updated configuration: {}", e);
                            }
                        }
                    }
                    Ok(false) => {
                        tracing::debug!("No configuration updates");
                    }
                    Err(e) => {
                        tracing::error!("Failed to check for updates: {}", e);
                    }
                }
            }
        });
    }

    /// Get cached configuration
    pub async fn get_cached_config(&self) -> Option<Config> {
        self.cached_config.read().await.clone()
    }

    /// Update specific configuration
    pub async fn update_config(
        &self,
        key: &str,
        value: serde_json::Value,
    ) -> Result<(), ApiGatewayError> {
        let url = format!(
            "{}/config/{}/update",
            self.config_service_url, self.service_name
        );

        let update_request = ConfigUpdateRequest {
            key: key.to_string(),
            value,
            reason: "Updated via API Gateway".to_string(),
        };

        let response = self
            .http_client
            .put(&url)
            .header("X-Service-Name", &self.service_name)
            .json(&update_request)
            .send()
            .await
            .map_err(|e| ApiGatewayError::ConfigUpdateError(e.to_string()))?;

        if !response.status().is_success() {
            return Err(ApiGatewayError::ConfigUpdateError(format!(
                "Config service returned: {}",
                response.status()
            )));
        }

        // Fetch updated configuration
        self.fetch_config().await?;

        Ok(())
    }

    /// Register for configuration change notifications
    pub async fn register_for_notifications(
        &self,
        callback_url: &str,
    ) -> Result<(), ApiGatewayError> {
        let url = format!("{}/config/subscribe", self.config_service_url);

        let subscription = NotificationSubscription {
            service: self.service_name.clone(),
            callback_url: callback_url.to_string(),
            events: vec!["update".to_string(), "delete".to_string()],
        };

        let response = self
            .http_client
            .post(&url)
            .json(&subscription)
            .send()
            .await
            .map_err(|e| ApiGatewayError::ConfigSubscriptionError(e.to_string()))?;

        if !response.status().is_success() {
            return Err(ApiGatewayError::ConfigSubscriptionError(format!(
                "Failed to subscribe: {}",
                response.status()
            )));
        }

        Ok(())
    }


    /// Verify configuration checksum
    fn verify_checksum(&self, _response: &ConfigResponse) -> bool {
        // TODO: Implement checksum verification
        // For now, just return true
        true
    }
}

impl Clone for ConfigClient {
    fn clone(&self) -> Self {
        Self {
            config_service_url: self.config_service_url.clone(),
            service_name: self.service_name.clone(),
            cached_config: self.cached_config.clone(),
            current_version: self.current_version.clone(),
            http_client: self.http_client.clone(),
        }
    }
}

/// Version information
#[derive(Debug, Deserialize)]
struct VersionInfo {
    version: u64,
    last_updated: String,
}

/// Configuration update request
#[derive(Debug, Serialize)]
struct ConfigUpdateRequest {
    key: String,
    value: serde_json::Value,
    reason: String,
}

/// Notification subscription
#[derive(Debug, Serialize)]
struct NotificationSubscription {
    service: String,
    callback_url: String,
    events: Vec<String>,
}

// Config structures are imported from crate::config
