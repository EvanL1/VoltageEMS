//! InfluxDB client for alarm historical storage
//! 
//! Based on hissrv InfluxDB client but adapted for alarm-specific data models.

use super::{AlarmDataPoint, InfluxDBConfig};
use anyhow::Result;
use reqwest::Client;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, error, info};

/// InfluxDB 3.2 client for alarm data
#[derive(Clone)]
pub struct InfluxDBClient {
    config: InfluxDBConfig,
    client: Client,
    batch_buffer: Arc<Mutex<Vec<AlarmDataPoint>>>,
}

impl InfluxDBClient {
    /// Create new InfluxDB client
    pub fn new(config: InfluxDBConfig) -> Self {
        let client = Client::new();
        let batch_buffer = Arc::new(Mutex::new(Vec::with_capacity(config.batch_size)));

        Self {
            config,
            client,
            batch_buffer,
        }
    }

    /// Test connection to InfluxDB
    pub async fn ping(&self) -> Result<()> {
        let url = format!("{}/health", self.config.url);
        
        let mut request = self.client.get(&url);
        
        // Add authentication header if token is provided
        if let Some(ref token) = self.config.token {
            request = request.header("Authorization", format!("Bearer {}", token));
        }

        let response = request.send().await.map_err(|e| {
            anyhow::anyhow!("Failed to connect to InfluxDB: {}", e)
        })?;

        if response.status().is_success() {
            info!("InfluxDB connection successful");
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "InfluxDB health check failed: {}",
                response.status()
            ))
        }
    }

    /// Write single alarm data point
    pub async fn write_alarm_point(&self, point: AlarmDataPoint) -> Result<()> {
        let mut buffer = self.batch_buffer.lock().await;
        buffer.push(point);

        // Trigger write if batch size is reached
        if buffer.len() >= self.config.batch_size {
            let points = buffer.drain(..).collect();
            drop(buffer); // Release lock
            self.flush_points(points).await?;
        }

        Ok(())
    }

    /// Write multiple alarm data points
    pub async fn write_alarm_points(&self, points: Vec<AlarmDataPoint>) -> Result<()> {
        for point in points {
            self.write_alarm_point(point).await?;
        }
        Ok(())
    }

    /// Force flush buffer
    pub async fn flush(&self) -> Result<()> {
        let mut buffer = self.batch_buffer.lock().await;
        if buffer.is_empty() {
            return Ok(());
        }

        let points = buffer.drain(..).collect();
        drop(buffer); // Release lock
        self.flush_points(points).await
    }

    /// Execute actual write operation
    async fn flush_points(&self, points: Vec<AlarmDataPoint>) -> Result<()> {
        if points.is_empty() {
            return Ok(());
        }

        debug!("Writing {} alarm data points to InfluxDB", points.len());

        // Build Line Protocol data
        let line_protocol: Vec<String> = points
            .iter()
            .map(|point| point.to_line_protocol())
            .collect();
        let body = line_protocol.join("\n");

        // Build write URL
        let mut url = format!("{}/api/v2/write", self.config.url);
        
        // Add query parameters
        let mut query_params = vec![
            ("bucket", self.config.database.as_str()),
            ("precision", "ns"), // Nanosecond precision
        ];

        if let Some(ref org) = self.config.organization {
            query_params.push(("org", org.as_str()));
        }

        url.push('?');
        url.push_str(
            &query_params
                .iter()
                .map(|(k, v)| format!("{}={}", k, urlencoding::encode(v)))
                .collect::<Vec<_>>()
                .join("&"),
        );

        // Build request
        let mut request = self.client.post(&url)
            .header("Content-Type", "text/plain; charset=utf-8")
            .body(body);

        // Add authentication header
        if let Some(ref token) = self.config.token {
            request = request.header("Authorization", format!("Bearer {}", token));
        }

        // Send request
        let response = request.send().await.map_err(|e| {
            anyhow::anyhow!("Failed to write to InfluxDB: {}", e)
        })?;

        if response.status().is_success() {
            debug!("Successfully wrote {} alarm data points", points.len());
            Ok(())
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            error!("InfluxDB write failed: {} - {}", status, body);
            Err(anyhow::anyhow!(
                "Write failed: {} - {}",
                status, body
            ))
        }
    }

    /// Query alarm history (basic SQL query support)
    pub async fn query_alarm_history(&self, sql: &str) -> Result<Value> {
        let url = format!("{}/api/v2/query", self.config.url);
        
        let mut request = self.client.post(&url)
            .header("Content-Type", "application/json");

        // Add authentication header
        if let Some(ref token) = self.config.token {
            request = request.header("Authorization", format!("Bearer {}", token));
        }

        // Build query body
        let query_body = serde_json::json!({
            "query": sql,
            "type": "sql"
        });

        let response = request.json(&query_body).send().await.map_err(|e| {
            anyhow::anyhow!("Failed to query InfluxDB: {}", e)
        })?;

        if response.status().is_success() {
            let result: Value = response.json().await.map_err(|e| {
                anyhow::anyhow!("Failed to parse query result: {}", e)
            })?;
            Ok(result)
        } else {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            Err(anyhow::anyhow!(
                "Query failed: {} - {}",
                status, body
            ))
        }
    }

    /// Get configuration
    pub fn config(&self) -> &InfluxDBConfig {
        &self.config
    }

    /// Get current buffer size (for monitoring)
    pub async fn buffer_size(&self) -> usize {
        let buffer = self.batch_buffer.lock().await;
        buffer.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use std::collections::HashMap;

    #[test]
    fn test_client_creation() {
        let config = InfluxDBConfig::default();
        let client = InfluxDBClient::new(config.clone());
        assert_eq!(client.config.database, config.database);
        assert_eq!(client.config.batch_size, config.batch_size);
    }

    #[tokio::test]
    async fn test_buffer_management() {
        let config = InfluxDBConfig {
            batch_size: 2, // Small batch size for testing
            ..Default::default()
        };
        let client = InfluxDBClient::new(config);

        assert_eq!(client.buffer_size().await, 0);

        // Create test alarm data point
        let point = AlarmDataPoint::from_alarm_data(
            "test_alarm_001",
            "Warning",
            "New",
            "Test Alarm",
            "Test alarm description",
            Some("test_module"),
            Some("test_point"),
            Utc::now(),
        );

        // Buffer should increase but not trigger flush (batch size is 2)
        let result = client.write_alarm_point(point.clone()).await;
        // This might fail due to network, but buffer should still be managed
        assert_eq!(client.buffer_size().await, 1);
    }
}