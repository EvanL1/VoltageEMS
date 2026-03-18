use std::sync::Arc;

use sqlx::SqlitePool;
use tokio::sync::{Mutex, RwLock};
use voltage_rtdb::RedisRtdb;

use crate::config::EnvConfig;
use crate::models::{DataPoint, ServiceConfig, StorageSettings};
use crate::storage::StorageBackend;

/// Shared application state injected into every Axum handler.
pub struct AppState {
    /// Active storage backend, wrapped in RwLock so it can be replaced at
    /// runtime via `PUT /hisApi/storage` without restarting the service.
    /// Starts as `NullBackend` when storage is not yet configured.
    pub storage: Arc<RwLock<Arc<dyn StorageBackend>>>,
    /// Shared SQLite pool – used for the `hissrv_config` table
    /// (same database file as alarmsrv / apigateway).
    pub sqlite: SqlitePool,
    /// Redis connection – used by the collector.
    pub rtdb: Arc<RedisRtdb>,
    /// Static environment config (ports, URLs).
    #[allow(dead_code)]
    pub env: Arc<EnvConfig>,
    /// Operational config (intervals, patterns) – `/hisApi/config`.
    pub config: Arc<RwLock<ServiceConfig>>,
    /// Storage backend connection settings – `/hisApi/storage`.
    pub storage_settings: Arc<RwLock<StorageSettings>>,
    /// In-memory buffer: collector appends here, scheduler drains + writes.
    pub buffer: Arc<Mutex<Vec<DataPoint>>>,
}
