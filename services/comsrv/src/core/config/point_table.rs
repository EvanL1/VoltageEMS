use crate::utils::error::{ComSrvError, Result};
use crate::core::protocols::modbus::common::{ModbusRegisterMapping, ModbusDataType, ModbusRegisterType, ByteOrder};
use crate::core::metrics::DataPoint;
use std::path::Path;
use std::fs;
use std::collections::{HashMap, BTreeMap};
use std::time::{Duration, Instant, SystemTime};
use std::sync::Arc;
use csv::ReaderBuilder;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tokio::time::interval;

// Serde helper module for SystemTime serialization
mod timestamp_as_seconds {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::{SystemTime, UNIX_EPOCH};

    pub fn serialize<S>(time: &SystemTime, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let duration = time.duration_since(UNIX_EPOCH)
            .map_err(serde::ser::Error::custom)?;
        serializer.serialize_u64(duration.as_secs())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<SystemTime, D::Error>
    where
        D: Deserializer<'de>,
    {
        let seconds = u64::deserialize(deserializer)?;
        Ok(UNIX_EPOCH + std::time::Duration::from_secs(seconds))
    }
}

/// point table entry, used to deserialize from csv file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointTableEntry {
    /// point id
    pub id: u16,
    /// point name
    pub name: String,
    /// point description (optional)
    #[serde(default)]
    pub description: String,
    /// register address
    pub address: u16,
    /// point type (coil, discrete_input, holding_register, input_register)
    #[serde(rename = "type_")]
    pub type_: String,
    /// data type (for register type, such as uint16, int16, float32, etc.)
    #[serde(default)]
    pub data_type: String,
    /// whether writable
    #[serde(default)]
    pub writable: bool,
    /// byte order (ABCD, CDAB, BADC, DCBA)
    #[serde(default)]
    pub byte_order: String,
    /// scale factor
    #[serde(default)]
    pub scale: Option<f64>,
    /// offset
    #[serde(default)]
    pub offset: Option<f64>,
    /// unit
    #[serde(default)]
    pub unit: String,
    /// deadband
    #[serde(default)]
    pub deadband: Option<f64>,
}

/// Enhanced point table configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointTableConfig {
    /// Enable auto reload
    pub auto_reload: bool,
    /// Reload check interval in seconds
    pub reload_check_interval: u64,
    /// Enable point table optimization
    pub enable_optimization: bool,
    /// Enable point value caching
    pub enable_point_cache: bool,
    /// Cache TTL in seconds
    pub cache_ttl: u64,
    /// Enable performance monitoring
    pub enable_performance_monitoring: bool,
}

impl Default for PointTableConfig {
    fn default() -> Self {
        Self {
            auto_reload: false,
            reload_check_interval: 30,
            enable_optimization: false,
            enable_point_cache: false,
            cache_ttl: 300,
            enable_performance_monitoring: false,
        }
    }
}

/// Point table statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointTableStats {
    /// Total points count
    pub total_points: usize,
    /// Points grouped by register type
    pub points_by_register_type: HashMap<String, usize>,
    /// Points grouped by data type
    pub points_by_data_type: HashMap<String, usize>,
    /// Address range
    pub address_range: Option<(u16, u16)>,
    /// Last updated time (as seconds since Unix epoch)
    #[serde(with = "timestamp_as_seconds")]
    pub last_updated: SystemTime,
    /// Optimization suggestions
    pub optimization_suggestions: Vec<String>,
}

/// Point value cache entry
#[derive(Debug, Clone)]
pub struct PointCacheEntry {
    /// Data point
    pub data_point: DataPoint,
    /// Cache timestamp
    pub cached_at: Instant,
    /// Cache TTL
    pub ttl: Duration,
}

impl PointCacheEntry {
    /// Check if cache entry is expired
    pub fn is_expired(&self) -> bool {
        Instant::now() > self.cached_at + self.ttl
    }
}

/// Point table optimization suggestion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointTableOptimization {
    /// Suggestion type
    pub suggestion_type: OptimizationType,
    /// Description
    pub description: String,
    /// Affected points count
    pub affected_points: usize,
    /// Expected performance improvement
    pub performance_improvement: String,
}

/// Optimization types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OptimizationType {
    /// Address reordering
    AddressReordering,
    /// Batch reading
    BatchReading,
    /// Data type optimization
    DataTypeOptimization,
    /// Remove unused points
    RemoveUnusedPoints,
}

impl OptimizationType {
    fn to_string(&self) -> &'static str {
        match self {
            OptimizationType::AddressReordering => "Address Reordering",
            OptimizationType::BatchReading => "Batch Reading",
            OptimizationType::DataTypeOptimization => "Data Type Optimization",
            OptimizationType::RemoveUnusedPoints => "Remove Unused Points",
        }
    }
}

/// Enhanced point table manager
pub struct PointTableManager {
    /// Base config path
    base_path: String,
    /// Configuration
    config: PointTableConfig,
    /// Loaded point table mappings (channel_id -> mappings)
    loaded_mappings: Arc<RwLock<HashMap<u16, Vec<ModbusRegisterMapping>>>>,
    /// Point table statistics
    stats: Arc<RwLock<HashMap<u16, PointTableStats>>>,
    /// Point value cache
    point_cache: Arc<RwLock<HashMap<String, PointCacheEntry>>>,
    /// File monitors
    file_monitors: Arc<RwLock<HashMap<String, Instant>>>,
    /// Is running flag
    is_running: Arc<RwLock<bool>>,
}

impl PointTableManager {
    /// Create point table manager with default config (backward compatible)
    pub fn new(base_path: impl AsRef<Path>) -> Self {
        Self::new_with_config(base_path, PointTableConfig::default())
    }

    /// Create point table manager with custom config
    pub fn new_with_config(base_path: impl AsRef<Path>, config: PointTableConfig) -> Self {
        let path_str = base_path.as_ref().to_string_lossy().to_string();
        Self {
            base_path: path_str,
            config,
            loaded_mappings: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(HashMap::new())),
            point_cache: Arc::new(RwLock::new(HashMap::new())),
            file_monitors: Arc::new(RwLock::new(HashMap::new())),
            is_running: Arc::new(RwLock::new(false)),
        }
    }

    /// Start enhanced features (if enabled)
    pub async fn start(&self) -> Result<()> {
        {
            let mut running = self.is_running.write().await;
            if *running {
                return Err(ComSrvError::StateError("Point table manager is already running".to_string()));
            }
            *running = true;
        }

        if self.config.auto_reload {
            self.start_auto_reload_task().await;
        }

        if self.config.enable_point_cache {
            self.start_cache_cleanup_task().await;
        }

        log::info!("Point table manager started with enhanced features");
        Ok(())
    }

    /// Stop enhanced features
    pub async fn stop(&self) -> Result<()> {
        {
            let mut running = self.is_running.write().await;
            *running = false;
        }
        log::info!("Point table manager stopped");
        Ok(())
    }

    /// Load point table from csv or yaml file (enhanced version for channels)
    pub async fn load_channel_point_table(
        &self,
        channel_id: u16,
        point_table_path: &str,
    ) -> Result<Vec<ModbusRegisterMapping>> {
        // Load using basic functionality
        let mappings = self.load_point_table(point_table_path)?;

        // Apply enhancements if enabled
        let optimized_mappings = if self.config.enable_optimization {
            self.optimize_point_table(&mappings).await?
        } else {
            mappings
        };

        // Generate statistics if performance monitoring is enabled
        if self.config.enable_performance_monitoring {
            let stats = self.generate_point_table_stats(&optimized_mappings).await;
            let mut stats_map = self.stats.write().await;
            stats_map.insert(channel_id, stats);
        }

        // Store mappings
        {
            let mut loaded_mappings = self.loaded_mappings.write().await;
            loaded_mappings.insert(channel_id, optimized_mappings.clone());
        }

        // Record file monitoring info
        {
            let mut file_monitors = self.file_monitors.write().await;
            file_monitors.insert(point_table_path.to_string(), Instant::now());
        }

        log::info!(
            "Loaded {} points for channel {} from {}",
            optimized_mappings.len(),
            channel_id,
            point_table_path
        );

        Ok(optimized_mappings)
    }

    /// load point table from csv or yaml file (original functionality, kept for backward compatibility)
    pub fn load_point_table(&self, point_table_path: &str) -> Result<Vec<ModbusRegisterMapping>> {
        let full_path = self.get_full_path(point_table_path);
        let path = Path::new(&full_path);
        
        if !path.exists() {
            return Err(ComSrvError::PointTableError(format!(
                "Point table file not found: {}",
                full_path
            )));
        }
        
        let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        
        match extension.to_lowercase().as_str() {
            "csv" => self.load_csv_point_table(&full_path),
            "yaml" | "yml" => self.load_yaml_point_table(&full_path),
            _ => Err(ComSrvError::PointTableError(format!(
                "Unsupported point table file format: {}",
                extension
            ))),
        }
    }

    /// Get channel mappings (enhanced feature)
    pub async fn get_channel_mappings(&self, channel_id: u16) -> Option<Vec<ModbusRegisterMapping>> {
        let loaded_mappings = self.loaded_mappings.read().await;
        loaded_mappings.get(&channel_id).cloned()
    }

    /// Get channel statistics (enhanced feature)
    pub async fn get_channel_stats(&self, channel_id: u16) -> Option<PointTableStats> {
        let stats_map = self.stats.read().await;
        stats_map.get(&channel_id).cloned()
    }

    /// Cache point value (enhanced feature)
    pub async fn cache_point_value(&self, channel_id: u16, point_name: &str, data_point: DataPoint) {
        if !self.config.enable_point_cache {
            return;
        }

        let cache_key = format!("{}:{}", channel_id, point_name);
        let cache_entry = PointCacheEntry {
            data_point,
            cached_at: Instant::now(),
            ttl: Duration::from_secs(self.config.cache_ttl),
        };

        let mut cache = self.point_cache.write().await;
        cache.insert(cache_key, cache_entry);
    }

    /// Get cached point value (enhanced feature)
    pub async fn get_cached_point_value(&self, channel_id: u16, point_name: &str) -> Option<DataPoint> {
        if !self.config.enable_point_cache {
            return None;
        }

        let cache_key = format!("{}:{}", channel_id, point_name);
        let cache = self.point_cache.read().await;
        
        if let Some(entry) = cache.get(&cache_key) {
            if !entry.is_expired() {
                return Some(entry.data_point.clone());
            }
        }

        None
    }

    /// Clear channel cache (enhanced feature)
    pub async fn clear_channel_cache(&self, channel_id: u16) {
        let prefix = format!("{}:", channel_id);
        let mut cache = self.point_cache.write().await;
        cache.retain(|key, _| !key.starts_with(&prefix));
    }

    /// Validate point table configuration (enhanced feature)
    pub async fn validate_point_table(&self, mappings: &[ModbusRegisterMapping]) -> Result<Vec<String>> {
        let mut warnings = Vec::new();

        // Check for duplicate addresses
        let mut address_map: HashMap<(ModbusRegisterType, u16), Vec<&str>> = HashMap::new();
        for mapping in mappings {
            address_map.entry((mapping.register_type.clone(), mapping.address))
                .or_insert_with(Vec::new)
                .push(&mapping.name);
        }

        for ((register_type, address), names) in address_map {
            if names.len() > 1 {
                warnings.push(format!(
                    "Duplicate address {} for {:?} registers: {}",
                    address,
                    register_type,
                    names.join(", ")
                ));
            }
        }

        // Check for duplicate names
        let mut name_counts = HashMap::new();
        for mapping in mappings {
            *name_counts.entry(&mapping.name).or_insert(0) += 1;
        }

        for (name, count) in name_counts {
            if count > 1 {
                warnings.push(format!("Duplicate point name: {} (appears {} times)", name, count));
            }
        }

        // Check address ranges
        for mapping in mappings {
            if mapping.address > 65535 {
                warnings.push(format!("Point '{}' has invalid address: {}", mapping.name, mapping.address));
            }
        }

        Ok(warnings)
    }

    /// Get summary statistics for all channels (enhanced feature)
    pub async fn get_summary_stats(&self) -> HashMap<u16, PointTableStats> {
        self.stats.read().await.clone()
    }

    /// Optimize point table (enhanced feature)
    async fn optimize_point_table(&self, mappings: &[ModbusRegisterMapping]) -> Result<Vec<ModbusRegisterMapping>> {
        let mut optimized = mappings.to_vec();

        // Sort by address to optimize read efficiency
        optimized.sort_by_key(|m| (m.register_type.clone() as u8, m.address));

        // Generate optimization suggestions
        let suggestions = self.generate_optimization_suggestions(&optimized).await;
        
        if !suggestions.is_empty() {
            log::info!("Point table optimization suggestions:");
            for suggestion in &suggestions {
                log::info!("  - {}: {}", suggestion.suggestion_type.to_string(), suggestion.description);
            }
        }

        Ok(optimized)
    }

    /// Generate point table statistics (enhanced feature)
    async fn generate_point_table_stats(&self, mappings: &[ModbusRegisterMapping]) -> PointTableStats {
        let mut points_by_register_type = HashMap::new();
        let mut points_by_data_type = HashMap::new();
        let mut min_address = u16::MAX;
        let mut max_address = 0u16;

        for mapping in mappings {
            // Count by register type
            let register_type_str = format!("{:?}", mapping.register_type);
            *points_by_register_type.entry(register_type_str).or_insert(0) += 1;

            // Count by data type
            let data_type_str = format!("{:?}", mapping.data_type);
            *points_by_data_type.entry(data_type_str).or_insert(0) += 1;

            // Calculate address range
            min_address = min_address.min(mapping.address);
            max_address = max_address.max(mapping.address);
        }

        let address_range = if mappings.is_empty() {
            None
        } else {
            Some((min_address, max_address))
        };

        let optimization_suggestions = self.generate_optimization_suggestions(mappings).await;

        PointTableStats {
            total_points: mappings.len(),
            points_by_register_type,
            points_by_data_type,
            address_range,
            last_updated: SystemTime::now(),
            optimization_suggestions: optimization_suggestions.into_iter()
                .map(|s| format!("{}: {}", s.suggestion_type.to_string(), s.description))
                .collect(),
        }
    }

    /// Generate optimization suggestions (enhanced feature)
    async fn generate_optimization_suggestions(&self, mappings: &[ModbusRegisterMapping]) -> Vec<PointTableOptimization> {
        let mut suggestions = Vec::new();

        // Check address continuity
        let mut grouped_by_type: BTreeMap<ModbusRegisterType, Vec<&ModbusRegisterMapping>> = BTreeMap::new();
        for mapping in mappings {
            grouped_by_type.entry(mapping.register_type.clone()).or_insert_with(Vec::new).push(mapping);
        }

        for (register_type, group) in grouped_by_type {
            if group.len() > 1 {
                let mut addresses: Vec<u16> = group.iter().map(|m| m.address).collect();
                addresses.sort();
                
                // Check for consecutive addresses for batch reading
                let mut consecutive_groups = Vec::new();
                let mut current_group = vec![addresses[0]];
                
                for &addr in &addresses[1..] {
                    if addr == current_group.last().unwrap() + 1 {
                        current_group.push(addr);
                    } else {
                        if current_group.len() > 1 {
                            consecutive_groups.push(current_group.clone());
                        }
                        current_group = vec![addr];
                    }
                }
                
                if current_group.len() > 1 {
                    consecutive_groups.push(current_group);
                }

                if !consecutive_groups.is_empty() {
                    let total_consecutive = consecutive_groups.iter().map(|g| g.len()).sum::<usize>();
                    suggestions.push(PointTableOptimization {
                        suggestion_type: OptimizationType::BatchReading,
                        description: format!(
                            "{} consecutive address groups found for {:?} registers, {} points total",
                            consecutive_groups.len(), register_type, total_consecutive
                        ),
                        affected_points: total_consecutive,
                        performance_improvement: "20-50% reduction in communication overhead".to_string(),
                    });
                }
            }
        }

        // Check data type optimization opportunities
        let float32_count = mappings.iter().filter(|m| matches!(m.data_type, ModbusDataType::Float32)).count();
        if float32_count > 0 {
            suggestions.push(PointTableOptimization {
                suggestion_type: OptimizationType::DataTypeOptimization,
                description: format!("{} Float32 points could benefit from proper byte order configuration", float32_count),
                affected_points: float32_count,
                performance_improvement: "Improved data accuracy and reduced conversion overhead".to_string(),
            });
        }

        suggestions
    }

    /// Start auto reload task (enhanced feature)
    async fn start_auto_reload_task(&self) {
        let config = self.config.clone();
        let file_monitors = self.file_monitors.clone();
        let is_running = self.is_running.clone();

        tokio::spawn(async move {
            let mut reload_interval = interval(Duration::from_secs(config.reload_check_interval));

            while *is_running.read().await {
                reload_interval.tick().await;

                // Check file modification times
                log::debug!("Checking for point table file modifications");
            }
        });
    }

    /// Start cache cleanup task (enhanced feature)
    async fn start_cache_cleanup_task(&self) {
        let point_cache = self.point_cache.clone();
        let is_running = self.is_running.clone();

        tokio::spawn(async move {
            let mut cleanup_interval = interval(Duration::from_secs(60)); // Clean every minute

            while *is_running.read().await {
                cleanup_interval.tick().await;

                // Clean expired cache entries
                let mut cache = point_cache.write().await;
                let initial_count = cache.len();
                cache.retain(|_, entry| !entry.is_expired());
                let removed_count = initial_count - cache.len();

                if removed_count > 0 {
                    log::debug!("Cleaned up {} expired cache entries", removed_count);
                }
            }
        });
    }
    
    /// load point table from .csv
    fn load_csv_point_table(&self, path: &str) -> Result<Vec<ModbusRegisterMapping>> {
        let file = fs::File::open(path)
            .map_err(|e| ComSrvError::IoError(e.to_string()))?;
            
        let mut reader = ReaderBuilder::new()
            .has_headers(true)
            .delimiter(b',')
            .from_reader(file);
            
        let mut mappings = Vec::new();
        
        // iterate over CSV records
        for result in reader.deserialize() {
            let entry: PointTableEntry = result
                .map_err(|e| ComSrvError::PointTableError(format!(
                    "Failed to parse CSV record in {}: {}",
                    path, e
                )))?;
                
            // determine quantity based on type
            let (data_type, quantity) = self.parse_data_type(&entry.type_, &entry.data_type)?;
            
            // determine byte order
            let byte_order = self.parse_byte_order(&entry.byte_order)?;
            
            // create mapping
            let mapping = ModbusRegisterMapping {
                name: entry.name.clone(),
                display_name: Some(entry.name.clone()),
                register_type: match entry.type_.to_lowercase().as_str() {
                    "coil" | "do" => ModbusRegisterType::Coil,
                    "discrete_input" | "di" => ModbusRegisterType::DiscreteInput,
                    "holding_register" | "ao" => ModbusRegisterType::HoldingRegister,
                    "input_register" | "ai" => ModbusRegisterType::InputRegister,
                    _ => ModbusRegisterType::HoldingRegister,
                },
                address: entry.address,
                data_type,
                scale: entry.scale.unwrap_or(1.0),
                offset: entry.offset.unwrap_or(0.0),
                unit: if entry.unit.is_empty() { None } else { Some(entry.unit.clone()) },
                description: if entry.description.is_empty() { None } else { Some(entry.description.clone()) },
                access_mode: if entry.writable { "read_write".to_string() } else { "read".to_string() },
                group: None,
                byte_order,
            };
            
            mappings.push(mapping);
        }
        
        Ok(mappings)
    }
    
    /// load point table from yaml file
    fn load_yaml_point_table(&self, path: &str) -> Result<Vec<ModbusRegisterMapping>> {
        let content = fs::read_to_string(path)
            .map_err(|e| ComSrvError::IoError(e.to_string()))?;
            
        let points: HashMap<String, Vec<PointTableEntry>> = serde_yaml::from_str(&content)
            .map_err(|e| ComSrvError::PointTableError(format!(
                "Failed to parse YAML point table {}: {}",
                path, e
            )))?;
            
        let entries = points.get("points")
            .ok_or_else(|| ComSrvError::PointTableError(format!(
                "No 'points' section in YAML point table: {}",
                path
            )))?;
            
        let mut mappings = Vec::new();
        
        for entry in entries {
            // determine quantity based on type
            let (data_type, _quantity) = self.parse_data_type(&entry.type_, &entry.data_type)?;
            
            // determine byte order
            let byte_order = self.parse_byte_order(&entry.byte_order)?;
            
            // create mapping
            let mapping = ModbusRegisterMapping {
                name: entry.name.clone(),
                display_name: Some(entry.name.clone()),
                register_type: match entry.type_.to_lowercase().as_str() {
                    "coil" | "do" => ModbusRegisterType::Coil,
                    "discrete_input" | "di" => ModbusRegisterType::DiscreteInput,
                    "holding_register" | "ao" => ModbusRegisterType::HoldingRegister,
                    "input_register" | "ai" => ModbusRegisterType::InputRegister,
                    _ => ModbusRegisterType::HoldingRegister,
                },
                address: entry.address,
                data_type,
                scale: entry.scale.unwrap_or(1.0),
                offset: entry.offset.unwrap_or(0.0),
                unit: if entry.unit.is_empty() { None } else { Some(entry.unit.clone()) },
                description: if entry.description.is_empty() { None } else { Some(entry.description.clone()) },
                access_mode: if entry.writable { "read_write".to_string() } else { "read".to_string() },
                group: None,
                byte_order,
            };
            
            mappings.push(mapping);
        }
        
        Ok(mappings)
    }
    
    /// parse data type
    fn parse_data_type(&self, type_: &str, data_type: &str) -> Result<(ModbusDataType, u16)> {
        match type_.to_lowercase().as_str() {
            "coil" | "do" => Ok((ModbusDataType::Bool, 1)),
            "discrete_input" | "di" => Ok((ModbusDataType::Bool, 1)),
            "holding_register" | "ao" => {
                self.parse_register_data_type(data_type)
            },
            "input_register" | "ai" => {
                self.parse_register_data_type(data_type)
            },
            _ => Err(ComSrvError::PointTableError(format!(
                "Unsupported point type: {}",
                type_
            ))),
        }
    }
    
    /// parse register data type
    fn parse_register_data_type(&self, data_type: &str) -> Result<(ModbusDataType, u16)> {
        let data_type = data_type.trim();
        
        // Handle empty data type - default to uint16 for registers
        if data_type.is_empty() {
            return Ok((ModbusDataType::UInt16, 1));
        }
        
        match data_type.to_lowercase().as_str() {
            "bool" => Ok((ModbusDataType::Bool, 1)),
            "int16" => Ok((ModbusDataType::Int16, 1)),
            "uint16" => Ok((ModbusDataType::UInt16, 1)),
            "int32" => Ok((ModbusDataType::Int32, 2)),
            "uint32" => Ok((ModbusDataType::UInt32, 2)),
            "int64" => Ok((ModbusDataType::Int64, 4)),
            "uint64" => Ok((ModbusDataType::UInt64, 4)),
            "float32" | "float" => Ok((ModbusDataType::Float32, 2)),
            "float64" | "double" => Ok((ModbusDataType::Float64, 4)),
            s if s.starts_with("string") => {
                // try to parse string length
                let len = s.trim_start_matches("string")
                    .trim_start_matches(|c: char| !c.is_digit(10))
                    .parse::<usize>()
                    .unwrap_or(10); // default length is 10
                
                // string type, each register 2 characters
                let registers = (len + 1) / 2;
                Ok((ModbusDataType::String(len), registers as u16))
            },
            _ => Err(ComSrvError::PointTableError(format!(
                "Unsupported register data type: {}",
                data_type
            ))),
        }
    }
    
    /// parse byte order
    fn parse_byte_order(&self, byte_order: &str) -> Result<ByteOrder> {
        match byte_order.to_uppercase().as_str() {
            "" | "ABCD" => Ok(ByteOrder::BigEndian),
            "DCBA" => Ok(ByteOrder::LittleEndian),
            "BADC" => Ok(ByteOrder::BigEndianWordSwapped),
            "CDAB" => Ok(ByteOrder::LittleEndianWordSwapped),
            _ => Err(ComSrvError::PointTableError(format!(
                "Unsupported byte order: {}",
                byte_order
            ))),
        }
    }
    
    /// get full path
    fn get_full_path(&self, point_table_path: &str) -> String {
        if Path::new(point_table_path).is_absolute() {
            point_table_path.to_string()
        } else {
            format!("{}/{}", self.base_path, point_table_path)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs::File;
    use std::io::Write;


    fn create_test_csv_content() -> String {
        r#"id,name,description,address,type_,data_type,writable,byte_order,scale,offset,unit,deadband
1,Temperature,Temperature sensor,100,holding_register,float32,false,ABCD,0.1,0,°C,0.1
2,Pressure,Pressure sensor,102,holding_register,uint16,false,ABCD,1.0,0,Pa,
3,Flow_Rate,Flow rate measurement,104,input_register,float32,false,DCBA,1.0,0,L/min,0.5
4,Pump_Status,Pump on/off status,200,coil,,true,,,,,
5,Alarm_Status,Alarm discrete input,300,discrete_input,,false,,,,,
6,Set_Point,Temperature setpoint,106,holding_register,int16,true,ABCD,0.01,0,°C,0.1
7,Device_Name,Device name string,108,holding_register,string20,false,ABCD,,,,"#.to_string()
    }

    fn create_test_yaml_content() -> String {
        r#"points:
  - id: 1
    name: Temperature
    description: Temperature sensor
    address: 100
    type_: holding_register
    data_type: float32
    writable: false
    byte_order: ABCD
    scale: 0.1
    offset: 0
    unit: °C
    deadband: 0.1
  - id: 2
    name: Pressure
    description: Pressure sensor
    address: 102
    type_: holding_register
    data_type: uint16
    writable: false
    byte_order: ABCD
    scale: 1.0
    offset: 0
    unit: Pa
  - id: 3
    name: Pump_Status
    description: Pump on/off status
    address: 200
    type_: coil
    writable: true
  - id: 4
    name: Alarm_Status
    description: Alarm discrete input
    address: 300
    type_: discrete_input
    writable: false"#.to_string()
    }

    #[test]
    fn test_point_table_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        let manager = PointTableManager::new(temp_dir.path());
        
        assert!(!manager.base_path.is_empty());
        assert!(manager.base_path.contains(temp_dir.path().to_str().unwrap()));
    }

    #[test]
    fn test_load_csv_point_table() {
        let temp_dir = TempDir::new().unwrap();
        let csv_path = temp_dir.path().join("test_points.csv");
        
        // Create test CSV file
        let mut file = File::create(&csv_path).unwrap();
        file.write_all(create_test_csv_content().as_bytes()).unwrap();
        
        let manager = PointTableManager::new(temp_dir.path());
        let result = manager.load_point_table("test_points.csv");
        
        assert!(result.is_ok());
        let mappings = result.unwrap();
        assert_eq!(mappings.len(), 7);
        
        // Test first mapping (Temperature)
        let temp_mapping = &mappings[0];
        assert_eq!(temp_mapping.name, "Temperature");
        assert_eq!(temp_mapping.address, 100);
        assert!(matches!(temp_mapping.register_type, ModbusRegisterType::HoldingRegister));
        assert!(matches!(temp_mapping.data_type, ModbusDataType::Float32));
        assert_eq!(temp_mapping.scale, 0.1);
        assert_eq!(temp_mapping.unit, Some("°C".to_string()));
        assert!(matches!(temp_mapping.byte_order, ByteOrder::BigEndian));
        
        // Test coil mapping (Pump_Status)
        let pump_mapping = &mappings[3];
        assert_eq!(pump_mapping.name, "Pump_Status");
        assert_eq!(pump_mapping.address, 200);
        assert!(matches!(pump_mapping.register_type, ModbusRegisterType::Coil));
        assert!(matches!(pump_mapping.data_type, ModbusDataType::Bool));
        assert_eq!(pump_mapping.access_mode, "read_write");
    }

    #[test]
    fn test_load_yaml_point_table() {
        let temp_dir = TempDir::new().unwrap();
        let yaml_path = temp_dir.path().join("test_points.yaml");
        
        // Create test YAML file
        let mut file = File::create(&yaml_path).unwrap();
        file.write_all(create_test_yaml_content().as_bytes()).unwrap();
        
        let manager = PointTableManager::new(temp_dir.path());
        let result = manager.load_point_table("test_points.yaml");
        
        assert!(result.is_ok());
        let mappings = result.unwrap();
        assert_eq!(mappings.len(), 4);
        
        // Test temperature mapping
        let temp_mapping = &mappings[0];
        assert_eq!(temp_mapping.name, "Temperature");
        assert_eq!(temp_mapping.address, 100);
        assert!(matches!(temp_mapping.register_type, ModbusRegisterType::HoldingRegister));
        assert!(matches!(temp_mapping.data_type, ModbusDataType::Float32));
        
        // Test coil mapping
        let pump_mapping = &mappings[2];
        assert_eq!(pump_mapping.name, "Pump_Status");
        assert!(matches!(pump_mapping.register_type, ModbusRegisterType::Coil));
        assert_eq!(pump_mapping.access_mode, "read_write");
    }

    #[test]
    fn test_load_nonexistent_file() {
        let temp_dir = TempDir::new().unwrap();
        let manager = PointTableManager::new(temp_dir.path());
        
        let result = manager.load_point_table("nonexistent.csv");
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ComSrvError::PointTableError(_)));
    }

    #[test]
    fn test_unsupported_file_format() {
        let temp_dir = TempDir::new().unwrap();
        let txt_path = temp_dir.path().join("test.txt");
        
        // Create test file with unsupported extension
        let mut file = File::create(&txt_path).unwrap();
        file.write_all(b"test content").unwrap();
        
        let manager = PointTableManager::new(temp_dir.path());
        let result = manager.load_point_table("test.txt");
        
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ComSrvError::PointTableError(_)));
    }

    #[test]
    fn test_parse_data_types() {
        let manager = PointTableManager::new("/tmp");
        
        // Test coil types
        let result = manager.parse_data_type("coil", "");
        assert!(result.is_ok());
        let (data_type, quantity) = result.unwrap();
        assert!(matches!(data_type, ModbusDataType::Bool));
        assert_eq!(quantity, 1);
        
        // Test discrete input
        let result = manager.parse_data_type("discrete_input", "");
        assert!(result.is_ok());
        let (data_type, _) = result.unwrap();
        assert!(matches!(data_type, ModbusDataType::Bool));
        
        // Test holding register types
        let result = manager.parse_data_type("holding_register", "uint16");
        assert!(result.is_ok());
        let (data_type, quantity) = result.unwrap();
        assert!(matches!(data_type, ModbusDataType::UInt16));
        assert_eq!(quantity, 1);
        
        let result = manager.parse_data_type("holding_register", "float32");
        assert!(result.is_ok());
        let (data_type, quantity) = result.unwrap();
        assert!(matches!(data_type, ModbusDataType::Float32));
        assert_eq!(quantity, 2);
        
        // Test invalid type
        let result = manager.parse_data_type("invalid_type", "");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_register_data_types() {
        let manager = PointTableManager::new("/tmp");
        
        // Test basic types
        let test_cases = vec![
            ("bool", ModbusDataType::Bool, 1),
            ("int16", ModbusDataType::Int16, 1),
            ("uint16", ModbusDataType::UInt16, 1),
            ("int32", ModbusDataType::Int32, 2),
            ("uint32", ModbusDataType::UInt32, 2),
            ("float32", ModbusDataType::Float32, 2),
            ("float", ModbusDataType::Float32, 2),
            ("float64", ModbusDataType::Float64, 4),
            ("double", ModbusDataType::Float64, 4),
        ];
        
        for (input, expected_type, expected_quantity) in test_cases {
            let result = manager.parse_register_data_type(input);
            assert!(result.is_ok(), "Failed to parse: {}", input);
            let (data_type, quantity) = result.unwrap();
            assert!(std::mem::discriminant(&data_type) == std::mem::discriminant(&expected_type));
            assert_eq!(quantity, expected_quantity);
        }
        
        // Test string types
        let result = manager.parse_register_data_type("string10");
        assert!(result.is_ok());
        let (data_type, quantity) = result.unwrap();
        if let ModbusDataType::String(len) = data_type {
            assert_eq!(len, 10);
            assert_eq!(quantity, 5); // (10 + 1) / 2 = 5 registers
        } else {
            panic!("Expected String data type");
        }
        
        // Test default string length
        let result = manager.parse_register_data_type("string");
        assert!(result.is_ok());
        let (data_type, quantity) = result.unwrap();
        if let ModbusDataType::String(len) = data_type {
            assert_eq!(len, 10); // default length
            assert_eq!(quantity, 5);
        } else {
            panic!("Expected String data type");
        }
        
        // Test invalid type
        let result = manager.parse_register_data_type("invalid_type");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_byte_order() {
        let manager = PointTableManager::new("/tmp");
        
        let test_cases = vec![
            ("", ByteOrder::BigEndian),
            ("ABCD", ByteOrder::BigEndian),
            ("abcd", ByteOrder::BigEndian),
            ("DCBA", ByteOrder::LittleEndian),
            ("dcba", ByteOrder::LittleEndian),
            ("BADC", ByteOrder::BigEndianWordSwapped),
            ("badc", ByteOrder::BigEndianWordSwapped),
            ("CDAB", ByteOrder::LittleEndianWordSwapped),
            ("cdab", ByteOrder::LittleEndianWordSwapped),
        ];
        
        for (input, expected) in test_cases {
            let result = manager.parse_byte_order(input);
            assert!(result.is_ok(), "Failed to parse byte order: {}", input);
            let byte_order = result.unwrap();
            assert!(std::mem::discriminant(&byte_order) == std::mem::discriminant(&expected));
        }
        
        // Test invalid byte order
        let result = manager.parse_byte_order("INVALID");
        assert!(result.is_err());
    }

    #[test]
    fn test_get_full_path() {
        let manager = PointTableManager::new("/base/path");
        
        // Test relative path
        let full_path = manager.get_full_path("points.csv");
        assert_eq!(full_path, "/base/path/points.csv");
        
        // Test absolute path
        let full_path = manager.get_full_path("/absolute/path/points.csv");
        assert_eq!(full_path, "/absolute/path/points.csv");
        
        // Test path with subdirectory
        let full_path = manager.get_full_path("subdir/points.csv");
        assert_eq!(full_path, "/base/path/subdir/points.csv");
    }

    #[test]
    fn test_malformed_csv() {
        let temp_dir = TempDir::new().unwrap();
        let csv_path = temp_dir.path().join("malformed.csv");
        
        // Create malformed CSV (missing required fields)
        let malformed_content = r#"id,name
1,Temperature
2,Pressure"#;
        
        let mut file = File::create(&csv_path).unwrap();
        file.write_all(malformed_content.as_bytes()).unwrap();
        
        let manager = PointTableManager::new(temp_dir.path());
        let result = manager.load_point_table("malformed.csv");
        
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ComSrvError::PointTableError(_)));
    }

    #[test]
    fn test_malformed_yaml() {
        let temp_dir = TempDir::new().unwrap();
        let yaml_path = temp_dir.path().join("malformed.yaml");
        
        // Create malformed YAML
        let malformed_content = r#"invalid_yaml:
  - not_points: true
    missing_structure"#;
        
        let mut file = File::create(&yaml_path).unwrap();
        file.write_all(malformed_content.as_bytes()).unwrap();
        
        let manager = PointTableManager::new(temp_dir.path());
        let result = manager.load_point_table("malformed.yaml");
        
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ComSrvError::PointTableError(_)));
    }

    #[test]
    fn test_yaml_without_points_section() {
        let temp_dir = TempDir::new().unwrap();
        let yaml_path = temp_dir.path().join("no_points.yaml");
        
        // Create YAML without points section
        let content = r#"configuration:
  name: test
  version: 1.0"#;
        
        let mut file = File::create(&yaml_path).unwrap();
        file.write_all(content.as_bytes()).unwrap();
        
        let manager = PointTableManager::new(temp_dir.path());
        let result = manager.load_point_table("no_points.yaml");
        
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ComSrvError::PointTableError(_)));
    }

    #[test]
    fn test_point_table_with_optional_fields() {
        let temp_dir = TempDir::new().unwrap();
        let csv_path = temp_dir.path().join("minimal.csv");
        
        // Create CSV with minimal required fields
        let minimal_content = r#"id,name,address,type_
1,Temperature,100,holding_register
2,Pump_Status,200,coil"#;
        
        let mut file = File::create(&csv_path).unwrap();
        file.write_all(minimal_content.as_bytes()).unwrap();
        
        let manager = PointTableManager::new(temp_dir.path());
        let result = manager.load_point_table("minimal.csv");
        
        assert!(result.is_ok());
        let mappings = result.unwrap();
        assert_eq!(mappings.len(), 2);
        
        // Check that default values are applied
        let temp_mapping = &mappings[0];
        assert_eq!(temp_mapping.scale, 1.0);
        assert_eq!(temp_mapping.offset, 0.0);
        assert_eq!(temp_mapping.unit, None);
        assert_eq!(temp_mapping.description, None);
        assert_eq!(temp_mapping.access_mode, "read");
        assert!(matches!(temp_mapping.byte_order, ByteOrder::BigEndian));
    }

    #[test]
    fn test_string_data_type_variations() {
        let manager = PointTableManager::new("/tmp");
        
        // Test various string formats
        let string_types = vec![
            ("string", 10, 5),
            ("string5", 5, 3),
            ("string20", 20, 10),
            ("string100", 100, 50),
        ];
        
        for (input, expected_len, expected_regs) in string_types {
            let result = manager.parse_register_data_type(input);
            assert!(result.is_ok(), "Failed to parse string type: {}", input);
            let (data_type, quantity) = result.unwrap();
            
            if let ModbusDataType::String(len) = data_type {
                assert_eq!(len, expected_len);
                assert_eq!(quantity, expected_regs);
            } else {
                panic!("Expected String data type for input: {}", input);
            }
        }
    }

    #[test]
    fn test_register_type_aliases() {
        let temp_dir = TempDir::new().unwrap();
        let csv_path = temp_dir.path().join("aliases.csv");
        
        // Create CSV with register type aliases
        let content = r#"id,name,address,type_
1,Digital_Output,100,do
2,Digital_Input,200,di
3,Analog_Output,300,ao
4,Analog_Input,400,ai"#;
        
        let mut file = File::create(&csv_path).unwrap();
        file.write_all(content.as_bytes()).unwrap();
        
        let manager = PointTableManager::new(temp_dir.path());
        let result = manager.load_point_table("aliases.csv");
        
        assert!(result.is_ok());
        let mappings = result.unwrap();
        assert_eq!(mappings.len(), 4);
        
        assert!(matches!(mappings[0].register_type, ModbusRegisterType::Coil));
        assert!(matches!(mappings[1].register_type, ModbusRegisterType::DiscreteInput));
        assert!(matches!(mappings[2].register_type, ModbusRegisterType::HoldingRegister));
        assert!(matches!(mappings[3].register_type, ModbusRegisterType::InputRegister));
    }

    #[test]
    fn test_yml_extension() {
        let temp_dir = TempDir::new().unwrap();
        let yml_path = temp_dir.path().join("test_points.yml");
        
        // Create test YML file (same content as YAML)
        let mut file = File::create(&yml_path).unwrap();
        file.write_all(create_test_yaml_content().as_bytes()).unwrap();
        
        let manager = PointTableManager::new(temp_dir.path());
        let result = manager.load_point_table("test_points.yml");
        
        assert!(result.is_ok());
        let mappings = result.unwrap();
        assert_eq!(mappings.len(), 4);
    }

    #[test]
    fn test_point_table_entry_serialization() {
        let entry = PointTableEntry {
            id: 1,
            name: "Temperature".to_string(),
            description: "Temperature sensor".to_string(),
            address: 100,
            type_: "holding_register".to_string(),
            data_type: "float32".to_string(),
            writable: false,
            byte_order: "ABCD".to_string(),
            scale: Some(0.1),
            offset: Some(0.0),
            unit: "°C".to_string(),
            deadband: Some(0.1),
        };
        
        // Test serialization
        let serialized = serde_json::to_string(&entry).unwrap();
        assert!(serialized.contains("Temperature"));
        assert!(serialized.contains("holding_register"));
        
        // Test deserialization
        let deserialized: PointTableEntry = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.id, entry.id);
        assert_eq!(deserialized.name, entry.name);
        assert_eq!(deserialized.address, entry.address);
    }

    #[test]
    fn test_load_invalid_file() {
        let temp_dir = TempDir::new().unwrap();
        let manager = PointTableManager::new(temp_dir.path().to_str().unwrap().to_string());

        // Test loading non-existent file
        let result = manager.load_point_table("non_existent.csv");
        assert!(result.is_err());

        // Test loading invalid CSV format
        let invalid_csv_path = temp_dir.path().join("invalid.csv");
        let mut file = File::create(&invalid_csv_path).unwrap();
        file.write_all(b"invalid,csv,header\ndata,without,proper,format").unwrap();

        let result = manager.load_point_table("invalid.csv");
        assert!(result.is_err());
    }

    #[test]
    fn test_data_type_parsing() {
        let temp_dir = TempDir::new().unwrap();
        let manager = PointTableManager::new(temp_dir.path().to_str().unwrap().to_string());

        // Test various data type parsing
        assert!(manager.parse_register_data_type("uint16").is_ok());
        assert!(manager.parse_register_data_type("int32").is_ok());
        assert!(manager.parse_register_data_type("float32").is_ok());
        assert!(manager.parse_register_data_type("string10").is_ok());
        assert!(manager.parse_register_data_type("").is_ok()); // Empty should default to uint16
        assert!(manager.parse_register_data_type("invalid_type").is_err());
    }

    #[test]
    fn test_byte_order_parsing() {
        let temp_dir = TempDir::new().unwrap();
        let manager = PointTableManager::new(temp_dir.path().to_str().unwrap().to_string());

        // Test various byte order parsing
        assert!(manager.parse_byte_order("ABCD").is_ok());
        assert!(manager.parse_byte_order("DCBA").is_ok());
        assert!(manager.parse_byte_order("BADC").is_ok());
        assert!(manager.parse_byte_order("CDAB").is_ok());
        assert!(manager.parse_byte_order("").is_ok()); // Empty should default to ABCD
        assert!(manager.parse_byte_order("INVALID").is_err());
    }

    // Enhanced functionality tests

    #[test]
    fn test_point_table_config_default() {
        let config = PointTableConfig::default();
        assert!(!config.auto_reload);
        assert_eq!(config.reload_check_interval, 30);
        assert!(!config.enable_optimization);
        assert!(!config.enable_point_cache);
        assert_eq!(config.cache_ttl, 300);
        assert!(!config.enable_performance_monitoring);
    }

    #[test]
    fn test_point_cache_entry_creation() {
        let data_point = DataPoint {
            id: "test_point".to_string(),
            value: "123.45".to_string(),
            quality: 100,
            timestamp: SystemTime::now(),
            description: "Test data point".to_string(),
        };
        
        let cache_entry = PointCacheEntry {
            data_point: data_point.clone(),
            cached_at: Instant::now(),
            ttl: Duration::from_secs(10),
        };

        assert_eq!(cache_entry.data_point.id, data_point.id);
        assert!(!cache_entry.is_expired());
    }

    #[test]
    fn test_optimization_type_to_string() {
        assert_eq!(OptimizationType::AddressReordering.to_string(), "Address Reordering");
        assert_eq!(OptimizationType::BatchReading.to_string(), "Batch Reading");
        assert_eq!(OptimizationType::DataTypeOptimization.to_string(), "Data Type Optimization");
        assert_eq!(OptimizationType::RemoveUnusedPoints.to_string(), "Remove Unused Points");
    }

    #[tokio::test]
    async fn test_enhanced_point_table_manager_creation() {
        let temp_dir = TempDir::new().unwrap();
        let config = PointTableConfig {
            auto_reload: true,
            reload_check_interval: 1,
            enable_optimization: true,
            enable_point_cache: true,
            cache_ttl: 10,
            enable_performance_monitoring: true,
        };
        
        let manager = PointTableManager::new_with_config(temp_dir.path(), config);
        assert!(!*manager.is_running.read().await);
    }

    #[tokio::test]
    async fn test_cache_point_value() {
        let temp_dir = TempDir::new().unwrap();
        let config = PointTableConfig {
            auto_reload: false,
            reload_check_interval: 30,
            enable_optimization: false,
            enable_point_cache: true,
            cache_ttl: 10,
            enable_performance_monitoring: false,
        };
        
        let manager = PointTableManager::new_with_config(temp_dir.path(), config);
        let data_point = DataPoint {
            id: "test_point".to_string(),
            value: "123.45".to_string(),
            quality: 100,
            timestamp: SystemTime::now(),
            description: "Test data point".to_string(),
        };
        
        manager.cache_point_value(1, "test_point", data_point.clone()).await;
        let cached_value = manager.get_cached_point_value(1, "test_point").await;
        assert!(cached_value.is_some());
    }

    #[test]
    fn test_backward_compatibility() {
        let temp_dir = TempDir::new().unwrap();
        
        // Test that the original new() method still works
        let manager = PointTableManager::new(temp_dir.path());
        assert!(!manager.base_path.is_empty());
        
        // Test that original load_point_table still works
        let csv_path = temp_dir.path().join("test_points.csv");
        let mut file = File::create(&csv_path).unwrap();
        file.write_all(create_test_csv_content().as_bytes()).unwrap();
        
        let result = manager.load_point_table("test_points.csv");
        assert!(result.is_ok());
        let mappings = result.unwrap();
        assert_eq!(mappings.len(), 7);
    }
} 