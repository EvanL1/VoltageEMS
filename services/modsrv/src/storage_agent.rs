use crate::config::Config;
use crate::error::Result;
use crate::redis_handler::RedisConnection;
use crate::storage::{redis_store::RedisStore, DataStore};
use std::sync::Arc;
use tracing::{error, info};

/// Storage agent, responsible for managing Redis storage operations
pub struct StorageAgent {
    store: Arc<RedisStore>,
    config: Config,
}

impl StorageAgent {
    /// Create a new storage agent with Redis storage
    pub fn new(config: Config) -> Result<Self> {
        // Create Redis connection
        let redis_config = serde_json::json!({
            "host": config.redis.host,
            "port": config.redis.port
        });

        let redis_conn = RedisConnection::from_config(&redis_config)?;
        let store = Arc::new(RedisStore::new(redis_conn));

        info!(
            "Storage agent initialized with Redis backend at {}:{}",
            config.redis.host, config.redis.port
        );

        Ok(Self { store, config })
    }

    /// Get Redis store instance that implements DataStore trait
    pub fn store(&self) -> Arc<RedisStore> {
        self.store.clone()
    }

    /// Test Redis connection
    pub fn test_connection(&self) -> Result<()> {
        // Try a simple operation to test connectivity
        self.store.set_string("test:connection", "ok")?;
        let result = self.store.get_string("test:connection")?;
        if result == "ok" {
            self.store.delete("test:connection")?;
            info!("Redis connection test successful");
            Ok(())
        } else {
            Err(crate::error::ModelSrvError::RedisError(
                "Connection test failed".to_string(),
            ))
        }
    }

    /// Get configuration
    pub fn config(&self) -> &Config {
        &self.config
    }
}
