use std::sync::Arc;
use std::time::{Duration, Instant};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use tokio::sync::RwLock;
use std::collections::HashMap;
use serde_json;

use crate::core::config::config_manager::ChannelConfig;
use crate::utils::Result;

/// Channel status information
#[derive(Debug, Clone)]
pub struct ChannelStatus {
    /// Channel identifier
    pub id: String,
    /// Connection status
    pub connected: bool,
    /// Last response time in milliseconds
    pub last_response_time: f64,
    /// Last error message
    pub last_error: String,
    /// Last status update time
    pub last_update_time: DateTime<Utc>,
}

impl ChannelStatus {
    /// Create a new channel status
    pub fn new(channel_id: &str) -> Self {
        Self {
            id: channel_id.to_string(),
            connected: false,
            last_response_time: 0.0,
            last_error: String::new(),
            last_update_time: Utc::now(),
        }
    }

    /// Check if channel has an error
    pub fn has_error(&self) -> bool {
        !self.last_error.is_empty()
    }
}

/// Point data structure for real-time values
#[derive(Debug, Clone)]
pub struct PointData {
    /// Point ID
    pub id: String,
    /// Point value
    pub value: serde_json::Value,
    /// Data quality
    pub quality: bool,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

/// Communications interface trait
///
/// This trait defines the interface that all communication protocols must implement.
#[async_trait]
pub trait ComBase: Send + Sync {
    /// Get the name of the communication service
    fn name(&self) -> &str;
    /// Get the channel ID
    fn channel_id(&self) -> &str;
    /// Get protocol type
    fn protocol_type(&self) -> &str;
    /// Get protocol parameters
    fn get_parameters(&self) -> HashMap<String, String>;
    /// Check if the service is running
    async fn is_running(&self) -> bool;
    /// Start the communication service
    async fn start(&mut self) -> Result<()>;
    /// Stop the communication service
    async fn stop(&mut self) -> Result<()>;
    /// Get the current status of the channel
    async fn status(&self) -> ChannelStatus;
    /// Check if the channel has an error
    async fn has_error(&self) -> bool {
        self.status().await.has_error()
    }
    /// Get the last error message from the channel
    async fn last_error(&self) -> String {
        self.status().await.last_error
    }
    /// Get all points' real-time data from the channel
    async fn get_all_points(&self) -> Vec<PointData> {
        Vec::new() // 默认实现返回空数组
    }
}

/// Base implementation of ComBase trait
pub struct ComBaseImpl {
    /// Service name
    name: String,
    /// Channel ID
    channel_id: String,
    /// Protocol type
    protocol_type: String,
    /// Channel configuration
    config: ChannelConfig,
    /// Channel status
    status: Arc<RwLock<ChannelStatus>>,
    /// Running state
    running: Arc<RwLock<bool>>,
    /// Last error message
    last_error: Arc<RwLock<String>>,
}

impl ComBaseImpl {
    /// Create a new ComBaseImpl instance
    pub fn new(name: &str, protocol_type: &str, config: ChannelConfig) -> Self {
        let channel_id = config.id.clone();
        let status = ChannelStatus::new(&channel_id);
        
        Self {
            name: name.to_string(),
            channel_id,
            protocol_type: protocol_type.to_string(),
            config,
            status: Arc::new(RwLock::new(status)),
            running: Arc::new(RwLock::new(false)),
            last_error: Arc::new(RwLock::new(String::new())),
        }
    }
    
    /// Get the name of the communication service
    pub fn name(&self) -> &str {
        &self.name
    }
    
    /// Get the channel ID
    pub fn channel_id(&self) -> &str {
        &self.channel_id
    }
    
    /// Get protocol type
    pub fn protocol_type(&self) -> &str {
        &self.protocol_type
    }
    
    /// Get protocol parameters as HashMap
    pub fn get_parameters(&self) -> HashMap<String, String> {
        let mut params = HashMap::new();
        // 将配置转为HashMap
        params.insert("protocol".to_string(), self.protocol_type.clone());
        params.insert("channel_id".to_string(), self.channel_id.clone());
        // 实际实现中可以添加更多从config中提取的参数
        params
    }
    
    /// Get a reference to the channel configuration
    pub fn config(&self) -> &ChannelConfig {
        &self.config
    }
    
    /// Check if the service is running
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }
    
    /// Start the communication service base implementation
    pub async fn start(&self) -> Result<()> {
        let mut running = self.running.write().await;
        *running = true;
        Ok(())
    }
    
    /// Stop the communication service base implementation
    pub async fn stop(&self) -> Result<()> {
        let mut running = self.running.write().await;
        *running = false;
        Ok(())
    }
    
    /// Get the current status of the channel
    pub async fn status(&self) -> ChannelStatus {
        self.status.read().await.clone()
    }
    
    /// Update the channel status
    pub async fn update_status(&self, connected: bool, response_time: f64, error: Option<&str>) {
        let mut status = self.status.write().await;
        status.connected = connected;
        status.last_response_time = response_time;
        if let Some(err) = error {
            status.last_error = err.to_string();
        }
        status.last_update_time = Utc::now();
        
        // Also update the last error
        if let Some(err) = error {
            let mut last_error = self.last_error.write().await;
            *last_error = err.to_string();
        }
    }
    
    /// Measure the execution time of a function and update the status
    pub async fn measure_execution<F, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce() -> Result<T> + Send,
    {
        let start = Instant::now();
        let result = f();
        let duration = start.elapsed();
        
        // Update status based on the result
        match &result {
            Ok(_) => {
                self.update_status(true, duration.as_secs_f64() * 1000.0, None).await;
            }
            Err(e) => {
                self.update_status(false, duration.as_secs_f64() * 1000.0, Some(&e.to_string())).await;
            }
        }
        
        result
    }
    
    /// Measure the execution time of an async function and update the status
    pub async fn measure_execution_async<F, T, E>(&self, f: F) -> std::result::Result<T, E>
    where
        F: FnOnce() -> std::result::Result<T, E> + Send,
        E: ToString,
    {
        let start = Instant::now();
        let result = f();
        let duration = start.elapsed();
        
        // Update status based on the result
        match &result {
            Ok(_) => {
                self.update_status(true, duration.as_secs_f64() * 1000.0, None).await;
            }
            Err(e) => {
                self.update_status(false, duration.as_secs_f64() * 1000.0, Some(&e.to_string())).await;
            }
        }
        
        result
    }
    
    /// Set the last error message
    pub async fn set_error(&self, error: &str) {
        let mut last_error = self.last_error.write().await;
        *last_error = error.to_string();
        
        // Also update the status
        let mut status = self.status.write().await;
        status.last_error = error.to_string();
        status.connected = false;
        status.last_update_time = Utc::now();
    }
    
    /// Set the running state
    pub async fn set_running(&self, running: bool) {
        let mut r = self.running.write().await;
        *r = running;
    }
} 