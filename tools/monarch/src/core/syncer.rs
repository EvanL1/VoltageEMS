//! Configuration synchronization module
//!
//! This module is responsible for syncing configuration from YAML/CSV files
//! to the SQLite database.

use anyhow::{Context, Result};
use comsrv::core::config::ComsrvConfig;
use modsrv::config::ModsrvConfig;
use serde_json::Value as JsonValue;
use sqlx::{Sqlite, SqlitePool, Transaction};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

use super::file_utils::{flatten_json, load_csv, load_csv_typed};
use super::schema;

/// Normalize protocol_data numeric fields to JSON numbers (not strings)
///
/// Ensures type consistency between CSV import and runtime API operations.
/// Modbus/CAN numeric fields (slave_id, function_code, etc.) should be numbers.
#[allow(clippy::disallowed_methods)] // from_f64(1.0/0.0).unwrap() is safe for valid f64 constants
fn normalize_protocol_mapping(
    protocol: &str,
    mut mapping: HashMap<String, String>,
) -> HashMap<String, JsonValue> {
    use serde_json::Number;

    let mut normalized: HashMap<String, JsonValue> = HashMap::new();

    // Helper: convert string to JSON number if possible
    // Empty strings are treated as 0
    let to_number = |s: &str| -> Option<JsonValue> {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            // Empty strings default to 0
            Some(JsonValue::Number(Number::from(0)))
        } else if let Ok(n) = trimmed.parse::<i64>() {
            Some(JsonValue::Number(Number::from(n)))
        } else if let Ok(f) = trimmed.parse::<f64>() {
            Number::from_f64(f).map(JsonValue::Number)
        } else {
            None
        }
    };

    // Remove point_id from protocol_data (it's stored in separate column)
    mapping.remove("point_id");

    match protocol {
        "modbus_tcp" | "modbus_rtu" => {
            let numeric_fields = [
                "slave_id",
                "function_code",
                "register_address",
                "bit_position",
            ];
            for (key, value) in mapping {
                if numeric_fields.contains(&key.as_str()) {
                    normalized.insert(
                        key.clone(),
                        to_number(&value).unwrap_or(JsonValue::String(value)),
                    );
                } else {
                    normalized.insert(key, JsonValue::String(value));
                }
            }
            // Add missing fields with default values to ensure JSON completeness
            normalized
                .entry("bit_position".to_string())
                .or_insert(JsonValue::Number(Number::from(0)));

            // Normalize bit_position to integer (convert 0.0 → 0, 1.0 → 1)
            if let Some(JsonValue::Number(n)) = normalized.get("bit_position") {
                if let Some(f) = n.as_f64() {
                    let int_value = f.round() as i64;
                    normalized.insert(
                        "bit_position".to_string(),
                        JsonValue::Number(Number::from(int_value)),
                    );
                }
            }
        },
        "can" => {
            let numeric_fields = ["can_id", "start_bit", "bit_length", "scale", "offset"];
            for (key, value) in mapping {
                if numeric_fields.contains(&key.as_str()) {
                    normalized.insert(
                        key.clone(),
                        to_number(&value).unwrap_or(JsonValue::String(value)),
                    );
                } else {
                    normalized.insert(key, JsonValue::String(value));
                }
            }
            // Add missing fields with default values to ensure JSON completeness
            normalized
                .entry("signed".to_string())
                .or_insert(JsonValue::Bool(false));
            normalized
                .entry("scale".to_string())
                .or_insert(JsonValue::Number(Number::from_f64(1.0).unwrap()));
            normalized
                .entry("offset".to_string())
                .or_insert(JsonValue::Number(Number::from_f64(0.0).unwrap()));
        },
        "di_do" | "gpio" | "dido" => {
            let numeric_fields = ["gpio_number"];
            for (key, value) in mapping {
                if numeric_fields.contains(&key.as_str()) {
                    normalized.insert(
                        key.clone(),
                        to_number(&value).unwrap_or(JsonValue::String(value)),
                    );
                } else {
                    normalized.insert(key, JsonValue::String(value));
                }
            }
        },
        _ => {
            // Unknown protocol: keep all as strings
            for (key, value) in mapping {
                normalized.insert(key, JsonValue::String(value));
            }
        },
    }

    normalized
}

/// Error that occurred during sync
#[derive(Debug, Clone)]
pub struct SyncError {
    /// Item that caused the error
    pub item: String,
    /// Error message
    pub error: String,
}

impl SyncError {
    /// Convert CSV row error to sync error
    pub fn from_csv_error(csv_error: &crate::core::file_utils::CsvRowError, context: &str) -> Self {
        Self {
            item: format!("{}:row-{}", context, csv_error.row_number),
            error: csv_error.error.clone(),
        }
    }
}

/// Result of a sync operation
#[derive(Debug, Default)]
pub struct SyncResult {
    /// Number of items synced
    pub items_synced: usize,
    /// Number of items deleted
    pub items_deleted: usize,
    /// Errors encountered during sync
    pub errors: Vec<SyncError>,
}

/// Configuration syncer
pub struct ConfigSyncer {
    config_path: PathBuf,
    db_path: PathBuf,
}

impl ConfigSyncer {
    /// Create a new syncer
    pub fn new(config_path: impl AsRef<Path>, db_path: impl AsRef<Path>) -> Self {
        Self {
            config_path: config_path.as_ref().to_path_buf(),
            db_path: db_path.as_ref().to_path_buf(),
        }
    }

    /// Sync configuration for a specific service
    ///
    /// @input service: &str - Service name ("comsrv", "modsrv", "global")
    /// @output `Result<SyncResult>` - Sync statistics (items synced, deleted, errors)
    /// @throws anyhow::Error - Unknown service, database errors, file I/O errors
    /// @side-effects Clears and repopulates service database from YAML/CSV files
    pub async fn sync_service(&self, service: &str) -> Result<SyncResult> {
        info!("Sync: {}", service);

        match service {
            "comsrv" => self.sync_comsrv().await,
            "modsrv" => self.sync_modsrv().await,
            "global" => self.sync_global().await,
            _ => Err(anyhow::anyhow!("Unknown service: {}", service)),
        }
    }

    /// Sync global configuration (shared across all services)
    ///
    /// @input self - Syncer with config and db paths
    /// @output `Result<SyncResult>` - Items synced count
    /// @side-effects Database operations: DELETE global config, INSERT from config/global.yaml
    /// @transaction Full transaction - all or nothing
    pub async fn sync_global(&self) -> Result<SyncResult> {
        let mut stats = SyncResult::default();
        let global_yaml_path = self.config_path.join("global.yaml");

        // If global.yaml doesn't exist, skip (optional configuration)
        if !global_yaml_path.exists() {
            debug!("No global.yaml, skip");
            return Ok(stats);
        }

        debug!("Sync global: {:?}", global_yaml_path);

        // Read and parse YAML
        let yaml_content = std::fs::read_to_string(&global_yaml_path)
            .with_context(|| format!("Failed to read {:?}", global_yaml_path))?;
        let yaml_config: JsonValue =
            serde_yaml::from_str(&yaml_content).context("Failed to parse global.yaml")?;

        // Start transaction
        let db_file = self.db_path.join("voltage.db");
        let pool = SqlitePool::connect(&format!("sqlite:{}", db_file.display())).await?;
        let mut tx = pool.begin().await?;

        // Insert global configuration
        let config_count = self
            .insert_service_config(&mut tx, "global", &yaml_config)
            .await?;
        stats.items_synced += config_count;

        debug!("Global: {} items", config_count);

        // Update sync timestamp
        self.update_sync_timestamp(&mut tx, "global").await?;

        // Commit transaction
        tx.commit().await?;

        info!("Global: {} synced", stats.items_synced);

        Ok(stats)
    }

    /// Sync comsrv configuration
    ///
    /// @input self - Syncer with config and db paths
    /// @output `Result<SyncResult>` - Items synced/deleted counts
    /// @side-effects Database operations: DELETE all, INSERT from config
    /// @transaction Full transaction - all or nothing
    /// @order 1. Delete mappings, 2. Delete points, 3. Delete channels, 4. Insert new data
    async fn sync_comsrv(&self) -> Result<SyncResult> {
        let mut stats = SyncResult::default();
        let db_file = self.db_path.join("voltage.db");
        let config_dir = self.config_path.join("comsrv");

        debug!("Sync comsrv: {:?}", config_dir);

        // Ensure database directory exists
        if let Some(parent) = db_file.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Load and parse YAML as strongly-typed configuration
        let yaml_path = config_dir.join("comsrv.yaml");
        let yaml_content = std::fs::read_to_string(&yaml_path)
            .with_context(|| format!("Failed to read {:?}", yaml_path))?;
        let comsrv_config: ComsrvConfig =
            serde_yaml::from_str(&yaml_content).context("Failed to parse comsrv.yaml")?;

        // Convert to JsonValue for database storage (avoiding double parsing)
        let mut yaml_config =
            serde_json::to_value(&comsrv_config).context("Failed to convert config to JSON")?;

        // Extract channels array (if exists) - channels go to separate table
        let channels = yaml_config
            .as_object_mut()
            .and_then(|obj| obj.remove("channels"))
            .and_then(|v| v.as_array().cloned())
            .unwrap_or_default();

        // Validate channel name uniqueness
        let mut channel_names = std::collections::HashMap::new();
        for (idx, channel) in channels.iter().enumerate() {
            if let Some(name) = channel.get("name").and_then(|v| v.as_str()) {
                if let Some(existing_idx) = channel_names.insert(name.to_string(), idx) {
                    return Err(anyhow::anyhow!(
                        "Duplicate channel name '{}' found at indices {} and {}. \
                         Channel names must be unique. Please rename one of the channels in comsrv.yaml.",
                        name,
                        existing_idx,
                        idx
                    ));
                }
            }
        }

        // Note: Global CSV files don't exist. All point definitions are channel-specific

        // Initialize schema if needed (creates database file if not exists)
        schema::init_database(&db_file).await?;

        // Connect to database
        let pool = SqlitePool::connect(&format!("sqlite://{}?mode=rwc", db_file.display()))
            .await
            .context("Failed to connect to comsrv database")?;

        // Start transaction
        let mut tx = pool.begin().await?;

        // Clear existing configuration and track actual deleted records
        // Delete in reverse dependency order to avoid foreign key constraint violations
        // Delete all four point tables (they contain embedded protocol mappings)
        let deleted1a = sqlx::query("DELETE FROM telemetry_points")
            .execute(&mut *tx)
            .await?
            .rows_affected();
        let deleted1b = sqlx::query("DELETE FROM signal_points")
            .execute(&mut *tx)
            .await?
            .rows_affected();
        let deleted1c = sqlx::query("DELETE FROM control_points")
            .execute(&mut *tx)
            .await?
            .rows_affected();
        let deleted1d = sqlx::query("DELETE FROM adjustment_points")
            .execute(&mut *tx)
            .await?
            .rows_affected();
        let deleted_points = deleted1a + deleted1b + deleted1c + deleted1d;

        let deleted3 = sqlx::query("DELETE FROM channels") // channels is the parent table
            .execute(&mut *tx)
            .await?
            .rows_affected();
        let deleted4 = sqlx::query("DELETE FROM service_config WHERE service_name = ?")
            .bind("comsrv")
            .execute(&mut *tx)
            .await?
            .rows_affected();

        stats.items_deleted = (deleted_points + deleted3 + deleted4) as usize;

        // Insert service configuration
        let config_count = self
            .insert_service_config(&mut tx, "comsrv", &yaml_config)
            .await?;
        stats.items_synced += config_count;

        debug!("Config: {} items", config_count);

        // Insert channels first (before points, due to foreign key constraints)
        let channels_count = self.insert_channels(&mut tx, &channels).await?;
        stats.items_synced += channels_count;

        debug!("Channels: {}", channels_count);

        // No global point definitions to insert - all points are channel-specific

        // Load and insert channel-specific points
        let channel_points_count = self
            .insert_channel_specific_points(&mut tx, &config_dir, &mut stats.errors)
            .await?;
        stats.items_synced += channel_points_count;

        debug!("Points: {}", channel_points_count);

        // Note: Channel mappings are now embedded as JSON in the point tables
        // The insert_channel_mappings function is deprecated but kept for compatibility
        // It now returns 0 as mappings are handled in insert_channel_specific_points
        let mappings_count = 0;
        stats.items_synced += mappings_count;

        // Update sync timestamp
        self.update_sync_timestamp(&mut tx, "comsrv").await?;

        // Commit transaction
        tx.commit().await?;

        info!(
            "Comsrv: {} synced, {} del, {} err",
            stats.items_synced,
            stats.items_deleted,
            stats.errors.len()
        );

        Ok(stats)
    }

    /// Sync modsrv configuration
    ///
    /// @input self - Syncer with config and db paths
    /// @output `Result<SyncResult>` - Items synced/deleted counts with errors
    /// @side-effects Database operations: products, instances, point definitions
    /// @error-recovery Continues on individual item failures, collects all errors
    async fn sync_modsrv(&self) -> Result<SyncResult> {
        let mut stats = SyncResult::default();
        let db_file = self.db_path.join("voltage.db");
        let config_dir = self.config_path.join("modsrv");

        debug!("Sync modsrv: {:?}", config_dir);

        // Ensure database directory exists
        if let Some(parent) = db_file.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Load and parse YAML
        let yaml_path = config_dir.join("modsrv.yaml");
        let yaml_content = std::fs::read_to_string(&yaml_path)
            .with_context(|| format!("Failed to read {:?}", yaml_path))?;
        let _modsrv_config: ModsrvConfig =
            serde_yaml::from_str(&yaml_content).context("Failed to parse modsrv.yaml")?;

        let yaml_config = serde_yaml::from_str::<JsonValue>(&yaml_content)
            .context("Failed to parse modsrv.yaml as JSON")?;

        // Initialize schema if needed (creates database file if not exists)
        schema::init_database(&db_file).await?;

        // Connect to database
        let pool = SqlitePool::connect(&format!("sqlite://{}?mode=rwc", db_file.display()))
            .await
            .context("Failed to connect to modsrv database")?;

        // Start transaction
        let mut tx = pool.begin().await?;

        // Clear existing configuration
        sqlx::query("DELETE FROM service_config WHERE service_name = ?")
            .bind("modsrv")
            .execute(&mut *tx)
            .await?;

        // Delete in correct order: child tables first, parent tables last
        // First delete tables that reference instances
        sqlx::query("DELETE FROM measurement_routing")
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM action_routing")
            .execute(&mut *tx)
            .await?;
        // Skip instance_mappings table (deprecated)

        // Then delete instances (which references products)
        sqlx::query("DELETE FROM instances")
            .execute(&mut *tx)
            .await?;

        // Then delete tables that reference products (using modsrv's actual table names)
        sqlx::query("DELETE FROM measurement_points")
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM action_points")
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM property_templates")
            .execute(&mut *tx)
            .await?;

        // Finally delete products (parent table)
        sqlx::query("DELETE FROM products")
            .execute(&mut *tx)
            .await?;

        stats.items_deleted = 8; // Cleared 8 tables

        // Insert service configuration
        let config_count = self
            .insert_service_config(&mut tx, "modsrv", &yaml_config)
            .await?;
        stats.items_synced += config_count;

        debug!("Config: {}", config_count);

        // Sync products from CSV files
        let products_count = self.sync_modsrv_products(&mut tx, &config_dir).await?;
        stats.items_synced += products_count;

        debug!("Products: {}", products_count);

        // Load and sync instances
        let instances_path = config_dir.join("instances.yaml");
        if instances_path.exists() {
            let instances_count = self
                .sync_instances(&mut tx, &instances_path, &config_dir, &mut stats.errors)
                .await?;
            stats.items_synced += instances_count;
            debug!("Instances: {}", instances_count);
        }

        // Load and sync rules (part of modsrv)
        let rules_dir = config_dir.join("rules");
        if rules_dir.exists() {
            // Clear existing rules
            sqlx::query("DELETE FROM rules").execute(&mut *tx).await?;
            let rules_count = self.sync_rules(&mut tx, &rules_dir).await?;
            stats.items_synced += rules_count;
            debug!("Rules: {}", rules_count);
        }

        // Update sync timestamp
        self.update_sync_timestamp(&mut tx, "modsrv").await?;

        // Commit transaction
        tx.commit().await?;

        // Report errors if any
        if !stats.errors.is_empty() {
            warn!("{} sync errors:", stats.errors.len());
            for error in &stats.errors {
                warn!("  {}: {}", error.item, error.error);
            }
        }

        info!(
            "Modsrv: {} synced, {} del, {} err",
            stats.items_synced,
            stats.items_deleted,
            stats.errors.len()
        );

        Ok(stats)
    }

    // Helper methods

    /// Insert service configuration into database
    async fn insert_service_config(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        service_name: &str,
        config: &JsonValue,
    ) -> Result<usize> {
        // Delete existing config for this service
        sqlx::query("DELETE FROM service_config WHERE service_name = ?")
            .bind(service_name)
            .execute(&mut **tx)
            .await?;

        let flattened = flatten_json(config, None);
        let mut count = 0;

        for (key, value) in flattened {
            // Skip null values to prevent service-specific empty fields from overwriting global config
            if value.is_null() {
                continue;
            }

            let value_str = match &value {
                JsonValue::String(s) => s.clone(),
                _ => serde_json::to_string(&value)?,
            };

            let value_type = match &value {
                JsonValue::Bool(_) => "boolean",
                JsonValue::Number(_) => "number",
                JsonValue::Array(_) => "array",
                JsonValue::Object(_) => "object",
                _ => "string",
            };

            sqlx::query(
                "INSERT INTO service_config (service_name, key, value, type) VALUES (?, ?, ?, ?)",
            )
            .bind(service_name)
            .bind(&key)
            .bind(&value_str)
            .bind(value_type)
            .execute(&mut **tx)
            .await?;

            count += 1;
        }

        Ok(count)
    }

    // insert_comsrv_points() removed - all points are channel-specific

    /// Insert channels
    async fn insert_channels(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        channels: &[JsonValue],
    ) -> Result<usize> {
        let mut count = 0;
        for channel in channels {
            // Parse channel ID (must be u16 as defined in ChannelConfig)
            let channel_id = match channel.get("id").and_then(|v| v.as_u64()) {
                Some(id) if id > 0 && id <= u16::MAX as u64 => id as i32,
                Some(id) => {
                    warn!("Invalid channel ID {}: skip", id);
                    continue;
                },
                None => {
                    warn!("Channel missing id: skip");
                    continue;
                },
            };

            let name = channel
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let protocol = channel
                .get("protocol")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let enabled = channel
                .get("enabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);

            // Only serialize parameters, logging, and description (not core fields)
            // Core fields (id, name, protocol, enabled) are stored in dedicated columns
            let mut config_obj = serde_json::Map::new();

            if let Some(params) = channel.get("parameters") {
                config_obj.insert("parameters".to_string(), params.clone());
            }

            if let Some(logging) = channel.get("logging") {
                config_obj.insert("logging".to_string(), logging.clone());
            }

            if let Some(desc) = channel.get("description") {
                config_obj.insert("description".to_string(), desc.clone());
            }

            let config = serde_json::to_string(&config_obj)?;

            sqlx::query(
                "INSERT INTO channels (channel_id, name, protocol, enabled, config)
                VALUES (?, ?, ?, ?, ?)",
            )
            .bind(channel_id)
            .bind(&name)
            .bind(&protocol)
            .bind(enabled)
            .bind(&config)
            .execute(&mut **tx)
            .await?;

            count += 1;
        }

        Ok(count)
    }

    /// Insert channel-specific points from CSV files
    async fn insert_channel_specific_points(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        config_dir: &Path,
        errors: &mut Vec<SyncError>,
    ) -> Result<usize> {
        use crate::core::file_utils::{load_csv_typed_with_errors, load_csv_with_errors};
        use comsrv::core::config::{AdjustmentPoint, ControlPoint, SignalPoint, TelemetryPoint};

        let mut total_count = 0;

        // Iterate over every channel directory.
        for entry in std::fs::read_dir(config_dir)? {
            let entry = entry?;
            let path = entry.path();

            // Only process directories with numeric names (channel IDs).
            if !path.is_dir() {
                continue;
            }

            let dir_name = match path.file_name().and_then(|n| n.to_str()) {
                Some(name) if name.chars().all(|c| c.is_numeric()) => name,
                _ => continue,
            };

            let channel_id = match dir_name.parse::<i32>() {
                Ok(id) => id,
                Err(_) => continue,
            };

            // Query protocol for this channel (needed for normalization)
            let protocol: String =
                sqlx::query_scalar("SELECT protocol FROM channels WHERE channel_id = ?")
                    .bind(channel_id)
                    .fetch_one(&mut **tx)
                    .await
                    .unwrap_or_else(|_| "modbus_tcp".to_string()); // Default fallback

            // Load point definitions and mappings for each type.

            // Telemetry points with mappings
            let telemetry_file = path.join("telemetry.csv");
            if telemetry_file.exists() {
                let (points, csv_errors) =
                    load_csv_typed_with_errors::<TelemetryPoint, _>(&telemetry_file)?;

                // Collect CSV parsing errors
                for csv_error in &csv_errors {
                    errors.push(SyncError::from_csv_error(
                        csv_error,
                        &format!("channel-{}/telemetry.csv", channel_id),
                    ));
                }

                // Load corresponding mappings if they exist
                let mapping_file = path.join("mapping/telemetry_mapping.csv");
                let mappings_json = if mapping_file.exists() {
                    let (mappings, mapping_csv_errors) = load_csv_with_errors(&mapping_file)?;

                    // Collect mapping CSV errors
                    for csv_error in &mapping_csv_errors {
                        errors.push(SyncError::from_csv_error(
                            csv_error,
                            &format!("channel-{}/mapping/telemetry_mapping.csv", channel_id),
                        ));
                    }

                    // Normalize and convert to JSON, indexed by point_id
                    let mut mapping_map = HashMap::new();
                    for mapping in mappings {
                        if let Some(point_id) = mapping.get("point_id") {
                            // Clone point_id first to release borrow
                            let point_id = point_id.clone();
                            // Normalize protocol_data before storing
                            let normalized = normalize_protocol_mapping(&protocol, mapping);
                            mapping_map.insert(point_id, normalized);
                        }
                    }
                    Some(mapping_map)
                } else {
                    None
                };

                // Insert points with embedded mappings
                for point in points {
                    let protocol_mappings = mappings_json
                        .as_ref()
                        .and_then(|m| m.get(&point.base.point_id.to_string()))
                        .map(|m| serde_json::to_string(m).unwrap_or_else(|_| "{}".to_string()))
                        .unwrap_or_else(|| "null".to_string());

                    // Catch database insertion errors
                    if let Err(e) = sqlx::query(
                        "INSERT INTO telemetry_points (point_id, channel_id, signal_name, scale, offset, unit, reverse, data_type, description, protocol_mappings)
                         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
                    )
                    .bind(point.base.point_id)
                    .bind(channel_id)
                    .bind(&point.base.signal_name)
                    .bind(point.scale)
                    .bind(point.offset)
                    .bind(&point.base.unit)
                    .bind(point.reverse)
                    .bind(&point.data_type)
                    .bind(&point.base.description)
                    .bind(&protocol_mappings)
                    .execute(&mut **tx)
                    .await
                    {
                        errors.push(SyncError {
                            item: format!("channel-{}/telemetry/point-{}", channel_id, point.base.point_id),
                            error: e.to_string(),

                        });
                        continue;
                    }
                    total_count += 1;
                }
            }

            // Signal points with mappings
            let signal_file = path.join("signal.csv");
            if signal_file.exists() {
                let (points, csv_errors) =
                    load_csv_typed_with_errors::<SignalPoint, _>(&signal_file)?;

                // Collect CSV parsing errors
                for csv_error in &csv_errors {
                    errors.push(SyncError::from_csv_error(
                        csv_error,
                        &format!("channel-{}/signal.csv", channel_id),
                    ));
                }

                // Load corresponding mappings if they exist
                let mapping_file = path.join("mapping/signal_mapping.csv");
                let mappings_json = if mapping_file.exists() {
                    let (mappings, mapping_csv_errors) = load_csv_with_errors(&mapping_file)?;

                    // Collect mapping CSV errors
                    for csv_error in &mapping_csv_errors {
                        errors.push(SyncError::from_csv_error(
                            csv_error,
                            &format!("channel-{}/mapping/signal_mapping.csv", channel_id),
                        ));
                    }

                    // Normalize and convert to JSON, indexed by point_id
                    let mut mapping_map = HashMap::new();
                    for mapping in mappings {
                        if let Some(point_id) = mapping.get("point_id") {
                            // Clone point_id first to release borrow
                            let point_id = point_id.clone();
                            // Normalize protocol_data before storing
                            let normalized = normalize_protocol_mapping(&protocol, mapping);
                            mapping_map.insert(point_id, normalized);
                        }
                    }
                    Some(mapping_map)
                } else {
                    None
                };

                for point in points {
                    let protocol_mappings = mappings_json
                        .as_ref()
                        .and_then(|m| m.get(&point.base.point_id.to_string()))
                        .map(|m| serde_json::to_string(m).unwrap_or_else(|_| "{}".to_string()))
                        .unwrap_or_else(|| "null".to_string());

                    // Catch database insertion errors
                    if let Err(e) = sqlx::query(
                        "INSERT INTO signal_points (point_id, channel_id, signal_name, scale, offset, unit, reverse, data_type, description, protocol_mappings)
                         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
                    )
                    .bind(point.base.point_id)
                    .bind(channel_id)
                    .bind(&point.base.signal_name)
                    .bind(1.0)  // Default scale for signal
                    .bind(0.0)  // Default offset for signal
                    .bind(&point.base.unit)
                    .bind(point.reverse)
                    .bind("int")  // Default data type for signal
                    .bind(&point.base.description)
                    .bind(&protocol_mappings)
                    .execute(&mut **tx)
                    .await
                    {
                        errors.push(SyncError {
                            item: format!("channel-{}/signal/point-{}", channel_id, point.base.point_id),
                            error: e.to_string(),

                        });
                        continue;
                    }
                    total_count += 1;
                }
            }

            // Control points with mappings
            let control_file = path.join("control.csv");
            if control_file.exists() {
                let (points, csv_errors) =
                    load_csv_typed_with_errors::<ControlPoint, _>(&control_file)?;

                // Collect CSV parsing errors
                for csv_error in &csv_errors {
                    errors.push(SyncError::from_csv_error(
                        csv_error,
                        &format!("channel-{}/control.csv", channel_id),
                    ));
                }

                // Load corresponding mappings if they exist
                let mapping_file = path.join("mapping/control_mapping.csv");
                let mappings_json = if mapping_file.exists() {
                    let (mappings, mapping_csv_errors) = load_csv_with_errors(&mapping_file)?;

                    // Collect mapping CSV errors
                    for csv_error in &mapping_csv_errors {
                        errors.push(SyncError::from_csv_error(
                            csv_error,
                            &format!("channel-{}/mapping/control_mapping.csv", channel_id),
                        ));
                    }

                    // Normalize and convert to JSON, indexed by point_id
                    let mut mapping_map = HashMap::new();
                    for mapping in mappings {
                        if let Some(point_id) = mapping.get("point_id") {
                            // Clone point_id first to release borrow
                            let point_id = point_id.clone();
                            // Normalize protocol_data before storing
                            let normalized = normalize_protocol_mapping(&protocol, mapping);
                            mapping_map.insert(point_id, normalized);
                        }
                    }
                    Some(mapping_map)
                } else {
                    None
                };

                for point in points {
                    let protocol_mappings = mappings_json
                        .as_ref()
                        .and_then(|m| m.get(&point.base.point_id.to_string()))
                        .map(|m| serde_json::to_string(m).unwrap_or_else(|_| "{}".to_string()))
                        .unwrap_or_else(|| "null".to_string());

                    // Catch database insertion errors
                    if let Err(e) = sqlx::query(
                        "INSERT INTO control_points (point_id, channel_id, signal_name, scale, offset, unit, reverse, data_type, description, protocol_mappings)
                         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
                    )
                    .bind(point.base.point_id)
                    .bind(channel_id)
                    .bind(&point.base.signal_name)
                    .bind(1.0)  // Default scale for control
                    .bind(0.0)  // Default offset for control
                    .bind(&point.base.unit)
                    .bind(false)  // Default reverse for control
                    .bind("bool")  // Default data type for control
                    .bind(&point.base.description)
                    .bind(&protocol_mappings)
                    .execute(&mut **tx)
                    .await
                    {
                        errors.push(SyncError {
                            item: format!("channel-{}/control/point-{}", channel_id, point.base.point_id),
                            error: e.to_string(),

                        });
                        continue;
                    }
                    total_count += 1;
                }
            }

            // Adjustment points with mappings
            let adjustment_file = path.join("adjustment.csv");
            if adjustment_file.exists() {
                let (points, csv_errors) =
                    load_csv_typed_with_errors::<AdjustmentPoint, _>(&adjustment_file)?;

                // Collect CSV parsing errors
                for csv_error in &csv_errors {
                    errors.push(SyncError::from_csv_error(
                        csv_error,
                        &format!("channel-{}/adjustment.csv", channel_id),
                    ));
                }

                // Load corresponding mappings if they exist
                let mapping_file = path.join("mapping/adjustment_mapping.csv");
                let mappings_json = if mapping_file.exists() {
                    let (mappings, mapping_csv_errors) = load_csv_with_errors(&mapping_file)?;

                    // Collect mapping CSV errors
                    for csv_error in &mapping_csv_errors {
                        errors.push(SyncError::from_csv_error(
                            csv_error,
                            &format!("channel-{}/mapping/adjustment_mapping.csv", channel_id),
                        ));
                    }

                    // Normalize and convert to JSON, indexed by point_id
                    let mut mapping_map = HashMap::new();
                    for mapping in mappings {
                        if let Some(point_id) = mapping.get("point_id") {
                            // Clone point_id first to release borrow
                            let point_id = point_id.clone();
                            // Normalize protocol_data before storing
                            let normalized = normalize_protocol_mapping(&protocol, mapping);
                            mapping_map.insert(point_id, normalized);
                        }
                    }
                    Some(mapping_map)
                } else {
                    None
                };

                for point in points {
                    let protocol_mappings = mappings_json
                        .as_ref()
                        .and_then(|m| m.get(&point.base.point_id.to_string()))
                        .map(|m| serde_json::to_string(m).unwrap_or_else(|_| "{}".to_string()))
                        .unwrap_or_else(|| "null".to_string());

                    // Catch database insertion errors
                    if let Err(e) = sqlx::query(
                        "INSERT INTO adjustment_points (point_id, channel_id, signal_name, scale, offset, unit, reverse, data_type, description, protocol_mappings)
                         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
                    )
                    .bind(point.base.point_id)
                    .bind(channel_id)
                    .bind(&point.base.signal_name)
                    .bind(point.scale)
                    .bind(point.offset)
                    .bind(&point.base.unit)
                    .bind(false)  // Default reverse for adjustment
                    .bind(&point.data_type)
                    .bind(&point.base.description)
                    .bind(&protocol_mappings)
                    .execute(&mut **tx)
                    .await
                    {
                        errors.push(SyncError {
                            item: format!("channel-{}/adjustment/point-{}", channel_id, point.base.point_id),
                            error: e.to_string(),

                        });
                        continue;
                    }
                    total_count += 1;
                }
            }
        }

        Ok(total_count)
    }

    /// Sync modsrv products from configuration
    ///
    /// @input tx: &mut Transaction - Active database transaction
    /// @input config_dir: &Path - Directory containing product definitions
    /// @output `Result<usize>` - Number of items successfully synced
    /// @loads products.yaml - Product hierarchy definitions
    /// @loads {product}/measurements.csv - Measurement point definitions
    /// @loads {product}/actions.csv - Action point definitions
    /// @loads {product}/properties.csv - Property template definitions
    /// @side-effects Inserts into products, measurement_points, action_points, property_templates
    async fn sync_modsrv_products(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        config_dir: &Path,
    ) -> Result<usize> {
        let mut count = 0;

        // Locate the products directory.
        let products_dir = config_dir.join("products");
        if !products_dir.exists() {
            debug!("No products directory found at {:?}", products_dir);
            return Ok(0);
        }

        // Load products.yaml.
        let products_yaml = products_dir.join("products.yaml");
        if products_yaml.exists() {
            let yaml_str = std::fs::read_to_string(&products_yaml)?;
            let yaml: serde_yaml::Value = serde_yaml::from_str(&yaml_str)?;

            if let Some(products) = yaml.get("products").and_then(|p| p.as_mapping()) {
                for (key, value) in products {
                    let product_name = key.as_str().unwrap_or_default();
                    let parent_name = value.as_str();

                    sqlx::query("INSERT INTO products (product_name, parent_name) VALUES (?, ?)")
                        .bind(product_name)
                        .bind(parent_name)
                        .execute(&mut **tx)
                        .await?;

                    count += 1;
                }
            }
        }

        // Load product measurement points, action points, and property templates.
        for entry in std::fs::read_dir(&products_dir)? {
            let entry = entry?;
            let product_dir = entry.path();

            if !product_dir.is_dir() {
                continue;
            }

            let product_name = match product_dir.file_name().and_then(|n| n.to_str()) {
                Some(name) => name,
                None => continue,
            };

            // Load measurement point definitions.
            let measurements_file = product_dir.join("measurements.csv");
            if measurements_file.exists() {
                use modsrv::config::{MeasurementPoint, SqlInsertableProduct};
                eprintln!(
                    "DEBUG: About to load measurements from: {:?}",
                    measurements_file
                );
                let points: Vec<MeasurementPoint> = load_csv_typed(&measurements_file)?;
                eprintln!(
                    "DEBUG: Successfully loaded {} measurement points",
                    points.len()
                );
                for point in points {
                    point.insert_with(&mut **tx, product_name).await?;
                    count += 1;
                }
            }

            // Load action point definitions.
            let actions_file = product_dir.join("actions.csv");
            if actions_file.exists() {
                use modsrv::config::{ActionPoint, SqlInsertableProduct};
                let points: Vec<ActionPoint> = load_csv_typed(&actions_file)?;
                for point in points {
                    point.insert_with(&mut **tx, product_name).await?;
                    count += 1;
                }
            }

            // Load property template definitions.
            let properties_file = product_dir.join("properties.csv");
            if properties_file.exists() {
                use modsrv::config::{PropertyTemplate, SqlInsertableProduct};
                let templates: Vec<PropertyTemplate> = load_csv_typed(&properties_file)?;
                for template in templates {
                    template.insert_with(&mut **tx, product_name).await?;
                    count += 1;
                }
            }
        }

        Ok(count)
    }

    /// Sync instances and their mappings
    async fn sync_instances(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        instances_path: &Path,
        config_dir: &Path,
        errors: &mut Vec<SyncError>,
    ) -> Result<usize> {
        let mut count = 0;

        let yaml_content = std::fs::read_to_string(instances_path)?;
        let instances_data: JsonValue = serde_yaml::from_str(&yaml_content)?;

        // Support both array format (recommended) and legacy object format
        if let Some(instances_array) = instances_data.get("instances").and_then(|v| v.as_array()) {
            // Array format: instances: [{instance_id: 1, instance_name: "x", product_name: "y", ...}]
            for instance_data in instances_array {
                // Parse and validate instance_id (required, must be > 0)
                let instance_id = match instance_data.get("instance_id").and_then(|v| v.as_u64()) {
                    Some(id) if id > 0 => id as u32,
                    _ => {
                        errors.push(SyncError {
                            item: "Instance definition".to_string(),
                            error: format!(
                                "Invalid or missing instance_id: {:?}",
                                instance_data.get("instance_id")
                            ),
                        });
                        continue;
                    },
                };

                let instance_name = instance_data
                    .get("instance_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                let product_name = instance_data
                    .get("product_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                // Validate required fields
                if instance_name.is_empty() {
                    errors.push(SyncError {
                        item: format!("Instance with id {}", instance_id),
                        error: "Missing instance_name".to_string(),
                    });
                    continue;
                }

                if product_name.is_empty() {
                    errors.push(SyncError {
                        item: format!("Instance: {}", instance_name),
                        error: "Missing product_name".to_string(),
                    });
                    continue;
                }

                // Load properties from instance directory CSV
                let instance_dir = config_dir.join("instances").join(instance_name);
                eprintln!(
                    "[sync] Instance: {}, dir: {:?}, exists: {}",
                    instance_name,
                    instance_dir,
                    instance_dir.exists()
                );
                debug!(
                    "Instance: {}, dir exists: {}",
                    instance_name,
                    instance_dir.exists()
                );
                let properties = if instance_dir.exists() {
                    eprintln!(
                        "[sync] Calling load_instance_properties for {}",
                        instance_name
                    );
                    self.load_instance_properties(&instance_dir)
                        .unwrap_or_else(|e| {
                            eprintln!(
                                "[sync] Failed to load properties for {}: {}",
                                instance_name, e
                            );
                            debug!("Failed to load properties for {}: {}", instance_name, e);
                            "{}".to_string()
                        })
                } else {
                    eprintln!(
                        "[sync] Instance directory does not exist: {:?}",
                        instance_dir
                    );
                    debug!("Instance directory does not exist: {:?}", instance_dir);
                    "{}".to_string()
                };

                if let Err(e) = sqlx::query(
                    "INSERT INTO instances (instance_id, instance_name, product_name, properties) VALUES (?, ?, ?, ?)",
                )
                .bind(instance_id)
                .bind(instance_name)
                .bind(product_name)
                .bind(&properties)
                .execute(&mut **tx)
                .await
                {
                    errors.push(SyncError {
                        item: format!("Instance: {}", instance_name),
                        error: e.to_string(),

                    });
                    continue; // Skip to next instance
                }

                count += 1;

                // Load instance mappings
                if instance_dir.exists() {
                    let mappings_csv = instance_dir.join("channel_routing.csv");
                    if mappings_csv.exists() {
                        count += self
                            .insert_instance_mappings(tx, instance_name, &mappings_csv, errors)
                            .await?;
                    }
                }
            }
        } else if let Some(instances) = instances_data.get("instances").and_then(|v| v.as_object())
        {
            // Legacy object format: instances: {instance_name: {product_name: "x", ...}}
            for (instance_name, instance_data) in instances {
                let product_name = instance_data
                    .get("product_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");

                // Generate a new instance_id for legacy format
                let instance_id = self.get_next_instance_id(tx).await?;

                // Load properties from instance directory CSV
                let instance_dir = config_dir.join("instances").join(instance_name);
                eprintln!(
                    "[sync] Instance: {}, dir: {:?}, exists: {}",
                    instance_name,
                    instance_dir,
                    instance_dir.exists()
                );
                debug!(
                    "Instance: {}, dir exists: {}",
                    instance_name,
                    instance_dir.exists()
                );
                let properties = if instance_dir.exists() {
                    eprintln!(
                        "[sync] Calling load_instance_properties for {}",
                        instance_name
                    );
                    self.load_instance_properties(&instance_dir)
                        .unwrap_or_else(|e| {
                            eprintln!(
                                "[sync] Failed to load properties for {}: {}",
                                instance_name, e
                            );
                            debug!("Failed to load properties for {}: {}", instance_name, e);
                            "{}".to_string()
                        })
                } else {
                    eprintln!(
                        "[sync] Instance directory does not exist: {:?}",
                        instance_dir
                    );
                    debug!("Instance directory does not exist: {:?}", instance_dir);
                    "{}".to_string()
                };

                if let Err(e) = sqlx::query(
                    "INSERT INTO instances (instance_id, instance_name, product_name, properties) VALUES (?, ?, ?, ?)",
                )
                .bind(instance_id)
                .bind(instance_name)
                .bind(product_name)
                .bind(&properties)
                .execute(&mut **tx)
                .await
                {
                    errors.push(SyncError {
                        item: format!("Instance: {}", instance_name),
                        error: e.to_string(),

                    });
                    continue; // Skip to next instance
                }

                count += 1;

                // Load instance mappings
                if instance_dir.exists() {
                    let mappings_csv = instance_dir.join("channel_routing.csv");
                    if mappings_csv.exists() {
                        count += self
                            .insert_instance_mappings(tx, instance_name, &mappings_csv, errors)
                            .await?;
                    }
                }
            }
        }

        Ok(count)
    }

    /// Get next available instance_id
    async fn get_next_instance_id(&self, tx: &mut Transaction<'_, Sqlite>) -> Result<u32> {
        let max_id: Option<u32> = sqlx::query_scalar("SELECT MAX(instance_id) FROM instances")
            .fetch_optional(&mut **tx)
            .await?;

        Ok(max_id.unwrap_or(0) + 1)
    }

    /// Load instance properties from properties.csv
    /// Format: point_index,value
    /// Returns JSON string: {"1": "500.0", "2": "380.0", ...}
    fn load_instance_properties(&self, instance_dir: &Path) -> Result<String> {
        let properties_path = instance_dir.join("properties.csv");

        if !properties_path.exists() {
            return Ok("{}".to_string());
        }

        let properties_csv = load_csv(&properties_path)?;

        let mut properties_map: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();

        for row in properties_csv.iter() {
            if let (Some(point_index), Some(value)) = (row.get("point_index"), row.get("value")) {
                properties_map.insert(
                    point_index.clone(),
                    serde_json::Value::String(value.clone()),
                );
            }
        }

        let properties_json = serde_json::Value::Object(properties_map);
        let json_string = serde_json::to_string(&properties_json)?;
        Ok(json_string)
    }

    /// Insert instance mappings
    async fn insert_instance_mappings(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        instance_name: &str,
        mappings_path: &Path,
        errors: &mut Vec<SyncError>,
    ) -> Result<usize> {
        let mappings = match load_csv(mappings_path) {
            Ok(m) => m,
            Err(e) => {
                errors.push(SyncError {
                    item: format!("CSV file: {}", mappings_path.display()),
                    error: e.to_string(),
                });
                return Ok(0);
            },
        };

        let mut success_count = 0;
        for mapping in mappings.iter() {
            // Parse and validate channel_id (required, must be > 0)
            let channel_id = match mapping
                .get("channel_id")
                .and_then(|v| v.parse::<i32>().ok())
            {
                Some(id) if id > 0 => id,
                _ => {
                    errors.push(SyncError {
                        item: format!("Routing for {}", instance_name),
                        error: format!(
                            "Invalid or missing channel_id: {:?}",
                            mapping.get("channel_id")
                        ),
                    });
                    continue;
                },
            };
            let channel_type = mapping.get("channel_type").cloned().unwrap_or_default();

            // Validate channel_type before inserting
            if !["T", "S", "C", "A"].contains(&channel_type.as_str()) {
                errors.push(SyncError {
                    item: format!(
                        "Routing {}:{} for {}",
                        channel_id, channel_type, instance_name
                    ),
                    error: format!(
                        "Invalid channel_type '{}': must be T, S, C, or A",
                        channel_type
                    ),
                });
                continue;
            }

            // Parse and validate channel_point_id (required, must be > 0)
            let channel_point_id = match mapping
                .get("channel_point_id")
                .and_then(|v| v.parse::<i32>().ok())
            {
                Some(id) if id > 0 => id,
                _ => {
                    errors.push(SyncError {
                        item: format!(
                            "Routing {}:{} for {}",
                            channel_id, channel_type, instance_name
                        ),
                        error: format!(
                            "Invalid or missing channel_point_id: {:?}",
                            mapping.get("channel_point_id")
                        ),
                    });
                    continue;
                },
            };
            let instance_type = mapping.get("instance_type").cloned().unwrap_or_default();
            // Parse and validate instance_point_id (required, must be > 0)
            let instance_point_id = match mapping
                .get("instance_point_id")
                .and_then(|v| v.parse::<i32>().ok())
            {
                Some(id) if id > 0 => id,
                _ => {
                    errors.push(SyncError {
                        item: format!(
                            "Routing {}:{} for {}",
                            channel_id, channel_type, instance_name
                        ),
                        error: format!(
                            "Invalid or missing instance_point_id: {:?}",
                            mapping.get("instance_point_id")
                        ),
                    });
                    continue;
                },
            };

            // Insert into appropriate routing table based on instance_type
            // M (Measurement) points go to measurement_routing (from T/S channels)
            // A (Action) points go to action_routing (to C/A channels)
            let insert_result = if instance_type == "M" {
                // Measurement routing: T/S → M
                sqlx::query(
                    "INSERT INTO measurement_routing (instance_id, instance_name, channel_id, channel_type, channel_point_id, measurement_id)
                    VALUES ((SELECT instance_id FROM instances WHERE instance_name = ?), ?, ?, ?, ?, ?)"
                )
                .bind(instance_name)
                .bind(instance_name)
                .bind(channel_id)
                .bind(&channel_type)
                .bind(channel_point_id)
                .bind(instance_point_id)
                .execute(&mut **tx)
                .await
            } else if instance_type == "A" {
                // Action routing: A → C/A
                sqlx::query(
                    "INSERT INTO action_routing (instance_id, instance_name, action_id, channel_id, channel_type, channel_point_id)
                    VALUES ((SELECT instance_id FROM instances WHERE instance_name = ?), ?, ?, ?, ?, ?)"
                )
                .bind(instance_name)
                .bind(instance_name)
                .bind(instance_point_id)
                .bind(channel_id)
                .bind(&channel_type)
                .bind(channel_point_id)
                .execute(&mut **tx)
                .await
            } else {
                Err(sqlx::Error::Configuration(
                    format!(
                        "Invalid instance_type: {}. Must be 'M' or 'A'",
                        instance_type
                    )
                    .into(),
                ))
            };

            if let Err(e) = insert_result {
                errors.push(SyncError {
                    item: format!(
                        "{} routing {}:{}:{} for {}",
                        if instance_type == "M" {
                            "Measurement"
                        } else {
                            "Action"
                        },
                        channel_id,
                        channel_type,
                        channel_point_id,
                        instance_name
                    ),
                    error: e.to_string(),
                });
                continue; // Skip to next mapping
            }

            success_count += 1;
        }

        Ok(success_count)
    }

    /// Sync rules from JSON/YAML files (vue-flow/node-red compatible)
    async fn sync_rules(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        rules_dir: &Path,
    ) -> Result<usize> {
        let mut count = 0;

        for entry in std::fs::read_dir(rules_dir)? {
            let entry = entry?;
            let path = entry.path();

            let extension = path.extension().and_then(|e| e.to_str());

            // Support both JSON and YAML formats
            let rule_data: JsonValue = match extension {
                Some("json") => {
                    let json_content = std::fs::read_to_string(&path)?;
                    serde_json::from_str(&json_content)?
                },
                Some("yaml") | Some("yml") => {
                    let yaml_content = std::fs::read_to_string(&path)?;
                    serde_yaml::from_str(&yaml_content)?
                },
                _ => continue, // Skip non-JSON/YAML files
            };

            // id is auto-generated by SQLite (INTEGER PRIMARY KEY AUTOINCREMENT)
            let name = rule_data.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let description = rule_data
                .get("description")
                .and_then(|v| v.as_str())
                .map(String::from);
            let enabled = rule_data
                .get("enabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            let priority = rule_data
                .get("priority")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);

            // Store the complete flow_json (entire rule content for vue-flow/node-red)
            let flow_json = match rule_data.get("flow_json") {
                Some(v) => serde_json::to_string(v).map_err(|e| {
                    anyhow::anyhow!("Rule '{}': Failed to serialize flow_json: {}", name, e)
                })?,
                None => serde_json::to_string(&rule_data).map_err(|e| {
                    anyhow::anyhow!(
                        "Rule '{}': Failed to serialize rule_data as flow_json: {}",
                        name,
                        e
                    )
                })?,
            };

            // nodes_json is required - extract from rule_data or use empty array
            let nodes_json = match rule_data.get("nodes") {
                Some(v) => serde_json::to_string(v).map_err(|e| {
                    anyhow::anyhow!("Rule '{}': Failed to serialize nodes: {}", name, e)
                })?,
                None => "[]".to_string(),
            };

            sqlx::query(
                "INSERT INTO rules (name, description, flow_json, nodes_json, enabled, priority)
                VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(name)
            .bind(description)
            .bind(&flow_json)
            .bind(&nodes_json)
            .bind(enabled)
            .bind(priority)
            .execute(&mut **tx)
            .await?;

            count += 1;
        }

        Ok(count)
    }

    /// Update sync timestamp in sync_metadata table
    async fn update_sync_timestamp(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        service_name: &str,
    ) -> Result<()> {
        let timestamp = sqlx::types::chrono::Utc::now()
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();

        sqlx::query(
            "INSERT INTO sync_metadata (service, last_sync) VALUES (?, ?)
             ON CONFLICT(service) DO UPDATE SET last_sync = excluded.last_sync",
        )
        .bind(service_name)
        .bind(&timestamp)
        .execute(&mut **tx)
        .await?;

        Ok(())
    }
}

// ============================================================================
// Tests for sync_calculations
// ============================================================================

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Create test environment with in-memory SQLite and temp directory
    #[allow(dead_code)] // May be used by future tests
    async fn setup_test_env() -> (SqlitePool, TempDir, PathBuf) {
        let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
        let db_path = temp_dir.path().join("test.db");

        let pool = SqlitePool::connect(&format!("sqlite:{}?mode=rwc", db_path.display()))
            .await
            .expect("Failed to create SQLite pool");

        // Initialize modsrv schema (includes calculations table)
        common::test_utils::schema::init_modsrv_schema(&pool)
            .await
            .expect("Failed to init schema");

        let config_dir = temp_dir.path().join("config");
        std::fs::create_dir_all(&config_dir).expect("Failed to create config dir");

        (pool, temp_dir, config_dir)
    }
}
