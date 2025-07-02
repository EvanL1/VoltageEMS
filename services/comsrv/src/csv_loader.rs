//! CSV Point Table Loader for voltage-config integration
//!
//! This module provides CSV loading functionality that integrates with
//! the voltage-config framework, allowing comsrv to load point tables
//! from CSV files while using the unified configuration system.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::fs;
use csv::ReaderBuilder;
use figment::value::Value;

use crate::config_new::{
    ChannelConfig, TableConfig, FourTelemetryFiles, 
    FourTelemetryPoint, DataType, CombinedPoint
};

/// CSV Point Table Loader
pub struct CsvPointTableLoader {
    base_path: PathBuf,
}

impl CsvPointTableLoader {
    /// Create a new CSV loader with base path
    pub fn new<P: AsRef<Path>>(base_path: P) -> Self {
        Self {
            base_path: base_path.as_ref().to_path_buf(),
        }
    }
    
    /// Load all point tables for a channel
    pub async fn load_channel_tables(
        &self,
        channel: &ChannelConfig,
    ) -> Result<Vec<CombinedPoint>> {
        if let Some(table_config) = &channel.table_config {
            self.load_separated_tables(table_config, &channel.protocol).await
        } else {
            Ok(vec![])
        }
    }
    
    /// Load separated tables (four telemetry + protocol mappings)
    async fn load_separated_tables(
        &self,
        table_config: &TableConfig,
        protocol: &str,
    ) -> Result<Vec<CombinedPoint>> {
        // Load four telemetry points
        let mut telemetry_points = HashMap::new();
        
        // Load telemetry (YC)
        let yc_points = self.load_four_telemetry_csv(
            &table_config.four_telemetry_files.telemetry,
            "YC"
        ).await?;
        for point in yc_points {
            telemetry_points.insert((point.telemetry_type.clone(), point.point_number), point);
        }
        
        // Load control (YK)
        let yk_points = self.load_four_telemetry_csv(
            &table_config.four_telemetry_files.control,
            "YK"
        ).await?;
        for point in yk_points {
            telemetry_points.insert((point.telemetry_type.clone(), point.point_number), point);
        }
        
        // Load adjustment (YT)
        let yt_points = self.load_four_telemetry_csv(
            &table_config.four_telemetry_files.adjustment,
            "YT"
        ).await?;
        for point in yt_points {
            telemetry_points.insert((point.telemetry_type.clone(), point.point_number), point);
        }
        
        // Load signal (YX)
        let yx_points = self.load_four_telemetry_csv(
            &table_config.four_telemetry_files.signal,
            "YX"
        ).await?;
        for point in yx_points {
            telemetry_points.insert((point.telemetry_type.clone(), point.point_number), point);
        }
        
        // Load protocol mappings
        let protocol_mappings = if let Some(mapping_file) = 
            table_config.protocol_mapping_files.mappings.get(protocol) {
            self.load_protocol_mapping_csv(mapping_file, protocol).await?
        } else {
            HashMap::new()
        };
        
        // Combine telemetry points with protocol mappings
        let mut combined_points = Vec::new();
        for ((telemetry_type, point_number), telemetry_point) in telemetry_points {
            let addresses = protocol_mappings
                .get(&(telemetry_type, point_number))
                .cloned()
                .unwrap_or_default();
            
            combined_points.push(CombinedPoint {
                telemetry: telemetry_point,
                addresses,
            });
        }
        
        Ok(combined_points)
    }
    
    /// Load four telemetry CSV file
    async fn load_four_telemetry_csv(
        &self,
        filename: &str,
        telemetry_type: &str,
    ) -> Result<Vec<FourTelemetryPoint>> {
        let path = self.base_path.join(filename);
        let content = tokio::fs::read_to_string(&path)
            .await
            .with_context(|| format!("Failed to read CSV file: {}", path.display()))?;
        
        let mut reader = ReaderBuilder::new()
            .has_headers(true)
            .from_reader(content.as_bytes());
        
        let mut points = Vec::new();
        
        for result in reader.deserialize::<FourTelemetryCsvRow>() {
            let row = result.with_context(|| 
                format!("Failed to parse CSV row in {}", filename)
            )?;
            
            points.push(FourTelemetryPoint {
                point_number: row.point_number,
                telemetry_type: telemetry_type.to_string(),
                name: row.name,
                data_type: parse_data_type(&row.data_type),
                unit: if row.unit.is_empty() { None } else { Some(row.unit) },
                scale: row.scale,
            });
        }
        
        Ok(points)
    }
    
    /// Load protocol mapping CSV file
    async fn load_protocol_mapping_csv(
        &self,
        filename: &str,
        protocol: &str,
    ) -> Result<HashMap<(String, u32), HashMap<String, Value>>> {
        let path = self.base_path.join(filename);
        let content = tokio::fs::read_to_string(&path)
            .await
            .with_context(|| format!("Failed to read CSV file: {}", path.display()))?;
        
        let mut reader = ReaderBuilder::new()
            .has_headers(true)
            .flexible(true) // Allow variable number of fields
            .from_reader(content.as_bytes());
        
        let headers = reader.headers()?.clone();
        let mut mappings = HashMap::new();
        
        for result in reader.records() {
            let record = result.with_context(|| 
                format!("Failed to parse CSV record in {}", filename)
            )?;
            
            if record.len() < 3 {
                continue; // Skip invalid rows
            }
            
            let telemetry_type = record.get(0).unwrap().to_string();
            let point_number: u32 = record.get(1).unwrap().parse()
                .with_context(|| "Invalid point number")?;
            
            let mut addresses = HashMap::new();
            
            // Parse protocol-specific address based on protocol type
            match protocol {
                "modbus" => {
                    if let Some(addr_str) = record.get(2) {
                        let address: u16 = addr_str.parse()
                            .with_context(|| "Invalid Modbus address")?;
                        addresses.insert("address".to_string(), Value::from(address));
                        
                        // Optional function code
                        if let Some(fc_str) = record.get(3) {
                            if let Ok(fc) = fc_str.parse::<u8>() {
                                addresses.insert("function_code".to_string(), Value::from(fc));
                            }
                        }
                    }
                }
                "iec104" => {
                    if let Some(addr_str) = record.get(2) {
                        let address: u32 = addr_str.parse()
                            .with_context(|| "Invalid IEC104 address")?;
                        addresses.insert("ioa".to_string(), Value::from(address));
                        
                        // Optional type ID
                        if let Some(type_str) = record.get(3) {
                            if let Ok(type_id) = type_str.parse::<u8>() {
                                addresses.insert("type_id".to_string(), Value::from(type_id));
                            }
                        }
                    }
                }
                _ => {
                    // Generic address handling
                    if let Some(addr_str) = record.get(2) {
                        addresses.insert("address".to_string(), Value::from(addr_str.to_string()));
                    }
                }
            }
            
            mappings.insert((telemetry_type, point_number), addresses);
        }
        
        Ok(mappings)
    }
}

/// CSV row structure for four telemetry files
#[derive(Debug, Deserialize)]
struct FourTelemetryCsvRow {
    #[serde(rename = "点号")]
    point_number: u32,
    
    #[serde(rename = "名称")]
    name: String,
    
    #[serde(rename = "数据类型", default)]
    data_type: String,
    
    #[serde(rename = "单位", default)]
    unit: String,
    
    #[serde(rename = "比例系数", default)]
    scale: Option<f64>,
}

/// Parse data type string to enum
fn parse_data_type(type_str: &str) -> DataType {
    match type_str.to_lowercase().as_str() {
        "float" | "浮点" => DataType::Float,
        "int" | "整数" => DataType::Int,
        "bool" | "布尔" => DataType::Bool,
        "string" | "字符串" => DataType::String,
        _ => DataType::Float, // Default
    }
}

/// Integration with voltage-config
pub struct CsvConfigIntegration;

impl CsvConfigIntegration {
    /// Create CSV loader from voltage-config settings
    pub fn create_loader(config: &crate::config_new::ComServiceConfig) -> CsvPointTableLoader {
        CsvPointTableLoader::new(&config.default_paths.point_table_dir)
    }
    
    /// Load all channels' point tables
    pub async fn load_all_channels(
        config: &crate::config_new::ComServiceConfig,
    ) -> Result<HashMap<u16, Vec<CombinedPoint>>> {
        let loader = Self::create_loader(config);
        let mut channel_points = HashMap::new();
        
        for channel in &config.channels {
            let points = loader.load_channel_tables(channel).await?;
            channel_points.insert(channel.id, points);
        }
        
        Ok(channel_points)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_csv_loader_creation() {
        let config = crate::config_new::ComServiceConfig {
            base: Default::default(),
            api: Default::default(),
            default_paths: crate::config_new::DefaultPathConfig {
                config_dir: "config".to_string(),
                point_table_dir: "config/points".to_string(),
            },
            channels: vec![],
            protocols: Default::default(),
        };
        
        let loader = CsvConfigIntegration::create_loader(&config);
        assert_eq!(loader.base_path, PathBuf::from("config/points"));
    }
    
    #[test]
    fn test_parse_data_type() {
        assert!(matches!(parse_data_type("float"), DataType::Float));
        assert!(matches!(parse_data_type("浮点"), DataType::Float));
        assert!(matches!(parse_data_type("int"), DataType::Int));
        assert!(matches!(parse_data_type("整数"), DataType::Int));
        assert!(matches!(parse_data_type("bool"), DataType::Bool));
        assert!(matches!(parse_data_type("布尔"), DataType::Bool));
        assert!(matches!(parse_data_type("string"), DataType::String));
        assert!(matches!(parse_data_type("字符串"), DataType::String));
        assert!(matches!(parse_data_type("unknown"), DataType::Float)); // Default
    }
}