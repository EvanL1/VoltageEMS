use std::env;

/// Static configuration from environment variables.
/// All runtime settings (MQTT broker, topics, intervals, etc.) are stored
/// in the shared SQLite `netsrv_config` table – see `db_config.rs`.
pub struct EnvConfig {
    pub api_host: String,
    pub api_port: u16,
    pub redis_url: String,
    /// Shared SQLite database path (same as alarmsrv / apigateway).
    pub db_path: String,
    pub log_dir: String,
    /// Directory for TLS certificate files. Fixed at container build time;
    /// set via CERT_DIR env var (default: /app/config/cert).
    /// Mount a host path to this directory in docker-compose.
    pub cert_dir: String,
}

impl Default for EnvConfig {
    fn default() -> Self {
        let redis_host = env::var("REDIS_HOST").unwrap_or_else(|_| "localhost".to_string());
        let redis_port = env::var("REDIS_PORT").unwrap_or_else(|_| "6379".to_string());
        let redis_url = env::var("REDIS_URL")
            .unwrap_or_else(|_| format!("redis://{}:{}", redis_host, redis_port));

        Self {
            api_host: env::var("API_HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            api_port: env::var("API_PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(6006),
            redis_url,
            db_path: env::var("VOLTAGE_DB_PATH")
                .unwrap_or_else(|_| "/app/data/voltage.db".to_string()),
            log_dir: env::var("VOLTAGE_LOG_DIR").unwrap_or_else(|_| "logs".to_string()),
            cert_dir: env::var("CERT_DIR").unwrap_or_else(|_| "/app/config/cert".to_string()),
        }
    }
}
