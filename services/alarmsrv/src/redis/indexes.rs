use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use voltage_common::redis::RedisClient;

use crate::domain::{Alarm, AlarmLevel, AlarmStatus};
use crate::redis::AlarmRedisClient;

/// Manages Redis indexes for alarms
pub struct AlarmIndexManager {
    client: Arc<AlarmRedisClient>,
}

impl AlarmIndexManager {
    /// Create new index manager
    pub fn new(client: Arc<AlarmRedisClient>) -> Self {
        Self { client }
    }

    /// Add alarm to all relevant indexes
    pub async fn add_to_indexes(&self, conn: &RedisClient, alarm: &Alarm) -> Result<()> {
        // Add to category index
        let category_key = format!("ems:alarms:category:{}", alarm.classification.category);
        conn.sadd(&category_key, &alarm.id.to_string()).await?;

        // Add to level index
        let level_key = format!("ems:alarms:level:{:?}", alarm.level);
        conn.sadd(&level_key, &alarm.id.to_string()).await?;

        // Add to status index
        let status_key = format!("ems:alarms:status:{:?}", alarm.status);
        conn.sadd(&status_key, &alarm.id.to_string()).await?;

        // Add to time-based index
        let date_key = format!("ems:alarms:date:{}", alarm.created_at.format("%Y-%m-%d"));
        conn.sadd(&date_key, &alarm.id.to_string()).await?;

        // Add to realtime hash for quick access
        let realtime_key = "ems:alarms:realtime";
        let realtime_field = format!("{}:{}", "default", alarm.id); // channel:id
        let realtime_data = serde_json::json!({
            "id": alarm.id,
            "created_at": alarm.created_at.to_rfc3339(),
            "level": alarm.level,
            "category": alarm.classification.category,
        });
        conn.hset(&realtime_key, &realtime_field, &realtime_data.to_string())
            .await?;

        // Add to hourly bucket for time-based queries
        let bucket = alarm.created_at.format("%Y%m%d%H").to_string();
        let bucket_index_key = format!("ems:alarms:buckets:{}", bucket);
        conn.sadd(&bucket_index_key, &alarm.id.to_string()).await?;

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
                let category_key = format!("ems:alarms:category:{}", cat);
                alarm_ids = conn.smembers(&category_key).await?;
            } else if let Some(lvl) = level {
                let level_key = format!("ems:alarms:level:{:?}", lvl);
                alarm_ids = conn.smembers(&level_key).await?;
            } else if let Some(stat) = status {
                let status_key = format!("ems:alarms:status:{:?}", stat);
                alarm_ids = conn.smembers(&status_key).await?;
            } else {
                // Get all alarm IDs
                let pattern = "ems:alarms:*";
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
                    .map(|k| k.replace("ems:alarms:", ""))
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
            let unclassified_key = "ems:alarms:category:unclassified";
            alarm_ids = conn.smembers(&unclassified_key).await?;
        }

        Ok(alarm_ids)
    }

    /// Clean up alarm indexes
    pub async fn cleanup_alarm_indexes(&self, conn: &RedisClient, alarm_id: &str) -> Result<()> {
        // Remove from all possible indexes
        let patterns = vec![
            "ems:alarms:category:*",
            "ems:alarms:level:*",
            "ems:alarms:status:*",
            "ems:alarms:date:*",
        ];

        for pattern in patterns {
            let keys: Vec<String> = conn.keys(pattern).await?;
            for key in keys {
                conn.srem(&key, alarm_id).await?;
            }
        }

        // Remove from realtime hash
        let realtime_key = "ems:alarms:realtime";
        let all_fields: HashMap<String, String> = conn.hgetall(&realtime_key).await?;

        for (field, _) in all_fields {
            if field.ends_with(&format!(":{}", alarm_id)) {
                conn.hdel(&realtime_key, &[field.as_str()]).await?;
            }
        }

        Ok(())
    }
}
