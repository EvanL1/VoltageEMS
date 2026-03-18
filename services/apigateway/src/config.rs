use std::env;

pub struct GatewayConfig {
    pub api_host: String,
    pub api_port: u16,
    pub redis_url: String,
    pub db_path: String,
    pub jwt_secret: String,
    pub access_token_expire_minutes: i64,
    pub refresh_token_expire_days: i64,
    pub log_dir: String,
    pub network_config_dir: String,
    pub data_fetch_interval_secs: u64,
}

impl Default for GatewayConfig {
    fn default() -> Self {
        let redis_host = env::var("REDIS_HOST").unwrap_or_else(|_| "localhost".to_string());
        let redis_port = env::var("REDIS_PORT").unwrap_or_else(|_| "6379".to_string());
        let redis_url = env::var("REDIS_URL")
            .unwrap_or_else(|_| format!("redis://{}:{}", redis_host, redis_port));

        let db_path =
            env::var("VOLTAGE_DB_PATH").unwrap_or_else(|_| "/app/data/voltage.db".to_string());

        Self {
            api_host: env::var("API_HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),
            api_port: env::var("API_PORT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(6005),
            redis_url,
            db_path,
            jwt_secret: env::var("JWT_SECRET_KEY")
                .unwrap_or_else(|_| "your-secret-key-here-change-in-production".to_string()),
            access_token_expire_minutes: env::var("ACCESS_TOKEN_EXPIRE_MINUTES")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(30),
            refresh_token_expire_days: env::var("REFRESH_TOKEN_EXPIRE_DAYS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(7),
            log_dir: env::var("LOG_DIR").unwrap_or_else(|_| "logs".to_string()),
            network_config_dir: env::var("NETWORK_CONFIG_DIR")
                .unwrap_or_else(|_| "/etc/systemd/network".to_string()),
            data_fetch_interval_secs: env::var("DATA_FETCH_INTERVAL")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1),
        }
    }
}
