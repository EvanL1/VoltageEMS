use crate::core::protocols::common::combase::TelemetryType;
use crate::core::protocols::modbus::common::{
    ByteOrder, ModbusDataType, ModbusRegisterMapping, ModbusRegisterType,
};
use crate::utils::{ComSrvError, Result};
use csv::ReaderBuilder;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::time::SystemTime;

/// Simple data point structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataPoint {
    /// Point identifier
    pub id: String,
    /// Point value
    pub value: String,
    /// Data quality (0-100)
    pub quality: u8,
    /// Timestamp when the value was captured
    pub timestamp: SystemTime,
    /// Point description
    pub description: String,
}

impl DataPoint {
    /// Create a new data point
    pub fn new(id: String, value: String, quality: u8, description: String) -> Self {
        Self {
            id,
            value,
            quality,
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

/// 四遥类型枚举
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
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

    /// 从字符串解析
    pub fn from_str(s: &str) -> Result<Self> {
        match s {
            "遥测" => Ok(TelemetryCategory::Telemetry),
            "遥信" => Ok(TelemetryCategory::Signaling),
            "遥调" => Ok(TelemetryCategory::Setpoint),
            "遥控" => Ok(TelemetryCategory::Control),
            _ => Err(ComSrvError::ConfigError(format!(
                "Unknown telemetry category: {}",
                s
            ))),
        }
    }

    /// 获取表名后缀
    pub fn table_suffix(&self) -> &'static str {
        match self {
            TelemetryCategory::Telemetry => "遥测",
            TelemetryCategory::Signaling => "遥信",
            TelemetryCategory::Setpoint => "遥调",
            TelemetryCategory::Control => "遥控",
        }
    }
}

/// 协议配置记录 - Protocol-specific configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProtocolConfigRecord {
    /// 点位ID（表内独立编号）
    pub point_id: u32,
    /// 协议地址 (如Modbus寄存器地址)
    pub protocol_address: u16,
    /// 协议功能码 (如Modbus功能码)
    pub function_code: u8,
    /// 数据类型 (UInt16, Int16, UInt32, Int32, Float32, Bool)
    pub data_type: String,
    /// 字节序 (ABCD, DCBA, BADC, CDAB)
    pub byte_order: String,
    /// 描述
    pub description: String,
}

/// 通道点表记录 - Channel point configuration  
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChannelPointRecord {
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
}

// Default value functions
fn default_scale() -> f64 {
    1.0
}

/// 四遥分离表管理器
#[derive(Debug, Clone)]
pub struct FourTelemetryTableManager {
    /// 协议配置表 (channel_name -> telemetry_category -> protocol_configs)
    protocol_configs: HashMap<String, HashMap<TelemetryCategory, Vec<ProtocolConfigRecord>>>,
    /// 通道点表 (channel_name -> telemetry_category -> channel_points)  
    channel_points: HashMap<String, HashMap<TelemetryCategory, Vec<ChannelPointRecord>>>,
    /// 点位映射 (channel_name -> telemetry_category -> point_id -> (protocol_config, channel_point))
    point_mappings: HashMap<
        String,
        HashMap<TelemetryCategory, HashMap<u32, (ProtocolConfigRecord, ChannelPointRecord)>>,
    >,
}

impl FourTelemetryTableManager {
    /// 创建新的四遥表管理器
    pub fn new() -> Self {
        Self {
            protocol_configs: HashMap::new(),
            channel_points: HashMap::new(),
            point_mappings: HashMap::new(),
        }
    }

    /// 从目录加载所有CSV文件
    /// 期望目录结构：
    /// - {channel_name}_遥测_protocol.csv - 遥测协议配置表
    /// - {channel_name}_遥测_points.csv - 遥测通道点表
    /// - {channel_name}_遥信_protocol.csv - 遥信协议配置表  
    /// - {channel_name}_遥信_points.csv - 遥信通道点表
    /// - {channel_name}_遥控_protocol.csv - 遥控协议配置表
    /// - {channel_name}_遥控_points.csv - 遥控通道点表
    /// - {channel_name}_遥调_protocol.csv - 遥调协议配置表
    /// - {channel_name}_遥调_points.csv - 遥调通道点表
    pub fn load_from_directory<P: AsRef<Path>>(&mut self, dir_path: P) -> Result<()> {
        let dir_path = dir_path.as_ref();

        log::info!(
            "🔍 [FOUR CSV] Loading CSV files from directory: {}",
            dir_path.display()
        );

        if !dir_path.exists() || !dir_path.is_dir() {
            return Err(ComSrvError::ConfigError(format!(
                "CSV directory not found: {}",
                dir_path.display()
            )));
        }

        let entries = std::fs::read_dir(dir_path).map_err(|e| {
            ComSrvError::ConfigError(format!(
                "Failed to read directory {}: {}",
                dir_path.display(),
                e
            ))
        })?;

        let mut protocol_files = Vec::new();
        let mut point_files = Vec::new();

        // 收集所有CSV文件
        for entry in entries {
            let entry = entry.map_err(|e| {
                ComSrvError::ConfigError(format!("Failed to read directory entry: {}", e))
            })?;

            let path = entry.path();
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("csv") {
                let file_name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown");

                // 解析文件名格式: {channel_name}_{telemetry_type}_{table_type}.csv
                if let Some((channel_part, table_type)) = file_name.rsplit_once('_') {
                    if table_type == "protocol" {
                        if let Some((channel_name, telemetry_type)) = channel_part.rsplit_once('_')
                        {
                            if let Ok(category) = TelemetryCategory::from_str(telemetry_type) {
                                protocol_files.push((
                                    path.clone(),
                                    channel_name.to_string(),
                                    category,
                                ));
                            }
                        }
                    } else if table_type == "points" {
                        if let Some((channel_name, telemetry_type)) = channel_part.rsplit_once('_')
                        {
                            if let Ok(category) = TelemetryCategory::from_str(telemetry_type) {
                                point_files.push((
                                    path.clone(),
                                    channel_name.to_string(),
                                    category,
                                ));
                            }
                        }
                    }
                }
            }
        }

        log::info!(
            "🔍 [FOUR CSV] Found {} protocol files and {} point files",
            protocol_files.len(),
            point_files.len()
        );

        // 加载协议配置文件
        for (path, channel_name, category) in protocol_files {
            log::info!(
                "📁 [FOUR CSV] Loading protocol config: {} for channel '{}' category '{:?}'",
                path.display(),
                channel_name,
                category
            );
            self.load_protocol_config(&path, &channel_name, category)?;
        }

        // 加载通道点表文件
        for (path, channel_name, category) in point_files {
            log::info!(
                "📁 [FOUR CSV] Loading channel points: {} for channel '{}' category '{:?}'",
                path.display(),
                channel_name,
                category
            );
            self.load_channel_points(&path, &channel_name, category)?;
        }

        // 构建点位映射
        self.build_point_mappings()?;

        log::info!(
            "✅ [FOUR CSV] Successfully loaded CSV configuration for {} channels",
            self.get_channel_names().len()
        );

        Ok(())
    }

    /// 加载协议配置文件
    pub fn load_protocol_config<P: AsRef<Path>>(
        &mut self,
        file_path: P,
        channel_name: &str,
        category: TelemetryCategory,
    ) -> Result<()> {
        let file_path = file_path.as_ref();

        if !file_path.exists() {
            return Err(ComSrvError::ConfigError(format!(
                "Protocol config file not found: {}",
                file_path.display()
            )));
        }

        let mut reader = ReaderBuilder::new()
            .has_headers(true)
            .from_path(file_path)
            .map_err(|e| {
                ComSrvError::ConfigError(format!(
                    "Failed to open protocol config file {}: {}",
                    file_path.display(),
                    e
                ))
            })?;

        let mut records = Vec::new();

        for result in reader.deserialize() {
            let record: ProtocolConfigRecord = result.map_err(|e| {
                ComSrvError::ConfigError(format!(
                    "Failed to parse protocol config record in {}: {}",
                    file_path.display(),
                    e
                ))
            })?;

            // 验证记录
            self.validate_protocol_record(&record)?;
            records.push(record);
        }

        log::info!(
            "📊 [FOUR CSV] Loaded {} protocol config records for channel '{}' category '{:?}'",
            records.len(),
            channel_name,
            category
        );

        self.protocol_configs
            .entry(channel_name.to_string())
            .or_insert_with(HashMap::new)
            .insert(category, records);

        Ok(())
    }

    /// 加载通道点表文件
    pub fn load_channel_points<P: AsRef<Path>>(
        &mut self,
        file_path: P,
        channel_name: &str,
        category: TelemetryCategory,
    ) -> Result<()> {
        let file_path = file_path.as_ref();

        if !file_path.exists() {
            return Err(ComSrvError::ConfigError(format!(
                "Channel points file not found: {}",
                file_path.display()
            )));
        }

        let mut reader = ReaderBuilder::new()
            .has_headers(true)
            .from_path(file_path)
            .map_err(|e| {
                ComSrvError::ConfigError(format!(
                    "Failed to open channel points file {}: {}",
                    file_path.display(),
                    e
                ))
            })?;

        let mut records = Vec::new();

        for result in reader.deserialize() {
            let record: ChannelPointRecord = result.map_err(|e| {
                ComSrvError::ConfigError(format!(
                    "Failed to parse channel point record in {}: {}",
                    file_path.display(),
                    e
                ))
            })?;

            // 验证记录
            self.validate_channel_record(&record)?;
            records.push(record);
        }

        log::info!(
            "📊 [FOUR CSV] Loaded {} channel point records for channel '{}' category '{:?}'",
            records.len(),
            channel_name,
            category
        );

        self.channel_points
            .entry(channel_name.to_string())
            .or_insert_with(HashMap::new)
            .insert(category, records);

        Ok(())
    }

    /// 构建点位映射关系
    fn build_point_mappings(&mut self) -> Result<()> {
        for channel_name in self.get_channel_names() {
            let mut channel_mappings = HashMap::new();

            // 为每个四遥类型构建映射
            for category in [
                TelemetryCategory::Telemetry,
                TelemetryCategory::Signaling,
                TelemetryCategory::Control,
                TelemetryCategory::Setpoint,
            ] {
                let mut category_mappings = HashMap::new();

                let empty_protocol_configs = Vec::new();
                let empty_channel_points = Vec::new();
                let protocol_configs = self
                    .protocol_configs
                    .get(&channel_name)
                    .and_then(|ch| ch.get(&category))
                    .unwrap_or(&empty_protocol_configs);
                let channel_points = self
                    .channel_points
                    .get(&channel_name)
                    .and_then(|ch| ch.get(&category))
                    .unwrap_or(&empty_channel_points);

                // 创建通道点表的索引映射
                let mut points_by_id: HashMap<u32, &ChannelPointRecord> = HashMap::new();
                for point in channel_points {
                    points_by_id.insert(point.point_id, point);
                }

                // 匹配协议配置和通道点表
                for protocol_config in protocol_configs {
                    if let Some(channel_point) = points_by_id.get(&protocol_config.point_id) {
                        category_mappings.insert(
                            protocol_config.point_id,
                            (protocol_config.clone(), (*channel_point).clone()),
                        );
                    } else {
                        log::warn!("📊 [FOUR CSV] No matching channel point found for protocol config point {} in channel '{}' category '{:?}'",
                                   protocol_config.point_id, channel_name, category);
                    }
                }

                if !category_mappings.is_empty() {
                    log::info!(
                        "📊 [FOUR CSV] Built {} point mappings for channel '{}' category '{:?}'",
                        category_mappings.len(),
                        channel_name,
                        category
                    );
                    channel_mappings.insert(category, category_mappings);
                }
            }

            if !channel_mappings.is_empty() {
                self.point_mappings.insert(channel_name, channel_mappings);
            }
        }

        Ok(())
    }

    /// 获取所有通道名称
    pub fn get_channel_names(&self) -> Vec<String> {
        let mut channels = std::collections::HashSet::new();
        channels.extend(self.protocol_configs.keys().cloned());
        channels.extend(self.channel_points.keys().cloned());
        channels.into_iter().collect()
    }

    /// 获取通道的点位映射
    pub fn get_channel_mappings(
        &self,
        channel_name: &str,
    ) -> Option<&HashMap<TelemetryCategory, HashMap<u32, (ProtocolConfigRecord, ChannelPointRecord)>>>
    {
        self.point_mappings.get(channel_name)
    }

    /// 获取表名称（通道名称）- 兼容性方法
    pub fn get_table_names(&self) -> Vec<String> {
        self.get_channel_names()
    }

    /// 查找特定点位 - 兼容性方法
    pub fn find_point(&self, channel_name: &str, point_id: &str) -> Option<ChannelPointRecord> {
        let point_id = point_id.parse::<u32>().ok()?;

        let mappings = self.point_mappings.get(channel_name)?;
        for (_, category_mappings) in mappings {
            if let Some((_, channel_point)) = category_mappings.get(&point_id) {
                return Some(channel_point.clone());
            }
        }
        None
    }

    /// 插入或更新点位 - 兼容性方法
    pub fn upsert_point(&mut self, channel_name: &str, point: ChannelPointRecord) -> Result<()> {
        // 这是一个简化的实现，实际使用中需要确定四遥类型
        // 这里假设遥测类型作为默认
        let category = TelemetryCategory::Telemetry;

        self.channel_points
            .entry(channel_name.to_string())
            .or_insert_with(HashMap::new)
            .entry(category)
            .or_insert_with(Vec::new)
            .push(point);

        // 重建映射
        self.build_point_mappings()?;
        Ok(())
    }

    /// 删除点位 - 兼容性方法  
    pub fn remove_point(&mut self, channel_name: &str, point_id: &str) -> Result<bool> {
        let point_id = point_id
            .parse::<u32>()
            .map_err(|_| ComSrvError::ConfigError(format!("Invalid point ID: {}", point_id)))?;

        let mut removed = false;

        if let Some(channel_points) = self.channel_points.get_mut(channel_name) {
            for (_, category_points) in channel_points.iter_mut() {
                if let Some(pos) = category_points.iter().position(|p| p.point_id == point_id) {
                    category_points.remove(pos);
                    removed = true;
                    break;
                }
            }
        }

        if removed {
            // 重建映射
            self.build_point_mappings()?;
        }

        Ok(removed)
    }

    /// 获取通道点位 - 兼容性方法
    pub fn get_points(&self, channel_name: &str) -> Option<Vec<ChannelPointRecord>> {
        let channel_points = self.channel_points.get(channel_name)?;
        let mut all_points = Vec::new();

        for (_, category_points) in channel_points {
            all_points.extend(category_points.clone());
        }

        if all_points.is_empty() {
            None
        } else {
            Some(all_points)
        }
    }

    /// 获取表统计信息 - 兼容性方法
    pub fn get_table_stats(&self, channel_name: &str) -> Option<FourTelemetryStatistics> {
        if !self.point_mappings.contains_key(channel_name) {
            return None;
        }

        let mut stats = FourTelemetryStatistics {
            total_channels: 1,
            total_protocol_configs: 0,
            total_channel_points: 0,
            total_mapped_points: 0,
            telemetry_points: 0,
            signaling_points: 0,
            control_points: 0,
            setpoint_points: 0,
        };

        // 统计该通道的协议配置
        if let Some(channel_configs) = self.protocol_configs.get(channel_name) {
            for (category, configs) in channel_configs {
                stats.total_protocol_configs += configs.len();
                match category {
                    TelemetryCategory::Telemetry => stats.telemetry_points += configs.len(),
                    TelemetryCategory::Signaling => stats.signaling_points += configs.len(),
                    TelemetryCategory::Control => stats.control_points += configs.len(),
                    TelemetryCategory::Setpoint => stats.setpoint_points += configs.len(),
                }
            }
        }

        // 统计该通道的点表
        if let Some(channel_points) = self.channel_points.get(channel_name) {
            for (_, points) in channel_points {
                stats.total_channel_points += points.len();
            }
        }

        // 统计映射点位
        if let Some(channel_mappings) = self.point_mappings.get(channel_name) {
            for (_, category_mappings) in channel_mappings {
                stats.total_mapped_points += category_mappings.len();
            }
        }

        Some(stats)
    }

    /// 转换为Modbus寄存器映射  
    pub fn to_modbus_mappings(&self, channel_name: &str) -> Result<Vec<ModbusRegisterMapping>> {
        let mappings = self.point_mappings.get(channel_name).ok_or_else(|| {
            ComSrvError::ConfigError(format!(
                "No point mappings found for channel: {}",
                channel_name
            ))
        })?;

        let mut modbus_mappings = Vec::new();

        for (_category, category_mappings) in mappings {
            for (_point_id, (protocol_config, channel_point)) in category_mappings {
                let data_type = self.parse_data_type(&protocol_config.data_type)?;
                let register_type =
                    self.parse_function_code_to_register_type(protocol_config.function_code)?;
                let byte_order = self.parse_byte_order(&protocol_config.byte_order)?;

                let mapping = ModbusRegisterMapping {
                    name: channel_point.point_name.clone(),
                    display_name: Some(channel_point.point_name.clone()),
                    register_type,
                    address: protocol_config.protocol_address,
                    data_type,
                    scale: channel_point.scale,
                    offset: channel_point.offset,
                    unit: if channel_point.unit.is_empty() {
                        None
                    } else {
                        Some(channel_point.unit.clone())
                    },
                    description: if channel_point.description.is_empty() {
                        None
                    } else {
                        Some(channel_point.description.clone())
                    },
                    access_mode: if protocol_config.function_code <= 4 {
                        "read".to_string()
                    } else {
                        "write".to_string()
                    },
                    group: None,
                    byte_order,
                };

                modbus_mappings.push(mapping);
            }
        }

        log::info!(
            "📊 [FOUR CSV] Generated {} Modbus mappings for channel '{}'",
            modbus_mappings.len(),
            channel_name
        );

        Ok(modbus_mappings)
    }

    /// 验证协议配置记录
    fn validate_protocol_record(&self, record: &ProtocolConfigRecord) -> Result<()> {
        // 验证数据类型
        self.parse_data_type(&record.data_type)?;

        // 验证功能码
        if !(1..=16).contains(&record.function_code) {
            return Err(ComSrvError::ConfigError(format!(
                "Invalid Modbus function code: {}",
                record.function_code
            )));
        }

        // 验证字节序
        self.parse_byte_order(&record.byte_order)?;

        Ok(())
    }

    /// 验证通道点表记录
    fn validate_channel_record(&self, record: &ChannelPointRecord) -> Result<()> {
        // 验证系数不能为0
        if record.scale == 0.0 {
            return Err(ComSrvError::ConfigError(format!(
                "Scale factor cannot be zero for point: {}",
                record.point_name
            )));
        }

        Ok(())
    }

    /// 解析数据类型
    fn parse_data_type(&self, data_type: &str) -> Result<ModbusDataType> {
        match data_type {
            "UInt16" => Ok(ModbusDataType::UInt16),
            "Int16" => Ok(ModbusDataType::Int16),
            "UInt32" => Ok(ModbusDataType::UInt32),
            "Int32" => Ok(ModbusDataType::Int32),
            "Float32" => Ok(ModbusDataType::Float32),
            "Bool" => Ok(ModbusDataType::Bool),
            _ => Err(ComSrvError::ConfigError(format!(
                "Unsupported data type: {}",
                data_type
            ))),
        }
    }

    /// 解析功能码到寄存器类型
    fn parse_function_code_to_register_type(
        &self,
        function_code: u8,
    ) -> Result<ModbusRegisterType> {
        match function_code {
            1 => Ok(ModbusRegisterType::Coil),
            2 => Ok(ModbusRegisterType::DiscreteInput),
            3 => Ok(ModbusRegisterType::HoldingRegister),
            4 => Ok(ModbusRegisterType::InputRegister),
            5 | 15 => Ok(ModbusRegisterType::Coil),
            6 | 16 => Ok(ModbusRegisterType::HoldingRegister),
            _ => Err(ComSrvError::ConfigError(format!(
                "Unsupported function code: {}",
                function_code
            ))),
        }
    }

    /// 解析字节序 - 支持ABCD格式
    fn parse_byte_order(&self, byte_order: &str) -> Result<ByteOrder> {
        match byte_order {
            "ABCD" => Ok(ByteOrder::BigEndian),
            "DCBA" => Ok(ByteOrder::LittleEndian),
            "BADC" => Ok(ByteOrder::BigEndianWordSwapped),
            "CDAB" => Ok(ByteOrder::LittleEndianWordSwapped),
            _ => Err(ComSrvError::ConfigError(format!(
                "Unsupported byte order: {}",
                byte_order
            ))),
        }
    }

    /// 获取统计信息
    pub fn get_statistics(&self) -> FourTelemetryStatistics {
        let mut stats = FourTelemetryStatistics {
            total_channels: self.get_channel_names().len(),
            total_protocol_configs: 0,
            total_channel_points: 0,
            total_mapped_points: 0,
            telemetry_points: 0,
            signaling_points: 0,
            control_points: 0,
            setpoint_points: 0,
        };

        // 统计协议配置
        for channel_configs in self.protocol_configs.values() {
            for (category, configs) in channel_configs {
                stats.total_protocol_configs += configs.len();
                match category {
                    TelemetryCategory::Telemetry => stats.telemetry_points += configs.len(),
                    TelemetryCategory::Signaling => stats.signaling_points += configs.len(),
                    TelemetryCategory::Control => stats.control_points += configs.len(),
                    TelemetryCategory::Setpoint => stats.setpoint_points += configs.len(),
                }
            }
        }

        // 统计通道点表
        for channel_points in self.channel_points.values() {
            for (_, points) in channel_points {
                stats.total_channel_points += points.len();
            }
        }

        // 统计映射点位
        for channel_mappings in self.point_mappings.values() {
            for (_, category_mappings) in channel_mappings {
                stats.total_mapped_points += category_mappings.len();
            }
        }

        stats
    }
}

/// 四遥统计信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FourTelemetryStatistics {
    /// 总通道数
    pub total_channels: usize,
    /// 总协议配置数
    pub total_protocol_configs: usize,
    /// 总通道点表数
    pub total_channel_points: usize,
    /// 总映射点位数
    pub total_mapped_points: usize,
    /// 遥测点数
    pub telemetry_points: usize,
    /// 遥信点数
    pub signaling_points: usize,
    /// 遥控点数
    pub control_points: usize,
    /// 遥调点数
    pub setpoint_points: usize,
}

impl Default for FourTelemetryTableManager {
    fn default() -> Self {
        Self::new()
    }
}

// Legacy type aliases - use new types instead

// Removed ModbusCsvPointConfig - replaced by FourTelemetryTableManager structure

// Removed ModbusCsvPointManager - replaced by FourTelemetryTableManager

// Removed all ModbusCsvPointManager related code - replaced by FourTelemetryTableManager
