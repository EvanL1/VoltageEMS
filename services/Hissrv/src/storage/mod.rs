//! Storage module using voltage-storage library

use crate::error::Result;
use async_trait::async_trait;
use voltage_storage::{
    DataPoint, DataValue, QueryFilter, QueryResult, Result as StorageResult,
    Storage as VoltageStorage, StorageManager as VoltageStorageManager,
};

// Re-export types from voltage-storage
pub use voltage_storage::{
    batch::{BatchWriter, BatchWriterConfig},
    retention::{RetentionPolicy, RetentionType},
    types::StorageStats,
};

/// Storage manager wrapper for hissrv
pub struct StorageManager {
    inner: VoltageStorageManager,
}

impl StorageManager {
    /// Create new storage manager
    pub fn new() -> Self {
        Self {
            inner: VoltageStorageManager::new(),
        }
    }

    /// Add InfluxDB backend
    pub async fn add_influxdb_backend(
        &self,
        name: &str,
        url: &str,
        database: &str,
        username: Option<&str>,
        password: Option<&str>,
    ) -> crate::error::Result<()> {
        let backend = if let (Some(user), Some(pass)) = (username, password) {
            Box::new(
                voltage_storage::influxdb_backend::InfluxDBBackend::with_auth(
                    url, database, user, pass,
                ),
            )
        } else {
            Box::new(voltage_storage::influxdb_backend::InfluxDBBackend::new(
                url, database,
            ))
        };

        self.inner
            .add_backend(name, backend as Box<dyn VoltageStorage + Send + Sync>, 1)
            .await
            .map_err(|e| crate::error::HisSrvError::StorageError {
                backend: "influxdb".to_string(),
                message: e.to_string(),
                operation: "add_backend".to_string(),
            })
    }

    /// Add Redis backend for real-time data
    pub async fn add_redis_backend(&self, name: &str, url: &str) -> crate::error::Result<()> {
        let backend = voltage_storage::redis_backend::RedisBackend::new(url)
            .await
            .map_err(|e| crate::error::HisSrvError::StorageError {
                backend: "redis".to_string(),
                message: e.to_string(),
                operation: "create_backend".to_string(),
            })?;

        self.inner
            .add_backend(
                name,
                Box::new(backend) as Box<dyn VoltageStorage + Send + Sync>,
                2,
            )
            .await
            .map_err(|e| crate::error::HisSrvError::StorageError {
                backend: "redis".to_string(),
                message: e.to_string(),
                operation: "add_backend".to_string(),
            })
    }

    /// Connect all backends
    pub async fn connect_all(&self) -> crate::error::Result<()> {
        self.inner
            .connect_all()
            .await
            .map_err(|e| crate::error::HisSrvError::StorageError {
                backend: "all".to_string(),
                message: e.to_string(),
                operation: "connect_all".to_string(),
            })
    }

    /// Disconnect all backends
    pub async fn disconnect_all(&self) -> crate::error::Result<()> {
        self.inner
            .disconnect_all()
            .await
            .map_err(|e| crate::error::HisSrvError::StorageError {
                backend: "all".to_string(),
                message: e.to_string(),
                operation: "disconnect_all".to_string(),
            })
    }

    /// Store data points
    pub async fn store_points(&self, points: &[DataPoint]) -> crate::error::Result<()> {
        self.inner
            .store_points(points)
            .await
            .map_err(|e| crate::error::HisSrvError::WriteError {
                message: e.to_string(),
                points_affected: points.len(),
                partial_success: false,
            })
    }

    /// Query data points
    pub async fn query(&self, filter: &QueryFilter) -> crate::error::Result<QueryResult> {
        self.inner
            .query(filter)
            .await
            .map_err(|e| crate::error::HisSrvError::StorageError {
                backend: "query".to_string(),
                message: e.to_string(),
                operation: "query".to_string(),
            })
    }

    /// Get all statistics
    pub async fn get_all_statistics(&self) -> HashMap<String, StorageStats> {
        self.inner.get_all_stats().await
    }
}

impl Default for StorageManager {
    fn default() -> Self {
        Self::new()
    }
}

// Import for HashMap
use std::collections::HashMap;

// DataPoint, DataValue, QueryFilter, QueryResult are already imported at the top

/// Storage trait wrapper for backward compatibility
#[async_trait]
pub trait Storage: Send + Sync {
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

pub type StorageBackend = Box<dyn Storage + Send + Sync>;
