use anyhow::Result;
use chrono::{DateTime, Utc};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use crate::domain::{Alarm, AlarmLevel, AlarmStatus};
use crate::redis::{AlarmIndexManager, AlarmRedisClient};

/// Filter criteria for alarm queries
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AlarmFilter {
    pub category: Option<String>,
    pub level: Option<AlarmLevel>,
    pub status: Option<AlarmStatus>,
    pub keyword: Option<String>,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

/// Handles alarm queries from Redis
pub struct AlarmQueryService {
    client: Arc<AlarmRedisClient>,
    index_manager: Arc<AlarmIndexManager>,
}

impl AlarmQueryService {
    /// Create new query service
    pub fn new(client: Arc<AlarmRedisClient>) -> Self {
        let index_manager = Arc::new(AlarmIndexManager::new(client.clone()));
        Self {
            client,
            index_manager,
        }
    }

    /// Query alarms with advanced filtering
    pub async fn query(&self, filter: &AlarmFilter) -> Result<Vec<Alarm>> {
        self.get_alarms(
            filter.category.clone(),
            filter.level,
            filter.status,
            filter.limit,
        )
        .await
    }

    /// Get alarms with basic filtering
    pub async fn get_alarms(
        &self,
        category: Option<String>,
        level: Option<AlarmLevel>,
        status: Option<AlarmStatus>,
        limit: Option<usize>,
    ) -> Result<Vec<Alarm>> {
        let alarm_ids = self
            .index_manager
            .get_alarm_ids(category, level, status)
            .await?;

        let mut alarms = Vec::new();
        let mut client_guard = self.client.get_client().await?;

        if let Some(conn) = client_guard.as_mut() {
            // Apply limit
            let limited_ids = if let Some(limit_val) = limit {
                alarm_ids.into_iter().take(limit_val).collect()
            } else {
                alarm_ids
            };

            // Fetch alarm data
            for alarm_id in limited_ids {
                let alarm_key = format!("alarmsrv:{}", alarm_id);
                if let Ok(alarm_data) = conn
                    .get_connection_mut()
                    .hget::<_, _, String>(&alarm_key, "data")
                    .await
                {
                    if let Ok(alarm) = serde_json::from_str::<Alarm>(&alarm_data) {
                        alarms.push(alarm);
                    }
                }
            }
        }

        Ok(alarms)
    }

    /// Get alarms with pagination and advanced filtering
    pub async fn get_alarms_paginated(
        &self,
        category: Option<String>,
        level: Option<AlarmLevel>,
        status: Option<AlarmStatus>,
        start_time: Option<String>,
        end_time: Option<String>,
        keyword: Option<String>,
        offset: usize,
        limit: usize,
    ) -> Result<(Vec<Alarm>, usize)> {
        let alarm_ids = self
            .index_manager
            .get_alarm_ids(category, level, status)
            .await?;

        let mut all_alarms = Vec::new();
        let mut client_guard = self.client.get_client().await?;

        if let Some(conn) = client_guard.as_mut() {
            // Fetch all alarm data for filtering
            for alarm_id in alarm_ids {
                let alarm_key = format!("alarmsrv:{}", alarm_id);
                if let Ok(alarm_data) = conn
                    .get_connection_mut()
                    .hget::<_, _, String>(&alarm_key, "data")
                    .await
                {
                    if let Ok(alarm) = serde_json::from_str::<Alarm>(&alarm_data) {
                        // Apply time filter
                        let mut include = true;

                        if let Some(ref start) = start_time {
                            if let Ok(start_dt) = DateTime::parse_from_rfc3339(start) {
                                if alarm.created_at < start_dt.with_timezone(&Utc) {
                                    include = false;
                                }
                            }
                        }

                        if let Some(ref end) = end_time {
                            if let Ok(end_dt) = DateTime::parse_from_rfc3339(end) {
                                if alarm.created_at > end_dt.with_timezone(&Utc) {
                                    include = false;
                                }
                            }
                        }

                        // Apply keyword filter
                        if let Some(ref kw) = keyword {
                            let kw_lower = kw.to_lowercase();
                            if !alarm.title.to_lowercase().contains(&kw_lower)
                                && !alarm.description.to_lowercase().contains(&kw_lower)
                            {
                                include = false;
                            }
                        }

                        if include {
                            all_alarms.push(alarm);
                        }
                    }
                }
            }

            // Sort by created_at (newest first)
            all_alarms.sort_by(|a, b| b.created_at.cmp(&a.created_at));

            // Get total count
            let total = all_alarms.len();

            // Apply pagination
            let start = offset.min(total);
            let end = (start + limit).min(total);
            let paginated_alarms = all_alarms[start..end].to_vec();

            return Ok((paginated_alarms, total));
        }

        Ok((vec![], 0))
    }

    /// Get unclassified alarms
    pub async fn get_unclassified_alarms(&self) -> Result<Vec<Alarm>> {
        let alarm_ids = self.index_manager.get_unclassified_alarm_ids().await?;
        let mut alarms = Vec::new();
        let mut client_guard = self.client.get_client().await?;

        if let Some(conn) = client_guard.as_mut() {
            for alarm_id in alarm_ids {
                let alarm_key = format!("alarmsrv:{}", alarm_id);
                if let Ok(alarm_data) = conn
                    .get_connection_mut()
                    .hget::<_, _, String>(&alarm_key, "data")
                    .await
                {
                    if let Ok(alarm) = serde_json::from_str::<Alarm>(&alarm_data) {
                        alarms.push(alarm);
                    }
                }
            }
        }

        Ok(alarms)
    }

    /// Get recent alarms from realtime Hash with O(1) complexity
    pub async fn get_recent_alarms(
        &self,
        channel: Option<&str>,
        limit: usize,
    ) -> Result<Vec<Alarm>> {
        let mut client_guard = self.client.get_client().await?;
        let mut alarms = Vec::new();

        if let Some(conn) = client_guard.as_mut() {
            let realtime_key = "alarmsrv:realtime";
            let all_fields: HashMap<String, String> =
                conn.get_connection_mut().hgetall(&realtime_key).await?;

            // Filter by channel if specified
            let filtered: Vec<(String, String)> = if let Some(ch) = channel {
                all_fields
                    .into_iter()
                    .filter(|(field, _)| field.starts_with(&format!("{}:", ch)))
                    .collect()
            } else {
                all_fields.into_iter().collect()
            };

            // Parse alarm data and sort by created_at
            let mut alarm_data: Vec<(DateTime<Utc>, serde_json::Value)> = Vec::new();
            for (_, value) in filtered {
                if let Ok(data) = serde_json::from_str::<serde_json::Value>(&value) {
                    if let Some(created_at_str) = data["created_at"].as_str() {
                        if let Ok(created_at) = DateTime::parse_from_rfc3339(created_at_str) {
                            alarm_data.push((created_at.with_timezone(&Utc), data));
                        }
                    }
                }
            }

            // Sort by time (newest first) and take limit
            alarm_data.sort_by(|a, b| b.0.cmp(&a.0));
            alarm_data.truncate(limit);

            // Fetch full alarm data for the selected items
            for (_, data) in alarm_data {
                if let Some(id) = data["id"].as_str() {
                    let alarm_key = format!("alarmsrv:{}", id);
                    if let Ok(alarm_json) = conn
                        .get_connection_mut()
                        .hget::<_, _, String>(&alarm_key, "data")
                        .await
                    {
                        if let Ok(alarm) = serde_json::from_str::<Alarm>(&alarm_json) {
                            alarms.push(alarm);
                        }
                    }
                }
            }
        }

        Ok(alarms)
    }

    /// Query alarms by time range using time-based shards
    pub async fn get_alarms_by_time_range(
        &self,
        start_time: DateTime<Utc>,
        end_time: DateTime<Utc>,
        filters: Option<AlarmQueryFilters>,
    ) -> Result<Vec<Alarm>> {
        let mut client_guard = self.client.get_client().await?;
        let mut alarms = Vec::new();

        if let Some(conn) = client_guard.as_mut() {
            // Calculate hour buckets to query
            let mut current = start_time;
            let mut buckets = Vec::new();

            while current <= end_time {
                buckets.push(current.format("%Y%m%d%H").to_string());
                current = current + chrono::Duration::hours(1);
            }

            // Query each bucket
            for bucket in buckets {
                let bucket_index_key = format!("alarmsrv:buckets:{}", bucket);
                if let Ok(alarm_ids) = conn
                    .get_connection_mut()
                    .smembers::<_, Vec<String>>(&bucket_index_key)
                    .await
                {
                    for alarm_id in alarm_ids {
                        let alarm_key = format!("alarmsrv:shard:{}:{}", bucket, alarm_id);
                        if let Ok(alarm_json) = conn
                            .get_connection_mut()
                            .hget::<_, _, String>(&alarm_key, "data")
                            .await
                        {
                            if let Ok(alarm) = serde_json::from_str::<Alarm>(&alarm_json) {
                                // Apply filters if provided
                                if let Some(ref f) = filters {
                                    if !f.matches(&alarm) {
                                        continue;
                                    }
                                }
                                // Check if alarm is within exact time range
                                if alarm.created_at >= start_time && alarm.created_at <= end_time {
                                    alarms.push(alarm);
                                }
                            }
                        }
                    }
                }
            }

            // Sort by created_at descending
            alarms.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        }

        Ok(alarms)
    }
}

/// Query filters for alarms
#[derive(Debug, Clone)]
pub struct AlarmQueryFilters {
    pub level: Option<AlarmLevel>,
    pub status: Option<AlarmStatus>,
    pub category: Option<String>,
    pub keyword: Option<String>,
}

impl AlarmQueryFilters {
    pub fn matches(&self, alarm: &Alarm) -> bool {
        if let Some(ref lvl) = self.level {
            if &alarm.level != lvl {
                return false;
            }
        }
        if let Some(ref st) = self.status {
            if &alarm.status != st {
                return false;
            }
        }
        if let Some(ref cat) = self.category {
            // For now, we use general category
            if cat != "general" {
                return false;
            }
        }
        if let Some(ref kw) = self.keyword {
            let kw_lower = kw.to_lowercase();
            if !alarm.description.to_lowercase().contains(&kw_lower)
                && !alarm.title.to_lowercase().contains(&kw_lower)
            {
                return false;
            }
        }
        true
    }
}
