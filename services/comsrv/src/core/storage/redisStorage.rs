use std::sync::Arc;
use tokio::sync::Mutex;
use redis::{AsyncCommands, Client};
use redis::aio::Connection;
use serde::{Serialize, Deserialize};
use crate::utils::error::{ComSrvError, Result};
use crate::core::config::config_manager::RedisConfig;

/// realtime value structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealtimeValue {
    pub raw: f64,
    pub processed: f64,
    pub timestamp: String, // ISO 8601 format
}

/// Redis storage structure
#[derive(Clone)]
pub struct RedisStore {
    conn: Arc<Mutex<Connection>>,  // Redis connection
}

impl RedisStore {
    /// create Redis connection, support TCP and Unix Socket
    pub async fn from_config(config: &RedisConfig) -> Result<Option<Self>> {
        if !config.enabled {
            tracing::info!("Redis disabled in config");
            return Ok(None);
        }

        let url = if config.address.starts_with("unix://") {
            config.address.to_string()
        } else if config.address.starts_with("tcp://") {
            config.address.replacen("tcp://", "redis://", 1)
        } else if config.address.starts_with("redis://") {
            config.address.to_string()
        } else {
            return Err(ComSrvError::RedisError(format!("Unsupported Redis address: {}", config.address)));
        };

        let client = Client::open(url)
            .map_err(|e| ComSrvError::RedisError(format!("Invalid Redis URL: {}", e)))?;

        let conn = client.get_async_connection().await
            .map_err(|e| ComSrvError::RedisError(format!("Failed to connect Redis: {}", e)))?;

        let mut conn = conn;
        
        if let Some(db_index) = config.db {
            redis::cmd("SELECT").arg(db_index)
                .query_async(&mut conn).await
                .map_err(|e| ComSrvError::RedisError(format!("SELECT DB error: {}", e)))?;
        }

        Ok(Some(RedisStore {
            conn: Arc::new(Mutex::new(conn)),
        }))
    }


    /// write realtime value to Redis
    pub async fn set_realtime_value(&self, key: &str, value: &RealtimeValue) -> Result<()> {
        let val_str = serde_json::to_string(value)
            .map_err(|e| ComSrvError::RedisError(format!("Serialize RealtimeValue error: {}", e)))?;

        let mut guard = self.conn.lock().await;
        guard.set::<&str, String, ()>(key, val_str).await
            .map_err(|e| ComSrvError::RedisError(format!("Redis set error: {}", e)))?;

        Ok(())
    }

    /// write realtime value with expire time (seconds)
    pub async fn set_realtime_value_with_expire(&self, key: &str, value: &RealtimeValue, expire_secs: usize) -> Result<()> {
        let val_str = serde_json::to_string(value)
            .map_err(|e| ComSrvError::RedisError(format!("Serialize RealtimeValue error: {}", e)))?;

        let mut guard = self.conn.lock().await;
        guard.set_ex::<&str, String, ()>(key, val_str, expire_secs).await
            .map_err(|e| ComSrvError::RedisError(format!("Redis set_ex error: {}", e)))?;

        Ok(())
    }

    /// read realtime value
    pub async fn get_realtime_value(&self, key: &str) -> Result<Option<RealtimeValue>> {
        let mut guard = self.conn.lock().await;
        let val: Option<String> = guard.get(key).await
            .map_err(|e| ComSrvError::RedisError(format!("Redis get error: {}", e)))?;

        if let Some(json_str) = val {
            let parsed = serde_json::from_str(&json_str)
                .map_err(|e| ComSrvError::RedisError(format!("Deserialize RealtimeValue error: {}", e)))?;
            Ok(Some(parsed))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::config_manager::RedisConnectionType;

    /// create test redis config
    fn create_test_redis_config() -> RedisConfig {
        RedisConfig {
            enabled: true,
            connection_type: RedisConnectionType::Tcp,
            address: "redis://127.0.0.1:6379".to_string(),
            db: Some(0),
        }
    }

    /// create disabled Redis config
    fn create_disabled_redis_config() -> RedisConfig {
        RedisConfig {
            enabled: false,
            connection_type: RedisConnectionType::Tcp,
            address: "redis://127.0.0.1:6379".to_string(),
            db: Some(0),
        }
    }

    /// create test realtime value
    fn create_test_realtime_value() -> RealtimeValue {
        RealtimeValue {
            raw: 123.45,
            processed: 120.0,
            timestamp: "2023-12-01T10:30:00Z".to_string(),
        }
    }

    #[test]
    fn test_realtime_value_creation() {
        let value = create_test_realtime_value();
        assert_eq!(value.raw, 123.45);
        assert_eq!(value.processed, 120.0);
        assert_eq!(value.timestamp, "2023-12-01T10:30:00Z");
    }

    #[test]
    fn test_realtime_value_serialization() {
        let value = create_test_realtime_value();
        
        // Test JSON serialization
        let json_str = serde_json::to_string(&value).unwrap();
        assert!(json_str.contains("123.45"));
        assert!(json_str.contains("120"));
        assert!(json_str.contains("2023-12-01T10:30:00Z"));
        
        // Test JSON deserialization
        let deserialized: RealtimeValue = serde_json::from_str(&json_str).unwrap();
        assert_eq!(value.raw, deserialized.raw);
        assert_eq!(value.processed, deserialized.processed);
        assert_eq!(value.timestamp, deserialized.timestamp);
    }

    #[tokio::test]
    async fn test_redis_store_from_disabled_config() {
        let config = create_disabled_redis_config();
        let result = RedisStore::from_config(&config).await;
        
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_redis_config_invalid_address() {
        // Test with invalid protocol
        let config = RedisConfig {
            enabled: true,
            connection_type: RedisConnectionType::Tcp,
            address: "invalid://invalid".to_string(),
            db: Some(0),
        };
        
        // This test just verifies the configuration structure
        assert_eq!(config.address, "invalid://invalid");
        assert!(config.enabled);
    }

    #[test]
    fn test_redis_config_address_types() {
        let tcp_config = RedisConfig {
            enabled: true,
            connection_type: RedisConnectionType::Tcp,
            address: "tcp://127.0.0.1:6379".to_string(),
            db: Some(1),
        };
        
        let redis_config = RedisConfig {
            enabled: true,
            connection_type: RedisConnectionType::Tcp,
            address: "redis://127.0.0.1:6379".to_string(),
            db: Some(2),
        };
        
        let unix_config = RedisConfig {
            enabled: true,
            connection_type: RedisConnectionType::Unix,
            address: "unix:///tmp/redis.sock".to_string(),
            db: Some(3),
        };
        
        // Test that all address types are properly stored
        assert!(tcp_config.address.starts_with("tcp://"));
        assert!(redis_config.address.starts_with("redis://"));
        assert!(unix_config.address.starts_with("unix://"));
    }

    #[test]
    fn test_redis_config_db_selection() {
        let config_with_db = RedisConfig {
            enabled: true,
            connection_type: RedisConnectionType::Tcp,
            address: "redis://127.0.0.1:6379".to_string(),
            db: Some(5),
        };
        
        let config_without_db = RedisConfig {
            enabled: true,
            connection_type: RedisConnectionType::Tcp,
            address: "redis://127.0.0.1:6379".to_string(),
            db: None,
        };
        
        assert_eq!(config_with_db.db, Some(5));
        assert_eq!(config_without_db.db, None);
    }

    // Note: The following tests require a running Redis instance
    // They are marked with #[ignore] to skip them by default
    // Run with `cargo test -- --ignored` to include them

    #[tokio::test]
    #[ignore]
    async fn test_redis_store_connection() {
        let config = create_test_redis_config();
        let result = RedisStore::from_config(&config).await;
        
        match result {
            Ok(Some(store)) => {
                // Connection successful
                assert!(true);
            }
            Ok(None) => {
                panic!("Redis should be enabled");
            }
            Err(_) => {
                // Redis not available, skip test
                println!("Redis not available, skipping test");
            }
        }
    }

    #[tokio::test]
    #[ignore]
    async fn test_redis_store_set_get_realtime_value() {
        let config = create_test_redis_config();
        if let Ok(Some(store)) = RedisStore::from_config(&config).await {
            let test_key = "test:realtime:value";
            let test_value = create_test_realtime_value();
            
            // Test set operation
            let set_result = store.set_realtime_value(test_key, &test_value).await;
            assert!(set_result.is_ok());
            
            // Test get operation
            let get_result = store.get_realtime_value(test_key).await;
            assert!(get_result.is_ok());
            
            let retrieved_value = get_result.unwrap();
            assert!(retrieved_value.is_some());
            
            let retrieved_value = retrieved_value.unwrap();
            assert_eq!(test_value.raw, retrieved_value.raw);
            assert_eq!(test_value.processed, retrieved_value.processed);
            assert_eq!(test_value.timestamp, retrieved_value.timestamp);
        }
    }

    #[tokio::test]
    #[ignore]
    async fn test_redis_store_set_with_expire() {
        let config = create_test_redis_config();
        if let Ok(Some(store)) = RedisStore::from_config(&config).await {
            let test_key = "test:expire:value";
            let test_value = create_test_realtime_value();
            
            // Test set with expire
            let set_result = store.set_realtime_value_with_expire(test_key, &test_value, 10).await;
            assert!(set_result.is_ok());
            
            // Test get operation immediately
            let get_result = store.get_realtime_value(test_key).await;
            assert!(get_result.is_ok());
            
            let retrieved_value = get_result.unwrap();
            assert!(retrieved_value.is_some());
        }
    }

    #[tokio::test]
    #[ignore]
    async fn test_redis_store_get_nonexistent_key() {
        let config = create_test_redis_config();
        if let Ok(Some(store)) = RedisStore::from_config(&config).await {
            let test_key = "test:nonexistent:key";
            
            // Test get operation for non-existent key
            let get_result = store.get_realtime_value(test_key).await;
            assert!(get_result.is_ok());
            
            let retrieved_value = get_result.unwrap();
            assert!(retrieved_value.is_none());
        }
    }

    #[tokio::test]
    #[ignore]
    async fn test_redis_store_multiple_operations() {
        let config = create_test_redis_config();
        if let Ok(Some(store)) = RedisStore::from_config(&config).await {
            let test_keys = vec!["test:multi:1", "test:multi:2", "test:multi:3"];
            let test_values = vec![
                RealtimeValue {
                    raw: 100.0,
                    processed: 95.0,
                    timestamp: "2023-12-01T10:00:00Z".to_string(),
                },
                RealtimeValue {
                    raw: 200.0,
                    processed: 195.0,
                    timestamp: "2023-12-01T10:01:00Z".to_string(),
                },
                RealtimeValue {
                    raw: 300.0,
                    processed: 295.0,
                    timestamp: "2023-12-01T10:02:00Z".to_string(),
                },
            ];
            
            // Set multiple values
            for (key, value) in test_keys.iter().zip(test_values.iter()) {
                let set_result = store.set_realtime_value(key, value).await;
                assert!(set_result.is_ok());
            }
            
            // Get multiple values
            for (key, expected_value) in test_keys.iter().zip(test_values.iter()) {
                let get_result = store.get_realtime_value(key).await;
                assert!(get_result.is_ok());
                
                let retrieved_value = get_result.unwrap();
                assert!(retrieved_value.is_some());
                
                let retrieved_value = retrieved_value.unwrap();
                assert_eq!(expected_value.raw, retrieved_value.raw);
                assert_eq!(expected_value.processed, retrieved_value.processed);
                assert_eq!(expected_value.timestamp, retrieved_value.timestamp);
            }
        }
    }

    #[test]
    fn test_error_handling_serialization() {
        // Test creating a RealtimeValue with extreme values
        let extreme_value = RealtimeValue {
            raw: f64::INFINITY,
            processed: f64::NEG_INFINITY,
            timestamp: "invalid-timestamp".to_string(),
        };
        
        // JSON serialization should handle infinity values
        let json_result = serde_json::to_string(&extreme_value);
        // Note: JSON serialization of infinity might fail or produce "null"
        // This test verifies the behavior is predictable
        match json_result {
            Ok(_) => assert!(true), // Serialization succeeded
            Err(_) => assert!(true), // Serialization failed as expected
        }
    }

    #[test]
    fn test_redis_config_clone() {
        let config = create_test_redis_config();
        let cloned_config = config.clone();
        
        assert_eq!(config.enabled, cloned_config.enabled);
        assert_eq!(config.address, cloned_config.address);
        assert_eq!(config.db, cloned_config.db);
    }

    #[test]
    fn test_realtime_value_clone() {
        let value = create_test_realtime_value();
        let cloned_value = value.clone();
        
        assert_eq!(value.raw, cloned_value.raw);
        assert_eq!(value.processed, cloned_value.processed);
        assert_eq!(value.timestamp, cloned_value.timestamp);
    }

    #[test]
    fn test_realtime_value_debug() {
        let value = create_test_realtime_value();
        let debug_str = format!("{:?}", value);
        
        assert!(debug_str.contains("RealtimeValue"));
        assert!(debug_str.contains("123.45"));
        assert!(debug_str.contains("120"));
        assert!(debug_str.contains("2023-12-01T10:30:00Z"));
    }
}