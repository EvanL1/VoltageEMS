//! Configuration synchronization module
//!
//! This module is responsible for syncing configuration from YAML/CSV files
//! to the SQLite database.

use anyhow::{Context, Result};
use common::validation::CsvFields;
use serde::de::DeserializeOwned;
use serde_json::Value as JsonValue;
use sqlx::{Sqlite, SqlitePool, Transaction};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};
use voltage_config::comsrv::ComsrvConfig;
use voltage_config::modsrv::ModsrvConfig;

use super::file_utils::{flatten_json, load_csv, load_csv_typed_with_errors, load_csv_with_errors};
use super::schema;

/// Try parsing a string as a JSON number. Empty strings default to 0.
fn str_to_json_number(s: &str) -> Option<JsonValue> {
    use serde_json::Number;
    let trimmed = s.trim();
    if trimmed.is_empty() {
        Some(JsonValue::Number(Number::from(0)))
    } else if let Ok(n) = trimmed.parse::<i64>() {
        Some(JsonValue::Number(Number::from(n)))
    } else if let Ok(f) = trimmed.parse::<f64>() {
        Number::from_f64(f).map(JsonValue::Number)
    } else {
        None
    }
}

/// Convert mapping entries: numeric_fields become JSON numbers, rest stay strings.
fn convert_fields(
    mapping: HashMap<String, String>,
    numeric_fields: &[&str],
) -> HashMap<String, JsonValue> {
    mapping
        .into_iter()
        .map(|(k, v)| {
            let json_val = if numeric_fields.contains(&k.as_str()) {
                str_to_json_number(&v).unwrap_or(JsonValue::String(v))
            } else {
                JsonValue::String(v)
            };
            (k, json_val)
        })
        .collect()
}

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

    // point_id is stored in a separate column
    mapping.remove("point_id");

    match protocol {
        "modbus_tcp" | "modbus_rtu" => {
            let mut normalized = convert_fields(
                mapping,
                &[
                    "slave_id",
                    "function_code",
                    "register_address",
                    "bit_position",
                ],
            );
            // Ensure bit_position exists and is an integer (convert 0.0 → 0)
            let bp = normalized
                .entry("bit_position".to_string())
                .or_insert(JsonValue::Number(Number::from(0)));
            if let Some(f) = bp.as_f64() {
                *bp = JsonValue::Number(Number::from(f.round() as i64));
            }
            normalized
        },
        "can" => {
            let mut normalized = convert_fields(
                mapping,
                &["can_id", "start_bit", "bit_length", "scale", "offset"],
            );
            normalized
                .entry("signed".to_string())
                .or_insert(JsonValue::Bool(false));
            normalized
                .entry("scale".to_string())
                .or_insert(JsonValue::Number(Number::from(1)));
            normalized
                .entry("offset".to_string())
                .or_insert(JsonValue::Number(Number::from(0)));
            normalized
        },
        "di_do" | "gpio" | "dido" => convert_fields(mapping, &["gpio_number"]),
        _ => convert_fields(mapping, &[]),
    }
}

/// Normalize legacy product names to match built-in product library.
///
/// Maps old snake_case names (from config files) to the canonical names
/// used by `voltage-model` crate's built-in product definitions.
fn normalize_product_name(name: &str) -> &str {
    match name {
        "battery_pack" => {
            warn!("Legacy product name 'battery_pack' → 'Battery'. Please update instances.yaml");
            "Battery"
        },
        "pcs" => {
            warn!("Legacy product name 'pcs' → 'PCS'. Please update instances.yaml");
            "PCS"
        },
        "diesel_generator" => {
            warn!(
                "Legacy product name 'diesel_generator' → 'Diesel'. Please update instances.yaml"
            );
            "Diesel"
        },
        "pv_inverter" => {
            warn!("Legacy product name 'pv_inverter' → 'PVInverter'. Please update instances.yaml");
            "PVInverter"
        },
        "pv_dcdc" | "PV_DCDC" => {
            warn!("Legacy product name '{name}' → 'PV DCDC'. Please update instances.yaml");
            "PV DCDC"
        },
        _ => name,
    }
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

// ============================================================================
// Point Type Sync Helpers
// ============================================================================

/// Configuration for syncing a specific point type (T/S/C/A)
struct PointSyncConfig<'a> {
    /// CSV filename (e.g., "telemetry.csv")
    csv_filename: &'a str,
    /// Mapping filename (e.g., "mapping/telemetry_mapping.csv")
    mapping_filename: &'a str,
    /// Database table name (e.g., "telemetry_points")
    table_name: &'a str,
    /// Type label for error messages (e.g., "telemetry")
    type_label: &'a str,
}

/// Extracted point fields for database insertion
struct PointFields {
    point_id: u32,
    signal_name: String,
    unit: Option<String>,
    description: Option<String>,
    scale: f64,
    offset: f64,
    reverse: bool,
    data_type: String,
}

/// Generic point type sync function
///
/// Syncs points of a specific type (T/S/C/A) from CSV to SQLite.
/// Handles CSV loading, mapping file loading, and database insertion.
#[allow(clippy::too_many_arguments)]
async fn sync_point_type<T, F>(
    tx: &mut Transaction<'_, Sqlite>,
    path: &Path,
    channel_id: i32,
    protocol: &str,
    config: &PointSyncConfig<'_>,
    extract_fields: F,
    errors: &mut Vec<SyncError>,
) -> Result<usize>
where
    T: DeserializeOwned + CsvFields,
    F: Fn(&T) -> PointFields,
{
    let csv_file = path.join(config.csv_filename);
    if !csv_file.exists() {
        return Ok(0);
    }

    let (points, csv_errors) = load_csv_typed_with_errors::<T, _>(&csv_file)?;

    // Collect CSV parsing errors
    for csv_error in &csv_errors {
        errors.push(SyncError::from_csv_error(
            csv_error,
            &format!("channel-{}/{}", channel_id, config.csv_filename),
        ));
    }

    // Load corresponding mappings if they exist
    let mapping_file = path.join(config.mapping_filename);
    let mappings_json = if mapping_file.exists() {
        let (mappings, mapping_csv_errors) = load_csv_with_errors(&mapping_file)?;

        // Collect mapping CSV errors
        for csv_error in &mapping_csv_errors {
            errors.push(SyncError::from_csv_error(
                csv_error,
                &format!("channel-{}/{}", channel_id, config.mapping_filename),
            ));
        }

        // Normalize and convert to JSON, indexed by point_id
        let mut mapping_map = HashMap::new();
        for mapping in mappings {
            if let Some(point_id) = mapping.get("point_id") {
                let point_id = point_id.clone();
                let normalized = normalize_protocol_mapping(protocol, mapping);
                mapping_map.insert(point_id, normalized);
            }
        }
        Some(mapping_map)
    } else {
        None
    };

    let mut count = 0;

    // Build the INSERT OR REPLACE query with table name (safe for both force and UPSERT modes)
    let insert_sql = format!(
        "INSERT OR REPLACE INTO {} (point_id, channel_id, signal_name, scale, offset, unit, reverse, data_type, description, protocol_mappings)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        config.table_name
    );

    for point in points {
        let fields = extract_fields(&point);

        let protocol_mappings = mappings_json
            .as_ref()
            .and_then(|m| m.get(&fields.point_id.to_string()))
            .map(|m| serde_json::to_string(m).unwrap_or_else(|_| "{}".to_string()))
            .unwrap_or_else(|| "null".to_string());

        if let Err(e) = sqlx::query(&insert_sql)
            .bind(fields.point_id)
            .bind(channel_id)
            .bind(&fields.signal_name)
            .bind(fields.scale)
            .bind(fields.offset)
            .bind(&fields.unit)
            .bind(fields.reverse)
            .bind(&fields.data_type)
            .bind(&fields.description)
            .bind(&protocol_mappings)
            .execute(&mut **tx)
            .await
        {
            errors.push(SyncError {
                item: format!(
                    "channel-{}/{}/point-{}",
                    channel_id, config.type_label, fields.point_id
                ),
                error: e.to_string(),
            });
            continue;
        }
        count += 1;
    }

    Ok(count)
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
    /// When true, DELETE all rows before INSERT (full replace). Default: false (UPSERT).
    force: bool,
}

impl ConfigSyncer {
    /// Create a new syncer (default: UPSERT mode)
    pub fn new(config_path: impl AsRef<Path>, db_path: impl AsRef<Path>) -> Self {
        Self {
            config_path: config_path.as_ref().to_path_buf(),
            db_path: db_path.as_ref().to_path_buf(),
            force: false,
        }
    }

    /// Set force mode: DELETE all rows before INSERT (destructive full replace)
    pub fn with_force(mut self, force: bool) -> Self {
        self.force = force;
        self
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
            serde_yml::from_str(&yaml_content).context("Failed to parse global.yaml")?;

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
            serde_yml::from_str(&yaml_content).context("Failed to parse comsrv.yaml")?;

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

        // In force mode: DELETE all rows before INSERT (full replace).
        // In default UPSERT mode: skip DELETE; INSERT OR REPLACE handles conflicts.
        let mut deleted: u64 = 0;
        if self.force {
            for table in [
                "telemetry_points",
                "signal_points",
                "control_points",
                "adjustment_points",
                "channels",
            ] {
                deleted += sqlx::query(&format!("DELETE FROM {table}"))
                    .execute(&mut *tx)
                    .await?
                    .rows_affected();
            }
            deleted += sqlx::query("DELETE FROM service_config WHERE service_name = ?")
                .bind("comsrv")
                .execute(&mut *tx)
                .await?
                .rows_affected();
        }

        stats.items_deleted = deleted as usize;

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
            serde_yml::from_str(&yaml_content).context("Failed to parse modsrv.yaml")?;

        let yaml_config = serde_yml::from_str::<JsonValue>(&yaml_content)
            .context("Failed to parse modsrv.yaml as JSON")?;

        // Initialize schema if needed (creates database file if not exists)
        schema::init_database(&db_file).await?;

        // Connect to database
        let pool = SqlitePool::connect(&format!("sqlite://{}?mode=rwc", db_file.display()))
            .await
            .context("Failed to connect to modsrv database")?;

        // Start transaction
        let mut tx = pool.begin().await?;

        // In force mode: DELETE all rows before INSERT (full replace).
        // In default UPSERT mode: skip DELETE; INSERT OR REPLACE handles conflicts.
        if self.force {
            sqlx::query("DELETE FROM service_config WHERE service_name = ?")
                .bind("modsrv")
                .execute(&mut *tx)
                .await?;

            // Delete in correct order: child tables first, parent tables last
            sqlx::query("DELETE FROM measurement_routing")
                .execute(&mut *tx)
                .await?;
            sqlx::query("DELETE FROM action_routing")
                .execute(&mut *tx)
                .await?;

            // Note: Products are now compile-time built-in constants (voltage-model crate),
            // no need to clear or sync product-related tables
            sqlx::query("DELETE FROM instances")
                .execute(&mut *tx)
                .await?;

            stats.items_deleted = 4; // Cleared 4 tables (config, routing x2, instances)
        }

        // Insert service configuration
        let config_count = self
            .insert_service_config(&mut tx, "modsrv", &yaml_config)
            .await?;
        stats.items_synced += config_count;

        debug!("Config: {}", config_count);

        // Validate external product JSON files if directory exists
        let products_dir = config_dir.join("products");
        if products_dir.is_dir() {
            let product_errors = voltage_model::product_lib::validate_product_dir(&products_dir);
            for (filename, error) in &product_errors {
                stats.errors.push(SyncError {
                    item: format!("products/{}", filename),
                    error: error.clone(),
                });
            }
            if product_errors.is_empty() {
                // Try loading to verify override semantics
                match voltage_model::product_lib::ProductLibrary::load(Some(&products_dir)) {
                    Ok(lib) => {
                        info!("Products: {} (with external overrides)", lib.len());
                    },
                    Err(e) => {
                        stats.errors.push(SyncError {
                            item: "products/".to_string(),
                            error: format!("Failed to load product library: {}", e),
                        });
                    },
                }
            }
        }

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
            // In force mode: delete all rules before re-inserting.
            // In UPSERT mode: existing rules not in config files are preserved.
            if self.force {
                sqlx::query("DELETE FROM rules").execute(&mut *tx).await?;
            }
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

            let (value_str, value_type) = match value {
                JsonValue::String(s) => (s, "string"),
                JsonValue::Bool(b) => (b.to_string(), "boolean"),
                JsonValue::Number(n) => (n.to_string(), "number"),
                JsonValue::Array(a) => (serde_json::to_string(&JsonValue::Array(a))?, "array"),
                JsonValue::Object(o) => (serde_json::to_string(&JsonValue::Object(o))?, "object"),
                JsonValue::Null => continue,
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
                "INSERT OR REPLACE INTO channels (channel_id, name, protocol, enabled, config)
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
        use voltage_config::comsrv::{AdjustmentPoint, ControlPoint, SignalPoint, TelemetryPoint};

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

            // Load point definitions and mappings for each type (T/S/C/A)
            // Telemetry points
            total_count += sync_point_type::<TelemetryPoint, _>(
                tx,
                &path,
                channel_id,
                &protocol,
                &PointSyncConfig {
                    csv_filename: "telemetry.csv",
                    mapping_filename: "mapping/telemetry_mapping.csv",
                    table_name: "telemetry_points",
                    type_label: "telemetry",
                },
                |p| PointFields {
                    point_id: p.base.point_id,
                    signal_name: p.base.signal_name.clone(),
                    unit: p.base.unit.clone(),
                    description: p.base.description.clone(),
                    scale: p.scale,
                    offset: p.offset,
                    reverse: p.reverse,
                    data_type: p.data_type.clone(),
                },
                errors,
            )
            .await?;

            // Signal points (with defaults for scale/offset/data_type)
            total_count += sync_point_type::<SignalPoint, _>(
                tx,
                &path,
                channel_id,
                &protocol,
                &PointSyncConfig {
                    csv_filename: "signal.csv",
                    mapping_filename: "mapping/signal_mapping.csv",
                    table_name: "signal_points",
                    type_label: "signal",
                },
                |p| PointFields {
                    point_id: p.base.point_id,
                    signal_name: p.base.signal_name.clone(),
                    unit: p.base.unit.clone(),
                    description: p.base.description.clone(),
                    scale: 1.0,
                    offset: 0.0,
                    reverse: p.reverse,
                    data_type: "int".to_string(),
                },
                errors,
            )
            .await?;

            // Control points (with defaults)
            total_count += sync_point_type::<ControlPoint, _>(
                tx,
                &path,
                channel_id,
                &protocol,
                &PointSyncConfig {
                    csv_filename: "control.csv",
                    mapping_filename: "mapping/control_mapping.csv",
                    table_name: "control_points",
                    type_label: "control",
                },
                |p| PointFields {
                    point_id: p.base.point_id,
                    signal_name: p.base.signal_name.clone(),
                    unit: p.base.unit.clone(),
                    description: p.base.description.clone(),
                    scale: 1.0,
                    offset: 0.0,
                    reverse: false,
                    data_type: "bool".to_string(),
                },
                errors,
            )
            .await?;

            // Adjustment points (with default reverse)
            total_count += sync_point_type::<AdjustmentPoint, _>(
                tx,
                &path,
                channel_id,
                &protocol,
                &PointSyncConfig {
                    csv_filename: "adjustment.csv",
                    mapping_filename: "mapping/adjustment_mapping.csv",
                    table_name: "adjustment_points",
                    type_label: "adjustment",
                },
                |p| PointFields {
                    point_id: p.base.point_id,
                    signal_name: p.base.signal_name.clone(),
                    unit: p.base.unit.clone(),
                    description: p.base.description.clone(),
                    scale: p.scale,
                    offset: p.offset,
                    reverse: false,
                    data_type: p.data_type.clone(),
                },
                errors,
            )
            .await?;
        }

        Ok(total_count)
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
        let instances_data: JsonValue = serde_yml::from_str(&yaml_content)?;

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

                // Parse optional parent_id for topology hierarchy
                let parent_id = instance_data
                    .get("parent_id")
                    .and_then(|v| v.as_u64())
                    .map(|v| v as u32);

                count += self
                    .process_single_instance(
                        tx,
                        instance_id,
                        instance_name,
                        product_name,
                        parent_id,
                        config_dir,
                        errors,
                    )
                    .await?;
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

                count += self
                    .process_single_instance(
                        tx,
                        instance_id,
                        instance_name,
                        product_name,
                        None,
                        config_dir,
                        errors,
                    )
                    .await?;
            }
        }

        Ok(count)
    }

    /// Process a single instance: load properties, insert into DB, load mappings
    async fn process_single_instance(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        instance_id: u32,
        instance_name: &str,
        product_name: &str,
        parent_id: Option<u32>,
        config_dir: &Path,
        errors: &mut Vec<SyncError>,
    ) -> Result<usize> {
        let mut count = 0;

        // Load properties from instance directory CSV
        let instance_dir = config_dir.join("instances").join(instance_name);
        debug!(
            "Instance: {}, dir exists: {}",
            instance_name,
            instance_dir.exists()
        );
        let properties = if instance_dir.exists() {
            self.load_instance_properties(&instance_dir)
                .unwrap_or_else(|e| {
                    debug!("Failed to load properties for {}: {}", instance_name, e);
                    "{}".to_string()
                })
        } else {
            debug!("Instance directory does not exist: {:?}", instance_dir);
            "{}".to_string()
        };

        let normalized_product = normalize_product_name(product_name);

        if let Err(e) = sqlx::query(
            "INSERT OR REPLACE INTO instances (instance_id, instance_name, product_name, parent_id, properties) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(instance_id)
        .bind(instance_name)
        .bind(normalized_product)
        .bind(parent_id.map(|id| id as i64))
        .bind(&properties)
        .execute(&mut **tx)
        .await
        {
            errors.push(SyncError {
                item: format!("Instance: {}", instance_name),
                error: e.to_string(),
            });
            return Ok(0);
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
            // Parse required positive integer fields
            let parse_id = |field: &str| -> Option<i32> {
                mapping
                    .get(field)
                    .and_then(|v| v.parse::<i32>().ok())
                    .filter(|&id| id > 0)
            };

            let Some(channel_id) = parse_id("channel_id") else {
                errors.push(SyncError {
                    item: format!("Routing for {}", instance_name),
                    error: format!(
                        "Invalid or missing channel_id: {:?}",
                        mapping.get("channel_id")
                    ),
                });
                continue;
            };

            let channel_type = mapping.get("channel_type").cloned().unwrap_or_default();
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

            let route_ctx = format!("{}:{} for {}", channel_id, channel_type, instance_name);

            let Some(channel_point_id) = parse_id("channel_point_id") else {
                errors.push(SyncError {
                    item: format!("Routing {}", route_ctx),
                    error: format!(
                        "Invalid or missing channel_point_id: {:?}",
                        mapping.get("channel_point_id")
                    ),
                });
                continue;
            };

            let instance_type = mapping.get("instance_type").cloned().unwrap_or_default();

            let Some(instance_point_id) = parse_id("instance_point_id") else {
                errors.push(SyncError {
                    item: format!("Routing {}", route_ctx),
                    error: format!(
                        "Invalid or missing instance_point_id: {:?}",
                        mapping.get("instance_point_id")
                    ),
                });
                continue;
            };

            // M (Measurement) → measurement_routing, A (Action) → action_routing
            let insert_result = match instance_type.as_str() {
                "M" => {
                    sqlx::query(
                        "INSERT OR REPLACE INTO measurement_routing (instance_id, instance_name, channel_id, channel_type, channel_point_id, measurement_id)
                        VALUES ((SELECT instance_id FROM instances WHERE instance_name = ?), ?, ?, ?, ?, ?)"
                    )
                    .bind(instance_name).bind(instance_name)
                    .bind(channel_id).bind(&channel_type).bind(channel_point_id).bind(instance_point_id)
                    .execute(&mut **tx).await
                },
                "A" => {
                    sqlx::query(
                        "INSERT OR REPLACE INTO action_routing (instance_id, instance_name, action_id, channel_id, channel_type, channel_point_id)
                        VALUES ((SELECT instance_id FROM instances WHERE instance_name = ?), ?, ?, ?, ?, ?)"
                    )
                    .bind(instance_name).bind(instance_name)
                    .bind(instance_point_id).bind(channel_id).bind(&channel_type).bind(channel_point_id)
                    .execute(&mut **tx).await
                },
                _ => Err(sqlx::Error::Configuration(
                    format!("Invalid instance_type: {}. Must be 'M' or 'A'", instance_type).into(),
                )),
            };

            if let Err(e) = insert_result {
                let kind = if instance_type == "M" {
                    "Measurement"
                } else {
                    "Action"
                };
                errors.push(SyncError {
                    item: format!("{} routing {}:{}", kind, route_ctx, channel_point_id),
                    error: e.to_string(),
                });
                continue;
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
                    serde_yml::from_str(&yaml_content)?
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

            // Parse flow_json → compact RuleFlow for execution engine
            let flow_value: JsonValue = serde_json::from_str(&flow_json).map_err(|e| {
                anyhow::anyhow!("Rule '{}': Failed to parse flow_json: {}", name, e)
            })?;
            let nodes_json = match voltage_rules::parser::extract_rule_flow(&flow_value) {
                Ok(rule_flow) => serde_json::to_string(&rule_flow).map_err(|e| {
                    anyhow::anyhow!("Rule '{}': Failed to serialize RuleFlow: {}", name, e)
                })?,
                Err(e) => {
                    warn!(
                        "Rule '{}' ({}): not a Vue Flow rule, skipping. ({})",
                        name,
                        path.file_name().unwrap_or_default().to_string_lossy(),
                        e
                    );
                    continue;
                },
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
