use anyhow::Result;
use redis::{Client, Commands, Connection};
use serde_json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::debug;
use chrono::{DateTime, Utc, Duration};

use crate::config::AlarmConfig;
use crate::types::*;

/// Redis storage for alarms with classification and cloud integration
pub struct RedisStorage {
    client: Arc<Mutex<Option<Connection>>>,
    config: Arc<AlarmConfig>,
}

impl RedisStorage {
    /// Create new Redis storage instance
    pub async fn new(config: Arc<AlarmConfig>) -> Result<Self> {
        let redis_url = format!("redis://{}:{}", config.redis.host, config.redis.port);
        let client = Client::open(redis_url)?;
        let connection = client.get_connection()?;
        
        Ok(Self {
            client: Arc::new(Mutex::new(Some(connection))),
            config,
        })
    }
    
    /// Check if Redis connection is active
    pub async fn is_connected(&self) -> bool {
        let client = self.client.lock().await;
        client.is_some()
    }
    
    /// Store alarm in Redis with classification
    pub async fn store_alarm(&self, alarm: &Alarm) -> Result<()> {
        let mut client = self.client.lock().await;
        if let Some(conn) = client.as_mut() {
            let alarm_key = format!("ems:alarms:{}", alarm.id);
            let alarm_json = serde_json::to_string(alarm)?;
            
            // Store main alarm data
            conn.hset_multiple(&alarm_key, &[
                ("id", alarm.id.to_string()),
                ("title", alarm.title.clone()),
                ("description", alarm.description.clone()),
                ("level", serde_json::to_string(&alarm.level)?),
                ("status", serde_json::to_string(&alarm.status)?),
                ("category", alarm.classification.category.clone()),
                ("priority", alarm.classification.priority.to_string()),
                ("tags", serde_json::to_string(&alarm.classification.tags)?),
                ("created_at", alarm.created_at.to_rfc3339()),
                ("updated_at", alarm.updated_at.to_rfc3339()),
                ("data", alarm_json),
            ])?;
            
            // Add to category index
            let category_key = format!("ems:alarms:category:{}", alarm.classification.category);
            conn.sadd(&category_key, alarm.id.to_string())?;
            
            // Add to level index
            let level_key = format!("ems:alarms:level:{:?}", alarm.level);
            conn.sadd(&level_key, alarm.id.to_string())?;
            
            // Add to status index
            let status_key = format!("ems:alarms:status:{:?}", alarm.status);
            conn.sadd(&status_key, alarm.id.to_string())?;
            
            // Add to time-based index
            let date_key = format!("ems:alarms:date:{}", alarm.created_at.format("%Y-%m-%d"));
            conn.sadd(&date_key, alarm.id.to_string())?;
            
            // Update statistics
            self.update_statistics(conn, alarm, "created").await?;
            
            debug!("Stored alarm {} in Redis", alarm.id);
        }
        
        Ok(())
    }
    
    /// Get alarms with optional filtering
    pub async fn get_alarms(
        &self,
        category: Option<String>,
        level: Option<AlarmLevel>,
        status: Option<AlarmStatus>,
        limit: Option<usize>,
    ) -> Result<Vec<Alarm>> {
        let mut client = self.client.lock().await;
        let mut alarms = Vec::new();
        
        if let Some(conn) = client.as_mut() {
            let mut alarm_ids = Vec::new();
            
            // Get alarm IDs based on filters
            if let Some(cat) = category {
                let category_key = format!("ems:alarms:category:{}", cat);
                alarm_ids = conn.smembers(&category_key)?;
            } else if let Some(lvl) = level {
                let level_key = format!("ems:alarms:level:{:?}", lvl);
                alarm_ids = conn.smembers(&level_key)?;
            } else if let Some(stat) = status {
                let status_key = format!("ems:alarms:status:{:?}", stat);
                alarm_ids = conn.smembers(&status_key)?;
            } else {
                // Get all alarm IDs
                let pattern = "ems:alarms:*";
                let keys: Vec<String> = conn.keys(pattern)?;
                alarm_ids = keys.into_iter()
                    .filter(|k| !k.contains(":category:") && !k.contains(":level:") && !k.contains(":status:") && !k.contains(":date:"))
                    .map(|k| k.replace("ems:alarms:", ""))
                    .collect();
            }
            
            // Apply limit
            if let Some(limit_val) = limit {
                alarm_ids.truncate(limit_val);
            }
            
            // Fetch alarm data
            for alarm_id in alarm_ids {
                let alarm_key = format!("ems:alarms:{}", alarm_id);
                if let Ok(alarm_data) = conn.hget::<_, _, String>(&alarm_key, "data") {
                    if let Ok(alarm) = serde_json::from_str::<Alarm>(&alarm_data) {
                        alarms.push(alarm);
                    }
                }
            }
        }
        
        Ok(alarms)
    }
    
    /// Acknowledge alarm
    pub async fn acknowledge_alarm(&self, alarm_id: &str, user: String) -> Result<Alarm> {
        let mut client = self.client.lock().await;
        if let Some(conn) = client.as_mut() {
            let alarm_key = format!("ems:alarms:{}", alarm_id);
            
            // Get current alarm data
            let alarm_data: String = conn.hget(&alarm_key, "data")?;
            let mut alarm: Alarm = serde_json::from_str(&alarm_data)?;
            
            // Update alarm
            alarm.acknowledge(user);
            
            // Update in Redis
            let updated_data = serde_json::to_string(&alarm)?;
            conn.hset_multiple(&alarm_key, &[
                ("status", serde_json::to_string(&alarm.status)?),
                ("updated_at", alarm.updated_at.to_rfc3339()),
                ("acknowledged_at", alarm.acknowledged_at.unwrap().to_rfc3339()),
                ("acknowledged_by", alarm.acknowledged_by.as_ref().unwrap().clone()),
                ("data", updated_data),
            ])?;
            
            // Update status indexes
            conn.srem("ems:alarms:status:New", alarm_id)?;
            conn.sadd("ems:alarms:status:Acknowledged", alarm_id)?;
            
            // Update statistics
            self.update_statistics(conn, &alarm, "acknowledged").await?;
            
            return Ok(alarm);
        }
        
        Err(anyhow::anyhow!("Alarm not found"))
    }
    
    /// Resolve alarm
    pub async fn resolve_alarm(&self, alarm_id: &str, user: String) -> Result<Alarm> {
        let mut client = self.client.lock().await;
        if let Some(conn) = client.as_mut() {
            let alarm_key = format!("ems:alarms:{}", alarm_id);
            
            // Get current alarm data
            let alarm_data: String = conn.hget(&alarm_key, "data")?;
            let mut alarm: Alarm = serde_json::from_str(&alarm_data)?;
            
            // Update alarm
            alarm.resolve(user);
            
            // Update in Redis
            let updated_data = serde_json::to_string(&alarm)?;
            conn.hset_multiple(&alarm_key, &[
                ("status", serde_json::to_string(&alarm.status)?),
                ("updated_at", alarm.updated_at.to_rfc3339()),
                ("resolved_at", alarm.resolved_at.unwrap().to_rfc3339()),
                ("resolved_by", alarm.resolved_by.as_ref().unwrap().clone()),
                ("data", updated_data),
            ])?;
            
            // Update status indexes
            conn.srem("ems:alarms:status:New", alarm_id)?;
            conn.srem("ems:alarms:status:Acknowledged", alarm_id)?;
            conn.sadd("ems:alarms:status:Resolved", alarm_id)?;
            
            // Update statistics
            self.update_statistics(conn, &alarm, "resolved").await?;
            
            return Ok(alarm);
        }
        
        Err(anyhow::anyhow!("Alarm not found"))
    }
    
    /// Get unclassified alarms
    pub async fn get_unclassified_alarms(&self) -> Result<Vec<Alarm>> {
        let mut client = self.client.lock().await;
        let mut alarms = Vec::new();
        
        if let Some(conn) = client.as_mut() {
            let unclassified_key = "ems:alarms:category:unclassified";
            let alarm_ids: Vec<String> = conn.smembers(&unclassified_key)?;
            
            for alarm_id in alarm_ids {
                let alarm_key = format!("ems:alarms:{}", alarm_id);
                if let Ok(alarm_data) = conn.hget::<_, _, String>(&alarm_key, "data") {
                    if let Ok(alarm) = serde_json::from_str::<Alarm>(&alarm_data) {
                        alarms.push(alarm);
                    }
                }
            }
        }
        
        Ok(alarms)
    }
    
    /// Update alarm classification
    pub async fn update_alarm_classification(&self, alarm: &Alarm) -> Result<()> {
        let mut client = self.client.lock().await;
        if let Some(conn) = client.as_mut() {
            let alarm_key = format!("ems:alarms:{}", alarm.id);
            let alarm_json = serde_json::to_string(alarm)?;
            
            // Update classification data
            conn.hset_multiple(&alarm_key, &[
                ("category", alarm.classification.category.clone()),
                ("priority", alarm.classification.priority.to_string()),
                ("tags", serde_json::to_string(&alarm.classification.tags)?),
                ("updated_at", alarm.updated_at.to_rfc3339()),
                ("data", alarm_json),
            ])?;
            
            // Remove from unclassified
            conn.srem("ems:alarms:category:unclassified", alarm.id.to_string())?;
            
            // Add to new category
            let category_key = format!("ems:alarms:category:{}", alarm.classification.category);
            conn.sadd(&category_key, alarm.id.to_string())?;
            
            debug!("Updated alarm {} classification to {}", alarm.id, alarm.classification.category);
        }
        
        Ok(())
    }
    
    /// Update alarm (for escalation)
    pub async fn update_alarm(&self, alarm: &Alarm) -> Result<()> {
        let mut client = self.client.lock().await;
        if let Some(conn) = client.as_mut() {
            let alarm_key = format!("ems:alarms:{}", alarm.id);
            let alarm_json = serde_json::to_string(alarm)?;
            
            conn.hset_multiple(&alarm_key, &[
                ("level", serde_json::to_string(&alarm.level)?),
                ("updated_at", alarm.updated_at.to_rfc3339()),
                ("data", alarm_json),
            ])?;
            
            // Update level indexes
            // Note: This is simplified - in reality we'd need to remove from old level index
            let level_key = format!("ems:alarms:level:{:?}", alarm.level);
            conn.sadd(&level_key, alarm.id.to_string())?;
            
            // Update statistics
            self.update_statistics(conn, alarm, "escalated").await?;
            
            debug!("Updated alarm {} level to {:?}", alarm.id, alarm.level);
        }
        
        Ok(())
    }
    
    /// Get alarms for escalation based on rules
    pub async fn get_alarms_for_escalation(&self, rule: &EscalationRule) -> Result<Vec<Alarm>> {
        let mut client = self.client.lock().await;
        let mut alarms = Vec::new();
        
        if let Some(conn) = client.as_mut() {
            let status_key = format!("ems:alarms:status:{:?}", rule.from_status);
            let alarm_ids: Vec<String> = conn.smembers(&status_key)?;
            
            let cutoff_time = Utc::now() - Duration::minutes(rule.duration_minutes as i64);
            
            for alarm_id in alarm_ids {
                let alarm_key = format!("ems:alarms:{}", alarm_id);
                if let Ok(alarm_data) = conn.hget::<_, _, String>(&alarm_key, "data") {
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
        let mut client = self.client.lock().await;
        if let Some(conn) = client.as_mut() {
            let cloud_alarm = CloudAlarm::from_alarm(alarm);
            let cloud_data = serde_json::to_string(&cloud_alarm)?;
            
            // Publish to netsrv data channel
            conn.publish("ems:data:alarms", &cloud_data)?;
            
            // Also store for batch processing
            let cloud_queue_key = "ems:cloud:alarms:queue";
            conn.lpush(&cloud_queue_key, &cloud_data)?;
            
            debug!("Published alarm {} for cloud push", alarm.id);
        }
        
        Ok(())
    }
    
    /// Get alarm statistics
    pub async fn get_alarm_statistics(&self) -> Result<AlarmStatistics> {
        let mut client = self.client.lock().await;
        if let Some(conn) = client.as_mut() {
            let stats_key = "ems:alarms:stats";
            
            let total: i32 = conn.hget(&stats_key, "total").unwrap_or(0);
            let new: i32 = conn.hget(&stats_key, "new").unwrap_or(0);
            let acknowledged: i32 = conn.hget(&stats_key, "acknowledged").unwrap_or(0);
            let resolved: i32 = conn.hget(&stats_key, "resolved").unwrap_or(0);
            let critical: i32 = conn.hget(&stats_key, "critical").unwrap_or(0);
            let major: i32 = conn.hget(&stats_key, "major").unwrap_or(0);
            let minor: i32 = conn.hget(&stats_key, "minor").unwrap_or(0);
            let warning: i32 = conn.hget(&stats_key, "warning").unwrap_or(0);
            let info: i32 = conn.hget(&stats_key, "info").unwrap_or(0);
            
            // Get category statistics
            let categories = self.get_category_statistics(conn).await?;
            
            return Ok(AlarmStatistics {
                total: total as usize,
                by_status: AlarmStatusStats {
                    new: new as usize,
                    acknowledged: acknowledged as usize,
                    resolved: resolved as usize,
                },
                by_level: AlarmLevelStats {
                    critical: critical as usize,
                    major: major as usize,
                    minor: minor as usize,
                    warning: warning as usize,
                    info: info as usize,
                },
                by_category: categories,
            });
        }
        
        Err(anyhow::anyhow!("Failed to get statistics"))
    }
    
    /// Clean up old resolved alarms
    pub async fn cleanup_old_alarms(&self, retention_days: u32) -> Result<usize> {
        let mut client = self.client.lock().await;
        let mut cleaned_count = 0;
        
        if let Some(conn) = client.as_mut() {
            let cutoff_date = Utc::now() - Duration::days(retention_days as i64);
            let resolved_key = "ems:alarms:status:Resolved";
            let alarm_ids: Vec<String> = conn.smembers(&resolved_key)?;
            
            for alarm_id in alarm_ids {
                let alarm_key = format!("ems:alarms:{}", alarm_id);
                if let Ok(resolved_at_str) = conn.hget::<_, _, String>(&alarm_key, "resolved_at") {
                    if let Ok(resolved_at) = DateTime::parse_from_rfc3339(&resolved_at_str) {
                        if resolved_at.with_timezone(&Utc) < cutoff_date {
                            // Remove alarm and all its indexes
                            self.cleanup_alarm_indexes(conn, &alarm_id).await?;
                            conn.del(&alarm_key)?;
                            cleaned_count += 1;
                        }
                    }
                }
            }
        }
        
        Ok(cleaned_count)
    }
    
    /// Helper: Update statistics
    async fn update_statistics(&self, conn: &mut Connection, alarm: &Alarm, action: &str) -> Result<()> {
        let stats_key = "ems:alarms:stats";
        
        match action {
            "created" => {
                conn.hincr(&stats_key, "total", 1)?;
                conn.hincr(&stats_key, "new", 1)?;
                conn.hincr(&stats_key, &format!("{:?}", alarm.level).to_lowercase(), 1)?;
            }
            "acknowledged" => {
                conn.hincr(&stats_key, "new", -1)?;
                conn.hincr(&stats_key, "acknowledged", 1)?;
            }
            "resolved" => {
                conn.hincr(&stats_key, "acknowledged", -1)?;
                conn.hincr(&stats_key, "resolved", 1)?;
            }
            "escalated" => {
                conn.hincr(&stats_key, &format!("{:?}", alarm.level).to_lowercase(), 1)?;
            }
            _ => {}
        }
        
        Ok(())
    }
    
    /// Helper: Get category statistics
    async fn get_category_statistics(&self, conn: &mut Connection) -> Result<HashMap<String, usize>> {
        let mut categories = HashMap::new();
        let pattern = "ems:alarms:category:*";
        let keys: Vec<String> = conn.keys(pattern)?;
        
        for key in keys {
            let category = key.replace("ems:alarms:category:", "");
            let count: usize = conn.scard(&key).unwrap_or(0);
            categories.insert(category, count);
        }
        
        Ok(categories)
    }
    
    /// Helper: Clean up alarm indexes
    async fn cleanup_alarm_indexes(&self, conn: &mut Connection, alarm_id: &str) -> Result<()> {
        // Remove from all possible indexes
        let patterns = vec![
            "ems:alarms:category:*",
            "ems:alarms:level:*",
            "ems:alarms:status:*",
            "ems:alarms:date:*",
        ];
        
        for pattern in patterns {
            let keys: Vec<String> = conn.keys(pattern)?;
            for key in keys {
                conn.srem(&key, alarm_id)?;
            }
        }
        
        Ok(())
    }
} 