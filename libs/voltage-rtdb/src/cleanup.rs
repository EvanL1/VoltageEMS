//! Redis Data Cleanup Trait and Implementation
//!
//! Provides a generic framework for cleaning up invalid Redis keys based on
//! database configuration. Services implement the `CleanupProvider` trait
//! to define their specific cleanup logic.

use std::collections::HashSet;

use anyhow::Result;
use async_trait::async_trait;
use tracing::{debug, info, warn};

use crate::traits::Rtdb;

/// Provider trait for Redis cleanup operations
///
/// Services implement this trait to define how to:
/// - Get valid entity IDs from their database
/// - Extract entity IDs from Redis keys
/// - Identify system keys that should be preserved
#[async_trait]
pub trait CleanupProvider: Send + Sync {
    /// Get the set of valid entity IDs from the database
    ///
    /// # Returns
    /// A HashSet of valid ID strings (e.g., channel IDs, instance names)
    ///
    /// # Example
    /// ```ignore
    /// async fn get_valid_ids(&self) -> Result<HashSet<String>> {
    ///     let ids = sqlx::query_as::<_, (u16,)>("SELECT id FROM channels")
    ///         .fetch_all(&self.db)
    ///         .await?
    ///         .into_iter()
    ///         .map(|(id,)| id.to_string())
    ///         .collect();
    ///     Ok(ids)
    /// }
    /// ```
    async fn get_valid_ids(&self) -> Result<HashSet<String>>;

    /// Get the Redis key pattern to scan
    ///
    /// # Returns
    /// A pattern string for Redis KEYS command (e.g., "comsrv:*", "modsrv:*")
    fn key_pattern(&self) -> &str;

    /// Extract entity ID from a Redis key
    ///
    /// # Arguments
    /// * `key` - Redis key string
    ///
    /// # Returns
    /// Some(id) if the key represents an entity, None for system keys
    ///
    /// # Example
    /// ```ignore
    /// fn extract_id(&self, key: &str) -> Option<String> {
    ///     // comsrv:1:T -> Some("1")
    ///     // comsrv:stats:* -> None
    ///     let parts: Vec<&str> = key.split(':').collect();
    ///     if parts.len() >= 2 && parts[0] == "comsrv" {
    ///         parts[1].parse::<u16>().ok().map(|id| id.to_string())
    ///     } else {
    ///         None
    ///     }
    /// }
    /// ```
    fn extract_id(&self, key: &str) -> Option<String>;

    /// Check if a key is a system key that should be preserved
    ///
    /// # Arguments
    /// * `key` - Redis key string
    ///
    /// # Returns
    /// true if the key should be preserved, false otherwise
    ///
    /// # Default Implementation
    /// Returns false (no system keys). Override this method if your
    /// service has system-level keys that should not be deleted.
    fn is_system_key(&self, _key: &str) -> bool {
        false
    }

    /// Get the service name for logging
    fn service_name(&self) -> &str;

    /// Get valid point IDs for a specific entity (optional, for point-level cleanup)
    ///
    /// # Arguments
    /// * `entity_id` - Entity ID (channel_id or instance_id)
    /// * `key` - Full Redis key to determine point type (e.g., "comsrv:1:T", "inst:1:M")
    ///
    /// # Returns
    /// * `None` - Point-level cleanup not supported (default)
    /// * `Some(set)` - Set of valid point IDs for this entity and key
    ///
    /// # Example
    /// ```ignore
    /// async fn get_valid_point_ids_for_entity(&self, entity_id: &str, key: &str) -> Result<Option<HashSet<String>>> {
    ///     // For comsrv:1:T, query telemetry_points WHERE channel_id = 1
    ///     // Return Some(set) with valid point_ids
    /// }
    /// ```
    async fn get_valid_point_ids_for_entity(
        &self,
        _entity_id: &str,
        _key: &str,
    ) -> Result<Option<HashSet<String>>> {
        Ok(None) // Default: no point-level cleanup
    }
}

/// Clean up invalid Redis keys based on database configuration
///
/// This function performs the following steps:
/// 1. Get valid entity IDs from database via provider
/// 2. Scan Redis keys matching the pattern
/// 3. Delete keys for entities not in the valid set
/// 4. Preserve system keys
///
/// # Arguments
/// * `provider` - Cleanup provider implementation
/// * `redis` - Redis client with KeyValueStore trait
///
/// # Returns
/// Number of keys deleted, or error if cleanup fails
///
/// # Example
/// ```ignore
/// let provider = ComsrvCleanupProvider { db: pool };
/// let deleted = cleanup_invalid_keys(&provider, &redis).await?;
/// info!("Cleaned up {} invalid keys", deleted);
/// ```
pub async fn cleanup_invalid_keys<P, R>(provider: &P, redis: &R) -> Result<usize>
where
    P: CleanupProvider,
    R: Rtdb,
{
    // Check if cleanup is disabled via environment variable
    if std::env::var("SKIP_REDIS_CLEANUP")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false)
    {
        info!("{}: Cleanup skip (env)", provider.service_name());
        return Ok(0);
    }

    info!("{}: Cleanup starting", provider.service_name());

    // 1. Get valid entity IDs from database
    let valid_ids = provider.get_valid_ids().await?;
    info!("{}: {} valid IDs", provider.service_name(), valid_ids.len());
    debug!("{}: Valid IDs: {:?}", provider.service_name(), valid_ids);

    // 2. Scan Redis keys matching pattern
    let pattern = provider.key_pattern();
    let keys: Vec<String> = redis.scan_match(pattern).await?;
    info!(
        "{}: {} keys ({})",
        provider.service_name(),
        keys.len(),
        pattern
    );

    // 3. Analyze and delete invalid keys
    let mut deleted_count = 0;
    let mut preserved_system_keys = 0;
    let mut preserved_valid_keys = 0;
    let mut deleted_point_count = 0;

    for key in keys {
        // Check if it's a system key
        if provider.is_system_key(&key) {
            preserved_system_keys += 1;
            debug!("{}: Preserved system key: {}", provider.service_name(), key);
            continue;
        }

        // Extract entity ID and check validity
        match provider.extract_id(&key) {
            Some(id) => {
                if valid_ids.contains(&id) {
                    // Valid entity, keep the key
                    preserved_valid_keys += 1;

                    // Check for point-level cleanup
                    if let Some(valid_point_ids) =
                        provider.get_valid_point_ids_for_entity(&id, &key).await?
                    {
                        // Get all fields in the hash
                        let all_fields = redis.hash_get_all(&key).await?;

                        // Delete invalid point IDs
                        for field in all_fields.keys() {
                            if !valid_point_ids.contains(field) {
                                redis.hash_del(&key, field).await?;
                                deleted_point_count += 1;
                                debug!(
                                    "{}: Deleted invalid point field: {} in key {} (point '{}' not in config)",
                                    provider.service_name(),
                                    field,
                                    key,
                                    field
                                );
                            }
                        }
                    }
                } else {
                    // Invalid entity, delete the key
                    redis.del(&key).await?;
                    deleted_count += 1;
                    debug!(
                        "{}: Deleted invalid key: {} (entity '{}' not in config)",
                        provider.service_name(),
                        key,
                        id
                    );
                }
            },
            None => {
                // Could not extract ID, treat as system key
                preserved_system_keys += 1;
                debug!(
                    "{}: Preserved non-entity key: {}",
                    provider.service_name(),
                    key
                );
            },
        }
    }

    info!(
        "{}: Cleanup done (del:{}/{} keep:{}/{})",
        provider.service_name(),
        deleted_count,
        deleted_point_count,
        preserved_valid_keys,
        preserved_system_keys
    );

    if deleted_count > 0 || deleted_point_count > 0 {
        let mut messages = Vec::new();
        if deleted_count > 0 {
            messages.push(format!("{} keys", deleted_count));
        }
        if deleted_point_count > 0 {
            messages.push(format!("{} point fields", deleted_point_count));
        }
        warn!(
            "{}: Removed {}",
            provider.service_name(),
            messages.join(", ")
        );
    }

    Ok(deleted_count)
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;
    use crate::traits::Rtdb;
    use serial_test::serial;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    // Mock provider for testing
    struct MockProvider {
        valid_ids: HashSet<String>,
        system_keys: Vec<String>,
    }

    #[async_trait]
    impl CleanupProvider for MockProvider {
        async fn get_valid_ids(&self) -> Result<HashSet<String>> {
            Ok(self.valid_ids.clone())
        }

        fn key_pattern(&self) -> &str {
            "test:*"
        }

        fn extract_id(&self, key: &str) -> Option<String> {
            let parts: Vec<&str> = key.split(':').collect();
            if parts.len() >= 2 && parts[0] == "test" {
                Some(parts[1].to_string())
            } else {
                None
            }
        }

        fn is_system_key(&self, key: &str) -> bool {
            self.system_keys.contains(&key.to_string())
        }

        fn service_name(&self) -> &str {
            "test"
        }
    }

    // Mock Redis for testing
    struct MockRedis {
        keys: Arc<Mutex<HashSet<String>>>,
    }

    #[async_trait]
    impl Rtdb for MockRedis {
        // ========== Introspection ==========

        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        // ========== Basic Key-Value Operations ==========

        async fn get(&self, _key: &str) -> Result<Option<bytes::Bytes>> {
            unimplemented!()
        }

        async fn set(&self, _key: &str, _value: bytes::Bytes) -> Result<()> {
            unimplemented!()
        }

        async fn del(&self, key: &str) -> Result<bool> {
            Ok(self.keys.lock().await.remove(key))
        }

        async fn exists(&self, key: &str) -> Result<bool> {
            Ok(self.keys.lock().await.contains(key))
        }

        // ========== Structured Data Operations ==========

        async fn hash_set(&self, _key: &str, _field: &str, _value: bytes::Bytes) -> Result<()> {
            unimplemented!()
        }

        async fn hash_get(&self, _key: &str, _field: &str) -> Result<Option<bytes::Bytes>> {
            unimplemented!()
        }

        async fn hash_mset(&self, _key: &str, _fields: Vec<(String, bytes::Bytes)>) -> Result<()> {
            unimplemented!()
        }

        async fn hash_get_all(
            &self,
            _key: &str,
        ) -> Result<std::collections::HashMap<String, bytes::Bytes>> {
            unimplemented!()
        }

        async fn hash_del(&self, _key: &str, _field: &str) -> Result<bool> {
            unimplemented!()
        }

        async fn list_lpush(&self, _key: &str, _value: bytes::Bytes) -> Result<()> {
            unimplemented!()
        }

        async fn list_rpush(&self, _key: &str, _value: bytes::Bytes) -> Result<()> {
            unimplemented!()
        }

        async fn list_lpop(&self, _key: &str) -> Result<Option<bytes::Bytes>> {
            unimplemented!()
        }

        async fn list_range(
            &self,
            _key: &str,
            _start: isize,
            _stop: isize,
        ) -> Result<Vec<bytes::Bytes>> {
            unimplemented!()
        }

        async fn list_trim(&self, _key: &str, _start: isize, _stop: isize) -> Result<()> {
            unimplemented!()
        }

        async fn publish(&self, _channel: &str, _message: &str) -> Result<u32> {
            Ok(0)
        }

        async fn fcall(&self, _function: &str, _keys: &[&str], _args: &[&str]) -> Result<String> {
            Ok(String::new())
        }

        async fn scan_match(&self, _pattern: &str) -> Result<Vec<String>> {
            Ok(self.keys.lock().await.iter().cloned().collect())
        }

        async fn sadd(&self, _key: &str, _member: &str) -> Result<bool> {
            unimplemented!()
        }

        async fn srem(&self, _key: &str, _member: &str) -> Result<bool> {
            unimplemented!()
        }

        async fn smembers(&self, _key: &str) -> Result<Vec<String>> {
            unimplemented!()
        }

        async fn hincrby(&self, _key: &str, _field: &str, _increment: i64) -> Result<i64> {
            unimplemented!()
        }

        async fn time_millis(&self) -> Result<i64> {
            use std::time::{SystemTime, UNIX_EPOCH};
            Ok(SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as i64)
        }

        // ========== Missing Methods (Phase 1-4 extensions) ==========

        async fn incrbyfloat(&self, _key: &str, _increment: f64) -> Result<f64> {
            unimplemented!()
        }

        async fn hash_mget(
            &self,
            _key: &str,
            _fields: &[&str],
        ) -> Result<Vec<Option<bytes::Bytes>>> {
            unimplemented!()
        }

        async fn list_rpop(&self, _key: &str) -> Result<Option<bytes::Bytes>> {
            unimplemented!()
        }

        async fn list_blpop(
            &self,
            _keys: &[&str],
            _timeout_seconds: u64,
        ) -> Result<Option<(String, bytes::Bytes)>> {
            unimplemented!()
        }

        async fn hash_del_many(&self, _key: &str, _fields: &[String]) -> Result<usize> {
            unimplemented!()
        }

        async fn pipeline_hash_mset(
            &self,
            _operations: Vec<(String, Vec<(String, bytes::Bytes)>)>,
        ) -> Result<()> {
            unimplemented!()
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_cleanup_removes_invalid_keys() {
        // Ensure clean environment (prevent pollution from parallel tests)
        std::env::remove_var("SKIP_REDIS_CLEANUP");

        let mut valid_ids = HashSet::new();
        valid_ids.insert("1".to_string());
        valid_ids.insert("2".to_string());

        let provider = MockProvider {
            valid_ids,
            system_keys: vec!["test:stats:count".to_string()],
        };

        let mut initial_keys = HashSet::new();
        initial_keys.insert("test:1:data".to_string());
        initial_keys.insert("test:2:data".to_string());
        initial_keys.insert("test:999:data".to_string()); // Invalid
        initial_keys.insert("test:stats:count".to_string()); // System key

        let redis = MockRedis {
            keys: Arc::new(Mutex::new(initial_keys)),
        };

        let deleted = cleanup_invalid_keys(&provider, &redis).await.unwrap();

        assert_eq!(deleted, 1); // Only test:999:data should be deleted
        assert!(redis.exists("test:1:data").await.unwrap());
        assert!(redis.exists("test:2:data").await.unwrap());
        assert!(!redis.exists("test:999:data").await.unwrap());
        assert!(redis.exists("test:stats:count").await.unwrap());
    }

    #[tokio::test]
    #[serial]
    async fn test_cleanup_with_environment_variable() {
        // Ensure clean environment before setting test variable
        std::env::remove_var("SKIP_REDIS_CLEANUP");
        std::env::set_var("SKIP_REDIS_CLEANUP", "true");

        let provider = MockProvider {
            valid_ids: HashSet::new(),
            system_keys: vec![],
        };

        let redis = MockRedis {
            keys: Arc::new(Mutex::new(HashSet::new())),
        };

        let deleted = cleanup_invalid_keys(&provider, &redis).await.unwrap();
        assert_eq!(deleted, 0);

        // Clean up after test
        std::env::remove_var("SKIP_REDIS_CLEANUP");
    }
}
