use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use redis::AsyncCommands;
use serde_json;
use std::sync::Arc;
use tracing::debug;

use crate::domain::{Alarm, CloudAlarm, EscalationRule};
use crate::redis::{AlarmIndexManager, AlarmRedisClient, AlarmStatisticsManager};

/// Redis storage for alarms with classification and cloud integration
pub struct AlarmStore {
    client: Arc<AlarmRedisClient>,
    index_manager: Arc<AlarmIndexManager>,
    stats_manager: Arc<AlarmStatisticsManager>,
}

impl AlarmStore {
    /// Create new alarm store instance
    pub async fn new(client: Arc<AlarmRedisClient>) -> Result<Self> {
        let index_manager = Arc::new(AlarmIndexManager::new(client.clone()));
        let stats_manager = Arc::new(AlarmStatisticsManager::new(client.clone()));

        Ok(Self {
            client,
            index_manager,
            stats_manager,
        })
    }

    /// Store alarm in Redis with classification
    pub async fn store_alarm(&self, alarm: &Alarm) -> Result<()> {
        let mut client_guard = self.client.get_client().await?;
        if let Some(conn) = client_guard.as_mut() {
            let alarm_key = format!("ems:alarms:{}", alarm.id);
            let alarm_json = serde_json::to_string(alarm)?;

            // Store main alarm data
            let fields: Vec<(String, String)> = vec![
                ("id".to_string(), alarm.id.to_string()),
                ("title".to_string(), alarm.title.clone()),
                ("description".to_string(), alarm.description.clone()),
                ("level".to_string(), serde_json::to_string(&alarm.level)?),
                ("status".to_string(), serde_json::to_string(&alarm.status)?),
                ("category".to_string(), "general".to_string()),
                ("priority".to_string(), alarm.metadata.priority.to_string()),
                (
                    "tags".to_string(),
                    serde_json::to_string(&alarm.metadata.tags)?,
                ),
                ("created_at".to_string(), alarm.created_at.to_rfc3339()),
                ("updated_at".to_string(), alarm.updated_at.to_rfc3339()),
                ("data".to_string(), alarm_json),
            ];
            conn.get_connection_mut()
                .hset_multiple(&alarm_key, &fields)
                .await?;

            // Update indexes
            self.index_manager.add_to_indexes(conn, alarm).await?;

            // Update statistics
            self.stats_manager
                .update_statistics(conn, alarm, "created")
                .await?;

            debug!("Stored alarm {} in Redis", alarm.id);
        }

        Ok(())
    }

    /// Get alarm by ID
    pub async fn get_alarm(&self, alarm_id: &str) -> Result<Option<Alarm>> {
        let mut client_guard = self.client.get_client().await?;
        if let Some(conn) = client_guard.as_mut() {
            let alarm_key = format!("ems:alarms:{}", alarm_id);
            if let Ok(alarm_data) = conn
                .get_connection_mut()
                .hget::<_, _, String>(&alarm_key, "data")
                .await
            {
                if let Ok(alarm) = serde_json::from_str::<Alarm>(&alarm_data) {
                    return Ok(Some(alarm));
                }
            }
        }
        Ok(None)
    }

    /// Acknowledge alarm
    pub async fn acknowledge_alarm(&self, alarm_id: &str, user: String) -> Result<Alarm> {
        let mut client_guard = self.client.get_client().await?;
        if let Some(conn) = client_guard.as_mut() {
            let alarm_key = format!("ems:alarms:{}", alarm_id);

            // Get current alarm data
            let alarm_data: String = conn
                .get_connection_mut()
                .hget::<_, _, String>(&alarm_key, "data")
                .await
                .map_err(|_| anyhow::anyhow!("Alarm data not found"))?;
            let mut alarm: Alarm = serde_json::from_str(&alarm_data)?;

            // Update alarm
            alarm.acknowledge(user);

            // Update in Redis
            let updated_data = serde_json::to_string(&alarm)?;
            let fields: Vec<(String, String)> = vec![
                ("status".to_string(), serde_json::to_string(&alarm.status)?),
                ("updated_at".to_string(), alarm.updated_at.to_rfc3339()),
                (
                    "acknowledged_at".to_string(),
                    alarm.acknowledged_at.unwrap().to_rfc3339(),
                ),
                (
                    "acknowledged_by".to_string(),
                    alarm.acknowledged_by.as_ref().unwrap().clone(),
                ),
                ("data".to_string(), updated_data),
            ];
            conn.get_connection_mut()
                .hset_multiple(&alarm_key, &fields)
                .await?;

            // Update status indexes
            conn.get_connection_mut()
                .srem("ems:alarms:status:New", alarm_id)
                .await?;
            conn.get_connection_mut()
                .sadd("ems:alarms:status:Acknowledged", alarm_id)
                .await?;

            // Update statistics
            self.stats_manager
                .update_statistics(conn, &alarm, "acknowledged")
                .await?;

            return Ok(alarm);
        }

        Err(anyhow::anyhow!("Alarm not found"))
    }

    /// Resolve alarm
    pub async fn resolve_alarm(&self, alarm_id: &str, user: String) -> Result<Alarm> {
        let mut client_guard = self.client.get_client().await?;
        if let Some(conn) = client_guard.as_mut() {
            let alarm_key = format!("ems:alarms:{}", alarm_id);

            // Get current alarm data
            let alarm_data: String = conn
                .get_connection_mut()
                .hget::<_, _, String>(&alarm_key, "data")
                .await
                .map_err(|_| anyhow::anyhow!("Alarm data not found"))?;
            let mut alarm: Alarm = serde_json::from_str(&alarm_data)?;

            // Update alarm
            alarm.resolve(user);

            // Update in Redis
            let updated_data = serde_json::to_string(&alarm)?;
            let fields: Vec<(String, String)> = vec![
                ("status".to_string(), serde_json::to_string(&alarm.status)?),
                ("updated_at".to_string(), alarm.updated_at.to_rfc3339()),
                (
                    "resolved_at".to_string(),
                    alarm.resolved_at.unwrap().to_rfc3339(),
                ),
                (
                    "resolved_by".to_string(),
                    alarm.resolved_by.as_ref().unwrap().clone(),
                ),
                ("data".to_string(), updated_data),
            ];
            conn.get_connection_mut()
                .hset_multiple(&alarm_key, &fields)
                .await?;

            // Update status indexes
            conn.get_connection_mut()
                .srem("ems:alarms:status:New", alarm_id)
                .await?;
            conn.get_connection_mut()
                .srem("ems:alarms:status:Acknowledged", alarm_id)
                .await?;
            conn.get_connection_mut()
                .sadd("ems:alarms:status:Resolved", alarm_id)
                .await?;

            // Update statistics
            self.stats_manager
                .update_statistics(conn, &alarm, "resolved")
                .await?;

            return Ok(alarm);
        }

        Err(anyhow::anyhow!("Alarm not found"))
    }

    /// Update alarm classification
    pub async fn update_alarm_classification(&self, alarm: &Alarm) -> Result<()> {
        let mut client_guard = self.client.get_client().await?;
        if let Some(conn) = client_guard.as_mut() {
            let alarm_key = format!("ems:alarms:{}", alarm.id);
            let alarm_json = serde_json::to_string(alarm)?;

            // Update classification data
            let fields: Vec<(String, String)> = vec![
                ("category".to_string(), "general".to_string()),
                ("priority".to_string(), alarm.metadata.priority.to_string()),
                (
                    "tags".to_string(),
                    serde_json::to_string(&alarm.metadata.tags)?,
                ),
                ("updated_at".to_string(), alarm.updated_at.to_rfc3339()),
                ("data".to_string(), alarm_json),
            ];
            conn.get_connection_mut()
                .hset_multiple(&alarm_key, &fields)
                .await?;

            // Remove from unclassified
            conn.get_connection_mut()
                .srem("ems:alarms:category:unclassified", &alarm.id.to_string())
                .await?;

            // Add to general category
            let category_key = "ems:alarms:category:general".to_string();
            conn.get_connection_mut()
                .sadd(&category_key, &alarm.id.to_string())
                .await?;

            debug!("Updated alarm {} to general category", alarm.id);
        }

        Ok(())
    }

    /// Update alarm (for escalation)
    pub async fn update_alarm(&self, alarm: &Alarm) -> Result<()> {
        let mut client_guard = self.client.get_client().await?;
        if let Some(conn) = client_guard.as_mut() {
            let alarm_key = format!("ems:alarms:{}", alarm.id);
            let alarm_json = serde_json::to_string(alarm)?;

            let fields: Vec<(String, String)> = vec![
                ("level".to_string(), serde_json::to_string(&alarm.level)?),
                ("updated_at".to_string(), alarm.updated_at.to_rfc3339()),
                ("data".to_string(), alarm_json),
            ];
            conn.get_connection_mut()
                .hset_multiple(&alarm_key, &fields)
                .await?;

            // Update level indexes
            // Note: This is simplified - in reality we'd need to remove from old level index
            let level_key = format!("ems:alarms:level:{:?}", alarm.level);
            conn.get_connection_mut()
                .sadd(&level_key, &alarm.id.to_string())
                .await?;

            // Update statistics
            self.stats_manager
                .update_statistics(conn, alarm, "escalated")
                .await?;

            debug!("Updated alarm {} level to {:?}", alarm.id, alarm.level);
        }

        Ok(())
    }

    /// Get alarms for escalation based on rules
    pub async fn get_alarms_for_escalation(&self, rule: &EscalationRule) -> Result<Vec<Alarm>> {
        let mut client_guard = self.client.get_client().await?;
        let mut alarms = Vec::new();

        if let Some(conn) = client_guard.as_mut() {
            let status_key = format!("ems:alarms:status:{:?}", rule.from_status);
            let alarm_ids: Vec<String> = conn.get_connection_mut().smembers(&status_key).await?;

            let cutoff_time = Utc::now() - Duration::minutes(rule.duration_minutes as i64);

            for alarm_id in alarm_ids {
                let alarm_key = format!("ems:alarms:{}", alarm_id);
                if let Ok(alarm_data) = conn
                    .get_connection_mut()
                    .hget::<_, _, String>(&alarm_key, "data")
                    .await
                {
                    if let Ok(alarm) = serde_json::from_str::<Alarm>(&alarm_data) {
                        if alarm.created_at < cutoff_time && alarm.level == rule.from_level {
                            alarms.push(alarm);
                        }
                    }
                }
            }
        }

        Ok(alarms)
    }

    /// Publish alarm for cloud push via netsrv
    pub async fn publish_alarm_for_cloud(&self, alarm: &Alarm) -> Result<()> {
        let mut client_guard = self.client.get_client().await?;
        if let Some(conn) = client_guard.as_mut() {
            let cloud_alarm = CloudAlarm::from_alarm(alarm);
            let cloud_data = serde_json::to_string(&cloud_alarm)?;

            // Publish to netsrv data channel
            conn.publish("ems:data:alarms", &cloud_data).await?;

            // Also store for batch processing
            let cloud_queue_key = "ems:cloud:alarms:queue";
            conn.get_connection_mut()
                .lpush(&cloud_queue_key, &cloud_data)
                .await?;

            debug!("Published alarm {} for cloud push", alarm.id);
        }

        Ok(())
    }

    /// Clean up old resolved alarms
    pub async fn cleanup_old_alarms(&self, retention_days: u32) -> Result<usize> {
        let mut client_guard = self.client.get_client().await?;
        let mut cleaned_count = 0;

        if let Some(conn) = client_guard.as_mut() {
            let cutoff_date = Utc::now() - Duration::days(retention_days as i64);
            let resolved_key = "ems:alarms:status:Resolved";
            let alarm_ids: Vec<String> = conn.get_connection_mut().smembers(&resolved_key).await?;

            for alarm_id in alarm_ids {
                let alarm_key = format!("ems:alarms:{}", alarm_id);
                if let Ok(resolved_at_str) = conn
                    .get_connection_mut()
                    .hget::<_, _, String>(&alarm_key, "resolved_at")
                    .await
                {
                    if let Ok(resolved_at) = DateTime::parse_from_rfc3339(&resolved_at_str) {
                        if resolved_at.with_timezone(&Utc) < cutoff_date {
                            // Remove alarm and all its indexes
                            self.index_manager
                                .cleanup_alarm_indexes(conn, &alarm_id)
                                .await?;
                            conn.get_connection_mut()
                                .del(&[&alarm_key.as_str()])
                                .await?;
                            cleaned_count += 1;
                        }
                    }
                }
            }
        }

        Ok(cleaned_count)
    }
}
