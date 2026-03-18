//! Service configuration loaded from environment variables

use std::env;

#[derive(Debug, Clone)]
pub struct AlarmConfig {
    pub api_host: String,
    pub api_port: u16,
    pub redis_url: String,
    pub db_path: String,
    /// Monitoring check interval in seconds
    pub data_fetch_interval: u64,
    pub apigateway_url: String,
    pub netsrv_url: String,
    pub log_dir: String,
}

impl Default for AlarmConfig {
    fn default() -> Self {
        Self {
            api_host: "0.0.0.0".to_string(),
            api_port: env::var("SERVICE_PORT")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(6007),
            redis_url: env::var("REDIS_URL")
                .unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string()),
            db_path: env::var("VOLTAGE_DB_PATH")
                .unwrap_or_else(|_| "/app/data/voltage.db".to_string()),
            data_fetch_interval: env::var("DATA_FETCH_INTERVAL")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(5),
            apigateway_url: env::var("APIGATEWAY_URL")
                .unwrap_or_else(|_| "http://localhost:6005".to_string()),
            netsrv_url: env::var("NETSRV_URL")
                .unwrap_or_else(|_| "http://localhost:6006".to_string()),
            log_dir: env::var("VOLTAGE_LOG_DIR").unwrap_or_else(|_| "logs".to_string()),
        }
    }
}
