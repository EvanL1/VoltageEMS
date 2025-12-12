//! Channel-Instance Point Routing Loader
//! Loads routing configurations from CSV files
//!
//! CSV source uses a single file: `channel_mappings.csv` (defines both T/S → M and A → C/A)
//!
//! **DEPRECATED**: RoutingLoader functionality has been migrated to InstanceManager.
//! This module is kept for reference during migration and will be removed in a future version.

#![allow(dead_code)]

use anyhow::{Context, Result};
use csv::ReaderBuilder;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{debug, info, warn};
use voltage_config::{
    common::{ValidationLevel, ValidationResult},
    protocols::PointType,
    FourRemote, KeySpaceConfig,
};
use voltage_rtdb::Rtdb;

use crate::redis_state::{self, RoutingEntry};

/// CSV row structure for measurement routing (T/S → M)
///
/// `channel_id`, `channel_type`, and `channel_point_id` form a unit - all None means unbound
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeasurementRoutingRow {
    pub channel_id: Option<i32>,
    pub channel_type: Option<FourRemote>, // T or S only, None if unbound
    pub channel_point_id: Option<u32>,
    pub measurement_id: u32,
}

/// CSV row structure for action routing (A → C/A)
///
/// `channel_id`, `channel_type`, and `channel_point_id` form a unit - all None means unbound
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionRoutingRow {
    pub action_id: u32,
    pub channel_id: Option<i32>,
    pub channel_type: Option<FourRemote>, // C or A only, None if unbound
    pub channel_point_id: Option<u32>,
}

/// Measurement routing record from database
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct MeasurementRouting {
    pub routing_id: i32,
    pub instance_id: u16,
    pub instance_name: String,
    pub channel_id: Option<i32>,
    pub channel_type: Option<String>,
    pub channel_point_id: Option<u32>,
    pub measurement_id: u32,
    pub description: Option<String>,
    pub enabled: bool,
}

/// Action routing record from database
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ActionRouting {
    pub routing_id: i32,
    pub instance_id: u16,
    pub instance_name: String,
    pub action_id: u32,
    pub channel_id: Option<i32>,
    pub channel_type: Option<String>,
    pub channel_point_id: Option<u32>,
    pub description: Option<String>,
    pub enabled: bool,
}

/// Routing loader for channel-instance point routing
pub struct RoutingLoader {
    pool: SqlitePool,
    instances_dir: PathBuf,
}

impl RoutingLoader {
    pub fn new(instances_dir: impl Into<PathBuf>, pool: SqlitePool) -> Self {
        Self {
            pool,
            instances_dir: instances_dir.into(),
        }
    }

    /// Load all instance routing from CSV files
    pub async fn load_all_routing(&self) -> Result<()> {
        debug!("Loading routing: {:?}", self.instances_dir);

        if !self.instances_dir.exists() {
            debug!("Instances directory does not exist, skipping routing load");
            return Ok(());
        }

        // Scan for instance directories
        let mut entries = fs::read_dir(&self.instances_dir).await?;
        let mut measurement_count = 0;
        let mut action_count = 0;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_dir() {
                let instance_name = entry.file_name().to_string_lossy().to_string();

                // Prefer unified channel_mappings.csv
                let unified_csv = path.join("channel_mappings.csv");
                if unified_csv.exists() {
                    match self
                        .load_channel_mappings(&instance_name, &unified_csv)
                        .await
                    {
                        Ok((m, a)) => {
                            debug!("{}: {}M {}A", instance_name, m, a);
                            measurement_count += m;
                            action_count += a;
                        },
                        Err(e) => {
                            warn!("{} mapping err: {}", instance_name, e);
                        },
                    }
                }
            }
        }

        info!("Routing: {}M {}A", measurement_count, action_count);
        Ok(())
    }

    /// Load unified channel mappings for a specific instance from CSV
    async fn load_channel_mappings(
        &self,
        instance_name: &str,
        csv_path: &Path,
    ) -> Result<(usize, usize)> {
        let content = fs::read_to_string(csv_path).await?;
        let mut rdr = ReaderBuilder::new()
            .has_headers(true)
            .from_reader(content.as_bytes());

        let mut tx = self.pool.begin().await?;
        let mut m_count = 0usize;
        let mut a_count = 0usize;

        // Get instance_id from instance_name
        let instance_id: Option<u16> =
            sqlx::query_scalar("SELECT instance_id FROM instances WHERE instance_name = ?")
                .bind(instance_name)
                .fetch_optional(&mut *tx)
                .await?;

        let Some(instance_id) = instance_id else {
            warn!("Instance '{}' not found", instance_name);
            return Ok((0, 0));
        };

        for result in rdr.deserialize::<ChannelMappingRow>() {
            let row = result.context("Failed to parse CSV row")?;

            let inst_type = row.instance_type.trim().to_uppercase();
            if inst_type == "M" || inst_type == "MEASUREMENT" {
                // Validate channel_type
                if !row.channel_type.is_input() {
                    warn!("Invalid M channel_type: {}", row.channel_type);
                    continue;
                }

                sqlx::query(
                    r#"
                    INSERT INTO measurement_routing
                    (instance_id, instance_name, channel_id, channel_type, channel_point_id,
                     measurement_id)
                    VALUES (?, ?, ?, ?, ?, ?)
                    ON CONFLICT(instance_id, measurement_id)
                    DO UPDATE SET
                        channel_id = excluded.channel_id,
                        channel_type = excluded.channel_type,
                        channel_point_id = excluded.channel_point_id,
                        updated_at = CURRENT_TIMESTAMP
                    "#,
                )
                .bind(instance_id)
                .bind(instance_name)
                .bind(row.channel_id)
                .bind(row.channel_type.as_str())
                .bind(row.channel_point_id)
                .bind(row.instance_point_id)
                .execute(&mut *tx)
                .await?;
                m_count += 1;
            } else if inst_type == "A" || inst_type == "ACTION" {
                // Validate channel_type
                if !row.channel_type.is_output() {
                    warn!("Invalid A channel_type: {}", row.channel_type);
                    continue;
                }

                sqlx::query(
                    r#"
                    INSERT INTO action_routing
                    (instance_id, instance_name, action_id, channel_id, channel_type,
                     channel_point_id)
                    VALUES (?, ?, ?, ?, ?, ?)
                    ON CONFLICT(instance_id, action_id)
                    DO UPDATE SET
                        channel_id = excluded.channel_id,
                        channel_type = excluded.channel_type,
                        channel_point_id = excluded.channel_point_id,
                        updated_at = CURRENT_TIMESTAMP
                    "#,
                )
                .bind(instance_id)
                .bind(instance_name)
                .bind(row.instance_point_id)
                .bind(row.channel_id)
                .bind(row.channel_type.as_str())
                .bind(row.channel_point_id)
                .execute(&mut *tx)
                .await?;
                a_count += 1;
            } else {
                warn!("Unknown type '{}' (expected M/A)", row.instance_type);
                continue;
            }
        }

        tx.commit().await?;
        Ok((m_count, a_count))
    }

    // Removed legacy loaders for split CSV files

    /// Get all measurement routing for an instance
    pub async fn get_measurement_routing(
        &self,
        instance_name: &str,
    ) -> Result<Vec<MeasurementRouting>> {
        let routing = sqlx::query_as::<_, MeasurementRouting>(
            r#"
            SELECT * FROM measurement_routing
            WHERE instance_name = ? AND enabled = TRUE
            ORDER BY channel_id, channel_type, channel_point_id
            "#,
        )
        .bind(instance_name)
        .fetch_all(&self.pool)
        .await?;

        Ok(routing)
    }

    /// Get all action routing for an instance
    pub async fn get_action_routing(&self, instance_name: &str) -> Result<Vec<ActionRouting>> {
        let routing = sqlx::query_as::<_, ActionRouting>(
            r#"
            SELECT * FROM action_routing
            WHERE instance_name = ? AND enabled = TRUE
            ORDER BY action_id
            "#,
        )
        .bind(instance_name)
        .fetch_all(&self.pool)
        .await?;

        Ok(routing)
    }

    /// Sync all routing to Redis for fast runtime lookup
    pub async fn sync_routing_to_redis<R>(&self, redis: &R) -> Result<()>
    where
        R: Rtdb + ?Sized,
    {
        debug!("Syncing routing to Redis");

        // Clear existing routing first
        redis_state::clear_routing(redis).await?;
        debug!("Cleared existing routing tables");

        // Fetch all enabled measurement routing
        let measurement_routing = sqlx::query_as::<_, (u32, String, u32, String, u32, u32)>(
            r#"
            SELECT
                instance_id, instance_name, channel_id, channel_type, channel_point_id,
                measurement_id
            FROM measurement_routing
            WHERE enabled = TRUE
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        // Fetch all enabled action routing
        let action_routing = sqlx::query_as::<_, (u32, String, u32, u32, String, u32)>(
            r#"
            SELECT
                instance_id, instance_name, action_id, channel_id, channel_type,
                channel_point_id
            FROM action_routing
            WHERE enabled = TRUE
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        if measurement_routing.is_empty() && action_routing.is_empty() {
            debug!("No routing to sync");
            return Ok(());
        }

        // Build batch data for Redis
        let mut batch = Vec::new();
        let keyspace = KeySpaceConfig::production();

        // Add measurement routing (uplink: channel → instance)
        for (
            instance_id,
            _instance_name,
            channel_id,
            channel_type,
            channel_point_id,
            measurement_id,
        ) in measurement_routing
        {
            // Parse channel_type string to PointType (FourRemote is an alias)
            let point_type: PointType =
                serde_json::from_value(serde_json::Value::String(channel_type))
                    .with_context(|| "Invalid channel_type")?;

            batch.push(RoutingEntry {
                // C2M route key: {channel_id}:{type}:{point_id}
                comsrv_key: keyspace
                    .c2m_route_key(channel_id, point_type, &channel_point_id.to_string())
                    .to_string(),
                // Target: {instance_id}:M:{point_id} (M is PointRole, not PointType)
                modsrv_key: format!("{}:M:{}", instance_id, measurement_id),
                is_action: false,
            });
        }

        // Add action routing (downlink: instance → channel)
        for (instance_id, _instance_name, action_id, channel_id, channel_type, channel_point_id) in
            action_routing
        {
            // Parse channel_type string to PointType (FourRemote is an alias)
            let point_type: PointType =
                serde_json::from_value(serde_json::Value::String(channel_type))
                    .with_context(|| "Invalid channel_type")?;

            batch.push(RoutingEntry {
                // M2C route key: {instance_id}:A:{point_id}
                modsrv_key: keyspace
                    .m2c_route_key(instance_id, PointType::Adjustment, &action_id.to_string())
                    .to_string(),
                // Target: {channel_id}:{type}:{point_id}
                comsrv_key: keyspace
                    .c2m_route_key(channel_id, point_type, &channel_point_id.to_string())
                    .to_string(),
                is_action: true,
            });
        }

        redis_state::store_routing(redis, &batch).await?;

        info!("Synced {} routes", batch.len());
        Ok(())
    }

    /// Validate a measurement routing entry
    pub async fn validate_measurement_routing(
        &self,
        routing: &MeasurementRoutingRow,
        instance_name: &str,
    ) -> Result<ValidationResult> {
        let mut errors = Vec::new();

        // Validate instance exists
        let instance_exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM instances WHERE instance_name = ?)",
        )
        .bind(instance_name)
        .fetch_one(&self.pool)
        .await?;

        if !instance_exists {
            errors.push(format!("Instance {} does not exist", instance_name));
        }

        // Validate channel_type (skip if None - unbound routing is valid)
        if let Some(ref ct) = routing.channel_type {
            if !ct.is_input() {
                errors.push(format!(
                    "Invalid channel_type for measurement: {}. Must be T or S",
                    ct
                ));
            }
        }

        // Validate measurement point exists
        let point_exists = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM measurement_points mp
                JOIN instances i ON i.product_name = mp.product_name
                WHERE i.instance_name = ? AND mp.measurement_id = ?
            )
            "#,
        )
        .bind(instance_name)
        .bind(routing.measurement_id)
        .fetch_one(&self.pool)
        .await?;

        if !point_exists {
            errors.push(format!(
                "Measurement point {} not found for instance {}",
                routing.measurement_id, instance_name
            ));
        }

        let mut result = ValidationResult::new(ValidationLevel::Business);
        for error in errors {
            result.add_error(error);
        }
        Ok(result)
    }

    /// Validate an action routing entry
    pub async fn validate_action_routing(
        &self,
        routing: &ActionRoutingRow,
        instance_name: &str,
    ) -> Result<ValidationResult> {
        let mut errors = Vec::new();

        // Validate instance exists
        let instance_exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM instances WHERE instance_name = ?)",
        )
        .bind(instance_name)
        .fetch_one(&self.pool)
        .await?;

        if !instance_exists {
            errors.push(format!("Instance {} does not exist", instance_name));
        }

        // Validate channel_type (skip if None - unbound routing is valid)
        if let Some(ref ct) = routing.channel_type {
            if !ct.is_output() {
                errors.push(format!(
                    "Invalid channel_type for action: {}. Must be C or A",
                    ct
                ));
            }
        }

        // Validate action point exists
        let point_exists = sqlx::query_scalar::<_, bool>(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM action_points ap
                JOIN instances i ON i.product_name = ap.product_name
                WHERE i.instance_name = ? AND ap.action_id = ?
            )
            "#,
        )
        .bind(instance_name)
        .bind(routing.action_id)
        .fetch_one(&self.pool)
        .await?;

        if !point_exists {
            errors.push(format!(
                "Action point {} not found for instance {}",
                routing.action_id, instance_name
            ));
        }

        let mut result = ValidationResult::new(ValidationLevel::Business);
        for error in errors {
            result.add_error(error);
        }
        Ok(result)
    }

    /// Delete all routing for an instance
    pub async fn delete_instance_routing(&self, instance_name: &str) -> Result<(u64, u64)> {
        let measurement_result =
            sqlx::query("DELETE FROM measurement_routing WHERE instance_name = ?")
                .bind(instance_name)
                .execute(&self.pool)
                .await?;

        let action_result = sqlx::query("DELETE FROM action_routing WHERE instance_name = ?")
            .bind(instance_name)
            .execute(&self.pool)
            .await?;

        Ok((
            measurement_result.rows_affected(),
            action_result.rows_affected(),
        ))
    }
}

// ValidationResult is now imported from voltage-config

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn setup_test_env() -> Result<(RoutingLoader, TempDir)> {
        let temp_dir = TempDir::new()?;
        let db_path = temp_dir.path().join("test.db");
        let sqlite_url = format!("sqlite:{}?mode=rwc", db_path.display());
        let pool = SqlitePool::connect(&sqlite_url).await?;

        // Use standard modsrv schema from common test utils
        common::test_utils::schema::init_modsrv_schema(&pool).await?;

        let instances_dir = temp_dir.path().join("instances");
        tokio::fs::create_dir_all(&instances_dir).await?;

        let loader = RoutingLoader::new(instances_dir, pool);
        Ok((loader, temp_dir))
    }

    #[tokio::test]
    async fn test_routing_loader_creation() {
        let (loader, _temp) = setup_test_env().await.expect("Failed to setup test env");

        // Just verify the loader was created successfully
        assert!(loader.instances_dir.exists());
    }

    #[tokio::test]
    async fn test_load_empty_routing() {
        let (loader, _temp) = setup_test_env().await.expect("Failed to setup test env");

        // Should not fail on empty directory
        let result = loader.load_all_routing().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_delete_nonexistent_instance() {
        let (loader, _temp) = setup_test_env().await.expect("Failed to setup test env");

        let result = loader.delete_instance_routing("nonexistent").await;
        assert!(result.is_ok());
        let (measurement_count, action_count) = result.unwrap();
        assert_eq!(measurement_count, 0);
        assert_eq!(action_count, 0);
    }
}
/// Unified CSV row (channel_mappings.csv)
///
/// Columns:
/// - channel_id, channel_type(T/S/C/A), channel_point_id
/// - instance_type(M/A), instance_point_id
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelMappingRow {
    pub channel_id: i32,
    pub channel_type: FourRemote,
    pub channel_point_id: u32,
    pub instance_type: String, // "M"|"A" (case-insensitive), supports "measurement"/"action"
    pub instance_point_id: u32,
}
