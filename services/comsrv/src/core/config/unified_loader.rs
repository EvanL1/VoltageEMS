//! Unified configuration and CSV loader
//! 
//! This module provides a unified approach to loading configuration and CSV files,
//! consolidating the functionality from multiple scattered implementations.

use crate::utils::error::{ComSrvError, Result};
use super::types::{
    CombinedPoint, TableConfig,
};
use csv::ReaderBuilder;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use tracing::{info, warn, debug};

/// Unified CSV loader that supports multiple formats
pub struct UnifiedCsvLoader;

/// Four telemetry point from CSV
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FourTelemetryPoint {
    pub point_id: u32,
    pub signal_name: String,
    pub chinese_name: String,
    pub telemetry_type: String,
    pub scale: Option<f64>,
    pub offset: Option<f64>,
    pub unit: Option<String>,
    pub reverse: Option<bool>,
    pub data_type: String,
}

/// Protocol mapping from CSV
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

impl UnifiedCsvLoader {
    /// Load all CSV tables for a channel
    pub fn load_channel_tables(
        table_config: &TableConfig,
        config_dir: &Path,
    ) -> Result<Vec<CombinedPoint>> {
        info!("Loading CSV tables for channel");
        
        // Load four telemetry points
        let mut all_telemetry = HashMap::new();
        
        // Check for environment variable override
        let base_dir = std::env::var("COMSRV_CSV_BASE_PATH")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| config_dir.to_path_buf());
        
        debug!("Using CSV base directory: {}", base_dir.display());
        
        // Load telemetry files
        let base_path = base_dir.join(&table_config.four_telemetry_route);
        
        // YC - Telemetry
        if let Some(yc_points) = Self::load_telemetry_file(
            &base_path.join(&table_config.four_telemetry_files.telemetry_file),
            "YC"
        )? {
            for point in yc_points {
                all_telemetry.insert(point.point_id, point);
            }
        }
        
        // YX - Signal
        if let Some(yx_points) = Self::load_signal_file(
            &base_path.join(&table_config.four_telemetry_files.signal_file),
            "YX"
        )? {
            for point in yx_points {
                all_telemetry.insert(point.point_id, point);
            }
        }
        
        // YT - Adjustment
        if let Some(yt_points) = Self::load_telemetry_file(
            &base_path.join(&table_config.four_telemetry_files.adjustment_file),
            "YT"
        )? {
            for point in yt_points {
                all_telemetry.insert(point.point_id, point);
            }
        }
        
        // YK - Control
        if let Some(yk_points) = Self::load_signal_file(
            &base_path.join(&table_config.four_telemetry_files.control_file),
            "YK"
        )? {
            for point in yk_points {
                all_telemetry.insert(point.point_id, point);
            }
        }
        
        // Load protocol mappings (using same base_dir)
        let mapping_base = base_dir.join(&table_config.protocol_mapping_route);
        
        // Load all mapping types
        let mut all_mappings = HashMap::new();
        
        // Load telemetry mappings
        let telemetry_mappings = Self::load_modbus_mappings(
            &mapping_base.join(&table_config.protocol_mapping_files.telemetry_mapping)
        )?;
        all_mappings.extend(telemetry_mappings);
        
        // Load signal mappings
        let signal_mappings = Self::load_modbus_mappings(
            &mapping_base.join(&table_config.protocol_mapping_files.signal_mapping)
        )?;
        all_mappings.extend(signal_mappings);
        
        // Load adjustment mappings  
        let adjustment_mappings = Self::load_modbus_mappings(
            &mapping_base.join(&table_config.protocol_mapping_files.adjustment_mapping)
        )?;
        all_mappings.extend(adjustment_mappings);
        
        // Load control mappings
        let control_mappings = Self::load_modbus_mappings(
            &mapping_base.join(&table_config.protocol_mapping_files.control_mapping)
        )?;
        all_mappings.extend(control_mappings);
        
        // Combine telemetry with mappings
        let mut combined_points = Vec::new();
        for (point_id, telemetry) in all_telemetry {
            if let Some(mapping) = all_mappings.get(&point_id) {
                let combined = Self::combine_point(telemetry, mapping);
                combined_points.push(combined);
            } else {
                warn!("No protocol mapping found for point {}", point_id);
            }
        }
        
        info!("Successfully loaded {} combined points", combined_points.len());
        Ok(combined_points)
    }
    
    /// Load telemetry CSV (YC/YT format)
    fn load_telemetry_file(path: &Path, telemetry_type: &str) -> Result<Option<Vec<FourTelemetryPoint>>> {
        if !path.exists() {
            debug!("Telemetry file not found: {}", path.display());
            return Ok(None);
        }
        
        let mut reader = ReaderBuilder::new()
            .has_headers(true)
            .from_path(path)
            .map_err(|e| ComSrvError::ConfigError(
                format!("Failed to open CSV {}: {}", path.display(), e)
            ))?;
        
        let mut points = Vec::new();
        
        // Check header count to determine format
        let headers = reader.headers()
            .map_err(|e| ComSrvError::ConfigError(format!("Failed to read headers: {}", e)))?;
        let header_count = headers.len();
        
        for (idx, result) in reader.records().enumerate() {
            let record = result.map_err(|e| ComSrvError::ConfigError(
                format!("CSV error at row {}: {}", idx + 2, e)
            ))?;
            
            let point = if header_count >= 9 {
                // New format: point_id,signal_name,chinese_name,data_type,scale,offset,unit,description,group
                FourTelemetryPoint {
                    point_id: record.get(0).unwrap_or("0").parse().unwrap_or(0),
                    signal_name: record.get(1).unwrap_or("").to_string(),
                    chinese_name: record.get(2).unwrap_or("").to_string(),
                    telemetry_type: telemetry_type.to_string(),
                    data_type: record.get(3).unwrap_or("float32").to_string(),
                    scale: record.get(4).and_then(|s| s.parse().ok()),
                    offset: record.get(5).and_then(|s| s.parse().ok()),
                    unit: record.get(6).and_then(|s| if s.is_empty() { None } else { Some(s.to_string()) }),
                    reverse: None,
                }
            } else if header_count >= 6 {
                // Old format: point_id,signal_name,chinese_name,scale,offset,unit
                FourTelemetryPoint {
                    point_id: record.get(0).unwrap_or("0").parse().unwrap_or(0),
                    signal_name: record.get(1).unwrap_or("").to_string(),
                    chinese_name: record.get(2).unwrap_or("").to_string(),
                    telemetry_type: telemetry_type.to_string(),
                    data_type: "FLOAT".to_string(),
                    scale: record.get(3).and_then(|s| s.parse().ok()),
                    offset: record.get(4).and_then(|s| s.parse().ok()),
                    unit: record.get(5).and_then(|s| if s.is_empty() { None } else { Some(s.to_string()) }),
                    reverse: None,
                }
            } else {
                warn!("Unknown CSV format at row {}", idx + 2);
                continue;
            };
            
            points.push(point);
        }
        
        info!("Loaded {} {} points from {}", points.len(), telemetry_type, path.display());
        Ok(Some(points))
    }
    
    /// Load signal CSV (YX/YK format)
    fn load_signal_file(path: &Path, telemetry_type: &str) -> Result<Option<Vec<FourTelemetryPoint>>> {
        if !path.exists() {
            debug!("Signal file not found: {}", path.display());
            return Ok(None);
        }
        
        let mut reader = ReaderBuilder::new()
            .has_headers(true)
            .from_path(path)
            .map_err(|e| ComSrvError::ConfigError(
                format!("Failed to open CSV {}: {}", path.display(), e)
            ))?;
        
        let mut points = Vec::new();
        
        for (idx, result) in reader.records().enumerate() {
            let record = result.map_err(|e| ComSrvError::ConfigError(
                format!("CSV error at row {}: {}", idx + 2, e)
            ))?;
            
            // Signal format: point_id,signal_name,chinese_name,data_type,reverse
            let point = FourTelemetryPoint {
                point_id: record.get(0).unwrap_or("0").parse().unwrap_or(0),
                signal_name: record.get(1).unwrap_or("").to_string(),
                chinese_name: record.get(2).unwrap_or("").to_string(),
                telemetry_type: telemetry_type.to_string(),
                data_type: record.get(3).unwrap_or("bool").to_string(),
                scale: None,
                offset: None,
                unit: None,
                reverse: record.get(4).and_then(|s| s.parse::<u8>().ok()).map(|v| v != 0),
            };
            
            points.push(point);
        }
        
        info!("Loaded {} {} points from {}", points.len(), telemetry_type, path.display());
        Ok(Some(points))
    }
    
    /// Load Modbus protocol mappings
    fn load_modbus_mappings(path: &Path) -> Result<HashMap<u32, ModbusMapping>> {
        if !path.exists() {
            warn!("Protocol mapping file not found: {}", path.display());
            return Ok(HashMap::new());
        }
        
        let mut reader = ReaderBuilder::new()
            .has_headers(true)
            .from_path(path)
            .map_err(|e| ComSrvError::ConfigError(
                format!("Failed to open CSV {}: {}", path.display(), e)
            ))?;
        
        let mut mappings = HashMap::new();
        
        for (idx, result) in reader.records().enumerate() {
            let record = result.map_err(|e| ComSrvError::ConfigError(
                format!("CSV error at row {}: {}", idx + 2, e)
            ))?;
            
            // New format: point_id,signal_name,slave_id,function_code,register_address,data_format,bit_position,byte_order,register_count
            let bit_position = record.get(6)
                .and_then(|s| if s.is_empty() { None } else { s.parse::<u8>().ok() });
            
            let data_format = record.get(5).map(|s| s.to_string());
            
            let mapping = ModbusMapping {
                point_id: record.get(0).unwrap_or("0").parse().unwrap_or(0),
                slave_id: record.get(2).unwrap_or("1").parse().unwrap_or(1),
                function_code: record.get(3).unwrap_or("3").parse().unwrap_or(3),
                register_address: record.get(4).unwrap_or("0").parse().unwrap_or(0),
                bit_position,
                data_format,
                register_count: record.get(8).and_then(|s| s.parse().ok()),
            };
            
            mappings.insert(mapping.point_id, mapping);
        }
        
        info!("Loaded {} protocol mappings", mappings.len());
        Ok(mappings)
    }
    
    /// Combine telemetry point with Modbus mapping
    fn combine_point(telemetry: FourTelemetryPoint, mapping: &ModbusMapping) -> CombinedPoint {
        let mut protocol_params = HashMap::new();
        
        // Add basic protocol parameters
        protocol_params.insert("slave_id".to_string(), mapping.slave_id.to_string());
        protocol_params.insert("function_code".to_string(), mapping.function_code.to_string());
        protocol_params.insert("register_address".to_string(), mapping.register_address.to_string());
        
        // Add formatted address for compatibility
        protocol_params.insert("address".to_string(), 
            format!("{}:{}:{}", mapping.slave_id, mapping.function_code, mapping.register_address));
        
        if let Some(bit_pos) = mapping.bit_position {
            protocol_params.insert("bit_position".to_string(), bit_pos.to_string());
        }
        
        if let Some(ref data_fmt) = mapping.data_format {
            protocol_params.insert("data_format".to_string(), data_fmt.clone());
        }
        
        if let Some(reg_count) = mapping.register_count {
            protocol_params.insert("register_count".to_string(), reg_count.to_string());
        }
        
        // Add reverse flag for signal/control types
        if matches!(telemetry.telemetry_type.as_str(), "YX" | "YK") {
            if let Some(reverse) = telemetry.reverse {
                protocol_params.insert("reverse".to_string(), reverse.to_string());
            }
        }
        
        CombinedPoint {
            point_id: telemetry.point_id,
            signal_name: telemetry.signal_name,
            chinese_name: telemetry.chinese_name,
            telemetry_type: telemetry.telemetry_type.clone(),
            data_type: telemetry.data_type,
            protocol_params,
            scaling: if matches!(telemetry.telemetry_type.as_str(), "YC" | "YT") && 
                       (telemetry.scale.is_some() || telemetry.offset.is_some()) {
                Some(super::types::ScalingInfo {
                    scale: telemetry.scale.unwrap_or(1.0),
                    offset: telemetry.offset.unwrap_or(0.0),
                    unit: telemetry.unit,
                })
            } else {
                None
            },
        }
    }
}