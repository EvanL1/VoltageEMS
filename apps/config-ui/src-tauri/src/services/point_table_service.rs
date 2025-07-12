use crate::models::point_table::*;
use crate::models::protocol_mapping::*;
use chrono::Local;
use csv::{Reader, Writer};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct PointTableService {
    tables: Arc<RwLock<HashMap<String, PointTable>>>,
    base_path: PathBuf,
}

impl PointTableService {
    pub fn new() -> Self {
        let base_path = std::env::current_dir()
            .unwrap()
            .join("point_tables");
        
        std::fs::create_dir_all(&base_path).ok();
        
        Self {
            tables: Arc::new(RwLock::new(HashMap::new())),
            base_path,
        }
    }

    pub async fn get_all_tables(&self) -> Result<Vec<PointTableMetadata>, String> {
        let tables = self.tables.read().await;
        let metadata: Vec<PointTableMetadata> = tables
            .values()
            .map(|table| PointTableMetadata {
                id: table.id.clone(),
                name: table.name.clone(),
                protocol_type: table.protocol_type.clone(),
                channel_id: None,
                created_at: Local::now().to_rfc3339(),
                updated_at: Local::now().to_rfc3339(),
                point_counts: PointCounts {
                    telemetry: table.telemetry.len(),
                    signal: table.signal.len(),
                    control: table.control.len(),
                    adjustment: table.adjustment.len(),
                },
            })
            .collect();
        Ok(metadata)
    }

    pub async fn get_table(&self, id: &str) -> Result<PointTable, String> {
        let tables = self.tables.read().await;
        tables
            .get(id)
            .cloned()
            .ok_or_else(|| format!("Point table with id {} not found", id))
    }

    pub async fn create_table(
        &self,
        name: String,
        protocol_type: String,
    ) -> Result<PointTableMetadata, String> {
        let id = format!("{}_{}", protocol_type, chrono::Utc::now().timestamp());
        let table = PointTable {
            id: id.clone(),
            name: name.clone(),
            protocol_type: protocol_type.clone(),
            telemetry: Vec::new(),
            signal: Vec::new(),
            control: Vec::new(),
            adjustment: Vec::new(),
            telemetry_mapping: Vec::new(),
            signal_mapping: Vec::new(),
            control_mapping: Vec::new(),
            adjustment_mapping: Vec::new(),
        };

        let mut tables = self.tables.write().await;
        tables.insert(id.clone(), table);

        Ok(PointTableMetadata {
            id,
            name,
            protocol_type,
            channel_id: None,
            created_at: Local::now().to_rfc3339(),
            updated_at: Local::now().to_rfc3339(),
            point_counts: PointCounts {
                telemetry: 0,
                signal: 0,
                control: 0,
                adjustment: 0,
            },
        })
    }

    pub async fn delete_table(&self, id: &str) -> Result<(), String> {
        let mut tables = self.tables.write().await;
        tables
            .remove(id)
            .ok_or_else(|| format!("Point table with id {} not found", id))?;
        Ok(())
    }

    pub async fn upload_csv(&self, table_id: &str, csv_type: CsvType, content: String) -> Result<ValidationResult, String> {
        let mut tables = self.tables.write().await;
        let table = tables
            .get_mut(table_id)
            .ok_or_else(|| format!("Point table with id {} not found", table_id))?;

        let mut reader = Reader::from_reader(content.as_bytes());
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
                    CsvType::Telemetry => table.telemetry = definitions,
                    CsvType::Signal => table.signal = definitions,
                    CsvType::Control => table.control = definitions,
                    CsvType::Adjustment => table.adjustment = definitions,
                    _ => unreachable!(),
                }
            }
            CsvType::TelemetryMapping | CsvType::SignalMapping | CsvType::ControlMapping | CsvType::AdjustmentMapping => {
                // 根据协议类型解析不同的映射
                let mappings = match table.protocol_type.as_str() {
                    "modbus_tcp" | "modbus_rtu" => {
                        self.parse_modbus_mappings(&mut reader, &mut errors)
                    }
                    "iec60870" | "iec104" | "iec101" => {
                        self.parse_iec60870_mappings(&mut reader, &mut errors)
                    }
                    "can" => {
                        self.parse_can_mappings(&mut reader, &mut errors)
                    }
                    _ => {
                        errors.push(ValidationError {
                            row: None,
                            column: None,
                            message: format!("Unsupported protocol type: {}", table.protocol_type),
                        });
                        Vec::new()
                    }
                };

                match csv_type {
                    CsvType::TelemetryMapping => table.telemetry_mapping = mappings,
                    CsvType::SignalMapping => table.signal_mapping = mappings,
                    CsvType::ControlMapping => table.control_mapping = mappings,
                    CsvType::AdjustmentMapping => table.adjustment_mapping = mappings,
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

    fn parse_modbus_mappings(
        &self,
        reader: &mut Reader<&[u8]>,
        errors: &mut Vec<ValidationError>,
    ) -> Vec<ProtocolMappingEnum> {
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

    fn parse_iec60870_mappings(
        &self,
        reader: &mut Reader<&[u8]>,
        errors: &mut Vec<ValidationError>,
    ) -> Vec<ProtocolMappingEnum> {
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

    fn parse_can_mappings(
        &self,
        reader: &mut Reader<&[u8]>,
        errors: &mut Vec<ValidationError>,
    ) -> Vec<ProtocolMappingEnum> {
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

    pub async fn export_csv(&self, table_id: &str, csv_type: CsvType) -> Result<String, String> {
        let tables = self.tables.read().await;
        let table = tables
            .get(table_id)
            .ok_or_else(|| format!("Point table with id {} not found", table_id))?;

        let mut writer = Writer::from_writer(vec![]);

        match csv_type {
            CsvType::Telemetry => {
                for def in &table.telemetry {
                    writer.serialize(def).map_err(|e| e.to_string())?;
                }
            }
            CsvType::Signal => {
                for def in &table.signal {
                    writer.serialize(def).map_err(|e| e.to_string())?;
                }
            }
            CsvType::Control => {
                for def in &table.control {
                    writer.serialize(def).map_err(|e| e.to_string())?;
                }
            }
            CsvType::Adjustment => {
                for def in &table.adjustment {
                    writer.serialize(def).map_err(|e| e.to_string())?;
                }
            }
            CsvType::TelemetryMapping | CsvType::SignalMapping | CsvType::ControlMapping | CsvType::AdjustmentMapping => {
                let mappings = match csv_type {
                    CsvType::TelemetryMapping => &table.telemetry_mapping,
                    CsvType::SignalMapping => &table.signal_mapping,
                    CsvType::ControlMapping => &table.control_mapping,
                    CsvType::AdjustmentMapping => &table.adjustment_mapping,
                    _ => unreachable!(),
                };

                // 写入头部
                let headers = CsvHeaders::default();
                let header_row = match table.protocol_type.as_str() {
                    "modbus_tcp" | "modbus_rtu" => headers.modbus,
                    "iec60870" | "iec104" | "iec101" => headers.iec60870,
                    "can" => headers.can,
                    _ => return Err(format!("Unsupported protocol type: {}", table.protocol_type)),
                };
                writer.write_record(&header_row).map_err(|e| e.to_string())?;

                // 写入数据
                for mapping in mappings {
                    match mapping {
                        ProtocolMappingEnum::Modbus(m) => {
                            writer.serialize(m).map_err(|e| e.to_string())?;
                        }
                        ProtocolMappingEnum::IEC60870(m) => {
                            writer.serialize(m).map_err(|e| e.to_string())?;
                        }
                        ProtocolMappingEnum::CAN(m) => {
                            writer.serialize(m).map_err(|e| e.to_string())?;
                        }
                    }
                }
            }
        }

        String::from_utf8(writer.into_inner().map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())
    }

    pub async fn validate_table(&self, table_id: &str) -> Result<ValidationResult, String> {
        let tables = self.tables.read().await;
        let table = tables
            .get(table_id)
            .ok_or_else(|| format!("Point table with id {} not found", table_id))?;

        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        // 验证点ID唯一性
        let mut point_ids = std::collections::HashSet::new();
        for def in table.telemetry.iter()
            .chain(table.signal.iter())
            .chain(table.control.iter())
            .chain(table.adjustment.iter())
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
        for mapping in table.telemetry_mapping.iter()
            .chain(table.signal_mapping.iter())
            .chain(table.control_mapping.iter())
            .chain(table.adjustment_mapping.iter())
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

    pub async fn update_point(
        &self,
        table_id: &str,
        point_type: &str,
        point_id: u32,
        point_data: serde_json::Value,
    ) -> Result<(), String> {
        let mut tables = self.tables.write().await;
        let table = tables
            .get_mut(table_id)
            .ok_or_else(|| format!("Point table with id {} not found", table_id))?;

        let definition: PointDefinition = serde_json::from_value(point_data)
            .map_err(|e| format!("Invalid point data: {}", e))?;

        let points = match point_type {
            "telemetry" => &mut table.telemetry,
            "signal" => &mut table.signal,
            "control" => &mut table.control,
            "adjustment" => &mut table.adjustment,
            _ => return Err("Invalid point type".to_string()),
        };

        if let Some(point) = points.iter_mut().find(|p| p.point_id == point_id) {
            *point = definition;
        } else {
            return Err(format!("Point with id {} not found", point_id));
        }

        Ok(())
    }

    pub async fn delete_point(
        &self,
        table_id: &str,
        point_type: &str,
        point_id: u32,
    ) -> Result<(), String> {
        let mut tables = self.tables.write().await;
        let table = tables
            .get_mut(table_id)
            .ok_or_else(|| format!("Point table with id {} not found", table_id))?;

        let points = match point_type {
            "telemetry" => &mut table.telemetry,
            "signal" => &mut table.signal,
            "control" => &mut table.control,
            "adjustment" => &mut table.adjustment,
            _ => return Err("Invalid point type".to_string()),
        };

        points.retain(|p| p.point_id != point_id);
        Ok(())
    }

    pub async fn export_to_comsrv_format(&self, table_id: &str) -> Result<String, String> {
        let tables = self.tables.read().await;
        let table = tables
            .get(table_id)
            .ok_or_else(|| format!("Point table with id {} not found", table_id))?;

        let dir_name = format!("{}_{}", table.protocol_type, table.name.replace(" ", "_"));
        let export_path = self.base_path.join(&dir_name);
        std::fs::create_dir_all(&export_path).map_err(|e| e.to_string())?;

        // 导出四遥CSV文件
        for (csv_type, file_name) in [
            (CsvType::Telemetry, "telemetry.csv"),
            (CsvType::Signal, "signal.csv"),
            (CsvType::Control, "control.csv"),
            (CsvType::Adjustment, "adjustment.csv"),
            (CsvType::TelemetryMapping, "mapping_telemetry.csv"),
            (CsvType::SignalMapping, "mapping_signal.csv"),
            (CsvType::ControlMapping, "mapping_control.csv"),
            (CsvType::AdjustmentMapping, "mapping_adjustment.csv"),
        ] {
            let content = self.export_csv(table_id, csv_type).await?;
            let file_path = export_path.join(file_name);
            std::fs::write(&file_path, content).map_err(|e| e.to_string())?;
        }

        Ok(export_path.to_string_lossy().to_string())
    }

    pub async fn export_csv_from_channel(
        &self,
        channel: &crate::models::channel::Channel,
        csv_type: CsvType,
    ) -> Result<String, String> {
        let mut writer = Writer::from_writer(vec![]);

        match csv_type {
            CsvType::Telemetry => {
                for def in &channel.point_table.telemetry {
                    writer.serialize(def).map_err(|e| e.to_string())?;
                }
            }
            CsvType::Signal => {
                for def in &channel.point_table.signal {
                    writer.serialize(def).map_err(|e| e.to_string())?;
                }
            }
            CsvType::Control => {
                for def in &channel.point_table.control {
                    writer.serialize(def).map_err(|e| e.to_string())?;
                }
            }
            CsvType::Adjustment => {
                for def in &channel.point_table.adjustment {
                    writer.serialize(def).map_err(|e| e.to_string())?;
                }
            }
            CsvType::TelemetryMapping | CsvType::SignalMapping | CsvType::ControlMapping | CsvType::AdjustmentMapping => {
                let mappings = match csv_type {
                    CsvType::TelemetryMapping => &channel.point_table.telemetry_mapping,
                    CsvType::SignalMapping => &channel.point_table.signal_mapping,
                    CsvType::ControlMapping => &channel.point_table.control_mapping,
                    CsvType::AdjustmentMapping => &channel.point_table.adjustment_mapping,
                    _ => unreachable!(),
                };

                // 写入头部
                let headers = CsvHeaders::default();
                let header_row = match channel.protocol_type.as_str() {
                    "modbus_tcp" | "modbus_rtu" => headers.modbus,
                    "iec60870" | "iec104" | "iec101" => headers.iec60870,
                    "can" => headers.can,
                    _ => return Err(format!("Unsupported protocol type: {}", channel.protocol_type)),
                };
                writer.write_record(&header_row).map_err(|e| e.to_string())?;

                // 写入数据
                for mapping in mappings {
                    match mapping {
                        ProtocolMappingEnum::Modbus(m) => {
                            writer.serialize(m).map_err(|e| e.to_string())?;
                        }
                        ProtocolMappingEnum::IEC60870(m) => {
                            writer.serialize(m).map_err(|e| e.to_string())?;
                        }
                        ProtocolMappingEnum::CAN(m) => {
                            writer.serialize(m).map_err(|e| e.to_string())?;
                        }
                    }
                }
            }
        }

        String::from_utf8(writer.into_inner().map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())
    }

    pub async fn get_protocol_csv_template(&self, protocol_type: &str, csv_type: CsvType) -> Result<String, String> {
        let headers = CsvHeaders::default();
        let header_row = match protocol_type {
            "modbus_tcp" | "modbus_rtu" => headers.modbus,
            "iec60870" | "iec104" | "iec101" => headers.iec60870,
            "can" => headers.can,
            _ => return Err(format!("Unsupported protocol type: {}", protocol_type)),
        };

        match csv_type {
            CsvType::Telemetry | CsvType::Signal | CsvType::Control | CsvType::Adjustment => {
                // 定义CSV模板
                Ok("point_id,signal_name,chinese_name,data_type,scale,offset,reverse,unit,description,group\n\
                    1001,example_signal,示例信号,FLOAT,1.0,0.0,,V,示例描述,组1\n".to_string())
            }
            CsvType::TelemetryMapping | CsvType::SignalMapping | CsvType::ControlMapping | CsvType::AdjustmentMapping => {
                let mut writer = Writer::from_writer(vec![]);
                writer.write_record(&header_row).map_err(|e| e.to_string())?;
                
                // 添加示例行
                match protocol_type {
                    "modbus_tcp" | "modbus_rtu" => {
                        writer.write_record(&["1001", "example_signal", "1", "3", "0", "float32", "ABCD", "2", "", "示例描述"])
                            .map_err(|e| e.to_string())?;
                    }
                    "iec60870" | "iec104" | "iec101" => {
                        writer.write_record(&["1001", "example_signal", "1", "100", "1", "3", "", "示例描述"])
                            .map_err(|e| e.to_string())?;
                    }
                    "can" => {
                        writer.write_record(&["1001", "example_signal", "0x123", "0", "16", "motorola", "unsigned", "0.1", "0", "0", "100", "km/h", "示例描述"])
                            .map_err(|e| e.to_string())?;
                    }
                    _ => {}
                }

                String::from_utf8(writer.into_inner().map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())
            }
        }
    }
}