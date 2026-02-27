//! Modsrv Redis Cleanup Provider Implementation

use std::collections::HashSet;

use anyhow::Result;
use sqlx::SqlitePool;
use voltage_rtdb::cleanup::CleanupProvider;

/// Cleanup provider for modsrv service
///
/// Manages cleanup of invalid instance-related Redis keys based on
/// the current SQLite configuration.
pub struct ModsrvCleanupProvider {
    db: SqlitePool,
}

impl ModsrvCleanupProvider {
    /// Create a new cleanup provider
    pub fn new(db: SqlitePool) -> Self {
        Self { db }
    }
}

impl CleanupProvider for ModsrvCleanupProvider {
    async fn get_valid_ids(&self) -> Result<HashSet<String>> {
        let instances = sqlx::query_as::<_, (u32,)>("SELECT instance_id FROM instances")
            .fetch_all(&self.db)
            .await?;

        Ok(instances.into_iter().map(|(id,)| id.to_string()).collect())
    }

    fn key_pattern(&self) -> &str {
        "inst:*"
    }

    fn extract_id(&self, key: &str) -> Option<String> {
        let parts: Vec<&str> = key.split(':').collect();

        if parts.len() < 2 || parts[0] != "inst" {
            return None;
        }

        // For instance keys like "inst:1:M", "inst:1:config", "inst:1:status"
        // Extract the instance ID (second part)
        parts[1].parse::<u32>().ok().map(|id| id.to_string())
    }

    fn is_system_key(&self, _key: &str) -> bool {
        // No system keys for inst:* pattern
        false
    }

    fn service_name(&self) -> &str {
        "modsrv"
    }

    async fn get_valid_point_ids_for_entity(
        &self,
        entity_id: &str,
        key: &str,
    ) -> Result<Option<HashSet<String>>> {
        // Determine point role from key suffix
        let is_measurement = key.ends_with(":M");
        let is_action = key.ends_with(":A");

        if !is_measurement && !is_action {
            // Not a point data key (e.g., :config, :status, :measurement_points metadata)
            return Ok(None);
        }

        // Get product_name for this instance
        let instance_id: u32 = entity_id
            .parse()
            .map_err(|e| anyhow::anyhow!("Invalid instance_id: {}", e))?;

        let product_name = sqlx::query_as::<_, (String,)>(
            "SELECT product_name FROM instances WHERE instance_id = ?",
        )
        .bind(instance_id)
        .fetch_optional(&self.db)
        .await?;

        let product_name = match product_name {
            Some((name,)) => name,
            None => {
                // Instance not found in database, no valid points
                return Ok(Some(HashSet::new()));
            },
        };

        // Get valid point IDs from built-in product definitions (compile-time constants)
        let product = match voltage_model::product_lib::get_builtin_product(&product_name) {
            Some(p) => p,
            None => return Ok(Some(HashSet::new())),
        };

        let point_ids: HashSet<String> = if is_measurement {
            product
                .measurements
                .iter()
                .map(|m| m.id.to_string())
                .collect()
        } else {
            product.actions.iter().map(|a| a.id.to_string()).collect()
        };

        Ok(Some(point_ids))
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_extract_id() {
        let provider = ModsrvCleanupProvider {
            db: sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap(),
        };

        // Valid instance keys
        assert_eq!(provider.extract_id("inst:1:M"), Some("1".to_string()));
        assert_eq!(provider.extract_id("inst:5:A"), Some("5".to_string()));
        assert_eq!(
            provider.extract_id("inst:100:config"),
            Some("100".to_string())
        );
        assert_eq!(
            provider.extract_id("inst:1:measurement_points"),
            Some("1".to_string())
        );

        // Invalid keys
        assert_eq!(provider.extract_id("invalid:1:M"), None);
        assert_eq!(provider.extract_id("inst"), None);
        assert_eq!(provider.extract_id("inst:abc:M"), None); // Non-numeric ID
        assert_eq!(provider.extract_id("modsrv:stats:count"), None); // Old format
    }

    #[tokio::test]
    async fn test_is_system_key() {
        let provider = ModsrvCleanupProvider {
            db: sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap(),
        };

        // inst:* pattern has no system keys
        assert!(!provider.is_system_key("inst:1:M"));
        assert!(!provider.is_system_key("inst:1:A"));
        assert!(!provider.is_system_key("inst:1:config"));
        assert!(!provider.is_system_key("modsrv:stats:count")); // Old modsrv:* keys are not matched
    }
}
