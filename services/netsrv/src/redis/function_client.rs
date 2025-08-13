use crate::error::{NetSrvError, Result};
use serde_json::Value;
use std::collections::HashMap;
use tracing::{debug, error, info};
use voltage_libs::redis::RedisClient;

/// Redis Functions client for NetSrv
/// Uses Redis Functions for efficient data collection and forwarding
pub struct RedisFunctionClient {
    client: RedisClient,
}

impl RedisFunctionClient {
    /// Create a new Redis Functions client
    pub async fn new(redis_url: &str) -> Result<Self> {
        let client = RedisClient::new(redis_url)
            .await
            .map_err(|e| NetSrvError::Redis(format!("Failed to create Redis client: {}", e)))?;

        info!("Successfully created Redis Functions client");

        Ok(Self { client })
    }

    /// Collect data from Redis using Redis Functions
    /// This is more efficient than using KEYS command
    pub async fn collect_data(
        &mut self,
        source_pattern: &str,
        batch_size: usize,
        data_type: &str,
    ) -> Result<Value> {
        debug!(
            "Collecting data - pattern: {}, batch_size: {}, type: {}",
            source_pattern, batch_size, data_type
        );

        let args = vec![
            source_pattern.to_string(),
            batch_size.to_string(),
            data_type.to_string(),
        ];

        let result = self
            .client
            .fcall("netsrv_collect_data", &[], &args)
            .await
            .map_err(|e| NetSrvError::Redis(format!("Failed to collect data: {}", e)))?;

        debug!("Collected data successfully");

        // Parse the JSON result
        serde_json::from_str(&result)
            .map_err(|e| NetSrvError::Data(format!("Failed to parse collected data: {}", e)))
    }

    /// Forward collected data to a destination
    pub async fn forward_data(
        &mut self,
        destination: &str,
        data: &Value,
        forward_type: &str,
    ) -> Result<Value> {
        debug!(
            "Forwarding data to {} using {} method",
            destination, forward_type
        );

        let data_json = serde_json::to_string(data)
            .map_err(|e| NetSrvError::Data(format!("Failed to serialize data: {}", e)))?;

        let args = vec![data_json, forward_type.to_string()];

        let result = self
            .client
            .fcall("netsrv_forward_data", &[destination], &args)
            .await
            .map_err(|e| NetSrvError::Redis(format!("Failed to forward data: {}", e)))?;

        // Parse the result
        serde_json::from_str(&result)
            .map_err(|e| NetSrvError::Data(format!("Failed to parse forward result: {}", e)))
    }

    /// Configure a network route
    pub async fn configure_route(
        &mut self,
        route_id: &str,
        config: &HashMap<String, Value>,
    ) -> Result<()> {
        let config_json = serde_json::to_string(config)
            .map_err(|e| NetSrvError::Data(format!("Failed to serialize config: {}", e)))?;

        let args = vec![config_json];

        self.client
            .fcall("netsrv_configure_route", &[route_id], &args)
            .await
            .map_err(|e| NetSrvError::Redis(format!("Failed to configure route: {}", e)))?;

        info!("Configured route: {}", route_id);
        Ok(())
    }

    /// Get all configured routes
    pub async fn get_routes(&mut self) -> Result<Vec<HashMap<String, Value>>> {
        let result = self
            .client
            .fcall("netsrv_get_routes", &[], &[])
            .await
            .map_err(|e| NetSrvError::Redis(format!("Failed to get routes: {}", e)))?;

        serde_json::from_str(&result)
            .map_err(|e| NetSrvError::Data(format!("Failed to parse routes: {}", e)))
    }

    /// Get network statistics
    pub async fn get_stats(&mut self, stats_type: &str) -> Result<Value> {
        let args = vec![stats_type.to_string()];

        let result = self
            .client
            .fcall("netsrv_get_stats", &[], &args)
            .await
            .map_err(|e| NetSrvError::Redis(format!("Failed to get stats: {}", e)))?;

        serde_json::from_str(&result)
            .map_err(|e| NetSrvError::Data(format!("Failed to parse stats: {}", e)))
    }

    /// Clear network queues
    pub async fn clear_queues(&mut self, pattern: &str) -> Result<usize> {
        let args = vec![pattern.to_string()];

        let result = self
            .client
            .fcall("netsrv_clear_queues", &[], &args)
            .await
            .map_err(|e| NetSrvError::Redis(format!("Failed to clear queues: {}", e)))?;

        result
            .parse::<usize>()
            .map_err(|e| NetSrvError::Data(format!("Failed to parse clear result: {}", e)))
    }

    /// Collect and forward data in one atomic operation
    pub async fn collect_and_forward(
        &mut self,
        source_pattern: &str,
        destination: &str,
        batch_size: usize,
        forward_type: &str,
    ) -> Result<(usize, usize)> {
        // First collect data
        let collected_data = self
            .collect_data(source_pattern, batch_size, "hash")
            .await?;

        // Extract the data array
        let data = collected_data.get("data").ok_or_else(|| {
            NetSrvError::Data("Missing data field in collection result".to_string())
        })?;

        let total = collected_data
            .get("total")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;

        // Forward the collected data
        let forward_result = self.forward_data(destination, data, forward_type).await?;

        let forwarded = forward_result
            .get("success")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;

        Ok((total, forwarded))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires Redis with functions loaded
    async fn test_collect_data() {
        let mut client = RedisFunctionClient::new("redis://localhost:6379")
            .await
            .unwrap();

        let result = client.collect_data("test:*", 10, "hash").await.unwrap();

        assert!(result.is_object());
        assert!(result.get("total").is_some());
        assert!(result.get("data").is_some());
    }

    #[tokio::test]
    #[ignore] // Requires Redis with functions loaded
    async fn test_configure_route() {
        let mut client = RedisFunctionClient::new("redis://localhost:6379")
            .await
            .unwrap();

        let mut config = HashMap::new();
        config.insert("source".to_string(), json!("test:*"));
        config.insert("destination".to_string(), json!("remote1"));
        config.insert("type".to_string(), json!("publish"));
        config.insert("enabled".to_string(), json!(true));

        client.configure_route("test_route", &config).await.unwrap();

        let routes = client.get_routes().await.unwrap();
        assert!(!routes.is_empty());
    }
}
