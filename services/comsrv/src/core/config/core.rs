//! Configuration management core module
//!
//! Integrates configuration center, configuration manager and unified loader functionality

use super::loaders::{CachedCsvLoader, FourRemoteRecord, ModbusMappingRecord, PointMapper};
use super::types::{AppConfig, ChannelConfig, CombinedPoint, ServiceConfig, TableConfig};
use crate::utils::error::{ComSrvError, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{debug, info, warn};
use voltage_libs::config::utils::{get_global_log_level, get_global_redis_url};
use voltage_libs::config::ConfigLoader;

// ============================================================================
// Configuration center integration
// ============================================================================

/// Configuration response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigResponse {
    pub version: String,
    pub checksum: String,
    pub last_modified: String,
    pub content: serde_json::Value,
}

/// Configuration item response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigItemResponse {
    pub key: String,
    pub value: serde_json::Value,
    #[serde(rename = "type")]
    pub value_type: String,
}

/// Configuration change event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigChangeEvent {
    pub event: String,
    pub service: String,
    pub keys: Vec<String>,
    pub version: String,
}

/// Configuration source trait
#[async_trait]
pub trait ConfigSource: Send + Sync {
    /// Get complete configuration
    async fn fetch_config(&self, service_name: &str) -> Result<ConfigResponse>;

    /// Get specific configuration item
    async fn fetch_item(&self, service_name: &str, key: &str) -> Result<ConfigItemResponse>;

    /// Get source name
    fn name(&self) -> &str;
}

/// Configuration center client
#[derive(Debug, Clone)]
pub struct ConfigCenterClient {
    pub service_name: String,
    pub config_center_url: Option<String>,
    pub fallback_path: Option<String>,
    pub cache_duration: u64,
}

impl ConfigCenterClient {
    /// Create client from environment variables
    pub fn from_env(service_name: String) -> Self {
        let config_center_url = std::env::var("CONFIG_CENTER_URL").ok();
        let cache_duration = std::env::var("CONFIG_CACHE_DURATION")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(300); // 5 minutes default

        Self {
            service_name,
            config_center_url,
            fallback_path: None,
            cache_duration,
        }
    }

    /// Set fallback configuration path
    #[must_use]
    pub fn with_fallback(mut self, path: String) -> Self {
        self.fallback_path = Some(path);
        self
    }

    /// Get configuration (with cache and fallback)
    pub async fn get_config(&self) -> Result<Option<serde_json::Value>> {
        if let Some(url) = &self.config_center_url {
            debug!("Fetching config from config center: {}", url);
            // Configuration center not implemented - using local YAML files instead
            Ok(None)
        } else {
            Ok(None)
        }
    }
}

// ============================================================================
// Configuration manager
// ============================================================================

/// Configuration manager
#[derive(Debug)]
pub struct ConfigManager {
    /// Loaded application configuration
    config: AppConfig,
    /// Configuration center client
    #[allow(dead_code)]
    config_center: Option<ConfigCenterClient>,
    /// CSV loader
    csv_loader: CachedCsvLoader,
}

impl ConfigManager {
    /// Load configuration from file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let service_name = "comsrv";

        // Initialize configuration center client
        let config_center = if std::env::var("CONFIG_CENTER_URL").is_ok() {
            info!("Config center URL detected, initializing client");
            Some(
                ConfigCenterClient::from_env(service_name.to_string())
                    .with_fallback(path.to_string_lossy().to_string()),
            )
        } else {
            None
        };

        // Load configuration
        let config = if let Some(ref cc_client) = config_center {
            // Try to load from configuration center
            let runtime = tokio::runtime::Handle::try_current().unwrap_or_else(|_| {
                tokio::runtime::Runtime::new()
                    .expect("failed to create tokio runtime")
                    .handle()
                    .clone()
            });

            if let Ok(Some(remote_config)) = runtime.block_on(cc_client.get_config()) {
                info!("Successfully loaded configuration from config center");
                // JSON configuration loaded from configuration center
                serde_json::from_value::<AppConfig>(remote_config).map_err(|e| {
                    ComSrvError::ConfigError(format!("Failed to parse remote config: {e}"))
                })
            } else {
                // Failed to load from configuration center, use local file
                Self::load_from_file(path)
            }
        } else {
            // No configuration center, load directly from file
            Self::load_from_file(path)
        }?;

        Ok(Self {
            config,
            config_center,
            csv_loader: CachedCsvLoader::new(),
        })
    }

    /// Load configuration from file (using generic ConfigLoader)
    fn load_from_file(path: &Path) -> Result<AppConfig> {
        // Use generic ConfigLoader
        let loader = ConfigLoader::new()
            .with_defaults(AppConfig::default())
            .with_env_prefix("COMSRV")
            .with_yaml_file(&path.to_string_lossy());

        let mut config: AppConfig = loader
            .build()
            .map_err(|e| ComSrvError::ConfigError(format!("Failed to load config: {e}")))?;

        // Apply global environment variables (already handled in ConfigLoader, but ensure again here)
        // Only override if environment variables are actually set
        if std::env::var("VOLTAGE_REDIS_URL").is_ok() || std::env::var("COMSRV_REDIS_URL").is_ok() {
            config.service.redis.url = get_global_redis_url("COMSRV");
        }
        if std::env::var("VOLTAGE_LOG_LEVEL").is_ok() || std::env::var("COMSRV_LOG_LEVEL").is_ok() {
            config.service.logging.level = get_global_log_level("COMSRV");
        }

        Ok(config)
    }

    /// Asynchronously initialize CSV configuration
    pub async fn initialize_csv(&mut self, config_dir: &Path) -> Result<()> {
        debug!(
            "initialize_csv called with config_dir: {}",
            config_dir.display()
        );
        info!("Initializing CSV configurations");
        let result = Self::load_csv_configs(&mut self.config, config_dir, &self.csv_loader).await;
        debug!("load_csv_configs returned: {:?}", result.is_ok());

        // Debug: Print loaded points summary
        for channel in &self.config.channels {
            info!(
                "Channel {} after CSV load: {} telemetry, {} signal, {} control, {} adjustment points",
                channel.id,
                channel.telemetry_points.len(),
                channel.signal_points.len(),
                channel.control_points.len(),
                channel.adjustment_points.len()
            );
        }

        result
    }

    /// Load CSV configuration
    async fn load_csv_configs(
        config: &mut AppConfig,
        config_dir: &Path,
        csv_loader: &CachedCsvLoader,
    ) -> Result<()> {
        for channel in &mut config.channels {
            debug!("Processing channel {}", channel.id);
            if let Some(ref table_config) = channel.table_config {
                debug!("Loading CSV for channel {}", channel.id);
                debug!("Channel {} has table_config", channel.id);
                match Self::load_channel_tables_v2(table_config, config_dir, csv_loader).await {
                    Ok(points) => {
                        info!(
                            "Loaded {} four remote points for channel {}",
                            points.len(),
                            channel.id
                        );
                        // Add points to corresponding HashMap separately
                        for point in points {
                            let point_id = point.point_id;
                            if let Err(e) = channel.add_point(point) {
                                warn!("Failed to add point: {}", e);
                                debug!("Failed to add point {point_id}: {e}");
                            }
                        }
                    },
                    Err(e) => {
                        warn!("Failed to load CSV for channel {}: {}", channel.id, e);
                        debug!("Failed to load CSV for channel {}: {}", channel.id, e);
                    },
                }
            } else {
                debug!("Channel {} has no table_config", channel.id);
            }
        }
        Ok(())
    }

    /// Load channel tables using new CSV loader
    async fn load_channel_tables_v2(
        table_config: &TableConfig,
        config_dir: &Path,
        csv_loader: &CachedCsvLoader,
    ) -> Result<Vec<CombinedPoint>> {
        info!("Loading CSV tables for channel using new loader");
        debug!(
            "load_channel_tables_v2 called with config_dir: {}",
            config_dir.display()
        );
        debug!("four_remote_route: {}", table_config.four_remote_route);
        debug!(
            "protocol_mapping_route: {}",
            table_config.protocol_mapping_route
        );

        // Check environment variable override
        let base_dir = std::env::var("COMSRV_CSV_BASE_PATH")
            .map_or_else(|_| config_dir.to_path_buf(), PathBuf::from);

        debug!("Using CSV base directory: {}", base_dir.display());
        debug!("Base directory for CSV: {}", base_dir.display());

        let mut combined = Vec::new();

        // Four-telemetry file path
        let four_remote_base = base_dir.join(&table_config.four_remote_route);
        // Protocol mapping file path
        let protocol_base = base_dir.join(&table_config.protocol_mapping_route);

        debug!("four_remote_base: {}", four_remote_base.display());
        debug!("protocol_base: {}", protocol_base.display());

        // Debug: Print actual file paths to be loaded
        debug!(
            "Will load telemetry from: {}",
            four_remote_base
                .join(&table_config.four_remote_files.telemetry_file)
                .display()
        );
        debug!(
            "Will load telemetry mapping from: {}",
            protocol_base
                .join(&table_config.protocol_mapping_file.telemetry_mapping)
                .display()
        );

        // Load and merge telemetry points
        match Self::load_and_combine_telemetry(
            &four_remote_base.join(&table_config.four_remote_files.telemetry_file),
            &protocol_base.join(&table_config.protocol_mapping_file.telemetry_mapping),
            "Telemetry",
            csv_loader,
        )
        .await
        {
            Ok(points) => {
                debug!("Successfully loaded {} telemetry points", points.len());
                combined.extend(points);
            },
            Err(e) => {
                warn!("Failed to load telemetry points: {}", e);
            },
        }

        // Load and merge signal points
        match Self::load_and_combine_telemetry(
            &four_remote_base.join(&table_config.four_remote_files.signal_file),
            &protocol_base.join(&table_config.protocol_mapping_file.signal_mapping),
            "Signal",
            csv_loader,
        )
        .await
        {
            Ok(points) => {
                debug!("Successfully loaded {} signal points", points.len());
                combined.extend(points);
            },
            Err(e) => {
                warn!("Failed to load signal points: {}", e);
            },
        }

        // Load and merge control points
        match Self::load_and_combine_telemetry(
            &four_remote_base.join(&table_config.four_remote_files.control_file),
            &protocol_base.join(&table_config.protocol_mapping_file.control_mapping),
            "Control",
            csv_loader,
        )
        .await
        {
            Ok(points) => {
                debug!("Successfully loaded {} control points", points.len());
                combined.extend(points);
            },
            Err(e) => {
                warn!("Failed to load control points: {}", e);
            },
        }

        // Load and merge adjustment points
        match Self::load_and_combine_telemetry(
            &four_remote_base.join(&table_config.four_remote_files.adjustment_file),
            &protocol_base.join(&table_config.protocol_mapping_file.adjustment_mapping),
            "Adjustment",
            csv_loader,
        )
        .await
        {
            Ok(points) => {
                debug!("Successfully loaded {} adjustment points", points.len());
                combined.extend(points);
            },
            Err(e) => {
                warn!("Failed to load adjustment points: {}", e);
            },
        }

        info!("Loaded {} total combined points", combined.len());
        Ok(combined)
    }

    /// Load and merge points of single telemetry type
    async fn load_and_combine_telemetry(
        four_remote_path: &Path,
        protocol_mapping_path: &Path,
        telemetry_type: &str,
        csv_loader: &CachedCsvLoader,
    ) -> Result<Vec<CombinedPoint>> {
        debug!("load_and_combine_telemetry for {telemetry_type}");
        debug!("four_remote_path: {}", four_remote_path.display());
        debug!("protocol_mapping_path: {}", protocol_mapping_path.display());

        // Check if file exists
        debug!(
            "Checking if four_remote_path exists: {} - {}",
            four_remote_path.display(),
            four_remote_path.exists()
        );
        if !four_remote_path.exists() {
            debug!(
                "Four remote file not found: {}, skipping",
                four_remote_path.display()
            );
            return Ok(Vec::new());
        }

        debug!(
            "Checking if protocol_mapping_path exists: {} - {}",
            protocol_mapping_path.display(),
            protocol_mapping_path.exists()
        );
        if !protocol_mapping_path.exists() {
            debug!(
                "Protocol mapping file not found: {}, skipping",
                protocol_mapping_path.display()
            );
            debug!(
                "Protocol mapping file not found: {}",
                protocol_mapping_path.display()
            );
            return Ok(Vec::new());
        }

        // Load four-telemetry file
        debug!("Loading four_remote_path CSV...");
        let remote_points: Vec<FourRemoteRecord> = csv_loader
            .load_csv_cached(four_remote_path)
            .await
            .map_err(|e| {
                ComSrvError::ConfigError(format!(
                    "Failed to load {telemetry_type} four remote file: {e}"
                ))
            })?;
        debug!("Loaded {} four remote points", remote_points.len());

        // Load protocol mapping file
        debug!("Loading protocol_mapping_path CSV...");
        let modbus_mappings_result = csv_loader
            .load_csv_cached::<ModbusMappingRecord>(protocol_mapping_path)
            .await;

        let modbus_mappings = match modbus_mappings_result {
            Ok(mappings) => {
                debug!("Successfully loaded {} modbus mappings", mappings.len());
                if !mappings.is_empty() {
                    debug!("First modbus mapping: {:?}", mappings[0]);
                }
                mappings
            },
            Err(e) => {
                debug!("Error loading modbus mappings: {e}");
                return Err(ComSrvError::ConfigError(format!(
                    "Failed to load {telemetry_type} protocol mapping file: {e}"
                )));
            },
        };

        debug!(
            "Loaded {} {} remote points and {} mappings",
            remote_points.len(),
            telemetry_type,
            modbus_mappings.len()
        );
        debug!("Combining points...");

        // Use PointMapper to merge points
        let combined =
            PointMapper::combine_modbus_points(remote_points, modbus_mappings, telemetry_type)?;
        debug!("Combined {} points for {}", combined.len(), telemetry_type);

        Ok(combined)
    }

    /// Get service configuration
    pub fn service_config(&self) -> &ServiceConfig {
        &self.config.service
    }

    /// Get all channel configurations
    pub fn channels(&self) -> &[ChannelConfig] {
        &self.config.channels
    }

    /// Get channel configuration by ID
    pub fn get_channel(&self, channel_id: u16) -> Option<&ChannelConfig> {
        self.config.channels.iter().find(|c| c.id == channel_id)
    }

    /// Get channel count
    pub fn channel_count(&self) -> usize {
        self.config.channels.len()
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        // Check channel ID uniqueness
        let mut channel_ids = std::collections::HashSet::new();
        for channel in &self.config.channels {
            if !channel_ids.insert(channel.id) {
                return Err(ComSrvError::ConfigError(format!(
                    "Duplicate channel ID: {}",
                    channel.id
                )));
            }
        }

        Ok(())
    }

    // Under the four-telemetry separated architecture, unified mapping method is no longer needed
}

// ============================================================================
// CSV loader - deprecated, use new implementation in loaders.rs
// ============================================================================

// Note: The following CsvLoader has been replaced by CachedCsvLoader in loaders.rs
// These type definitions are retained only for backward compatibility and will be removed in future versions

/*
/// Unified CSV loader
#[derive(Debug)]
pub struct CsvLoader;

/// Four-telemetry point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FourRemotePoint {
    pub point_id: u32,
    pub signal_name: String,
    pub telemetry_type: String,
    pub scale: Option<f64>,
    pub offset: Option<f64>,
    pub unit: Option<String>,
    pub reverse: Option<bool>,
    pub data_type: String,
}

/// Modbus mapping
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModbusMapping {
    pub point_id: u32,
    pub slave_id: u8,
    pub function_code: u8,
    pub register_address: u16,
    pub bit_position: Option<u8>,
    pub data_format: Option<String>,
    pub register_count: Option<u16>,
}
*/

// Deprecated CsvLoader implementation, kept for reference
/*
impl CsvLoader {
    /// Load all CSV tables for channel
    pub fn load_channel_tables(
        table_config: &TableConfig,
        config_dir: &Path,
    ) -> Result<Vec<CombinedPoint>> {
        info!("Loading CSV tables for channel");

        // Store points separately by four-telemetry type
        let mut telemetry_points = HashMap::new();
        let mut signal_points = HashMap::new();
        let mut control_points = HashMap::new();
        let mut adjustment_points = HashMap::new();

        // Check environment variable override
        let base_dir = std::env::var("COMSRV_CSV_BASE_PATH")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| config_dir.to_path_buf());

        debug!("Using CSV base directory: {}", base_dir.display());

        // Load telemetry file
        let base_path = base_dir.join(&table_config.four_remote_route);

        // Telemetry
        if let Some(telemetry_data) = Self::load_telemetry_file(
            &base_path.join(&table_config.four_remote_files.telemetry_file),
            "Telemetry",
        )? {
            for point in telemetry_data {
                telemetry_points.insert(point.point_id, point);
            }
        }

        // Signal
        if let Some(signal_data) = Self::load_signal_file(
            &base_path.join(&table_config.four_remote_files.signal_file),
            "Signal",
        )? {
            for point in signal_data {
                signal_points.insert(point.point_id, point);
            }
        }

        // Adjustment
        if let Some(adjustment_data) = Self::load_telemetry_file(
            &base_path.join(&table_config.four_remote_files.adjustment_file),
            "Adjustment",
        )? {
            for point in adjustment_data {
                adjustment_points.insert(point.point_id, point);
            }
        }

        // Control
        if let Some(control_data) = Self::load_signal_file(
            &base_path.join(&table_config.four_remote_files.control_file),
            "Control",
        )? {
            for point in control_data {
                control_points.insert(point.point_id, point);
            }
        }

        // Load protocol mapping
        let protocol_path = base_dir.join(&table_config.protocol_mapping_route);

        // Load corresponding mapping files for each telemetry type separately to avoid point ID conflicts
        let mut combined = Vec::new();

        // Merge telemetry points
        if let Ok(telemetry_mappings) = Self::load_protocol_mappings(
            &protocol_path.join(&table_config.protocol_mapping_file.telemetry_mapping),
        ) {
            debug!("Loaded {} telemetry mappings", telemetry_mappings.len());
            let telemetry_combined = Self::combine_points_by_type(
                telemetry_points,
                &telemetry_mappings,
                "Telemetry",
            )?;
            combined.extend(telemetry_combined);
        }

        // Merge signal points
        if let Ok(signal_mappings) = Self::load_protocol_mappings(
            &protocol_path.join(&table_config.protocol_mapping_file.signal_mapping),
        ) {
            debug!("Loaded {} signal mappings", signal_mappings.len());
            let signal_combined =
                Self::combine_points_by_type(signal_points, &signal_mappings, "Signal")?;
            combined.extend(signal_combined);
        }

        // Merge adjustment points
        if let Ok(adjustment_mappings) = Self::load_protocol_mappings(
            &protocol_path.join(&table_config.protocol_mapping_file.adjustment_mapping),
        ) {
            debug!("Loaded {} adjustment mappings", adjustment_mappings.len());
            let adjustment_combined = Self::combine_points_by_type(
                adjustment_points,
                &adjustment_mappings,
                "Adjustment",
            )?;
            combined.extend(adjustment_combined);
        }

        // Merge control points
        if let Ok(control_mappings) = Self::load_protocol_mappings(
            &protocol_path.join(&table_config.protocol_mapping_file.control_mapping),
        ) {
            debug!("Loaded {} control mappings", control_mappings.len());
            let control_combined =
                Self::combine_points_by_type(control_points, &control_mappings, "Control")?;
            combined.extend(control_combined);
        }

        Ok(combined)
    }

    /// Load telemetry file (with scaling parameters)
    fn load_telemetry_file(
        path: &Path,
        telemetry_type: &str,
    ) -> Result<Option<Vec<FourRemotePoint>>> {
        if !path.exists() {
            debug!("File not found: {}, skipping", path.display());
            return Ok(None);
        }

        debug!("Loading {} file: {}", telemetry_type, path.display());

        let mut reader = ReaderBuilder::new()
            .has_headers(true)
            .from_path(path)
            .map_err(|e| ComSrvError::IoError(format!("Failed to open CSV file: {}", e)))?;

        let mut points = Vec::new();

        for result in reader.records() {
            let record = result
                .map_err(|e| ComSrvError::IoError(format!("Failed to read CSV record: {}", e)))?;

            let point = FourRemotePoint {
                point_id: record
                    .get(0)
                    .ok_or_else(|| ComSrvError::ConfigError("Missing point_id".to_string()))?
                    .parse()
                    .map_err(|_| ComSrvError::ConfigError("Invalid point_id".to_string()))?,
                signal_name: record.get(1).unwrap_or("Unknown").to_string(),
                telemetry_type: telemetry_type.to_string(),
                scale: record.get(2).and_then(|s| s.parse().ok()),
                offset: record.get(3).and_then(|s| s.parse().ok()),
                unit: record.get(4).map(|s| s.to_string()),
                reverse: record.get(5).and_then(|s| s.parse().ok()),
                data_type: record.get(6).unwrap_or("float").to_string(),
            };

            points.push(point);
        }

        debug!("Loaded {} {} points", points.len(), telemetry_type);
        Ok(Some(points))
    }

    /// Load signal file (without scaling parameters)
    fn load_signal_file(path: &Path, telemetry_type: &str) -> Result<Option<Vec<FourRemotePoint>>> {
        if !path.exists() {
            debug!("File not found: {}, skipping", path.display());
            return Ok(None);
        }

        debug!("Loading {} file: {}", telemetry_type, path.display());

        let mut reader = ReaderBuilder::new()
            .has_headers(true)
            .from_path(path)
            .map_err(|e| ComSrvError::IoError(format!("Failed to open CSV file: {}", e)))?;

        let mut points = Vec::new();

        for result in reader.records() {
            let record = result
                .map_err(|e| ComSrvError::IoError(format!("Failed to read CSV record: {}", e)))?;

            let point = FourRemotePoint {
                point_id: record
                    .get(0)
                    .ok_or_else(|| ComSrvError::ConfigError("Missing point_id".to_string()))?
                    .parse()
                    .map_err(|_| ComSrvError::ConfigError("Invalid point_id".to_string()))?,
                signal_name: record.get(1).unwrap_or("Unknown").to_string(),
                telemetry_type: telemetry_type.to_string(),
                scale: None,
                offset: None,
                unit: None,
                reverse: record.get(2).and_then(|s| s.parse().ok()),
                data_type: "bool".to_string(),
            };

            points.push(point);
        }

        debug!("Loaded {} {} points", points.len(), telemetry_type);
        Ok(Some(points))
    }

    /// Load protocol mapping
    fn load_protocol_mappings(path: &Path) -> Result<HashMap<u32, HashMap<String, String>>> {
        if !path.exists() {
            return Err(ComSrvError::ConfigError(format!(
                "Protocol mapping file not found: {}",
                path.display()
            )));
        }

        debug!("Loading protocol mappings: {}", path.display());

        let mut reader = ReaderBuilder::new()
            .has_headers(true)
            .from_path(path)
            .map_err(|e| ComSrvError::IoError(format!("Failed to open CSV file: {}", e)))?;

        let mut mappings = HashMap::new();

        for result in reader.records() {
            let record = result
                .map_err(|e| ComSrvError::IoError(format!("Failed to read CSV record: {}", e)))?;

            let point_id: u32 = record
                .get(0)
                .ok_or_else(|| ComSrvError::ConfigError("Missing point_id".to_string()))?
                .parse()
                .map_err(|_| ComSrvError::ConfigError("Invalid point_id".to_string()))?;

            let mut params = HashMap::new();

            // Modbus parameters
            if let (Some(slave_id), Some(function_code), Some(register_address)) =
                (record.get(1), record.get(2), record.get(3))
            {
                params.insert("slave_id".to_string(), slave_id.to_string());
                params.insert("function_code".to_string(), function_code.to_string());
                params.insert("register_address".to_string(), register_address.to_string());

                // Optional parameters
                if let Some(bit_position) = record.get(4) {
                    if !bit_position.is_empty() {
                        params.insert("bit_position".to_string(), bit_position.to_string());
                    }
                }
                if let Some(data_format) = record.get(5) {
                    if !data_format.is_empty() {
                        params.insert("data_format".to_string(), data_format.to_string());
                    }
                }
                if let Some(register_count) = record.get(6) {
                    if !register_count.is_empty() {
                        params.insert("register_count".to_string(), register_count.to_string());
                    }
                }
            }

            mappings.insert(point_id, params);
        }

        debug!("Loaded {} protocol mappings", mappings.len());
        Ok(mappings)
    }

    /// Merge point information by type, maintaining four-telemetry separation
    fn combine_points_by_type(
        telemetry_points: HashMap<u32, FourRemotePoint>,
        protocol_mappings: &HashMap<u32, HashMap<String, String>>,
        telemetry_type: &str,
    ) -> Result<Vec<CombinedPoint>> {
        let mut combined = Vec::new();

        for (point_id, telemetry_point) in telemetry_points {
            if let Some(protocol_params) = protocol_mappings.get(&point_id) {
                let point = CombinedPoint {
                    point_id,
                    signal_name: telemetry_point.signal_name,
                    telemetry_type: telemetry_point.telemetry_type,
                    data_type: telemetry_point.data_type,
                    protocol_params: protocol_params.clone(),
                    scaling: if telemetry_point.scale.is_some()
                        || telemetry_point.offset.is_some()
                        || telemetry_point.reverse.is_some()
                    {
                        Some(super::types::ScalingInfo {
                            scale: telemetry_point.scale.unwrap_or(1.0),
                            offset: telemetry_point.offset.unwrap_or(0.0),
                            unit: telemetry_point.unit,
                            reverse: telemetry_point.reverse,
                        })
                    } else {
                        None
                    },
                };
                combined.push(point);
            } else {
                debug!(
                    "No protocol mapping found for {} point_id: {}",
                    telemetry_type, point_id
                );
            }
        }

        debug!(
            "Combined {} {} points with protocol mappings",
            combined.len(),
            telemetry_type
        );
        Ok(combined)
    }
}
*/

// ============================================================================
// File system configuration source implementation
// ============================================================================

/// File system configuration source
#[derive(Debug)]
pub struct FileSystemSource {
    base_path: String,
}

impl FileSystemSource {
    pub fn new(base_path: String) -> Self {
        Self { base_path }
    }
}

#[async_trait]
impl ConfigSource for FileSystemSource {
    async fn fetch_config(&self, service_name: &str) -> Result<ConfigResponse> {
        let path = Path::new(&self.base_path).join(format!("{service_name}.yml"));

        let content = fs::read_to_string(&path)
            .await
            .map_err(|e| ComSrvError::IoError(format!("Failed to read config file: {e}")))?;

        let value: serde_json::Value = serde_yaml::from_str(&content)
            .map_err(|e| ComSrvError::ConfigError(format!("Failed to parse YAML: {e}")))?;

        Ok(ConfigResponse {
            version: "1.0.0".to_string(),
            checksum: format!("{:x}", md5::compute(&content)),
            last_modified: std::fs::metadata(&path)
                .ok()
                .and_then(|m| m.modified().ok())
                .map_or_else(|| "Unknown".to_string(), |t| format!("{t:?}")),
            content: value,
        })
    }

    async fn fetch_item(&self, service_name: &str, key: &str) -> Result<ConfigItemResponse> {
        let config = self.fetch_config(service_name).await?;

        let value = config
            .content
            .get(key)
            .ok_or_else(|| ComSrvError::ConfigError(format!("Key '{key}' not found")))?
            .clone();

        Ok(ConfigItemResponse {
            key: key.to_string(),
            value_type: match &value {
                serde_json::Value::Bool(_) => "bool",
                serde_json::Value::Number(_) => "number",
                serde_json::Value::String(_) => "string",
                serde_json::Value::Array(_) => "array",
                serde_json::Value::Object(_) => "object",
                serde_json::Value::Null => "null",
            }
            .to_string(),
            value,
        })
    }

    fn name(&self) -> &'static str {
        "filesystem"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_center_client_creation() {
        let client = ConfigCenterClient::from_env("test_service".to_string());
        assert_eq!(client.service_name, "test_service");
        assert_eq!(client.cache_duration, 300);
    }

    #[tokio::test]
    async fn test_filesystem_source() {
        use tempfile::tempdir;
        use tokio::fs;

        let dir = tempdir().expect("failed to create temporary directory for test");
        let config_path = dir.path().join("test.yml");

        let config_content = r"
service:
  name: test
  version: 1.0.0
";

        fs::write(&config_path, config_content)
            .await
            .expect("failed to write test config file");

        let source = FileSystemSource::new(dir.path().to_string_lossy().to_string());
        let config = source
            .fetch_config("test")
            .await
            .expect("failed to fetch test config");

        assert_eq!(config.version, "1.0.0");
        assert!(config.content.get("service").is_some());
    }
}
