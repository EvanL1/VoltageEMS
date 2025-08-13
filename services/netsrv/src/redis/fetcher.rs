use crate::config::RedisConfig;
use crate::error::{NetSrvError, Result};
use crate::redis::RedisFunctionClient;
use serde_json::Value;
use std::time::{Duration, Instant};
use tokio::time;
use tracing::{debug, error, info};

/// Optimized data fetcher using Redis Functions
/// Replaces the inefficient KEYS-based approach with server-side functions
pub struct OptimizedDataFetcher {
    client: RedisFunctionClient,
    config: RedisConfig,
    poll_interval: Duration,
    last_fetch_time: Instant,
    batch_size: usize,
}

impl OptimizedDataFetcher {
    pub async fn new(
        config: RedisConfig,
        poll_interval_secs: u64,
        batch_size: usize,
    ) -> Result<Self> {
        let client = RedisFunctionClient::new(&config.url).await?;

        Ok(Self {
            client,
            config,
            poll_interval: Duration::from_secs(poll_interval_secs),
            last_fetch_time: Instant::now(),
            batch_size,
        })
    }

    /// Fetch data using Redis Functions for better performance
    pub async fn fetch_data(&mut self) -> Result<Value> {
        let mut all_data = json!({});
        let data_obj = all_data.as_object_mut().unwrap();

        // Iterate through configured data key patterns
        for pattern in &self.config.data_keys {
            debug!("Fetching data for pattern: {}", pattern);

            // Use Redis Function to collect data efficiently
            match self
                .client
                .collect_data(pattern, self.batch_size, "hash")
                .await
            {
                Ok(result) => {
                    let total = result.get("total").and_then(|v| v.as_u64()).unwrap_or(0);

                    debug!("Collected {} items for pattern: {}", total, pattern);

                    if let Some(data_array) = result.get("data").and_then(|v| v.as_array()) {
                        // Process collected data
                        for item in data_array {
                            if let (Some(key), Some(data)) =
                                (item.get("key").and_then(|v| v.as_str()), item.get("data"))
                            {
                                // Strip prefix if configured
                                let key_without_prefix = if key.starts_with(&self.config.prefix) {
                                    &key[self.config.prefix.len()..]
                                } else {
                                    key
                                };

                                data_obj.insert(key_without_prefix.to_string(), data.clone());
                            }
                        }
                    }
                },
                Err(e) => {
                    error!("Failed to collect data for pattern {}: {}", pattern, e);
                },
            }
        }

        self.last_fetch_time = Instant::now();
        Ok(all_data)
    }

    /// Fetch optimized channel and module data
    pub async fn fetch_optimized_data(&mut self) -> Result<Value> {
        let mut all_data = json!({});

        // Collect comsrv channel data
        let channel_pattern = "comsrv:*:T";
        match self.client.collect_data(channel_pattern, 100, "hash").await {
            Ok(result) => {
                if let Some(data_array) = result.get("data").and_then(|v| v.as_array()) {
                    for item in data_array {
                        if let (Some(key), Some(data)) =
                            (item.get("key").and_then(|v| v.as_str()), item.get("data"))
                        {
                            // Parse channel ID from key (e.g., "comsrv:1001:T" -> "channel_1001")
                            if let Some(channel_id) = key.split(':').nth(1) {
                                all_data[format!("channel_{}", channel_id)] = data.clone();
                            }
                        }
                    }
                }

                let total = result.get("total").and_then(|v| v.as_u64()).unwrap_or(0);
                info!("Collected {} channels", total);
            },
            Err(e) => {
                error!("Failed to collect channel data: {}", e);
            },
        }

        // Collect modsrv module data
        let module_pattern = "modsrv:realtime:module:*";
        match self.client.collect_data(module_pattern, 100, "hash").await {
            Ok(result) => {
                if let Some(data_array) = result.get("data").and_then(|v| v.as_array()) {
                    for item in data_array {
                        if let (Some(key), Some(data)) =
                            (item.get("key").and_then(|v| v.as_str()), item.get("data"))
                        {
                            // Extract module ID from key
                            if let Some(module_id) = key.split(':').last() {
                                all_data[module_id] = data.clone();
                            }
                        }
                    }
                }
            },
            Err(e) => {
                error!("Failed to collect module data: {}", e);
            },
        }

        self.last_fetch_time = Instant::now();
        Ok(all_data)
    }

    /// Start polling with optimized data collection
    pub async fn start_polling(&mut self, tx: tokio::sync::mpsc::Sender<Value>) -> Result<()> {
        let mut interval = time::interval(self.poll_interval);

        loop {
            interval.tick().await;

            match self.fetch_optimized_data().await {
                Ok(data) => {
                    if let Err(e) = tx.send(data).await {
                        error!("Failed to send data to channel: {}", e);
                    }
                },
                Err(e) => {
                    error!("Failed to fetch data: {}", e);
                    // Wait before retry
                    time::sleep(Duration::from_secs(5)).await;
                },
            }
        }
    }

    /// Configure data collection routes
    pub async fn configure_routes(&mut self, routes: Vec<RouteConfig>) -> Result<()> {
        for route in routes {
            let mut config = HashMap::new();
            config.insert("source".to_string(), json!(route.source_pattern));
            config.insert("destination".to_string(), json!(route.destination));
            config.insert("type".to_string(), json!(route.forward_type));
            config.insert("enabled".to_string(), json!(route.enabled));

            if let Some(filter) = route.filter {
                config.insert("filter".to_string(), json!(filter));
            }

            self.client.configure_route(&route.id, &config).await?;
        }

        Ok(())
    }

    /// Get network statistics
    pub async fn get_stats(&mut self) -> Result<NetworkStats> {
        let stats = self.client.get_stats("summary").await?;

        Ok(NetworkStats {
            total_forwarded: stats.get("forwarded").and_then(|v| v.as_u64()).unwrap_or(0),
            total_failed: stats.get("failed").and_then(|v| v.as_u64()).unwrap_or(0),
            total_queues: stats
                .get("total_queues")
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
            total_queued: stats
                .get("total_queued")
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
        })
    }
}

/// Route configuration
#[derive(Debug, Clone)]
pub struct RouteConfig {
    pub id: String,
    pub source_pattern: String,
    pub destination: String,
    pub forward_type: String,
    pub enabled: bool,
    pub filter: Option<HashMap<String, Value>>,
}

/// Network statistics
#[derive(Debug, Clone)]
pub struct NetworkStats {
    pub total_forwarded: u64,
    pub total_failed: u64,
    pub total_queues: u64,
    pub total_queued: u64,
}

use std::collections::HashMap;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires Redis with functions
    async fn test_optimized_fetch() {
        let config = RedisConfig {
            url: "redis://localhost:6379".to_string(),
            prefix: "test:".to_string(),
            data_keys: vec!["test:*".to_string()],
            poll_interval_ms: 1000,
        };

        let mut fetcher = OptimizedDataFetcher::new(config, 1, 100).await.unwrap();

        let data = fetcher.fetch_data().await.unwrap();
        assert!(data.is_object());
    }
}
