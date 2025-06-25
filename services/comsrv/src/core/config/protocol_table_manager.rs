//! Protocol Table Manager
//! 
//! This module provides a polymorphic architecture for managing four telemetry types (四遥)
//! with protocol-specific implementations and comprehensive validation.
//! 
//! # Architecture
//! 
//! - **StandardPointRecord**: Generic four telemetry point records (protocol-agnostic)
//! - **ProtocolConfig**: Trait for protocol-specific configurations (Modbus, IEC104, CAN, etc.)
//! - **FourTelemetryTableManager**: Trait for managing four telemetry tables
//! - **ProtocolConfigValidator**: Trait for validating configurations
//! - **StandardFourTelemetryManager**: Concrete implementation with validation

use crate::core::protocols::common::combase::TelemetryType;
use crate::core::protocols::modbus::common::{
    ByteOrder, ModbusDataType, ModbusRegisterMapping, ModbusRegisterType,
};
use crate::utils::error::{ComSrvError, Result};
use csv::ReaderBuilder;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::time::SystemTime;
use log;
use serde_json;

/// Simple data point structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataPoint {
    /// Point identifier
    pub id: String,
    /// Point value
    pub value: String,
    /// Timestamp when the value was captured
    pub timestamp: SystemTime,
    /// Point description
    pub description: String,
}

impl DataPoint {
    /// Create a new data point
    pub fn new(id: String, value: String, description: String) -> Self {
        Self {
            id,
            value,
            timestamp: SystemTime::now(),
            description,
        }
    }
}

// Serde helper module for SystemTime serialization
mod timestamp_as_seconds {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::{SystemTime, UNIX_EPOCH};

    pub fn serialize<S>(time: &SystemTime, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let duration = time
            .duration_since(UNIX_EPOCH)
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

/// 协议类型枚举
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProtocolType {
    Modbus,
    IEC104,
    CAN,
    // 可扩展其他协议
}

impl ProtocolType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProtocolType::Modbus => "modbus",
            ProtocolType::IEC104 => "iec104", 
            ProtocolType::CAN => "can",
        }
    }
}

/// 四遥类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TelemetryCategory {
    /// 遥测 - Telemetry (analog measurements)
    Telemetry,
    /// 遥信 - Signaling (digital inputs)  
    Signaling,
    /// 遥调 - Setpoint (analog outputs)
    Setpoint,
    /// 遥控 - Control (digital outputs)
    Control,
}

impl TelemetryCategory {
    /// 转换为TelemetryType
    pub fn to_telemetry_type(&self) -> TelemetryType {
        match self {
            TelemetryCategory::Telemetry => TelemetryType::Telemetry,
            TelemetryCategory::Signaling => TelemetryType::Signaling,
            TelemetryCategory::Setpoint => TelemetryType::Setpoint,
            TelemetryCategory::Control => TelemetryType::Control,
        }
    }

    pub fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "telemetry" => Ok(TelemetryCategory::Telemetry),
            "signaling" => Ok(TelemetryCategory::Signaling),
            "setpoint" => Ok(TelemetryCategory::Setpoint),
            "control" => Ok(TelemetryCategory::Control),
            _ => Err(ComSrvError::ConfigError(format!(
                "Unknown telemetry category: {}",
                s
            ))),
        }
    }

    pub fn table_suffix(&self) -> &'static str {
        match self {
            TelemetryCategory::Telemetry => "telemetry",
            TelemetryCategory::Signaling => "signaling",
            TelemetryCategory::Setpoint => "setpoint",
            TelemetryCategory::Control => "control",
        }
    }
}

/// 通用四遥点表记录（标准、通用）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StandardPointRecord {
    /// 点位ID（表内独立编号）
    pub point_id: u32,
    /// 点位名称
    pub point_name: String,
    /// 数据单位
    #[serde(default)]
    pub unit: String,
    /// 系数 - 用于数据转换
    #[serde(default = "default_scale")]
    pub scale: f64,
    /// 偏移量 - 用于数据转换  
    #[serde(default)]
    pub offset: f64,
    /// 描述
    pub description: String,
    /// 四遥类型
    #[serde(skip, default = "default_telemetry_category")]
    pub telemetry_category: TelemetryCategory,
}

fn default_scale() -> f64 {
    1.0
}

fn default_telemetry_category() -> TelemetryCategory {
    TelemetryCategory::Telemetry
}

/// 协议配置验证trait
pub trait ProtocolConfigValidator: Send + Sync {
    /// 验证协议配置记录
    fn validate_protocol_config(&self, config: &dyn ProtocolConfig) -> Result<Vec<String>>;
    
    /// 验证点表记录
    fn validate_point_record(&self, record: &StandardPointRecord) -> Result<Vec<String>>;
    
    /// 验证点表与协议配置的匹配性
    fn validate_mapping(&self, point: &StandardPointRecord, config: &dyn ProtocolConfig) -> Result<Vec<String>>;
}

/// 协议配置trait
pub trait ProtocolConfig: std::fmt::Debug + Send + Sync {
    /// 获取点位ID
    fn point_id(&self) -> u32;
    
    /// 获取协议地址
    fn protocol_address(&self) -> u32;
    
    /// 获取数据类型字符串
    fn data_type(&self) -> &str;
    
    /// 获取描述
    fn description(&self) -> &str;
    
    /// 获取协议类型
    fn protocol_type(&self) -> ProtocolType;
    
    /// 克隆为Box
    fn clone_box(&self) -> Box<dyn ProtocolConfig>;
    
    /// 验证配置有效性
    fn validate(&self) -> Result<()>;
}

/// 四遥表管理trait（多态接口）
pub trait FourTelemetryTableManagerTrait {
    /// 加载标准点表
    fn load_standard_points<P: AsRef<Path>>(&mut self, file_path: P, channel_name: &str, category: TelemetryCategory) -> Result<()>;
    
    /// 加载协议配置
    fn load_protocol_config<P: AsRef<Path>>(&mut self, file_path: P, channel_name: &str, category: TelemetryCategory) -> Result<()>;
    
    /// 构建点位映射
    fn build_mappings(&mut self) -> Result<()>;
    
    /// 获取通道名列表
    fn get_channel_names(&self) -> Vec<String>;
    
    /// 验证配置
    fn validate_configuration(&self) -> Result<ValidationReport>;
    
    /// 获取统计信息
    fn get_statistics(&self) -> TableStatistics;
}

/// 验证报告
#[derive(Debug, Clone)]
pub struct ValidationReport {
    /// 是否通过验证
    pub is_valid: bool,
    /// 错误信息
    pub errors: Vec<String>,
    /// 警告信息  
    pub warnings: Vec<String>,
    /// 验证的通道数
    pub validated_channels: usize,
    /// 验证的点位数
    pub validated_points: usize,
}

/// 表统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableStatistics {
    /// 总通道数
    pub total_channels: usize,
    /// 总协议配置数
    pub total_protocol_configs: usize,
    /// 总标准点表数
    pub total_standard_points: usize,
    /// 总映射点位数
    pub total_mapped_points: usize,
    /// 各类型点数统计
    pub points_by_category: HashMap<TelemetryCategory, usize>,
    /// 各协议点数统计
    pub points_by_protocol: HashMap<ProtocolType, usize>,
}

// ============ Modbus协议特定实现 ============

/// Modbus协议配置记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModbusProtocolConfig {
    /// 点位ID（表内独立编号）
    pub point_id: u32,
    /// Modbus寄存器地址
    pub register_address: u16,
    /// Modbus功能码 (1-16)
    pub function_code: u8,
    /// 数据类型 (UInt16, Int16, UInt32, Int32, Float32, Bool)
    pub data_type: String,
    /// 字节序 (ABCD, DCBA, BADC, CDAB)
    pub byte_order: String,
    /// 描述
    pub description: String,
}

impl ProtocolConfig for ModbusProtocolConfig {
    fn point_id(&self) -> u32 {
        self.point_id
    }
    
    fn protocol_address(&self) -> u32 {
        self.register_address as u32
    }
    
    fn data_type(&self) -> &str {
        &self.data_type
    }
    
    fn description(&self) -> &str {
        &self.description
    }
    
    fn protocol_type(&self) -> ProtocolType {
        ProtocolType::Modbus
    }
    
    fn clone_box(&self) -> Box<dyn ProtocolConfig> {
        Box::new(self.clone())
    }
    
    fn validate(&self) -> Result<()> {
        // 验证功能码
        if !(1..=16).contains(&self.function_code) {
            return Err(ComSrvError::ConfigError(format!(
                "Invalid Modbus function code: {} (must be 1-16)", 
                self.function_code
            )));
        }
        
        // 验证数据类型
        match self.data_type.as_str() {
            "UInt16" | "Int16" | "UInt32" | "Int32" | "Float32" | "Bool" => {},
            _ => return Err(ComSrvError::ConfigError(format!(
                "Unsupported Modbus data type: {}", 
                self.data_type
            ))),
        }
        
        // 验证字节序
        match self.byte_order.as_str() {
            "ABCD" | "DCBA" | "BADC" | "CDAB" => {},
            _ => return Err(ComSrvError::ConfigError(format!(
                "Invalid byte order: {} (must be ABCD, DCBA, BADC, or CDAB)", 
                self.byte_order
            ))),
        }
        
        Ok(())
    }
}

/// Modbus协议验证器
pub struct ModbusValidator;

impl ProtocolConfigValidator for ModbusValidator {
    fn validate_protocol_config(&self, config: &dyn ProtocolConfig) -> Result<Vec<String>> {
        let mut warnings = Vec::new();
        
        // 基础验证
        config.validate()?;
        
        // Modbus特定验证
        if config.protocol_address() > 65535 {
            warnings.push(format!(
                "Modbus address {} exceeds 16-bit limit", 
                config.protocol_address()
            ));
        }
        
        Ok(warnings)
    }
    
    fn validate_point_record(&self, record: &StandardPointRecord) -> Result<Vec<String>> {
        let mut warnings = Vec::new();
        
        // 验证系数不能为0
        if record.scale == 0.0 {
            return Err(ComSrvError::ConfigError(format!(
                "Scale factor cannot be zero for point: {}", 
                record.point_name
            )));
        }
        
        // 验证点位名称不为空
        if record.point_name.trim().is_empty() {
            return Err(ComSrvError::ConfigError(
                "Point name cannot be empty".to_string()
            ));
        }
        
        // 警告：建议添加单位
        if record.unit.is_empty() && matches!(record.telemetry_category, TelemetryCategory::Telemetry | TelemetryCategory::Setpoint) {
            warnings.push(format!(
                "Point '{}' missing unit (recommended for {} points)", 
                record.point_name, record.telemetry_category.table_suffix()
            ));
        }
        
        Ok(warnings)
    }
    
    fn validate_mapping(&self, point: &StandardPointRecord, config: &dyn ProtocolConfig) -> Result<Vec<String>> {
        let mut warnings = Vec::new();
        
        // 验证点位ID匹配
        if point.point_id != config.point_id() {
            return Err(ComSrvError::ConfigError(format!(
                "Point ID mismatch: standard table has {}, protocol config has {}", 
                point.point_id, config.point_id()
            )));
        }
        
        // 数据类型兼容性检查
        let is_analog = matches!(point.telemetry_category, TelemetryCategory::Telemetry | TelemetryCategory::Setpoint);
        let is_bool_type = config.data_type() == "Bool";
        
        if is_analog && is_bool_type {
            warnings.push(format!(
                "Point '{}' is analog type but configured as Bool in protocol", 
                point.point_name
            ));
        } else if !is_analog && !is_bool_type {
            warnings.push(format!(
                "Point '{}' is digital type but configured as non-Bool in protocol", 
                point.point_name
            ));
        }
        
        Ok(warnings)
    }
}

// ============ 具体实现类 ============

/// 协议无关的四遥表管理器实现
pub struct StandardFourTelemetryManager {
    /// 标准点表 (channel_name -> telemetry_category -> points)
    standard_points: HashMap<String, HashMap<TelemetryCategory, Vec<StandardPointRecord>>>,
    /// 协议配置 (channel_name -> telemetry_category -> configs)
    protocol_configs: HashMap<String, HashMap<TelemetryCategory, Vec<Box<dyn ProtocolConfig>>>>,
    /// 点位映射 (channel_name -> telemetry_category -> point_id -> (standard_point, protocol_config))
    point_mappings: HashMap<String, HashMap<TelemetryCategory, HashMap<u32, (StandardPointRecord, Box<dyn ProtocolConfig>)>>>,
    /// 协议验证器
    validators: HashMap<ProtocolType, Box<dyn ProtocolConfigValidator>>,
}

impl StandardFourTelemetryManager {
    pub fn new() -> Self {
        let mut validators: HashMap<ProtocolType, Box<dyn ProtocolConfigValidator>> = HashMap::new();
        validators.insert(ProtocolType::Modbus, Box::new(ModbusValidator));
        
        Self {
            standard_points: HashMap::new(),
            protocol_configs: HashMap::new(),
            point_mappings: HashMap::new(),
            validators,
        }
    }
    
    /// Get table statistics for a specific channel name (used by CSV point manager)
    pub fn get_table_stats(&self, channel_name: &str) -> Option<serde_json::Value> {
        if let Some(channel_points) = self.standard_points.get(channel_name) {
            let mut total_points = 0;
            let mut points_by_category = std::collections::HashMap::new();
            
            for (category, points) in channel_points {
                let count = points.len();
                total_points += count;
                points_by_category.insert(category.table_suffix(), count);
            }
            
            let stats = serde_json::json!({
                "total_points": total_points,
                "points_by_category": points_by_category,
                "channel_name": channel_name
            });
            
            Some(stats)
        } else {
            None
        }
    }
    
    /// Get all points for a specific channel (used by CSV point manager)
    pub fn get_points(&self, channel_name: &str) -> Option<Vec<StandardPointRecord>> {
        if let Some(channel_points) = self.standard_points.get(channel_name) {
            let mut all_points = Vec::new();
            for (_category, points) in channel_points {
                all_points.extend(points.clone());
            }
            Some(all_points)
        } else {
            None
        }
    }
    
    /// Find a specific point by ID in a channel (used by CSV point manager)
    pub fn find_point(&self, channel_name: &str, point_id: &str) -> Option<StandardPointRecord> {
        if let Some(channel_points) = self.standard_points.get(channel_name) {
            for (_category, points) in channel_points {
                for point in points {
                    if point.point_id.to_string() == point_id || point.point_name == point_id {
                        return Some(point.clone());
                    }
                }
            }
        }
        None
    }
    
    /// Upsert (insert or update) a point in a channel (used by CSV point manager)
    pub fn upsert_point(&mut self, channel_name: &str, point: StandardPointRecord) -> Result<()> {
        let channel_points = self.standard_points
            .entry(channel_name.to_string())
            .or_insert_with(HashMap::new);
        
        let category_points = channel_points
            .entry(point.telemetry_category.clone())
            .or_insert_with(Vec::new);
        
        // Try to find existing point by ID or name
        if let Some(existing_index) = category_points.iter().position(|p| 
            p.point_id == point.point_id || p.point_name == point.point_name
        ) {
            // Update existing point
            category_points[existing_index] = point;
        } else {
            // Insert new point
            category_points.push(point);
        }
        
        Ok(())
    }
    
    /// Remove a point from a channel (used by CSV point manager)
    pub fn remove_point(&mut self, channel_name: &str, point_id: &str) -> Result<bool> {
        if let Some(channel_points) = self.standard_points.get_mut(channel_name) {
            for (_category, points) in channel_points {
                if let Some(index) = points.iter().position(|p| 
                    p.point_id.to_string() == point_id || p.point_name == point_id
                ) {
                    points.remove(index);
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }
    
    /// 加载Modbus协议配置的便捷方法
    pub fn load_modbus_protocol_config<P: AsRef<Path>>(&mut self, file_path: P, channel_name: &str, category: TelemetryCategory) -> Result<()> {
        let path = file_path.as_ref();
        let file = std::fs::File::open(path)
            .map_err(|e| ComSrvError::IoError(format!("Failed to open file {}: {}", path.display(), e)))?;

        let mut reader = ReaderBuilder::new()
            .has_headers(true)
            .delimiter(b',')
            .from_reader(file);

        let mut configs = Vec::new();
        for result in reader.deserialize() {
            let config: ModbusProtocolConfig = result.map_err(|e| {
                ComSrvError::ConfigError(format!("Failed to parse Modbus config in {}: {}", path.display(), e))
            })?;
            
            // 验证配置
            config.validate()?;
            configs.push(Box::new(config) as Box<dyn ProtocolConfig>);
        }

        let config_count = configs.len();
        self.protocol_configs
            .entry(channel_name.to_string())
            .or_insert_with(HashMap::new)
            .insert(category.clone(), configs);

        log::info!("📋 [PROTOCOL] Loaded {} Modbus {} configs for channel '{}'", 
                  config_count, 
                  category.table_suffix(), 
                  channel_name);

        Ok(())
    }

    /// 转换为Modbus寄存器映射（向后兼容）
    pub fn to_modbus_mappings(&self, channel_name: &str) -> Result<Vec<ModbusRegisterMapping>> {
        let mappings = self.point_mappings.get(channel_name).ok_or_else(|| {
            ComSrvError::ConfigError(format!(
                "No point mappings found for channel: {}",
                channel_name
            ))
        })?;

        let mut modbus_mappings = Vec::new();

        for (_category, category_mappings) in mappings {
            for (_point_id, (standard_point, protocol_config)) in category_mappings {
                // 只处理Modbus协议配置
                if protocol_config.protocol_type() != ProtocolType::Modbus {
                    continue;
                }

                let data_type = self.parse_modbus_data_type(protocol_config.data_type())?;
                let register_type = self.parse_modbus_function_code(protocol_config.protocol_address() as u8)?;
                let byte_order = self.parse_modbus_byte_order("ABCD")?; // 默认字节序

                let mapping = ModbusRegisterMapping {
                    name: standard_point.point_name.clone(),
                    display_name: Some(standard_point.point_name.clone()),
                    register_type,
                    address: protocol_config.protocol_address() as u16,
                    data_type,
                    scale: standard_point.scale,
                    offset: standard_point.offset,
                    unit: if standard_point.unit.is_empty() {
                        None
                    } else {
                        Some(standard_point.unit.clone())
                    },
                    description: if standard_point.description.is_empty() {
                        None
                    } else {
                        Some(standard_point.description.clone())
                    },
                    access_mode: "read".to_string(),
                    group: None,
                    byte_order,
                };

                modbus_mappings.push(mapping);
            }
        }

        log::info!(
            "📊 [MODBUS] Generated {} Modbus mappings for channel '{}'",
            modbus_mappings.len(),
            channel_name
        );

        Ok(modbus_mappings)
    }

    /// 解析Modbus数据类型（向后兼容）
    fn parse_modbus_data_type(&self, data_type: &str) -> Result<ModbusDataType> {
        match data_type {
            "UInt16" => Ok(ModbusDataType::UInt16),
            "Int16" => Ok(ModbusDataType::Int16),
            "UInt32" => Ok(ModbusDataType::UInt32),
            "Int32" => Ok(ModbusDataType::Int32),
            "Float32" => Ok(ModbusDataType::Float32),
            "Bool" => Ok(ModbusDataType::Bool),
            _ => Err(ComSrvError::ConfigError(format!(
                "Unsupported Modbus data type: {}",
                data_type
            ))),
        }
    }

    /// 解析Modbus功能码（向后兼容）
    fn parse_modbus_function_code(&self, function_code: u8) -> Result<ModbusRegisterType> {
        match function_code {
            1 => Ok(ModbusRegisterType::Coil),
            2 => Ok(ModbusRegisterType::DiscreteInput),
            3 => Ok(ModbusRegisterType::HoldingRegister),
            4 => Ok(ModbusRegisterType::InputRegister),
            5 | 15 => Ok(ModbusRegisterType::Coil),
            6 | 16 => Ok(ModbusRegisterType::HoldingRegister),
            _ => Err(ComSrvError::ConfigError(format!(
                "Unsupported Modbus function code: {}",
                function_code
            ))),
        }
    }

    /// 解析Modbus字节序（向后兼容）
    fn parse_modbus_byte_order(&self, byte_order: &str) -> Result<ByteOrder> {
        match byte_order {
            "ABCD" => Ok(ByteOrder::BigEndian),
            "DCBA" => Ok(ByteOrder::LittleEndian),
            "BADC" => Ok(ByteOrder::BigEndianWordSwapped),
            "CDAB" => Ok(ByteOrder::LittleEndianWordSwapped),
            _ => Err(ComSrvError::ConfigError(format!(
                "Invalid byte order: {}",
                byte_order
            ))),
        }
    }
}

impl FourTelemetryTableManagerTrait for StandardFourTelemetryManager {
    fn load_standard_points<P: AsRef<Path>>(&mut self, file_path: P, channel_name: &str, category: TelemetryCategory) -> Result<()> {
        let path = file_path.as_ref();
        let file = std::fs::File::open(path)
            .map_err(|e| ComSrvError::IoError(format!("Failed to open file {}: {}", path.display(), e)))?;

        let mut reader = ReaderBuilder::new()
            .has_headers(true)
            .delimiter(b',')
            .from_reader(file);

        let mut points = Vec::new();
        for result in reader.deserialize() {
            let mut record: StandardPointRecord = result.map_err(|e| {
                ComSrvError::ConfigError(format!("Failed to parse standard point in {}: {}", path.display(), e))
            })?;
            
            // 设置四遥类型
            record.telemetry_category = category.clone();
            
            // 验证记录
            if let Some(validator) = self.validators.get(&ProtocolType::Modbus) {
                let warnings = validator.validate_point_record(&record)?;
                for warning in warnings {
                    log::warn!("⚠️ [VALIDATION] {}", warning);
                }
            }
            
            points.push(record);
        }

        let points_count = points.len();
        self.standard_points
            .entry(channel_name.to_string())
            .or_insert_with(HashMap::new)
            .insert(category.clone(), points);

        log::info!("📊 [STANDARD] Loaded {} {} points for channel '{}'", 
                  points_count, 
                  category.table_suffix(), 
                  channel_name);

        Ok(())
    }

    fn load_protocol_config<P: AsRef<Path>>(&mut self, file_path: P, channel_name: &str, category: TelemetryCategory) -> Result<()> {
        // 默认加载Modbus配置，可以根据文件名或其他方式判断协议类型
        self.load_modbus_protocol_config(file_path, channel_name, category)
    }

    fn build_mappings(&mut self) -> Result<()> {
        self.point_mappings.clear();

        for (channel_name, channel_standards) in &self.standard_points {
            if let Some(channel_protocols) = self.protocol_configs.get(channel_name) {
                let mut channel_mappings = HashMap::new();

                for (category, standard_points) in channel_standards {
                    if let Some(protocol_configs) = channel_protocols.get(category) {
                        let mut category_mappings = HashMap::new();

                        // 创建协议配置的点位ID索引
                        let mut protocol_by_id: HashMap<u32, &Box<dyn ProtocolConfig>> = HashMap::new();
                        for config in protocol_configs {
                            protocol_by_id.insert(config.point_id(), config);
                        }

                        // 匹配标准点表和协议配置
                        for standard_point in standard_points {
                            if let Some(protocol_config) = protocol_by_id.get(&standard_point.point_id) {
                                // 验证映射
                                if let Some(validator) = self.validators.get(&protocol_config.protocol_type()) {
                                    let warnings = validator.validate_mapping(standard_point, protocol_config.as_ref())?;
                                    for warning in warnings {
                                        log::warn!("⚠️ [MAPPING] {}", warning);
                                    }
                                }

                                category_mappings.insert(
                                    standard_point.point_id,
                                    (standard_point.clone(), protocol_config.clone_box()),
                                );
                            } else {
                                log::warn!("⚠️ [MAPPING] No protocol config found for standard point ID {} in channel '{}'", 
                                          standard_point.point_id, channel_name);
                            }
                        }

                        if !category_mappings.is_empty() {
                            channel_mappings.insert(category.clone(), category_mappings);
                        }
                    }
                }

                if !channel_mappings.is_empty() {
                    self.point_mappings.insert(channel_name.clone(), channel_mappings);
                }
            }
        }

        let total_mappings: usize = self.point_mappings.values()
            .map(|channel| channel.values().map(|category| category.len()).sum::<usize>())
            .sum();

        log::info!("🔗 [MAPPING] Built {} total point mappings across {} channels", 
                  total_mappings, self.point_mappings.len());

        Ok(())
    }

    fn get_channel_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.standard_points.keys().cloned().collect();
        names.sort();
        names
    }

    fn validate_configuration(&self) -> Result<ValidationReport> {
        let mut report = ValidationReport {
            is_valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
            validated_channels: 0,
            validated_points: 0,
        };

        for (channel_name, channel_standards) in &self.standard_points {
            report.validated_channels += 1;

            // 检查是否有对应的协议配置
            if !self.protocol_configs.contains_key(channel_name) {
                report.errors.push(format!("Channel '{}' has standard points but no protocol configuration", channel_name));
                report.is_valid = false;
                continue;
            }

            let channel_protocols = &self.protocol_configs[channel_name];

            for (category, standard_points) in channel_standards {
                // 检查四遥类型是否都有协议配置
                if !channel_protocols.contains_key(category) {
                    report.warnings.push(format!("Channel '{}' missing {} protocol configuration", 
                                                channel_name, category.table_suffix()));
                    continue;
                }

                let protocol_configs = &channel_protocols[category];
                report.validated_points += standard_points.len();

                // 检查点位数量匹配
                if standard_points.len() != protocol_configs.len() {
                    report.warnings.push(format!(
                        "Channel '{}' {} points count mismatch: {} standard vs {} protocol", 
                        channel_name, category.table_suffix(), 
                        standard_points.len(), protocol_configs.len()
                    ));
                }

                // 检查点位ID覆盖
                let standard_ids: std::collections::HashSet<u32> = standard_points.iter().map(|p| p.point_id).collect();
                let protocol_ids: std::collections::HashSet<u32> = protocol_configs.iter().map(|p| p.point_id()).collect();

                let missing_in_protocol: Vec<u32> = standard_ids.difference(&protocol_ids).cloned().collect();
                let missing_in_standard: Vec<u32> = protocol_ids.difference(&standard_ids).cloned().collect();

                if !missing_in_protocol.is_empty() {
                    report.errors.push(format!(
                        "Channel '{}' {} missing protocol configs for point IDs: {:?}", 
                        channel_name, category.table_suffix(), missing_in_protocol
                    ));
                    report.is_valid = false;
                }

                if !missing_in_standard.is_empty() {
                    report.warnings.push(format!(
                        "Channel '{}' {} has unused protocol configs for point IDs: {:?}", 
                        channel_name, category.table_suffix(), missing_in_standard
                    ));
                }
            }
        }

        Ok(report)
    }

    fn get_statistics(&self) -> TableStatistics {
        let mut stats = TableStatistics {
            total_channels: self.standard_points.len(),
            total_protocol_configs: 0,
            total_standard_points: 0,
            total_mapped_points: 0,
            points_by_category: HashMap::new(),
            points_by_protocol: HashMap::new(),
        };

        // 统计标准点表
        for (_, channel_standards) in &self.standard_points {
            for (category, points) in channel_standards {
                stats.total_standard_points += points.len();
                *stats.points_by_category.entry(category.clone()).or_insert(0) += points.len();
            }
        }

        // 统计协议配置
        for (_, channel_protocols) in &self.protocol_configs {
            for (_, configs) in channel_protocols {
                stats.total_protocol_configs += configs.len();
                for config in configs {
                    *stats.points_by_protocol.entry(config.protocol_type()).or_insert(0) += 1;
                }
            }
        }

        // 统计映射点位
        for (_, channel_mappings) in &self.point_mappings {
            for (_, category_mappings) in channel_mappings {
                stats.total_mapped_points += category_mappings.len();
            }
        }

        stats
    }
}

impl Default for StandardFourTelemetryManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for StandardFourTelemetryManager {
    fn clone(&self) -> Self {
        let mut new_manager = Self::new();
        
        // Clone standard points
        new_manager.standard_points = self.standard_points.clone();
        
        // Clone protocol configs
        for (channel_name, channel_configs) in &self.protocol_configs {
            let mut new_channel_configs = HashMap::new();
            for (category, configs) in channel_configs {
                let cloned_configs: Vec<Box<dyn ProtocolConfig>> = configs.iter()
                    .map(|config| config.clone_box())
                    .collect();
                new_channel_configs.insert(*category, cloned_configs);
            }
            new_manager.protocol_configs.insert(channel_name.clone(), new_channel_configs);
        }
        
        // Clone point mappings
        for (channel_name, channel_mappings) in &self.point_mappings {
            let mut new_channel_mappings = HashMap::new();
            for (category, category_mappings) in channel_mappings {
                let mut new_category_mappings = HashMap::new();
                for (point_id, (point, config)) in category_mappings {
                    new_category_mappings.insert(*point_id, (point.clone(), config.clone_box()));
                }
                new_channel_mappings.insert(*category, new_category_mappings);
            }
            new_manager.point_mappings.insert(channel_name.clone(), new_channel_mappings);
        }
        
        new_manager
    }
}

// ============ 向后兼容的类型别名 ============

/// 向后兼容：协议配置记录
pub type ProtocolConfigRecord = ModbusProtocolConfig;

/// 向后兼容：通道点表记录
pub type ChannelPointRecord = StandardPointRecord;

/// 向后兼容：四遥表统计
pub type FourTelemetryStatistics = TableStatistics;

/// 向后兼容：四遥表管理器（具体类型，不是trait）
pub type FourTelemetryTableManager = StandardFourTelemetryManager;
