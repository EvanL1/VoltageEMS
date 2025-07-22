//! Alarm historical storage service using InfluxDB

use super::{AlarmDataPoint, DataValue, InfluxDBClient};
use crate::domain::Alarm;
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::collections::HashMap;
use tokio::time::{interval, Duration};
use tracing::{debug, error, info};

/// Alarm historical storage service
pub struct AlarmHistoryStorage {
    client: InfluxDBClient,
    flush_interval: Duration,
}

impl AlarmHistoryStorage {
    /// Create new alarm history storage service
    pub fn new(client: InfluxDBClient) -> Self {
        let flush_interval = Duration::from_secs(client.config().flush_interval_seconds);
        
        Self {
            client,
            flush_interval,
        }
    }

    /// Start the storage service with periodic flush
    pub async fn start(&self) -> Result<()> {
        info!("Starting alarm history storage service");
        
        // Test connection first
        if let Err(e) = self.client.ping().await {
            error!("Failed to connect to InfluxDB: {}", e);
            return Err(e);
        }

        // Start periodic flush task
        let client = self.client.clone();
        let mut flush_timer = interval(self.flush_interval);
        
        tokio::spawn(async move {
            loop {
                flush_timer.tick().await;
                if let Err(e) = client.flush().await {
                    error!("Failed to flush alarm data to InfluxDB: {}", e);
                }
            }
        });

        info!("Alarm history storage service started successfully");
        Ok(())
    }

    /// Store alarm creation event
    pub async fn store_alarm_created(&self, alarm: &Alarm) -> Result<()> {
        let point = AlarmDataPoint::from_alarm_data(
            &alarm.id.to_string(),
            &format!("{:?}", alarm.level),
            &format!("{:?}", alarm.status),
            &alarm.title,
            &alarm.description,
            None, // module_id from context if available
            None, // point_name from context if available  
            alarm.created_at,
        );

        self.client.write_alarm_point(point).await?;
        debug!("Stored alarm creation: {}", alarm.id);
        Ok(())
    }

    /// Store alarm status change event
    pub async fn store_alarm_status_change(
        &self,
        alarm: &Alarm,
        old_status: &str,
        new_status: &str,
        user: Option<&str>,
    ) -> Result<()> {
        let mut tags = HashMap::new();
        tags.insert("alarm_id".to_string(), alarm.id.to_string());
        tags.insert("level".to_string(), format!("{:?}", alarm.level));
        tags.insert("old_status".to_string(), old_status.to_string());
        tags.insert("new_status".to_string(), new_status.to_string());
        
        if let Some(u) = user {
            tags.insert("user".to_string(), u.to_string());
        }

        let mut fields = HashMap::new();
        fields.insert("title".to_string(), DataValue::String(alarm.title.clone()));
        fields.insert("description".to_string(), DataValue::String(alarm.description.clone()));
        fields.insert("event_type".to_string(), DataValue::String("status_change".to_string()));

        let point = AlarmDataPoint::new(tags, fields, Utc::now());
        self.client.write_alarm_point(point).await?;
        
        debug!("Stored alarm status change: {} {} -> {}", alarm.id, old_status, new_status);
        Ok(())
    }

    /// Store alarm level escalation event
    pub async fn store_alarm_escalation(
        &self,
        alarm: &Alarm,
        old_level: &str,
        new_level: &str,
    ) -> Result<()> {
        let mut tags = HashMap::new();
        tags.insert("alarm_id".to_string(), alarm.id.to_string());
        tags.insert("status".to_string(), format!("{:?}", alarm.status));
        tags.insert("old_level".to_string(), old_level.to_string());
        tags.insert("new_level".to_string(), new_level.to_string());

        let mut fields = HashMap::new();
        fields.insert("title".to_string(), DataValue::String(alarm.title.clone()));
        fields.insert("description".to_string(), DataValue::String(alarm.description.clone()));
        fields.insert("event_type".to_string(), DataValue::String("escalation".to_string()));

        let point = AlarmDataPoint::new(tags, fields, Utc::now());
        self.client.write_alarm_point(point).await?;
        
        debug!("Stored alarm escalation: {} {} -> {}", alarm.id, old_level, new_level);
        Ok(())
    }

    /// Store alarm deletion (auto-resolution) event
    pub async fn store_alarm_deleted(&self, alarm_id: &str, reason: &str) -> Result<()> {
        let mut tags = HashMap::new();
        tags.insert("alarm_id".to_string(), alarm_id.to_string());
        tags.insert("event_type".to_string(), "deletion".to_string());

        let mut fields = HashMap::new();
        fields.insert("reason".to_string(), DataValue::String(reason.to_string()));
        fields.insert("event_type".to_string(), DataValue::String("deletion".to_string()));

        let point = AlarmDataPoint::new(tags, fields, Utc::now());
        self.client.write_alarm_point(point).await?;
        
        debug!("Stored alarm deletion: {} (reason: {})", alarm_id, reason);
        Ok(())
    }

    /// Query alarm history by alarm ID
    pub async fn query_alarm_history(&self, alarm_id: &str, limit: Option<u32>) -> Result<Value> {
        let limit_clause = limit.map(|l| format!("LIMIT {}", l)).unwrap_or_default();
        
        let sql = format!(
            r#"
            SELECT * FROM alarms 
            WHERE alarm_id = '{}' 
            ORDER BY time DESC
            {}
            "#,
            alarm_id, limit_clause
        );

        self.client.query_alarm_history(&sql).await
    }

    /// Query alarms by time range
    pub async fn query_alarms_by_time_range(
        &self,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
        level_filter: Option<&str>,
        limit: Option<u32>,
    ) -> Result<Value> {
        let level_clause = level_filter
            .map(|level| format!("AND level = '{}'", level))
            .unwrap_or_default();
        
        let limit_clause = limit.map(|l| format!("LIMIT {}", l)).unwrap_or_default();
        
        let sql = format!(
            r#"
            SELECT * FROM alarms 
            WHERE time >= '{}' AND time <= '{}'
            {}
            ORDER BY time DESC
            {}
            "#,
            start_time.to_rfc3339(),
            end_time.to_rfc3339(),
            level_clause,
            limit_clause
        );

        self.client.query_alarm_history(&sql).await
    }

    /// Query alarm statistics by level
    pub async fn query_alarm_statistics(
        &self,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
    ) -> Result<Value> {
        let sql = format!(
            r#"
            SELECT level, COUNT(*) as count
            FROM alarms 
            WHERE time >= '{}' AND time <= '{}'
            GROUP BY level
            ORDER BY count DESC
            "#,
            start_time.to_rfc3339(),
            end_time.to_rfc3339()
        );

        self.client.query_alarm_history(&sql).await
    }

    /// Force flush pending data
    pub async fn flush(&self) -> Result<()> {
        self.client.flush().await
    }

    /// Get current buffer size
    pub async fn buffer_size(&self) -> usize {
        self.client.buffer_size().await
    }
}

/// Alarm history query parameters
#[derive(Debug, Clone)]
pub struct AlarmHistoryQuery {
    pub alarm_id: Option<String>,
    pub level: Option<String>,
    pub status: Option<String>,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub limit: Option<u32>,
}

impl AlarmHistoryQuery {
    /// Build SQL query from parameters
    pub fn to_sql(&self) -> String {
        let mut conditions = Vec::new();
        
        if let Some(ref alarm_id) = self.alarm_id {
            conditions.push(format!("alarm_id = '{}'", alarm_id));
        }
        
        if let Some(ref level) = self.level {
            conditions.push(format!("level = '{}'", level));
        }
        
        if let Some(ref status) = self.status {
            conditions.push(format!("status = '{}'", status));
        }
        
        if let Some(start_time) = self.start_time {
            conditions.push(format!("time >= '{}'", start_time.to_rfc3339()));
        }
        
        if let Some(end_time) = self.end_time {
            conditions.push(format!("time <= '{}'", end_time.to_rfc3339()));
        }

        let where_clause = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        let limit_clause = self.limit
            .map(|l| format!("LIMIT {}", l))
            .unwrap_or_default();

        format!(
            "SELECT * FROM alarms {} ORDER BY time DESC {}",
            where_clause, limit_clause
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::influx::InfluxDBConfig;

    #[test]
    fn test_alarm_history_query_builder() {
        let query = AlarmHistoryQuery {
            alarm_id: Some("test_001".to_string()),
            level: Some("Critical".to_string()),
            status: None,
            start_time: Some(DateTime::from_timestamp(1642681200, 0).unwrap()),
            end_time: Some(DateTime::from_timestamp(1642684800, 0).unwrap()),
            limit: Some(100),
        };

        let sql = query.to_sql();
        assert!(sql.contains("alarm_id = 'test_001'"));
        assert!(sql.contains("level = 'Critical'"));
        assert!(sql.contains("time >= '2022-01-20T11:00:00+00:00'"));
        assert!(sql.contains("time <= '2022-01-20T12:00:00+00:00'"));
        assert!(sql.contains("LIMIT 100"));
    }

    #[test]
    fn test_empty_query() {
        let query = AlarmHistoryQuery {
            alarm_id: None,
            level: None,
            status: None,
            start_time: None,
            end_time: None,
            limit: None,
        };

        let sql = query.to_sql();
        assert_eq!(sql, "SELECT * FROM alarms  ORDER BY time DESC ");
    }

    #[tokio::test]
    async fn test_storage_creation() {
        let config = InfluxDBConfig::default();
        let client = InfluxDBClient::new(config);
        let storage = AlarmHistoryStorage::new(client);
        
        assert_eq!(storage.flush_interval, Duration::from_secs(30));
    }
}