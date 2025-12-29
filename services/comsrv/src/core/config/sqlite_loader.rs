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
            let config_json: Option<String> = row
                .try_get("config")
                .map_err(|e| ComSrvError::ConfigError(format!("Failed to get config: {}", e)))?;
            let config_json = config_json.unwrap_or_else(|| "{}".to_string());

            // Parse additional config from JSON
            let extra_config: serde_json::Value =
                serde_json::from_str(&config_json).map_err(|e| {
                    ComSrvError::ConfigError(format!(
                        "Invalid channel config JSON for channel {}: {}",
                        channel_id, e
                    ))
                })?;
            let extra_config_obj = extra_config.as_object().ok_or_else(|| {
                ComSrvError::ConfigError(format!(
                    "Invalid channel config for channel {}: expected JSON object",
                    channel_id
                ))
            })?;

            let description = match extra_config_obj.get("description") {
                None => None,
                Some(serde_json::Value::String(s)) => Some(s.clone()),
                Some(_) => {
                    return Err(ComSrvError::ConfigError(format!(
                        "Invalid channel config for channel {}: 'description' must be a string",
                        channel_id
                    )));
                },
            };

            // Parse parameters from config JSON
            // Read from the "parameters" field in the JSON, not from top level
            let mut parameters = HashMap::new();
            match extra_config_obj.get("parameters") {
                None => {},
                Some(serde_json::Value::Object(obj)) => {
                    for (key, value) in obj {
                        // Use JSON value directly (parameters field expects serde_json::Value)
                        parameters.insert(key.clone(), value.clone());
                    }
                },
                Some(_) => {
                    return Err(ComSrvError::ConfigError(format!(
                        "Invalid channel config for channel {}: 'parameters' must be an object",
                        channel_id
                    )));
                },
            }

            // Create channel config (without runtime fields)
            let channel = ChannelConfig {
                core: crate::core::config::ChannelCore {
                    id: channel_id,
                    name: name.clone(),
                    description,
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
        // Clear existing points
        runtime_config.telemetry_points.clear();
        runtime_config.signal_points.clear();
        runtime_config.control_points.clear();
        runtime_config.adjustment_points.clear();

        let channel_id = runtime_config.id();

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
            let protocol_mappings: Option<String> = row
                .try_get("protocol_mappings")
                .ok()
                .filter(|s: &String| !s.is_empty() && s != "null" && s != "{}");

            let base_point = Point {
                point_id: point_id as u32,
                signal_name,
                description,
                unit,
                protocol_mappings,
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
            let description: Option<String> = row.try_get("description").ok();
            let protocol_mappings: Option<String> = row
                .try_get("protocol_mappings")
                .ok()
                .filter(|s: &String| !s.is_empty() && s != "null" && s != "{}");

            let base_point = Point {
                point_id: point_id as u32,
                signal_name,
                description,
                unit,
                protocol_mappings,
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
            let description: Option<String> = row.try_get("description").ok();
            let protocol_mappings: Option<String> = row
                .try_get("protocol_mappings")
                .ok()
                .filter(|s: &String| !s.is_empty() && s != "null" && s != "{}");

            let base_point = Point {
                point_id: point_id as u32,
                signal_name,
                description,
                unit,
                protocol_mappings,
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
            let data_type: String = row
                .try_get("data_type")
                .unwrap_or_else(|_| "float32".to_string());
            let description: Option<String> = row.try_get("description").ok();
            let protocol_mappings: Option<String> = row
                .try_get("protocol_mappings")
                .ok()
                .filter(|s: &String| !s.is_empty() && s != "null" && s != "{}");

            let base_point = Point {
                point_id: point_id as u32,
                signal_name,
                description,
                unit,
                protocol_mappings,
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

        // Update telemetry_points with protocol_mappings for Modbus channel
        sqlx::query(
            "UPDATE telemetry_points SET protocol_mappings = '{\"slave_id\":1,\"function_code\":3,\"register_address\":100,\"data_type\":\"float32\",\"byte_order\":\"ABCD\"}'
             WHERE channel_id = 1001 AND point_id = 1",
        )
        .execute(&pool)
        .await
        .unwrap();

        // Update signal_points with protocol_mappings for Modbus channel
        sqlx::query(
            "UPDATE signal_points SET protocol_mappings = '{\"slave_id\":1,\"function_code\":3,\"register_address\":102,\"data_type\":\"uint16\",\"byte_order\":\"ABCD\"}'
             WHERE channel_id = 1001 AND point_id = 2",
        )
        .execute(&pool)
        .await
        .unwrap();

        // Update telemetry_points with protocol_mappings for Virtual channel
        sqlx::query(
            "UPDATE telemetry_points SET protocol_mappings = '{\"update_interval\":1000,\"initial_value\":25.0,\"noise_range\":2.0}'
             WHERE channel_id = 1002 AND point_id = 1",
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

        // Verify protocol_mappings embedded in telemetry point
        let telem_point = &runtime_config.telemetry_points[0];
        assert!(telem_point.base.protocol_mappings.is_some());
        let mappings_json = telem_point.base.protocol_mappings.as_ref().unwrap();
        assert!(mappings_json.contains("register_address"));
        assert!(mappings_json.contains("100"));
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

        // Verify protocol_mappings embedded in telemetry point
        let telem_point = &runtime_config.telemetry_points[0];
        assert!(telem_point.base.protocol_mappings.is_some());
        let mappings_json = telem_point.base.protocol_mappings.as_ref().unwrap();
        assert!(mappings_json.contains("update_interval"));
        assert!(mappings_json.contains("1000"));
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
