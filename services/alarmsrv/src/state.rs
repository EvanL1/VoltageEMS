//! Shared application state

use std::sync::Arc;

use sqlx::SqlitePool;
use tokio::sync::RwLock;
use voltage_rtdb::RedisRtdb;

use crate::broadcast::Broadcaster;
use crate::config::AlarmConfig;
use crate::models::MonitorStatus;

pub struct AppState {
    pub db: SqlitePool,
    pub rtdb: Arc<RedisRtdb>,
    pub config: Arc<AlarmConfig>,
    pub broadcaster: Broadcaster,
    pub monitor_status: Arc<RwLock<MonitorStatus>>,
}
