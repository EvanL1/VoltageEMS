use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RedisConfig {
    pub host: String,
    pub port: u16,
    pub password: String,
    pub socket: String,
    pub prefix: String,
    pub data_keys: Vec<String>,
    pub poll_interval_ms: u64,
}

impl Default for RedisConfig {
    fn default() -> Self {
        RedisConfig {
            host: "localhost".to_string(),
            port: 6379,
            password: "".to_string(),
            socket: "".to_string(),
            prefix: "ems:".to_string(),
            data_keys: vec![
                "ems:model:output:*".to_string(),
                "ems:data:*".to_string(),
            ],
            poll_interval_ms: 1000,
        }
    }
} 