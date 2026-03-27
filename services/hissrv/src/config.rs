use std::env;

/// Static configuration loaded from environment variables at startup.
/// Dynamic per-service settings (intervals, patterns, storage backend, etc.)
/// live in the `hissrv_config` table in the shared SQLite database – see
/// `db_config.rs`.  Storage backend can be configured and toggled at runtime
/// via the `PUT /hisApi/storage` API endpoint.
pub struct EnvConfig {
    pub api_host: String,
    pub api_port: u16,
    pub redis_url: String,
    /// Shared SQLite database path (same as alarmsrv / apigateway).
    pub db_path: String,
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
                .unwrap_or(6004),
            redis_url,
            db_path: env::var("VOLTAGE_DB_PATH")
                .unwrap_or_else(|_| "/app/data/voltage.db".to_string()),
        }
    }
}
