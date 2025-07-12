use crate::models::channel::*;
use crate::models::point_table::*;
use crate::models::protocol_mapping::*;
use crate::services::point_table_service::PointTableService;
use chrono::Local;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct ChannelService {
    channels: Arc<RwLock<HashMap<u32, Channel>>>,
    point_table_service: Arc<PointTableService>,
}

impl ChannelService {
    pub fn new(point_table_service: Arc<PointTableService>) -> Self {
        Self {
            channels: Arc::new(RwLock::new(HashMap::new())),
            point_table_service,
        }
    }

    pub async fn get_all_channels(&self) -> Result<Vec<ChannelInfo>, String> {
        let channels = self.channels.read().await;
        let mut channel_infos = Vec::new();

        for channel in channels.values() {
            let point_counts = ChannelPointCounts {
                telemetry: channel.point_table.telemetry.len(),
                signal: channel.point_table.signal.len(),
                control: channel.point_table.control.len(),
                adjustment: channel.point_table.adjustment.len(),
            };

            channel_infos.push(ChannelInfo {
                id: channel.id,
                name: channel.name.clone(),
                protocol: channel.protocol.clone(),
                protocol_type: channel.protocol_type.clone(),
                enabled: channel.enabled,
                status: if channel.enabled { 
                    ChannelStatus::Online 
                } else { 
                    ChannelStatus::Offline 
                },
                point_counts,
                last_update: Local::now().to_rfc3339(),
            });
        }

        Ok(channel_infos)
    }

    pub async fn get_channel(&self, id: u32) -> Result<Channel, String> {
        let channels = self.channels.read().await;
        channels
            .get(&id)
            .cloned()
            .ok_or_else(|| format!("Channel with id {} not found", id))
    }

    pub async fn create_channel(&self, mut channel: Channel) -> Result<Channel, String> {
        // 确保ID唯一
        let mut channels = self.channels.write().await;
        if channels.contains_key(&channel.id) {
            return Err(format!("Channel with id {} already exists", channel.id));
        }

        // 初始化空的点表配置
        if channel.point_table.telemetry.is_empty() {
            channel.point_table = PointTableConfig {
                telemetry: Vec::new(),
                signal: Vec::new(),
                control: Vec::new(),
                adjustment: Vec::new(),
                telemetry_mapping: Vec::new(),
                signal_mapping: Vec::new(),
                control_mapping: Vec::new(),
                adjustment_mapping: Vec::new(),
                csv_config: None,
            };
        }

        channels.insert(channel.id, channel.clone());
        Ok(channel)
    }

    pub async fn update_channel(&self, id: u32, channel: Channel) -> Result<(), String> {
        let mut channels = self.channels.write().await;
        if !channels.contains_key(&id) {
            return Err(format!("Channel with id {} not found", id));
        }

        channels.insert(id, channel);
        Ok(())
    }

    pub async fn delete_channel(&self, id: u32) -> Result<(), String> {
        let mut channels = self.channels.write().await;
        channels
            .remove(&id)
            .ok_or_else(|| format!("Channel with id {} not found", id))?;
        Ok(())
    }

    pub async fn upload_channel_csv(
        &self,
        channel_id: u32,
        csv_type: CsvType,
        content: String,
    ) -> Result<ValidationResult, String> {
        let mut channels = self.channels.write().await;
        let channel = channels
            .get_mut(&channel_id)
            .ok_or_else(|| format!("Channel with id {} not found", channel_id))?;

        // 使用 point_table_service 的 CSV 解析功能
        let mut reader = csv::Reader::from_reader(content.as_bytes());
        let mut errors = Vec::new();
        let warnings = Vec::new();

        match csv_type {
            CsvType::Telemetry | CsvType::Signal | CsvType::Control | CsvType::Adjustment => {
                let mut definitions = Vec::new();
                for (row_idx, result) in reader.deserialize::<PointDefinition>().enumerate() {
                    match result {
                        Ok(def) => definitions.push(def),
                        Err(e) => errors.push(ValidationError {
                            row: Some(row_idx + 2),
                            column: None,
                            message: format!("Failed to parse row: {}", e),
                        }),
                    }
                }

                match csv_type {
                    CsvType::Telemetry => channel.point_table.telemetry = definitions,
                    CsvType::Signal => channel.point_table.signal = definitions,
                    CsvType::Control => channel.point_table.control = definitions,
                    CsvType::Adjustment => channel.point_table.adjustment = definitions,
                    _ => unreachable!(),
                }
            }
            CsvType::TelemetryMapping | CsvType::SignalMapping | CsvType::ControlMapping | CsvType::AdjustmentMapping => {
                // 根据协议类型解析不同的映射
                let mappings = self.parse_protocol_mappings(
                    &channel.protocol_type,
                    &mut reader,
                    &mut errors,
                );

                match csv_type {
                    CsvType::TelemetryMapping => channel.point_table.telemetry_mapping = mappings,
                    CsvType::SignalMapping => channel.point_table.signal_mapping = mappings,
                    CsvType::ControlMapping => channel.point_table.control_mapping = mappings,
                    CsvType::AdjustmentMapping => channel.point_table.adjustment_mapping = mappings,
                    _ => unreachable!(),
                }
            }
        }

        Ok(ValidationResult {
            is_valid: errors.is_empty(),
            errors,
            warnings,
        })
    }

    fn parse_protocol_mappings(
        &self,
        protocol_type: &str,
        reader: &mut csv::Reader<&[u8]>,
        errors: &mut Vec<ValidationError>,
    ) -> Vec<ProtocolMappingEnum> {
        match protocol_type {
            "modbus_tcp" | "modbus_rtu" => {
                let mut mappings = Vec::new();
                for (row_idx, result) in reader.deserialize::<ModbusMapping>().enumerate() {
                    match result {
                        Ok(mapping) => {
                            if let Err(e) = mapping.validate() {
                                errors.push(ValidationError {
                                    row: Some(row_idx + 2),
                                    column: None,
                                    message: e,
                                });
                            } else {
                                mappings.push(ProtocolMappingEnum::Modbus(mapping));
                            }
                        }
                        Err(e) => errors.push(ValidationError {
                            row: Some(row_idx + 2),
                            column: None,
                            message: format!("Failed to parse row: {}", e),
                        }),
                    }
                }
                mappings
            }
            "iec60870" | "iec104" | "iec101" => {
                let mut mappings = Vec::new();
                for (row_idx, result) in reader.deserialize::<IEC60870Mapping>().enumerate() {
                    match result {
                        Ok(mapping) => {
                            if let Err(e) = mapping.validate() {
                                errors.push(ValidationError {
                                    row: Some(row_idx + 2),
                                    column: None,
                                    message: e,
                                });
                            } else {
                                mappings.push(ProtocolMappingEnum::IEC60870(mapping));
                            }
                        }
                        Err(e) => errors.push(ValidationError {
                            row: Some(row_idx + 2),
                            column: None,
                            message: format!("Failed to parse row: {}", e),
                        }),
                    }
                }
                mappings
            }
            "can" => {
                let mut mappings = Vec::new();
                for (row_idx, result) in reader.deserialize::<CanMapping>().enumerate() {
                    match result {
                        Ok(mapping) => {
                            if let Err(e) = mapping.validate() {
                                errors.push(ValidationError {
                                    row: Some(row_idx + 2),
                                    column: None,
                                    message: e,
                                });
                            } else {
                                mappings.push(ProtocolMappingEnum::CAN(mapping));
                            }
                        }
                        Err(e) => errors.push(ValidationError {
                            row: Some(row_idx + 2),
                            column: None,
                            message: format!("Failed to parse row: {}", e),
                        }),
                    }
                }
                mappings
            }
            _ => {
                errors.push(ValidationError {
                    row: None,
                    column: None,
                    message: format!("Unsupported protocol type: {}", protocol_type),
                });
                Vec::new()
            }
        }
    }

    pub async fn export_channel_csv(
        &self,
        channel_id: u32,
        csv_type: CsvType,
    ) -> Result<String, String> {
        let channels = self.channels.read().await;
        let channel = channels
            .get(&channel_id)
            .ok_or_else(|| format!("Channel with id {} not found", channel_id))?;

        // 使用 point_table_service 的导出功能
        self.point_table_service
            .export_csv_from_channel(channel, csv_type)
            .await
    }

    pub async fn get_channel_protocol_template(
        &self,
        protocol_type: &str,
        csv_type: CsvType,
    ) -> Result<String, String> {
        self.point_table_service
            .get_protocol_csv_template(protocol_type, csv_type)
            .await
    }

    pub async fn validate_channel_points(&self, channel_id: u32) -> Result<ValidationResult, String> {
        let channels = self.channels.read().await;
        let channel = channels
            .get(&channel_id)
            .ok_or_else(|| format!("Channel with id {} not found", channel_id))?;

        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        // 验证点ID唯一性
        let mut point_ids = std::collections::HashSet::new();
        for def in channel.point_table.telemetry.iter()
            .chain(channel.point_table.signal.iter())
            .chain(channel.point_table.control.iter())
            .chain(channel.point_table.adjustment.iter())
        {
            if !point_ids.insert(def.point_id) {
                errors.push(ValidationError {
                    row: None,
                    column: Some("point_id".to_string()),
                    message: format!("Duplicate point ID: {}", def.point_id),
                });
            }
        }

        // 验证映射与定义的匹配
        for mapping in channel.point_table.telemetry_mapping.iter()
            .chain(channel.point_table.signal_mapping.iter())
            .chain(channel.point_table.control_mapping.iter())
            .chain(channel.point_table.adjustment_mapping.iter())
        {
            let mapping_point_id = mapping.point_id();
            if !point_ids.contains(&mapping_point_id) {
                warnings.push(ValidationError {
                    row: None,
                    column: Some("point_id".to_string()),
                    message: format!("Mapping references undefined point ID: {}", mapping_point_id),
                });
            }

            // 验证映射本身
            if let Err(e) = mapping.validate() {
                errors.push(ValidationError {
                    row: None,
                    column: None,
                    message: e,
                });
            }
        }

        Ok(ValidationResult {
            is_valid: errors.is_empty(),
            errors,
            warnings,
        })
    }

    pub async fn export_channel_config(&self, channel_id: u32) -> Result<String, String> {
        let channels = self.channels.read().await;
        let channel = channels
            .get(&channel_id)
            .ok_or_else(|| format!("Channel with id {} not found", channel_id))?;

        // 导出为 YAML 格式的通道配置
        serde_yaml::to_string(channel)
            .map_err(|e| format!("Failed to serialize channel config: {}", e))
    }

    pub async fn import_channel_config(&self, yaml_content: String) -> Result<Channel, String> {
        let channel: Channel = serde_yaml::from_str(&yaml_content)
            .map_err(|e| format!("Failed to parse channel config: {}", e))?;

        self.create_channel(channel).await
    }
}