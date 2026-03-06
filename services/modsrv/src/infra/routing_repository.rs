//! Routing Repository
//!
//! Trait and SQLite implementation for routing persistence.
//! Extracts routing SQL operations from InstanceManager for testability.

use std::collections::HashMap;

use anyhow::Result;
use async_trait::async_trait;
use sqlx::SqlitePool;
use voltage_model::KeySpaceConfig;

use crate::routing_loader::{ActionRouting, MeasurementRouting};

/// Routing persistence operations
#[async_trait]
pub trait RoutingRepository: Send + Sync {
    /// Upsert a measurement routing entry
    async fn upsert_measurement(
        &self,
        instance_id: u32,
        instance_name: &str,
        point_id: u32,
        channel_id: Option<i32>,
        channel_type: Option<&str>,
        channel_point_id: Option<u32>,
        enabled: bool,
    ) -> Result<()>;

    /// Upsert an action routing entry
    async fn upsert_action(
        &self,
        instance_id: u32,
        instance_name: &str,
        point_id: u32,
        channel_id: Option<i32>,
        channel_type: Option<&str>,
        channel_point_id: Option<u32>,
        enabled: bool,
    ) -> Result<()>;

    /// Delete a measurement routing entry, returns rows affected
    async fn delete_measurement(&self, instance_id: u32, point_id: u32) -> Result<u64>;

    /// Delete an action routing entry, returns rows affected
    async fn delete_action(&self, instance_id: u32, point_id: u32) -> Result<u64>;

    /// Toggle measurement routing enabled state
    async fn toggle_measurement(
        &self,
        instance_id: u32,
        point_id: u32,
        enabled: bool,
    ) -> Result<u64>;

    /// Toggle action routing enabled state
    async fn toggle_action(&self, instance_id: u32, point_id: u32, enabled: bool) -> Result<u64>;

    /// Get enabled measurement routing for an instance
    async fn get_measurements(&self, instance_id: u32) -> Result<Vec<MeasurementRouting>>;

    /// Get enabled action routing for an instance
    async fn get_actions(&self, instance_id: u32) -> Result<Vec<ActionRouting>>;

    /// Delete all routing for an instance, returns (measurement_count, action_count)
    async fn delete_all(&self, instance_id: u32) -> Result<(u64, u64)>;

    /// Load all routing maps in batch for Redis sync
    /// Returns (measurement_routings, action_routings) keyed by instance_id
    async fn load_all_routings_batch(
        &self,
    ) -> Result<(
        HashMap<u32, HashMap<u32, String>>,
        HashMap<u32, HashMap<u32, String>>,
    )>;

    /// Fetch routing maps for a single instance (for Redis key generation)
    async fn fetch_routing_maps(
        &self,
        instance_id: u32,
    ) -> Result<(HashMap<u32, String>, HashMap<u32, String>)>;
}

/// SQLite implementation of RoutingRepository
pub struct SqliteRoutingRepository {
    pool: SqlitePool,
}

impl SqliteRoutingRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
#[allow(clippy::too_many_arguments)]
impl RoutingRepository for SqliteRoutingRepository {
    async fn upsert_measurement(
        &self,
        instance_id: u32,
        instance_name: &str,
        point_id: u32,
        channel_id: Option<i32>,
        channel_type: Option<&str>,
        channel_point_id: Option<u32>,
        enabled: bool,
    ) -> Result<()> {
        sqlx::query(
            r#"INSERT INTO measurement_routing
               (instance_id, instance_name, channel_id, channel_type, channel_point_id,
                measurement_id, enabled)
               VALUES (?, ?, ?, ?, ?, ?, ?)
               ON CONFLICT(instance_id, measurement_id)
               DO UPDATE SET
                   channel_id = excluded.channel_id,
                   channel_type = excluded.channel_type,
                   channel_point_id = excluded.channel_point_id,
                   enabled = excluded.enabled,
                   updated_at = CURRENT_TIMESTAMP"#,
        )
        .bind(instance_id as i64)
        .bind(instance_name)
        .bind(channel_id)
        .bind(channel_type)
        .bind(channel_point_id)
        .bind(point_id)
        .bind(enabled)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn upsert_action(
        &self,
        instance_id: u32,
        instance_name: &str,
        point_id: u32,
        channel_id: Option<i32>,
        channel_type: Option<&str>,
        channel_point_id: Option<u32>,
        enabled: bool,
    ) -> Result<()> {
        sqlx::query(
            r#"INSERT INTO action_routing
               (instance_id, instance_name, action_id, channel_id, channel_type,
                channel_point_id, enabled)
               VALUES (?, ?, ?, ?, ?, ?, ?)
               ON CONFLICT(instance_id, action_id)
               DO UPDATE SET
                   channel_id = excluded.channel_id,
                   channel_type = excluded.channel_type,
                   channel_point_id = excluded.channel_point_id,
                   enabled = excluded.enabled,
                   updated_at = CURRENT_TIMESTAMP"#,
        )
        .bind(instance_id as i64)
        .bind(instance_name)
        .bind(point_id)
        .bind(channel_id)
        .bind(channel_type)
        .bind(channel_point_id)
        .bind(enabled)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn delete_measurement(&self, instance_id: u32, point_id: u32) -> Result<u64> {
        let result = sqlx::query(
            "DELETE FROM measurement_routing WHERE instance_id = ? AND measurement_id = ?",
        )
        .bind(instance_id as i64)
        .bind(point_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    async fn delete_action(&self, instance_id: u32, point_id: u32) -> Result<u64> {
        let result =
            sqlx::query("DELETE FROM action_routing WHERE instance_id = ? AND action_id = ?")
                .bind(instance_id as i64)
                .bind(point_id)
                .execute(&self.pool)
                .await?;
        Ok(result.rows_affected())
    }

    async fn toggle_measurement(
        &self,
        instance_id: u32,
        point_id: u32,
        enabled: bool,
    ) -> Result<u64> {
        let result = sqlx::query(
            r#"UPDATE measurement_routing
               SET enabled = ?, updated_at = CURRENT_TIMESTAMP
               WHERE instance_id = ? AND measurement_id = ?"#,
        )
        .bind(enabled)
        .bind(instance_id as i64)
        .bind(point_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    async fn toggle_action(&self, instance_id: u32, point_id: u32, enabled: bool) -> Result<u64> {
        let result = sqlx::query(
            r#"UPDATE action_routing
               SET enabled = ?, updated_at = CURRENT_TIMESTAMP
               WHERE instance_id = ? AND action_id = ?"#,
        )
        .bind(enabled)
        .bind(instance_id as i64)
        .bind(point_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    async fn get_measurements(&self, instance_id: u32) -> Result<Vec<MeasurementRouting>> {
        let routing = sqlx::query_as::<_, MeasurementRouting>(
            r#"SELECT * FROM measurement_routing
               WHERE instance_id = ? AND enabled = TRUE
               ORDER BY channel_id, channel_type, channel_point_id"#,
        )
        .bind(instance_id as i64)
        .fetch_all(&self.pool)
        .await?;
        Ok(routing)
    }

    async fn get_actions(&self, instance_id: u32) -> Result<Vec<ActionRouting>> {
        let routing = sqlx::query_as::<_, ActionRouting>(
            r#"SELECT * FROM action_routing
               WHERE instance_id = ? AND enabled = TRUE
               ORDER BY action_id"#,
        )
        .bind(instance_id as i64)
        .fetch_all(&self.pool)
        .await?;
        Ok(routing)
    }

    async fn delete_all(&self, instance_id: u32) -> Result<(u64, u64)> {
        let m = sqlx::query("DELETE FROM measurement_routing WHERE instance_id = ?")
            .bind(instance_id as i64)
            .execute(&self.pool)
            .await?;

        let a = sqlx::query("DELETE FROM action_routing WHERE instance_id = ?")
            .bind(instance_id as i64)
            .execute(&self.pool)
            .await?;

        Ok((m.rows_affected(), a.rows_affected()))
    }

    async fn load_all_routings_batch(
        &self,
    ) -> Result<(
        HashMap<u32, HashMap<u32, String>>,
        HashMap<u32, HashMap<u32, String>>,
    )> {
        let all_measurements: Vec<(i32, i32)> =
            sqlx::query_as("SELECT instance_id, measurement_id FROM measurement_routing")
                .fetch_all(&self.pool)
                .await?;

        let all_actions: Vec<(i32, i32)> =
            sqlx::query_as("SELECT instance_id, action_id FROM action_routing")
                .fetch_all(&self.pool)
                .await?;

        let ks = KeySpaceConfig::production_cached();
        let mut ibuf = itoa::Buffer::new();

        let mut measurement_routings: HashMap<u32, HashMap<u32, String>> = HashMap::new();
        for (instance_id, point_id) in all_measurements {
            let instance_id = instance_id as u32;
            let point_id = point_id as u32;
            let redis_key = ks.instance_measurement_point_key(instance_id, ibuf.format(point_id));
            measurement_routings
                .entry(instance_id)
                .or_default()
                .insert(point_id, redis_key);
        }

        let mut action_routings: HashMap<u32, HashMap<u32, String>> = HashMap::new();
        for (instance_id, point_id) in all_actions {
            let instance_id = instance_id as u32;
            let point_id = point_id as u32;
            let redis_key = ks.instance_action_point_key(instance_id, ibuf.format(point_id));
            action_routings
                .entry(instance_id)
                .or_default()
                .insert(point_id, redis_key);
        }

        Ok((measurement_routings, action_routings))
    }

    async fn fetch_routing_maps(
        &self,
        instance_id: u32,
    ) -> Result<(HashMap<u32, String>, HashMap<u32, String>)> {
        let ks = KeySpaceConfig::production_cached();

        let measurement_points: Vec<(u32,)> =
            sqlx::query_as("SELECT measurement_id FROM measurement_routing WHERE instance_id = ?")
                .bind(instance_id)
                .fetch_all(&self.pool)
                .await?;

        let measurement_map: HashMap<u32, String> = measurement_points
            .into_iter()
            .map(|(id,)| {
                let key = ks.instance_measurement_point_key(instance_id, &id.to_string());
                (id, key)
            })
            .collect();

        let action_points: Vec<(u32,)> =
            sqlx::query_as("SELECT action_id FROM action_routing WHERE instance_id = ?")
                .bind(instance_id)
                .fetch_all(&self.pool)
                .await?;

        let action_map: HashMap<u32, String> = action_points
            .into_iter()
            .map(|(id,)| {
                let key = ks.instance_action_point_key(instance_id, &id.to_string());
                (id, key)
            })
            .collect();

        Ok((measurement_map, action_map))
    }
}
