use anyhow::Result;
use redis::AsyncCommands;
use std::collections::HashMap;
use std::sync::Arc;
use voltage_libs::redis::RedisClient;

use crate::domain::{Alarm, AlarmLevel, AlarmStatus};
use crate::redis::AlarmRedisClient;

/// Manages Redis indexes for alarms
pub struct AlarmIndexManager {
    #[allow(dead_code)]
    client: Arc<AlarmRedisClient>,
}

impl AlarmIndexManager {
    /// Create new index manager
    pub fn new(client: Arc<AlarmRedisClient>) -> Self {
        Self { client }
    }

    /// Add alarm to all relevant indexes
    pub async fn add_to_indexes(&self, conn: &mut RedisClient, alarm: &Alarm) -> Result<()> {
        // Add to category index
        // For now, just store all alarms in a general category
        let category_key = "alarmsrv:category:general".to_string();
        conn.get_connection_mut()
            .sadd(&category_key, &alarm.id.to_string())
            .await?;

        // Add to level index
        let level_key = format!("alarmsrv:level:{:?}", alarm.level);
        conn.get_connection_mut()
            .sadd(&level_key, &alarm.id.to_string())
            .await?;

        // Add to status index
        let status_key = format!("alarmsrv:status:{:?}", alarm.status);
        conn.get_connection_mut()
            .sadd(&status_key, &alarm.id.to_string())
            .await?;

        // Add to time-based index
        let date_key = format!("alarmsrv:date:{}", alarm.created_at.format("%Y-%m-%d"));
        conn.get_connection_mut()
            .sadd(&date_key, &alarm.id.to_string())
            .await?;

        // Add to realtime hash for quick access
        let realtime_key = "alarmsrv:realtime";
        let realtime_field = format!("{}:{}", "default", alarm.id); // channel:id
        let realtime_data = serde_json::json!({
            "id": alarm.id,
            "created_at": alarm.created_at.to_rfc3339(),
            "level": alarm.level,
            "category": "general",
        });
        conn.get_connection_mut()
            .hset(&realtime_key, &realtime_field, &realtime_data.to_string())
            .await?;

        // Add to hourly bucket for time-based queries
        let bucket = alarm.created_at.format("%Y%m%d%H").to_string();
        let bucket_index_key = format!("alarmsrv:buckets:{}", bucket);
        conn.get_connection_mut()
            .sadd(&bucket_index_key, &alarm.id.to_string())
            .await?;

        Ok(())
    }

    /// Get alarm IDs from indexes based on filters
    pub async fn get_alarm_ids(
        &self,
        category: Option<String>,
        level: Option<AlarmLevel>,
        status: Option<AlarmStatus>,
    ) -> Result<Vec<String>> {
        let mut client_guard = self.client.get_client().await?;
        let mut alarm_ids = Vec::new();

        if let Some(conn) = client_guard.as_mut() {
            if let Some(cat) = category {
                let category_key = format!("alarmsrv:category:{}", cat);
                alarm_ids = conn.get_connection_mut().smembers(&category_key).await?;
            } else if let Some(lvl) = level {
                let level_key = format!("alarmsrv:level:{:?}", lvl);
                alarm_ids = conn.get_connection_mut().smembers(&level_key).await?;
            } else if let Some(stat) = status {
                let status_key = format!("alarmsrv:status:{:?}", stat);
                alarm_ids = conn.get_connection_mut().smembers(&status_key).await?;
            } else {
                // Get all alarm IDs
                let pattern = "alarmsrv:*";
                let keys: Vec<String> = conn.keys(pattern).await?;
                alarm_ids = keys
                    .into_iter()
                    .filter(|k| {
                        !k.contains(":category:")
                            && !k.contains(":level:")
                            && !k.contains(":status:")
                            && !k.contains(":date:")
                            && !k.contains(":stats:")
                            && !k.contains(":buckets:")
                            && !k.contains(":realtime")
                            && !k.contains(":handled:")
                    })
                    .map(|k| k.replace("alarmsrv:", ""))
                    .collect();
            }
        }

        Ok(alarm_ids)
    }

    /// Get unclassified alarm IDs
    pub async fn get_unclassified_alarm_ids(&self) -> Result<Vec<String>> {
        let mut client_guard = self.client.get_client().await?;
        let mut alarm_ids = Vec::new();

        if let Some(conn) = client_guard.as_mut() {
            let unclassified_key = "alarmsrv:category:unclassified";
            alarm_ids = conn
                .get_connection_mut()
                .smembers(&unclassified_key)
                .await?;
        }

        Ok(alarm_ids)
    }

    /// Clean up alarm indexes
    pub async fn cleanup_alarm_indexes(
        &self,
        conn: &mut RedisClient,
        alarm_id: &str,
    ) -> Result<()> {
        // Remove from all possible indexes
        let patterns = vec![
            "alarmsrv:category:*",
            "alarmsrv:level:*",
            "alarmsrv:status:*",
            "alarmsrv:date:*",
        ];

        for pattern in patterns {
            let keys: Vec<String> = conn.keys(pattern).await?;
            for key in keys {
                conn.get_connection_mut().srem(&key, alarm_id).await?;
            }
        }

        // Remove from realtime hash
        let realtime_key = "alarmsrv:realtime";
        let all_fields: HashMap<String, String> =
            conn.get_connection_mut().hgetall(&realtime_key).await?;

        for (field, _) in all_fields {
            if field.ends_with(&format!(":{}", alarm_id)) {
                conn.get_connection_mut()
                    .hdel(&realtime_key, &[field.as_str()])
                    .await?;
            }
        }

        Ok(())
    }
}
