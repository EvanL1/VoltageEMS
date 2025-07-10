use crate::error::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub mod influxdb_storage;
pub mod redis_storage;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataPoint {
    pub key: String,
    pub timestamp: DateTime<Utc>,
    pub value: DataValue,
    pub tags: HashMap<String, String>,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DataValue {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Json(serde_json::Value),
    Binary(Vec<u8>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryFilter {
    pub key_pattern: Option<String>,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub tags: HashMap<String, String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    pub data_points: Vec<DataPoint>,
    pub total_count: Option<u64>,
    pub has_more: bool,
}

pub type StorageBackend = Box<dyn Storage + Send + Sync>;

#[async_trait]
pub trait Storage {
    async fn connect(&mut self) -> Result<()>;
    async fn disconnect(&mut self) -> Result<()>;
    async fn is_connected(&self) -> bool;

    async fn store_data_point(&mut self, data_point: &DataPoint) -> Result<()>;
    async fn store_data_points(&mut self, data_points: &[DataPoint]) -> Result<()>;

    async fn query_data_points(&self, filter: &QueryFilter) -> Result<QueryResult>;
    async fn delete_data_points(&mut self, filter: &QueryFilter) -> Result<u64>;

    async fn get_keys(&self, pattern: Option<&str>) -> Result<Vec<String>>;
    async fn get_statistics(&self) -> Result<StorageStats>;

    fn get_name(&self) -> &str;
    fn get_config(&self) -> serde_json::Value;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageStats {
    pub total_data_points: u64,
    pub storage_size_bytes: u64,
    pub last_write_time: Option<DateTime<Utc>>,
    pub last_read_time: Option<DateTime<Utc>>,
    pub connection_status: String,
}

pub struct StorageManager {
    backends: HashMap<String, StorageBackend>,
    default_backend: String,
}

impl StorageManager {
    pub fn new() -> Self {
        Self {
            backends: HashMap::new(),
            default_backend: String::new(),
        }
    }

    pub fn add_backend(&mut self, name: String, backend: StorageBackend) {
        self.backends.insert(name, backend);
    }

    pub fn set_default_backend(&mut self, name: String) {
        self.default_backend = name;
    }

    pub fn get_backend(&mut self, name: Option<&str>) -> Option<&mut StorageBackend> {
        let backend_name = name.unwrap_or(&self.default_backend);
        self.backends.get_mut(backend_name)
    }

    pub fn get_backend_readonly(&self, name: Option<&str>) -> Option<&StorageBackend> {
        let backend_name = name.unwrap_or(&self.default_backend);
        self.backends.get(backend_name)
    }

    pub async fn connect_all(&mut self) -> Result<()> {
        for (name, backend) in &mut self.backends {
            tracing::info!("Connecting to storage backend: {}", name);
            if let Err(e) = backend.connect().await {
                tracing::error!("Failed to connect to storage backend {}: {}", name, e);
            }
        }
        Ok(())
    }

    pub async fn disconnect_all(&mut self) -> Result<()> {
        for (name, backend) in &mut self.backends {
            tracing::info!("Disconnecting from storage backend: {}", name);
            if let Err(e) = backend.disconnect().await {
                tracing::error!("Failed to disconnect from storage backend {}: {}", name, e);
            }
        }
        Ok(())
    }

    pub async fn get_all_statistics(&self) -> HashMap<String, StorageStats> {
        let mut stats = HashMap::new();
        for (name, backend) in &self.backends {
            if let Ok(backend_stats) = backend.get_statistics().await {
                stats.insert(name.clone(), backend_stats);
            }
        }
        stats
    }
}
