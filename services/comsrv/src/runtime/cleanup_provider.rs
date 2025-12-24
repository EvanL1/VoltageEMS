//! Comsrv Redis Cleanup Provider Implementation

use std::collections::HashSet;

use anyhow::Result;
use sqlx::SqlitePool;
use voltage_rtdb::cleanup::CleanupProvider;

/// Cleanup provider for comsrv service
///
/// Manages cleanup of invalid channel-related Redis keys based on
/// the current SQLite configuration.
pub struct ComsrvCleanupProvider {
    db: SqlitePool,
}

impl ComsrvCleanupProvider {
    /// Create a new cleanup provider
    pub fn new(db: SqlitePool) -> Self {
        Self { db }
    }
}

impl CleanupProvider for ComsrvCleanupProvider {
    async fn get_valid_ids(&self) -> Result<HashSet<String>> {
        let channels =
            sqlx::query_as::<_, (u32,)>("SELECT channel_id FROM channels WHERE enabled = 1")
                .fetch_all(&self.db)
                .await?;

        Ok(channels.into_iter().map(|(id,)| id.to_string()).collect())
    }

    fn key_pattern(&self) -> &str {
        "comsrv:*"
    }

    fn extract_id(&self, key: &str) -> Option<String> {
        let parts: Vec<&str> = key.split(':').collect();

        if parts.len() < 2 || parts[0] != "comsrv" {
            return None;
        }

        // Skip system-level keys (stats, config, metadata, etc.)
        let reserved_prefixes = ["stats", "config", "meta", "system"];
        if reserved_prefixes.contains(&parts[1]) {
            return None;
        }

        // Try to parse second part as channel ID
        parts[1].parse::<u16>().ok().map(|id| id.to_string())
    }

    fn is_system_key(&self, key: &str) -> bool {
        // Preserve system-level keys
        key.starts_with("comsrv:stats:")
            || key.starts_with("comsrv:config:")
            || key.starts_with("comsrv:meta:")
            || key.starts_with("comsrv:system:")
    }

    fn service_name(&self) -> &str {
        "comsrv"
    }

    async fn get_valid_point_ids_for_entity(
        &self,
        entity_id: &str,
        key: &str,
    ) -> Result<Option<HashSet<String>>> {
        // Determine point type from key suffix
        let table = if key.contains(":T:") || key.ends_with(":T") {
            "telemetry_points"
        } else if key.contains(":S:") || key.ends_with(":S") {
            "signal_points"
        } else if key.contains(":C:") || key.ends_with(":C") {
            "control_points"
        } else if key.contains(":A:") || key.ends_with(":A") {
            "adjustment_points"
        } else {
            // Not a point data key (e.g., :TODO keys, metadata keys)
            return Ok(None);
        };

        // Query database for valid point IDs for this channel
        let channel_id: u32 = entity_id
            .parse()
            .map_err(|e| anyhow::anyhow!("Invalid channel_id: {}", e))?;

        let query = format!("SELECT point_id FROM {} WHERE channel_id = ?", table);

        let points = sqlx::query_as::<_, (u32,)>(&query)
            .bind(channel_id)
            .fetch_all(&self.db)
            .await?;

        Ok(Some(
            points.into_iter().map(|(id,)| id.to_string()).collect(),
        ))
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_extract_id() {
        let provider = ComsrvCleanupProvider {
            db: sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap(),
        };

        // Valid channel keys
        assert_eq!(provider.extract_id("comsrv:1:T"), Some("1".to_string()));
        assert_eq!(
            provider.extract_id("comsrv:1001:T"),
            Some("1001".to_string())
        );
        assert_eq!(provider.extract_id("comsrv:2:S:ts"), Some("2".to_string()));

        // System keys (should return None)
        assert_eq!(provider.extract_id("comsrv:stats:unmapped"), None);
        assert_eq!(provider.extract_id("comsrv:config:version"), None);

        // Invalid keys
        assert_eq!(provider.extract_id("invalid:1:T"), None);
        assert_eq!(provider.extract_id("comsrv"), None);
    }

    #[tokio::test]
    async fn test_is_system_key() {
        let provider = ComsrvCleanupProvider {
            db: sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap(),
        };

        assert!(provider.is_system_key("comsrv:stats:unmapped"));
        assert!(provider.is_system_key("comsrv:config:version"));
        assert!(provider.is_system_key("comsrv:meta:info"));
        assert!(!provider.is_system_key("comsrv:1:T"));
        assert!(!provider.is_system_key("comsrv:1001:S:ts"));
    }
}
