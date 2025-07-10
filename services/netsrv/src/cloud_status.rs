//! Cloud synchronization status management
//!
//! This module manages the status of cloud synchronization for each network client,
//! storing the information in Redis using optimized Hash structures.

use crate::error::{NetSrvError, Result};
use crate::redis::RedisConnection;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, error, info};

/// Cloud synchronization status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudSyncStatus {
    /// Network name
    pub network_name: String,
    /// Connection status
    pub connected: bool,
    /// Last successful sync time
    pub last_sync_time: Option<DateTime<Utc>>,
    /// Last error message
    pub last_error: Option<String>,
    /// Total messages sent
    pub messages_sent: u64,
    /// Total messages failed
    pub messages_failed: u64,
    /// Current queue size
    pub queue_size: usize,
}

impl CloudSyncStatus {
    pub fn new(network_name: String) -> Self {
        Self {
            network_name,
            connected: false,
            last_sync_time: None,
            last_error: None,
            messages_sent: 0,
            messages_failed: 0,
            queue_size: 0,
        }
    }

    /// Convert to Redis hash fields
    pub fn to_redis_fields(&self) -> Vec<(&str, String)> {
        vec![
            ("connected", self.connected.to_string()),
            (
                "last_sync_time",
                self.last_sync_time
                    .map(|t| t.to_rfc3339())
                    .unwrap_or_default(),
            ),
            ("last_error", self.last_error.clone().unwrap_or_default()),
            ("messages_sent", self.messages_sent.to_string()),
            ("messages_failed", self.messages_failed.to_string()),
            ("queue_size", self.queue_size.to_string()),
            ("updated_at", Utc::now().to_rfc3339()),
        ]
    }
}

/// Cloud status manager for managing synchronization status
pub struct CloudStatusManager {
    redis: RedisConnection,
    /// Base key for cloud status: netsrv:cloud:status
    base_key: String,
}

impl CloudStatusManager {
    pub fn new(redis: RedisConnection) -> Self {
        Self {
            redis,
            base_key: "netsrv:cloud:status".to_string(),
        }
    }

    /// Update cloud synchronization status for a network
    pub fn update_status(&mut self, network_name: &str, status: &CloudSyncStatus) -> Result<()> {
        let key = format!("{}:{}", self.base_key, network_name);
        let fields: Vec<(&str, String)> = status.to_redis_fields();
        let fields_ref: Vec<(&str, &str)> = fields.iter().map(|(k, v)| (*k, v.as_str())).collect();

        self.redis.set_hash_multiple(&key, fields_ref)?;

        debug!(
            "Updated cloud status for network '{}': connected={}, sent={}, failed={}",
            network_name, status.connected, status.messages_sent, status.messages_failed
        );

        Ok(())
    }

    /// Get cloud synchronization status for a network
    pub fn get_status(&mut self, network_name: &str) -> Result<CloudSyncStatus> {
        let key = format!("{}:{}", self.base_key, network_name);
        let data = self.redis.get_hash(&key)?;

        let mut status = CloudSyncStatus::new(network_name.to_string());

        if let Some(connected) = data.get("connected") {
            status.connected = connected.parse().unwrap_or(false);
        }

        if let Some(last_sync) = data.get("last_sync_time") {
            if !last_sync.is_empty() {
                status.last_sync_time = DateTime::parse_from_rfc3339(last_sync)
                    .ok()
                    .map(|dt| dt.with_timezone(&Utc));
            }
        }

        if let Some(error) = data.get("last_error") {
            if !error.is_empty() {
                status.last_error = Some(error.clone());
            }
        }

        if let Some(sent) = data.get("messages_sent") {
            status.messages_sent = sent.parse().unwrap_or(0);
        }

        if let Some(failed) = data.get("messages_failed") {
            status.messages_failed = failed.parse().unwrap_or(0);
        }

        if let Some(queue) = data.get("queue_size") {
            status.queue_size = queue.parse().unwrap_or(0);
        }

        Ok(status)
    }

    /// Get all cloud synchronization statuses
    pub fn get_all_statuses(&mut self) -> Result<HashMap<String, CloudSyncStatus>> {
        let pattern = format!("{}:*", self.base_key);
        let keys = self.redis.get_keys(&pattern)?;

        let mut statuses = HashMap::new();

        for key in keys {
            if let Some(network_name) = key.strip_prefix(&format!("{}:", self.base_key)) {
                match self.get_status(network_name) {
                    Ok(status) => {
                        statuses.insert(network_name.to_string(), status);
                    }
                    Err(e) => {
                        error!("Failed to get status for network '{}': {}", network_name, e);
                    }
                }
            }
        }

        Ok(statuses)
    }

    /// Update connection status
    pub fn update_connection_status(&mut self, network_name: &str, connected: bool) -> Result<()> {
        let key = format!("{}:{}", self.base_key, network_name);
        let fields = vec![
            ("connected", connected.to_string()),
            ("updated_at", Utc::now().to_rfc3339()),
        ];
        let fields_ref: Vec<(&str, &str)> = fields.iter().map(|(k, v)| (*k, v.as_str())).collect();

        self.redis.set_hash_multiple(&key, fields_ref)?;

        info!(
            "Updated connection status for network '{}': {}",
            network_name, connected
        );
        Ok(())
    }

    /// Record successful message send
    pub fn record_success(&mut self, network_name: &str) -> Result<()> {
        let mut status = self.get_status(network_name)?;
        status.messages_sent += 1;
        status.last_sync_time = Some(Utc::now());
        status.last_error = None; // Clear error on success

        self.update_status(network_name, &status)
    }

    /// Record failed message send
    pub fn record_failure(&mut self, network_name: &str, error: &str) -> Result<()> {
        let mut status = self.get_status(network_name)?;
        status.messages_failed += 1;
        status.last_error = Some(error.to_string());

        self.update_status(network_name, &status)
    }

    /// Update queue size
    pub fn update_queue_size(&mut self, network_name: &str, queue_size: usize) -> Result<()> {
        let key = format!("{}:{}", self.base_key, network_name);
        let fields = vec![
            ("queue_size", queue_size.to_string()),
            ("updated_at", Utc::now().to_rfc3339()),
        ];
        let fields_ref: Vec<(&str, &str)> = fields.iter().map(|(k, v)| (*k, v.as_str())).collect();

        self.redis.set_hash_multiple(&key, fields_ref)?;
        Ok(())
    }
}

/// Helper function to create a summary of all cloud statuses
pub fn create_cloud_summary(statuses: &HashMap<String, CloudSyncStatus>) -> serde_json::Value {
    let total_sent: u64 = statuses.values().map(|s| s.messages_sent).sum();
    let total_failed: u64 = statuses.values().map(|s| s.messages_failed).sum();
    let connected_count = statuses.values().filter(|s| s.connected).count();
    let total_count = statuses.len();

    serde_json::json!({
        "summary": {
            "total_networks": total_count,
            "connected_networks": connected_count,
            "total_messages_sent": total_sent,
            "total_messages_failed": total_failed,
            "success_rate": if total_sent + total_failed > 0 {
                (total_sent as f64 / (total_sent + total_failed) as f64) * 100.0
            } else {
                0.0
            }
        },
        "networks": statuses
    })
}
