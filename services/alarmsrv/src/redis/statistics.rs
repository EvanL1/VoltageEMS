use anyhow::Result;
use chrono::Utc;
use redis::AsyncCommands;
use std::collections::HashMap;
use std::sync::Arc;
use voltage_libs::redis::RedisClient;

use crate::domain::{Alarm, AlarmLevelStats, AlarmStatistics, AlarmStatus, AlarmStatusStats};
use crate::redis::AlarmRedisClient;

/// Manages alarm statistics in Redis
pub struct AlarmStatisticsManager {
    client: Arc<AlarmRedisClient>,
}

impl AlarmStatisticsManager {
    /// Create new statistics manager
    pub fn new(client: Arc<AlarmRedisClient>) -> Self {
        Self { client }
    }

    /// Update statistics based on alarm action
    pub async fn update_statistics(
        &self,
        conn: &mut RedisClient,
        alarm: &Alarm,
        action: &str,
    ) -> Result<()> {
        let stats_key = "ems:alarms:stats";

        match action {
            "created" => {
                conn.get_connection_mut()
                    .hincr(&stats_key, "total", 1)
                    .await?;
                conn.get_connection_mut()
                    .hincr(&stats_key, "new", 1)
                    .await?;
                conn.get_connection_mut()
                    .hincr(&stats_key, &format!("{:?}", alarm.level).to_lowercase(), 1)
                    .await?;
            }
            "acknowledged" => {
                conn.get_connection_mut()
                    .hincr(&stats_key, "new", -1)
                    .await?;
                conn.get_connection_mut()
                    .hincr(&stats_key, "acknowledged", 1)
                    .await?;

                // Update today's handled count
                let today = Utc::now().format("%Y-%m-%d").to_string();
                let today_handled_key = format!("ems:alarms:handled:{}", today);
                conn.get_connection_mut()
                    .incr(&today_handled_key, 1)
                    .await?;
                // Set expiration to 7 days
                conn.expire(&today_handled_key, 7 * 24 * 3600).await?;
            }
            "resolved" => {
                if alarm.status == AlarmStatus::Acknowledged {
                    conn.get_connection_mut()
                        .hincr(&stats_key, "acknowledged", -1)
                        .await?;
                } else if alarm.status == AlarmStatus::New {
                    conn.get_connection_mut()
                        .hincr(&stats_key, "new", -1)
                        .await?;
                }
                conn.get_connection_mut()
                    .hincr(&stats_key, "resolved", 1)
                    .await?;

                // Update today's handled count
                let today = Utc::now().format("%Y-%m-%d").to_string();
                let today_handled_key = format!("ems:alarms:handled:{}", today);
                conn.get_connection_mut()
                    .incr(&today_handled_key, 1)
                    .await?;
                // Set expiration to 7 days
                conn.expire(&today_handled_key, 7 * 24 * 3600).await?;
            }
            "escalated" => {
                conn.get_connection_mut()
                    .hincr(&stats_key, &format!("{:?}", alarm.level).to_lowercase(), 1)
                    .await?;
            }
            _ => {}
        }

        Ok(())
    }

    /// Get alarm statistics
    pub async fn get_alarm_statistics(&self) -> Result<AlarmStatistics> {
        let mut client_guard = self.client.get_client().await?;
        if let Some(conn) = client_guard.as_mut() {
            let stats_key = "ems:alarms:stats";

            let total: i32 = conn
                .get_connection_mut()
                .hget(&stats_key, "total")
                .await
                .ok()
                .flatten()
                .and_then(|s: String| s.parse().ok())
                .unwrap_or(0);
            let new: i32 = conn
                .get_connection_mut()
                .hget(&stats_key, "new")
                .await
                .ok()
                .flatten()
                .and_then(|s: String| s.parse().ok())
                .unwrap_or(0);
            let acknowledged: i32 = conn
                .get_connection_mut()
                .hget(&stats_key, "acknowledged")
                .await
                .ok()
                .flatten()
                .and_then(|s: String| s.parse().ok())
                .unwrap_or(0);
            let resolved: i32 = conn
                .get_connection_mut()
                .hget(&stats_key, "resolved")
                .await
                .ok()
                .flatten()
                .and_then(|s: String| s.parse().ok())
                .unwrap_or(0);
            let critical: i32 = conn
                .get_connection_mut()
                .hget(&stats_key, "critical")
                .await
                .ok()
                .flatten()
                .and_then(|s: String| s.parse().ok())
                .unwrap_or(0);
            let major: i32 = conn
                .get_connection_mut()
                .hget(&stats_key, "major")
                .await
                .ok()
                .flatten()
                .and_then(|s: String| s.parse().ok())
                .unwrap_or(0);
            let minor: i32 = conn
                .get_connection_mut()
                .hget(&stats_key, "minor")
                .await
                .ok()
                .flatten()
                .and_then(|s: String| s.parse().ok())
                .unwrap_or(0);
            let warning: i32 = conn
                .get_connection_mut()
                .hget(&stats_key, "warning")
                .await
                .ok()
                .flatten()
                .and_then(|s: String| s.parse().ok())
                .unwrap_or(0);
            let info: i32 = conn
                .get_connection_mut()
                .hget(&stats_key, "info")
                .await
                .ok()
                .flatten()
                .and_then(|s: String| s.parse().ok())
                .unwrap_or(0);

            // Get category statistics
            let categories = self.get_category_statistics(conn).await?;

            // Get today's handled alarms count
            let today = Utc::now().format("%Y-%m-%d").to_string();
            let today_handled_key = format!("ems:alarms:handled:{}", today);
            let today_handled: i32 = conn
                .get(&today_handled_key)
                .await
                .ok()
                .flatten()
                .and_then(|s: String| s.parse().ok())
                .unwrap_or(0);

            // Calculate active alarms
            let active = (new + acknowledged) as usize;

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
                today_handled: today_handled as usize,
                active,
            });
        }

        Err(anyhow::anyhow!("Failed to get statistics"))
    }

    /// Get category statistics
    async fn get_category_statistics(
        &self,
        conn: &mut RedisClient,
    ) -> Result<HashMap<String, usize>> {
        let mut categories = HashMap::new();
        let pattern = "ems:alarms:category:*";
        let keys: Vec<String> = conn.keys(pattern).await?;

        for key in keys {
            let category = key.replace("ems:alarms:category:", "");
            let count: usize = conn.get_connection_mut().scard(&key).await.unwrap_or(0);
            categories.insert(category, count);
        }

        Ok(categories)
    }
}
