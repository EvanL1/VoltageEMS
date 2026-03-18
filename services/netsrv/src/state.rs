use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use sqlx::SqlitePool;
use tokio::sync::{Mutex, Notify, RwLock};
use voltage_rtdb::RedisRtdb;

use crate::config::EnvConfig;
use crate::device::{DeviceIdentity, Topics};
use crate::models::NetConfig;

/// Shared application state.
pub struct AppState {
    /// Shared SQLite pool – netsrv_config table.
    pub sqlite: SqlitePool,
    /// Redis connection for data collection and point read/write.
    pub rtdb: Arc<RedisRtdb>,
    /// Static env config.
    #[allow(dead_code)]
    pub env: Arc<EnvConfig>,
    /// Dynamic config reloaded from `netsrv_config`.
    pub config: Arc<RwLock<NetConfig>>,
    /// Resolved device identity (product SN + device SN).
    pub device: Arc<DeviceIdentity>,
    /// MQTT topics derived from device identity.
    pub topics: Arc<Topics>,
    /// Current MQTT publish client – None while disconnected.
    pub mqtt_client: Arc<Mutex<Option<rumqttc::AsyncClient>>>,
    /// True when MQTT is connected and ready.
    pub mqtt_connected: Arc<AtomicBool>,
    /// Signal for the MQTT task to reconnect (config changed or explicit API call).
    pub reconnect_signal: Arc<Notify>,
    /// HTTP client for outbound calls (call-alarm → alarmsrv).
    pub http_client: reqwest::Client,
}
