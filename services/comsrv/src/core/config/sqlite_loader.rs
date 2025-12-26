//! SQLite configuration loader for comsrv
//!
//! Loads channel configurations, point tables, and mappings from SQLite database

use crate::core::config::Point;
use crate::core::config::{
    AdjustmentPoint, AppConfig, ChannelConfig, ControlPoint, RuntimeChannelConfig, ServiceConfig,
    SignalPoint, TelemetryPoint,
};
#[cfg(test)]
use crate::core::config::{
    ADJUSTMENT_POINTS_TABLE, CHANNELS_TABLE, CONTROL_POINTS_TABLE, SERVICE_CONFIG_TABLE,
    SIGNAL_POINTS_TABLE, TELEMETRY_POINTS_TABLE,
};
use crate::error::{ComSrvError, Result};
use common::sqlite::ServiceConfigLoader;
use common::DEFAULT_API_HOST;
use sqlx::{Row, SqlitePool};
use std::collections::HashMap;
use std::path::Path;
use tracing::info;

/// Comsrv-specific SQLite configuration loader
pub struct ComsrvSqliteLoader {
    base_loader: Option<ServiceConfigLoader>,
    pool: SqlitePool,
}

impl ComsrvSqliteLoader {
    /// Create a new comsrv SQLite loader
    pub async fn new(db_path: impl AsRef<Path>) -> Result<Self> {
        let db_path = db_path.as_ref();

        // Check if database exists
        if !db_path.exists() {
            return Err(ComSrvError::ConfigError(format!(
                "Comsrv database not found: {:?}. Please run: monarch sync comsrv",
                db_path
            )));
        }

        // Create base service config loader (single connection pool)
        let base_loader = ServiceConfigLoader::new(db_path, "comsrv")
            .await
            .map_err(|e| {
                ComSrvError::ConfigError(format!("Failed to initialize SQLite loader: {}", e))
            })?;

        // Get pool reference from base_loader
        let pool = base_loader.pool().clone();

        info!("Connected to comsrv database: {:?}", db_path);

        Ok(Self {
            base_loader: Some(base_loader),
            pool,
        })
    }

    /// Create a loader from an existing pool (for connection reuse)
    /// Used by factory when pool is already available
    pub fn with_pool(pool: SqlitePool) -> Self {
        Self {
            base_loader: None,
            pool,
        }
    }

    /// Get the database pool for custom queries
    fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Load complete application configuration from database
    pub async fn load_config(&self) -> Result<AppConfig> {
        // Load base service configuration
        let base_loader = self.base_loader.as_ref().ok_or_else(|| {
            ComSrvError::ConfigError(
                "Base loader not available (created with with_pool)".to_string(),
            )
        })?;
        let service_config = base_loader.load_config().await.map_err(|e| {
            ComSrvError::ConfigError(format!("Failed to load service config: {}", e))
        })?;

        // Convert to comsrv config
        let service = ServiceConfig {
            name: service_config.service_name.clone(),
            description: service_config
                .extra_config
                .get("description")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            version: service_config
                .extra_config
                .get("version")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
        };

        // Create API configuration
        let api = crate::core::config::ApiConfig {
            host: DEFAULT_API_HOST.to_string(),
            port: service_config.port,
        };

        // Create Redis configuration
        let redis = crate::core::config::RedisConfig {
            url: service_config.redis_url.clone(),
            enabled: true,
        };

        // Load channels
        let channels = self.load_channels().await?;

        Ok(AppConfig {
            service,
            api,
            redis,
            logging: crate::core::config::LoggingConfig::default(),
            channels,
        })
    }

    /// Load all channel configurations from database
    async fn load_channels(&self) -> Result<Vec<ChannelConfig>> {
        let rows = sqlx::query(
            "SELECT channel_id, name, protocol, enabled, config FROM channels ORDER BY channel_id",
        )
        .fetch_all(self.pool())
        .await
        .map_err(|e| ComSrvError::ConfigError(format!("Failed to load channels: {}", e)))?;

        let mut channels = Vec::new();

        for row in rows {
            let channel_id: u32 = row.try_get("channel_id").map_err(|e| {
                ComSrvError::ConfigError(format!("Failed to get channel_id: {}", e))
            })?;
            let name: String = row
                .try_get("name")
                .map_err(|e| ComSrvError::ConfigError(format!("Failed to get name: {}", e)))?;
            let protocol: String = row
                .try_get("protocol")
                .map_err(|e| ComSrvError::ConfigError(format!("Failed to get protocol: {}", e)))?;
            let enabled: bool = row
                .try_get("enabled")
                .map_err(|e| ComSrvError::ConfigError(format!("Failed to get enabled: {}", e)))?;
            let config_json: String = row.try_get("config").unwrap_or_else(|_| "{}".to_string());

            // Parse additional config from JSON
            let extra_config: serde_json::Value =
                serde_json::from_str(&config_json).unwrap_or_else(|_| serde_json::json!({}));

            // Parse parameters from config JSON
            // Read from the "parameters" field in the JSON, not from top level
            let mut parameters = HashMap::new();
            if let Some(serde_json::Value::Object(obj)) = extra_config.get("parameters") {
                for (key, value) in obj {
                    // Use JSON value directly (parameters field expects serde_json::Value)
                    parameters.insert(key.clone(), value.clone());
                }
            }

            // Create channel config (without runtime fields)
            let channel = ChannelConfig {
                core: crate::core::config::ChannelCore {
                    id: channel_id,
                    name: name.clone(),
                    description: extra_config
                        .get("description")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()),
                    protocol: protocol.clone(),
                    enabled,
                },
                parameters,
                logging: crate::core::config::ChannelLoggingConfig::default(),
            };

            // Note: Points will be loaded at runtime when creating RuntimeChannelConfig
            channels.push(channel);

            info!(
                "Loaded channel {} ({}) - points will be loaded at runtime",
                channel_id, name
            );
        }

        Ok(channels)
    }

    /// Load all points for a RuntimeChannelConfig with protocol-aware mapping
    pub async fn load_runtime_channel_points(
        &self,
        runtime_config: &mut RuntimeChannelConfig,
    ) -> Result<()> {
        use crate::core::config::{GpioMapping, ModbusMapping, VirtualMapping};

        // Clear existing points and mappings
        runtime_config.telemetry_points.clear();
        runtime_config.signal_points.clear();
        runtime_config.control_points.clear();
        runtime_config.adjustment_points.clear();
        runtime_config.modbus_mappings.clear();
        runtime_config.virtual_mappings.clear();
        runtime_config.can_mappings.clear();

        let channel_id = runtime_config.id();
        let protocol = runtime_config.protocol().to_string(); // Clone to avoid borrow conflict

        // Load telemetry points from telemetry_points table (with embedded protocol mappings)
        let telem_rows = sqlx::query(
            "SELECT point_id, signal_name, scale, offset, unit, reverse, data_type, description, protocol_mappings
             FROM telemetry_points
             WHERE channel_id = ?
             ORDER BY point_id",
        )
        .bind(channel_id as i64)
        .fetch_all(self.pool())
        .await
        .map_err(|e| ComSrvError::ConfigError(format!("Failed to load telemetry points: {}", e)))?;

        // Load signal points from signal_points table (with embedded protocol mappings)
        let signal_rows = sqlx::query(
            "SELECT point_id, signal_name, unit, reverse, data_type, description, protocol_mappings
             FROM signal_points
             WHERE channel_id = ?
             ORDER BY point_id",
        )
        .bind(channel_id as i64)
        .fetch_all(self.pool())
        .await
        .map_err(|e| ComSrvError::ConfigError(format!("Failed to load signal points: {}", e)))?;

        // Load control points from control_points table (with embedded protocol mappings)
        let control_rows = sqlx::query(
            "SELECT point_id, signal_name, unit, reverse, data_type, description, protocol_mappings
             FROM control_points
             WHERE channel_id = ?
             ORDER BY point_id",
        )
        .bind(channel_id as i64)
        .fetch_all(self.pool())
        .await
        .map_err(|e| ComSrvError::ConfigError(format!("Failed to load control points: {}", e)))?;

        // Load adjustment points from adjustment_points table (with embedded protocol mappings)
        let adjustment_rows = sqlx::query(
            "SELECT point_id, signal_name, scale, offset, unit, reverse, data_type, description, protocol_mappings
             FROM adjustment_points
             WHERE channel_id = ?
             ORDER BY point_id",
        )
        .bind(channel_id as i64)
        .fetch_all(self.pool())
        .await
        .map_err(|e| {
            ComSrvError::ConfigError(format!("Failed to load adjustment points: {}", e))
        })?;

        // Helper function to parse protocol mappings from JSON field
        let mut parse_mappings = |protocol_mappings_json: Option<String>,
                                  point_id: u32,
                                  telemetry_type: &str,
                                  data_type: &str,
                                  scale: f64,
                                  offset: f64|
         -> Result<()> {
            if let Some(json_str) = protocol_mappings_json {
                if json_str.trim().is_empty() || json_str == "null" || json_str == "{}" {
                    return Ok(());
                }

                match protocol.to_lowercase().as_str() {
                    "modbus_tcp" | "modbus_rtu" | "modbus" => {
                        if let Ok(mapping_data) =
                            serde_json::from_str::<serde_json::Value>(&json_str)
                        {
                            let mapping = ModbusMapping {
                                channel_id,
                                point_id,
                                telemetry_type: telemetry_type.to_string(),
                                slave_id: mapping_data
                                    .get("slave_id")
                                    .and_then(|v| v.as_i64())
                                    .unwrap_or(1) as u8,
                                function_code: mapping_data
                                    .get("function_code")
                                    .and_then(|v| v.as_i64())
                                    .unwrap_or(3)
                                    as u8,
                                register_address: mapping_data
                                    .get("register_address")
                                    .and_then(|v| v.as_i64())
                                    .unwrap_or(0)
                                    as u16,
                                data_type: mapping_data
                                    .get("data_type")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("uint16")
                                    .to_string(),
                                byte_order: mapping_data
                                    .get("byte_order")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("ABCD")
                                    .to_string(),
                                bit_position: mapping_data
                                    .get("bit_position")
                                    .and_then(|v| v.as_i64())
                                    .map(|v| v as u8)
                                    .unwrap_or(0),
                            };
                            runtime_config.modbus_mappings.push(mapping);
                        }
                    },
                    "virtual" => {
                        if let Ok(mapping_data) =
                            serde_json::from_str::<serde_json::Value>(&json_str)
                        {
                            let mapping = VirtualMapping {
                                channel_id,
                                point_id,
                                telemetry_type: telemetry_type.to_string(),
                                expression: mapping_data
                                    .get("expression")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_string()),
                                update_interval: mapping_data
                                    .get("update_interval")
                                    .and_then(|v| v.as_i64())
                                    .map(|v| v as u32),
                                initial_value: mapping_data
                                    .get("initial_value")
                                    .and_then(|v| v.as_f64()),
                                noise_range: mapping_data
                                    .get("noise_range")
                                    .and_then(|v| v.as_f64()),
                            };
                            runtime_config.virtual_mappings.push(mapping);
                        }
                    },
                    "di_do" | "gpio" | "dido" => {
                        if let Ok(mapping_data) =
                            serde_json::from_str::<serde_json::Value>(&json_str)
                        {
                            let mapping = GpioMapping {
                                channel_id,
                                point_id,
                                telemetry_type: telemetry_type.to_string(),
                                gpio_number: mapping_data
                                    .get("gpio_number")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0)
                                    as u32,
                            };
                            runtime_config.gpio_mappings.push(mapping);
                        }
                    },
                    "can" => {
                        use crate::core::config::CanMapping;
                        if let Ok(mapping_data) =
                            serde_json::from_str::<serde_json::Value>(&json_str)
                        {
                            // Extract byte_offset and bit_position from JSON
                            let byte_offset = mapping_data
                                .get("byte_offset")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0) as u32;
                            let bit_position = mapping_data
                                .get("bit_position")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0) as u32;
                            
                            // Calculate start_bit = byte_offset * 8 + bit_position
                            let start_bit = byte_offset * 8 + bit_position;
                            
                            let mapping = CanMapping {
                                channel_id,
                                point_id,
                                telemetry_type: telemetry_type.to_string(),
                                can_id: mapping_data
                                    .get("can_id")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0) as u32,
                                msg_name: mapping_data
                                    .get("msg_name")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_string()),
                                signal_name: mapping_data
                                    .get("signal_name")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_string()),
                                start_bit,
                                bit_length: mapping_data
                                    .get("bit_length")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(0) as u32,
                                byte_order: mapping_data
                                    .get("byte_order")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("ABCD")
                                    .to_string(),
                                data_type: data_type.to_string(),
                                signed: mapping_data
                                    .get("signed")
                                    .and_then(|v| v.as_bool())
                                    .unwrap_or(false),
                                // Use scale/offset from telemetry_points table, not from JSON
                                scale,
                                offset,
                                min_value: mapping_data
                                    .get("min_value")
                                    .and_then(|v| v.as_f64()),
                                max_value: mapping_data
                                    .get("max_value")
                                    .and_then(|v| v.as_f64()),
                                unit: mapping_data
                                    .get("unit")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_string()),
                            };
                            runtime_config.can_mappings.push(mapping);
                        }
                    },
                    _ => {
                        // Other protocols don't have mappings yet
                    },
                }
            }
            Ok(())
        };

        // Process telemetry points
        for row in telem_rows {
            let point_id: i64 = row
                .try_get("point_id")
                .map_err(|e| ComSrvError::ConfigError(format!("Failed to get point_id: {}", e)))?;
            let signal_name: String = row.try_get("signal_name").map_err(|e| {
                ComSrvError::ConfigError(format!("Failed to get signal_name: {}", e))
            })?;
            let scale: f64 = row.try_get("scale").unwrap_or(1.0);
            let offset: f64 = row.try_get("offset").unwrap_or(0.0);
            let unit: Option<String> = row.try_get("unit").ok().filter(|s: &String| !s.is_empty());
            let reverse: bool = row.try_get("reverse").unwrap_or(false);
            let data_type: String = row
                .try_get("data_type")
                .unwrap_or_else(|_| "float32".to_string());
            let description: Option<String> = row.try_get("description").ok();

            // Parse protocol mappings from JSON field
            let protocol_mappings: Option<String> = row.try_get("protocol_mappings").ok();
            parse_mappings(protocol_mappings, point_id as u32, "T", &data_type, scale, offset)?;

            let base_point = Point {
                point_id: point_id as u32,
                signal_name,
                description,
                unit,
            };

            let point = TelemetryPoint {
                base: base_point,
                scale,
                offset,
                data_type,
                reverse,
            };
            runtime_config.telemetry_points.push(point);
        }

        // Process signal points
        for row in signal_rows {
            let point_id: i64 = row
                .try_get("point_id")
                .map_err(|e| ComSrvError::ConfigError(format!("Failed to get point_id: {}", e)))?;
            let signal_name: String = row.try_get("signal_name").map_err(|e| {
                ComSrvError::ConfigError(format!("Failed to get signal_name: {}", e))
            })?;
            let unit: Option<String> = row.try_get("unit").ok().filter(|s: &String| !s.is_empty());
            let reverse: bool = row.try_get("reverse").unwrap_or(false);
            let data_type: String = row
                .try_get("data_type")
                .unwrap_or_else(|_| "uint16".to_string());
            let description: Option<String> = row.try_get("description").ok();

            // Parse protocol mappings from JSON field
            let protocol_mappings: Option<String> = row.try_get("protocol_mappings").ok();
            parse_mappings(protocol_mappings, point_id as u32, "S", &data_type, 1.0, 0.0)?;

            let base_point = Point {
                point_id: point_id as u32,
                signal_name,
                description,
                unit,
            };

            let point = SignalPoint {
                base: base_point,
                reverse,
            };
            runtime_config.signal_points.push(point);
        }

        // Process control points
        for row in control_rows {
            let point_id: i64 = row
                .try_get("point_id")
                .map_err(|e| ComSrvError::ConfigError(format!("Failed to get point_id: {}", e)))?;
            let signal_name: String = row.try_get("signal_name").map_err(|e| {
                ComSrvError::ConfigError(format!("Failed to get signal_name: {}", e))
            })?;
            let unit: Option<String> = row.try_get("unit").ok().filter(|s: &String| !s.is_empty());
            let reverse: bool = row.try_get("reverse").unwrap_or(false);
            let data_type: String = row
                .try_get("data_type")
                .unwrap_or_else(|_| "bool".to_string());
            let description: Option<String> = row.try_get("description").ok();

            // Parse protocol mappings from JSON field
            let protocol_mappings: Option<String> = row.try_get("protocol_mappings").ok();
            parse_mappings(protocol_mappings, point_id as u32, "C", &data_type, 1.0, 0.0)?;

            let base_point = Point {
                point_id: point_id as u32,
                signal_name,
                description,
                unit,
            };

            let point = ControlPoint {
                base: base_point,
                reverse,
                control_type: "momentary".to_string(),
                on_value: 1,
                off_value: 0,
                pulse_duration_ms: Some(100),
            };
            runtime_config.control_points.push(point);
        }

        // Process adjustment points
        for row in adjustment_rows {
            let point_id: i64 = row
                .try_get("point_id")
                .map_err(|e| ComSrvError::ConfigError(format!("Failed to get point_id: {}", e)))?;
            let signal_name: String = row.try_get("signal_name").map_err(|e| {
                ComSrvError::ConfigError(format!("Failed to get signal_name: {}", e))
            })?;
            let scale: f64 = row.try_get("scale").unwrap_or(1.0);
            let offset: f64 = row.try_get("offset").unwrap_or(0.0);
            let unit: Option<String> = row.try_get("unit").ok().filter(|s: &String| !s.is_empty());
            let _reverse: bool = row.try_get("reverse").unwrap_or(false);
            let data_type: String = row
                .try_get("data_type")
                .unwrap_or_else(|_| "float32".to_string());
            let description: Option<String> = row.try_get("description").ok();

            // Parse protocol mappings from JSON field
            let protocol_mappings: Option<String> = row.try_get("protocol_mappings").ok();
            parse_mappings(protocol_mappings, point_id as u32, "A", &data_type, scale, offset)?;

            let base_point = Point {
                point_id: point_id as u32,
                signal_name,
                description,
                unit,
            };

            let point = AdjustmentPoint {
                base: base_point,
                min_value: None,
                max_value: None,
                step: 1.0,
                data_type,
                scale,
                offset,
            };
            runtime_config.adjustment_points.push(point);
        }

        info!(
            "Loaded {} points for channel {}: {} telemetry, {} signal, {} control, {} adjustment",
            runtime_config.telemetry_points.len()
                + runtime_config.signal_points.len()
                + runtime_config.control_points.len()
                + runtime_config.adjustment_points.len(),
            runtime_config.id(),
            runtime_config.telemetry_points.len(),
            runtime_config.signal_points.len(),
            runtime_config.control_points.len(),
            runtime_config.adjustment_points.len()
        );

        Ok(())
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;
    use sqlx::SqlitePool;
    use tempfile::TempDir;

    /// Create a test database with basic schema and sample data
    async fn create_test_database() -> (TempDir, String) {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test_voltage.db");
        let db_url = format!("sqlite://{}?mode=rwc", db_path.display());

        let pool = SqlitePool::connect(&db_url).await.unwrap();

        // Create service_config table
        sqlx::query(SERVICE_CONFIG_TABLE)
            .execute(&pool)
            .await
            .unwrap();

        // Insert basic service config (with service_name column)
        sqlx::query(
            "INSERT INTO service_config (service_name, key, value) VALUES
                ('comsrv', 'service_name', 'comsrv'),
                ('comsrv', 'port', '6001'),
                ('comsrv', 'redis_url', 'redis://localhost:6379'),
                ('comsrv', 'description', 'Test Service'),
                ('comsrv', 'version', '1.0.0')",
        )
        .execute(&pool)
        .await
        .unwrap();

        // Create channels table
        sqlx::query(CHANNELS_TABLE).execute(&pool).await.unwrap();

        // Insert test channels
        sqlx::query(
            "INSERT INTO channels (channel_id, name, protocol, enabled, config) VALUES
                (1001, 'Test Modbus Channel', 'modbus_tcp', 1, '{\"parameters\":{\"host\":\"192.168.1.100\",\"port\":502}}'),
                (1002, 'Test Virtual Channel', 'virtual', 1, '{\"parameters\":{\"point_count\":5}}'),
                (1003, 'Test Disabled Channel', 'virtual', 0, '{}')",
        )
        .execute(&pool)
        .await
        .unwrap();

        // Create telemetry_points table
        sqlx::query(TELEMETRY_POINTS_TABLE)
            .execute(&pool)
            .await
            .unwrap();

        // Create signal_points table
        sqlx::query(SIGNAL_POINTS_TABLE)
            .execute(&pool)
            .await
            .unwrap();

        // Create control_points table
        sqlx::query(CONTROL_POINTS_TABLE)
            .execute(&pool)
            .await
            .unwrap();

        // Create adjustment_points table
        sqlx::query(ADJUSTMENT_POINTS_TABLE)
            .execute(&pool)
            .await
            .unwrap();

        // Insert test telemetry points (with protocol_mappings JSON)
        sqlx::query(
            "INSERT INTO telemetry_points (channel_id, point_id, signal_name, scale, offset, unit, reverse, data_type, description, protocol_mappings) VALUES
                (1001, 1, 'Temperature', 0.1, 0.0, '°C', 0, 'float32', 'Test temperature', '{\"slave_id\":1,\"function_code\":3,\"register_address\":100,\"data_type\":\"float32\",\"byte_order\":\"ABCD\"}'),
                (1002, 1, 'Virtual Point 1', 1.0, 0.0, '', 0, 'float32', 'Test virtual point', '{\"update_interval\":1000,\"initial_value\":25.0,\"noise_range\":2.0}')",
        )
        .execute(&pool)
        .await
        .unwrap();

        // Insert test signal points (with protocol_mappings JSON)
        sqlx::query(
            "INSERT INTO signal_points (channel_id, point_id, signal_name, unit, reverse, data_type, description, protocol_mappings) VALUES
                (1001, 2, 'Status', '', 0, 'uint16', 'Device status', '{\"slave_id\":1,\"function_code\":3,\"register_address\":102,\"data_type\":\"uint16\",\"byte_order\":\"ABCD\"}')",
        )
        .execute(&pool)
        .await
        .unwrap();

        // Insert test control points
        sqlx::query(
            "INSERT INTO control_points (channel_id, point_id, signal_name, unit, data_type, description) VALUES
                (1001, 3, 'Start', '', 'bool', 'Start control')",
        )
        .execute(&pool)
        .await
        .unwrap();

        // Insert test adjustment points
        sqlx::query(
            "INSERT INTO adjustment_points (channel_id, point_id, signal_name, scale, offset, unit, reverse, data_type, description) VALUES
                (1001, 4, 'Setpoint', 1.0, 0.0, '°C', 0, 'float32', 'Temperature setpoint')",
        )
        .execute(&pool)
        .await
        .unwrap();

        // Create modbus_mappings table
        sqlx::query(
            "CREATE TABLE modbus_mappings (
                channel_id INTEGER,
                point_id INTEGER,
                telemetry_type TEXT,
                slave_id INTEGER,
                function_code INTEGER,
                register_address INTEGER,
                data_type TEXT DEFAULT 'uint16',
                byte_order TEXT DEFAULT 'ABCD',
                bit_position INTEGER,
                PRIMARY KEY (channel_id, point_id, telemetry_type)
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        // Insert modbus mappings
        sqlx::query(
            "INSERT INTO modbus_mappings VALUES
                (1001, 1, 'T', 1, 3, 100, 'float32', 'ABCD', NULL),
                (1001, 2, 'S', 1, 3, 102, 'uint16', 'ABCD', NULL)",
        )
        .execute(&pool)
        .await
        .unwrap();

        // Create virtual_mappings table
        sqlx::query(
            "CREATE TABLE virtual_mappings (
                channel_id INTEGER,
                point_id INTEGER,
                telemetry_type TEXT,
                expression TEXT,
                update_interval INTEGER,
                initial_value REAL,
                noise_range REAL,
                PRIMARY KEY (channel_id, point_id, telemetry_type)
            )",
        )
        .execute(&pool)
        .await
        .unwrap();

        // Insert virtual mappings
        sqlx::query(
            "INSERT INTO virtual_mappings VALUES
                (1002, 1, 'T', NULL, 1000, 25.0, 2.0)",
        )
        .execute(&pool)
        .await
        .unwrap();

        pool.close().await;

        (temp_dir, db_path.to_string_lossy().to_string())
    }

    #[tokio::test]
    async fn test_loader_creation_success() {
        let (_temp_dir, db_path) = create_test_database().await;

        let loader = ComsrvSqliteLoader::new(&db_path).await;
        assert!(loader.is_ok(), "Should create loader successfully");
    }

    #[tokio::test]
    async fn test_loader_creation_missing_database() {
        let result = ComsrvSqliteLoader::new("/nonexistent/database.db").await;
        assert!(result.is_err(), "Should fail with missing database");

        match result {
            Err(e) => {
                let err_msg = e.to_string();
                assert!(
                    err_msg.contains("not found"),
                    "Error should mention database not found, got: {}",
                    err_msg
                );
            },
            Ok(_) => panic!("Expected error, got Ok"),
        }
    }

    #[tokio::test]
    async fn test_load_complete_config() {
        let (_temp_dir, db_path) = create_test_database().await;
        let loader = ComsrvSqliteLoader::new(&db_path).await.unwrap();

        let config = loader.load_config().await;
        assert!(config.is_ok(), "Should load config successfully");

        let config = config.unwrap();
        assert_eq!(config.service.name, "comsrv");
        assert_eq!(config.api.port, 6001); // Default port (test uses wrong key 'port' instead of 'service.port')
        assert_eq!(config.redis.url, "redis://localhost:6379");
        assert_eq!(config.channels.len(), 3, "Should load all 3 channels");
    }

    #[tokio::test]
    async fn test_load_channels() {
        let (_temp_dir, db_path) = create_test_database().await;
        let loader = ComsrvSqliteLoader::new(&db_path).await.unwrap();

        let config = loader.load_config().await.unwrap();

        // Verify first channel (Modbus)
        let channel1 = &config.channels[0];
        assert_eq!(channel1.id(), 1001);
        assert_eq!(channel1.name(), "Test Modbus Channel");
        assert_eq!(channel1.protocol(), "modbus_tcp");
        assert!(channel1.is_enabled());
        assert!(channel1.parameters.contains_key("host"));

        // Verify second channel (Virtual)
        let channel2 = &config.channels[1];
        assert_eq!(channel2.id(), 1002);
        assert_eq!(channel2.protocol(), "virtual");
        assert!(channel2.is_enabled());

        // Verify third channel (Disabled)
        let channel3 = &config.channels[2];
        assert_eq!(channel3.id(), 1003);
        assert!(!channel3.is_enabled());
    }

    #[tokio::test]
    async fn test_load_runtime_channel_points_modbus() {
        let (_temp_dir, db_path) = create_test_database().await;
        let loader = ComsrvSqliteLoader::new(&db_path).await.unwrap();
        let config = loader.load_config().await.unwrap();

        // Create runtime config for first channel (Modbus)
        let channel = config.channels.into_iter().next().unwrap();
        let mut runtime_config = RuntimeChannelConfig::from_base(channel);

        // Load points
        let result = loader
            .load_runtime_channel_points(&mut runtime_config)
            .await;
        assert!(result.is_ok(), "Should load points successfully");

        // Verify loaded points
        assert_eq!(runtime_config.telemetry_points.len(), 1);
        assert_eq!(runtime_config.signal_points.len(), 1);
        assert_eq!(runtime_config.control_points.len(), 1);
        assert_eq!(runtime_config.adjustment_points.len(), 1);

        // Verify Modbus mappings loaded
        assert_eq!(runtime_config.modbus_mappings.len(), 2);
        assert_eq!(runtime_config.modbus_mappings[0].point_id, 1);
        assert_eq!(runtime_config.modbus_mappings[0].register_address, 100);
    }

    #[tokio::test]
    async fn test_load_runtime_channel_points_virtual() {
        let (_temp_dir, db_path) = create_test_database().await;
        let loader = ComsrvSqliteLoader::new(&db_path).await.unwrap();
        let config = loader.load_config().await.unwrap();

        // Get second channel (Virtual)
        let channel = config.channels.into_iter().nth(1).unwrap();
        let mut runtime_config = RuntimeChannelConfig::from_base(channel);

        // Load points
        let result = loader
            .load_runtime_channel_points(&mut runtime_config)
            .await;
        assert!(result.is_ok(), "Should load points successfully");

        // Verify loaded points
        assert_eq!(runtime_config.telemetry_points.len(), 1);

        // Verify Virtual mappings loaded
        assert_eq!(runtime_config.virtual_mappings.len(), 1);
        assert_eq!(runtime_config.virtual_mappings[0].point_id, 1);
        assert_eq!(
            runtime_config.virtual_mappings[0].update_interval,
            Some(1000)
        );
    }

    #[tokio::test]
    async fn test_parameter_preservation() {
        let (_temp_dir, db_path) = create_test_database().await;
        let loader = ComsrvSqliteLoader::new(&db_path).await.unwrap();
        let config = loader.load_config().await.unwrap();

        // Check that custom parameters from database are preserved
        let modbus_channel = config
            .channels
            .iter()
            .find(|c| c.protocol() == "modbus_tcp")
            .unwrap();
        assert_eq!(
            modbus_channel.parameters.get("host").unwrap().as_str(),
            Some("192.168.1.100")
        );
        assert_eq!(
            modbus_channel.parameters.get("port").unwrap().as_i64(),
            Some(502)
        );
    }

    #[tokio::test]
    async fn test_point_data_types() {
        let (_temp_dir, db_path) = create_test_database().await;
        let loader = ComsrvSqliteLoader::new(&db_path).await.unwrap();
        let config = loader.load_config().await.unwrap();

        let channel = config.channels.into_iter().next().unwrap();
        let mut runtime_config = RuntimeChannelConfig::from_base(channel);

        loader
            .load_runtime_channel_points(&mut runtime_config)
            .await
            .unwrap();

        // Check telemetry point
        let telem = &runtime_config.telemetry_points[0];
        assert_eq!(telem.base.point_id, 1);
        assert_eq!(telem.base.signal_name, "Temperature");
        assert_eq!(telem.scale, 0.1);
        assert_eq!(telem.offset, 0.0);
        assert_eq!(telem.data_type, "float32");
        assert!(!telem.reverse);

        // Check signal point
        let signal = &runtime_config.signal_points[0];
        assert_eq!(signal.base.point_id, 2);
        assert_eq!(signal.base.signal_name, "Status");

        // Check control point
        let control = &runtime_config.control_points[0];
        assert_eq!(control.base.point_id, 3);
        assert_eq!(control.base.signal_name, "Start");

        // Check adjustment point
        let adj = &runtime_config.adjustment_points[0];
        assert_eq!(adj.base.point_id, 4);
        assert_eq!(adj.base.signal_name, "Setpoint");
    }

    #[tokio::test]
    async fn test_empty_database() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("empty.db");
        let db_url = format!("sqlite://{}?mode=rwc", db_path.display());

        // Create empty database with only service_config
        let pool = SqlitePool::connect(&db_url).await.unwrap();
        sqlx::query(SERVICE_CONFIG_TABLE)
            .execute(&pool)
            .await
            .unwrap();

        sqlx::query(
            "INSERT INTO service_config (service_name, key, value) VALUES
                ('comsrv', 'service_name', 'comsrv'),
                ('comsrv', 'port', '6001'),
                ('comsrv', 'redis_url', 'redis://localhost:6379')",
        )
        .execute(&pool)
        .await
        .unwrap();

        // Create empty channels table
        sqlx::query(CHANNELS_TABLE).execute(&pool).await.unwrap();

        pool.close().await;

        // Should load successfully with no channels
        let loader = ComsrvSqliteLoader::new(db_path).await.unwrap();
        let config = loader.load_config().await.unwrap();
        assert_eq!(config.channels.len(), 0);
    }
}
