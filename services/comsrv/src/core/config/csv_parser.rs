use std::path::Path;
use std::collections::HashMap;
use csv::ReaderBuilder;
use serde::{Deserialize, Serialize};
use crate::utils::{ComSrvError, Result};
use crate::core::protocols::modbus::common::{ModbusRegisterMapping, ModbusDataType, ModbusRegisterType};

/// CSV point table record structure
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CsvPointRecord {
    pub id: String,
    pub name: String,
    pub address: u16,
    pub unit: Option<String>,
    pub scale: f64,
    pub offset: f64,
    pub data_type: String,
    pub register_type: Option<String>,
    pub description: Option<String>,
    pub access: Option<String>,  // read, write, read_write
    pub group: Option<String>,   
}

/// CSV point table manager
#[derive(Debug, Clone)]
pub struct CsvPointManager {
    point_tables: HashMap<String, Vec<CsvPointRecord>>,
}

impl CsvPointManager {
    /// Create a new CSV point table manager
    pub fn new() -> Self {
        Self {
            point_tables: HashMap::new(),
        }
    }

    /// Load a point table from a CSV file
    pub fn load_from_csv<P: AsRef<Path>>(&mut self, file_path: P, table_name: &str) -> Result<()> {
        let file_path = file_path.as_ref();
        
        if !file_path.exists() {
            return Err(ComSrvError::ConfigError(format!(
                "CSV file not found: {}", 
                file_path.display()
            )));
        }

        let mut reader = ReaderBuilder::new()
            .has_headers(true)
            .from_path(file_path)
            .map_err(|e| ComSrvError::ConfigError(format!(
                "Failed to open CSV file {}: {}", 
                file_path.display(), e
            )))?;

        let mut records = Vec::new();
        
        for result in reader.deserialize() {
            let record: CsvPointRecord = result.map_err(|e| ComSrvError::ConfigError(format!(
                "Failed to parse CSV record in {}: {}", 
                file_path.display(), e
            )))?;
            
            // Validate the record
            self.validate_record(&record)?;
            records.push(record);
        }

        tracing::info!("Loaded {} points from CSV file: {}", records.len(), file_path.display());
        self.point_tables.insert(table_name.to_string(), records);
        
        Ok(())
    }

    /// Load all CSV point tables from a directory
    pub fn load_from_directory<P: AsRef<Path>>(&mut self, dir_path: P) -> Result<()> {
        let dir_path = dir_path.as_ref();
        
        if !dir_path.exists() || !dir_path.is_dir() {
            return Err(ComSrvError::ConfigError(format!(
                "Point table directory not found: {}", 
                dir_path.display()
            )));
        }

        let entries = std::fs::read_dir(dir_path)
            .map_err(|e| ComSrvError::ConfigError(format!(
                "Failed to read directory {}: {}", 
                dir_path.display(), e
            )))?;

        for entry in entries {
            let entry = entry.map_err(|e| ComSrvError::ConfigError(format!(
                "Failed to read directory entry: {}", e
            )))?;

            let path = entry.path();
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("csv") {
                let table_name = path.file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
                    .to_string();
                
                if let Err(e) = self.load_from_csv(&path, &table_name) {
                    tracing::warn!("Failed to load CSV file {}: {}", path.display(), e);
                }
            }
        }

        Ok(())
    }

    /// Get all points from a specified table
    pub fn get_points(&self, table_name: &str) -> Option<&Vec<CsvPointRecord>> {
        self.point_tables.get(table_name)
    }

    /// Find a point by its ID
    pub fn find_point(&self, table_name: &str, point_id: &str) -> Option<&CsvPointRecord> {
        self.point_tables.get(table_name)?
            .iter()
            .find(|p| p.id == point_id)
    }

    /// Get all table names
    pub fn get_table_names(&self) -> Vec<String> {
        self.point_tables.keys().cloned().collect()
    }

    /// Get the statistics of a table
    pub fn get_table_stats(&self, table_name: &str) -> Option<PointTableStats> {
        let points = self.point_tables.get(table_name)?;
        
        let mut stats = PointTableStats {
            total_points: points.len(),
            read_points: 0,
            write_points: 0,
            read_write_points: 0,
            data_types: HashMap::new(),
            groups: HashMap::new(),
        };

        for point in points {
            // Count access types
            match point.access.as_deref() {
                Some("read") => stats.read_points += 1,
                Some("write") => stats.write_points += 1,
                Some("read_write") => stats.read_write_points += 1,
                _ => stats.read_points += 1, // Default to read only
            }

            // Count data types
            *stats.data_types.entry(point.data_type.clone()).or_insert(0) += 1;

            // Count groups
            if let Some(group) = &point.group {
                *stats.groups.entry(group.clone()).or_insert(0) += 1;
            }
        }

        Some(stats)
    }

    /// Convert a CSV point table to Modbus register mappings
    pub fn to_modbus_mappings(&self, table_name: &str) -> Result<Vec<ModbusRegisterMapping>> {
        let points = self.point_tables.get(table_name)
            .ok_or_else(|| ComSrvError::ConfigError(format!("Point table not found: {}", table_name)))?;

        let mut mappings = Vec::new();
        
        for point in points {
            let data_type = self.parse_data_type(&point.data_type)?;
            let register_type = self.parse_register_type(&point.register_type)?;
            
            let mapping = ModbusRegisterMapping {
                name: point.id.clone(),
                display_name: Some(point.name.clone()),
                register_type,
                address: point.address,
                data_type,
                scale: point.scale,
                offset: point.offset,
                unit: point.unit.clone(),
                description: point.description.clone(),
                access_mode: point.access.clone().unwrap_or_else(|| "read".to_string()),
                group: point.group.clone(),
                byte_order: crate::core::protocols::modbus::common::ByteOrder::BigEndian,
            };
            
            mappings.push(mapping);
        }

        Ok(mappings)
    }

    /// Validate a CSV record
    fn validate_record(&self, record: &CsvPointRecord) -> Result<()> {
        if record.id.is_empty() {
            return Err(ComSrvError::ConfigError("Point ID cannot be empty".to_string()));
        }

        if record.name.is_empty() {
            return Err(ComSrvError::ConfigError(format!("Point name cannot be empty for ID: {}", record.id)));
        }

        // validate data type
        self.parse_data_type(&record.data_type)?;

        Ok(())
    }

    /// Parse the data type
    fn parse_data_type(&self, data_type: &str) -> Result<ModbusDataType> {
        match data_type.to_lowercase().as_str() {
            "bool" | "boolean" => Ok(ModbusDataType::Bool),
            "int16" | "i16" => Ok(ModbusDataType::Int16),
            "uint16" | "u16" => Ok(ModbusDataType::UInt16),
            "int32" | "i32" => Ok(ModbusDataType::Int32),
            "uint32" | "u32" => Ok(ModbusDataType::UInt32),
            "float32" | "f32" | "float" => Ok(ModbusDataType::Float32),
            "string" | "str" => Ok(ModbusDataType::String(10)), // default length 10
            _ => Err(ComSrvError::ConfigError(format!("Unsupported data type: {}", data_type))),
        }
    }

    /// Parse the register type
    fn parse_register_type(&self, register_type: &Option<String>) -> Result<ModbusRegisterType> {
        match register_type.as_deref() {
            Some("coil") | Some("coils") => Ok(ModbusRegisterType::Coil),
            Some("discrete_input") | Some("discrete") => Ok(ModbusRegisterType::DiscreteInput),
            Some("input_register") | Some("input") => Ok(ModbusRegisterType::InputRegister),
            Some("holding_register") | Some("holding") => Ok(ModbusRegisterType::HoldingRegister),
            None => {
                // infer register type based on address range
                Ok(ModbusRegisterType::InputRegister) // default to input register
            },
            Some(other) => Err(ComSrvError::ConfigError(format!("Unsupported register type: {}", other))),
        }
    }

    /// Save point table to a CSV file
    pub fn save_to_csv<P: AsRef<Path>>(&self, table_name: &str, file_path: P) -> Result<()> {
        let points = self.point_tables.get(table_name)
            .ok_or_else(|| ComSrvError::ConfigError(format!("Point table not found: {}", table_name)))?;

        let file_path = file_path.as_ref();
        let mut writer = csv::Writer::from_path(file_path)
            .map_err(|e| ComSrvError::ConfigError(format!(
                "Failed to create CSV file {}: {}", 
                file_path.display(), e
            )))?;

        for point in points {
            writer.serialize(point)
                .map_err(|e| ComSrvError::ConfigError(format!(
                    "Failed to write CSV record: {}", e
                )))?;
        }

        writer.flush()
            .map_err(|e| ComSrvError::ConfigError(format!(
                "Failed to flush CSV file: {}", e
            )))?;

        tracing::info!("Saved {} points to CSV file: {}", points.len(), file_path.display());
        Ok(())
    }

    /// Add or update a point
    pub fn upsert_point(&mut self, table_name: &str, point: CsvPointRecord) -> Result<()> {
        self.validate_record(&point)?;
        
        let points = self.point_tables.entry(table_name.to_string()).or_insert_with(Vec::new);
        
        // check if a point with the same ID exists
        if let Some(existing) = points.iter_mut().find(|p| p.id == point.id) {
            *existing = point;
        } else {
            points.push(point);
        }
        
        Ok(())
    }

    /// Remove a point
    pub fn remove_point(&mut self, table_name: &str, point_id: &str) -> Result<bool> {
        let points = self.point_tables.get_mut(table_name)
            .ok_or_else(|| ComSrvError::ConfigError(format!("Point table not found: {}", table_name)))?;
        
        let initial_len = points.len();
        points.retain(|p| p.id != point_id);
        
        Ok(points.len() < initial_len)
    }
}

/// Point table statistics information
#[derive(Debug, Clone, Serialize)]
pub struct PointTableStats {
    pub total_points: usize,
    pub read_points: usize,
    pub write_points: usize,
    pub read_write_points: usize,
    pub data_types: HashMap<String, usize>,
    pub groups: HashMap<String, usize>,
}

impl Default for CsvPointManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs;

    #[test]
    fn test_csv_parsing() {
        let csv_content = r#"id,name,address,unit,scale,offset,data_type,register_type,description
temp_01,Temperature Sensor 1,1000,°C,0.1,0,float32,input_register,Environment temperature
press_01,Pressure Sensor 1,1001,bar,0.01,0,float32,input_register,System pressure
pump_status,Pump Status,1,,1,0,bool,coil,Water pump on/off status"#;

        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test_points.csv");
        fs::write(&file_path, csv_content).unwrap();

        let mut manager = CsvPointManager::new();
        manager.load_from_csv(&file_path, "test_table").unwrap();

        let points = manager.get_points("test_table").unwrap();
        assert_eq!(points.len(), 3);

        // test point lookup
        let temp_point = manager.find_point("test_table", "temp_01").unwrap();
        assert_eq!(temp_point.name, "Temperature Sensor 1");
        assert_eq!(temp_point.address, 1000);
        assert_eq!(temp_point.scale, 0.1);

        // test statistics
        let stats = manager.get_table_stats("test_table").unwrap();
        assert_eq!(stats.total_points, 3);
    }

    #[test]
    fn test_modbus_mapping_conversion() {
        let mut manager = CsvPointManager::new();
        
        let point = CsvPointRecord {
            id: "test_point".to_string(),
            name: "Test Point".to_string(),
            address: 1000,
            unit: Some("V".to_string()),
            scale: 0.1,
            offset: 0.0,
            data_type: "float32".to_string(),
            register_type: Some("input_register".to_string()),
            description: Some("Test description".to_string()),
            access: Some("read".to_string()),
            group: Some("sensors".to_string()),
        };
        
        manager.upsert_point("test_table", point).unwrap();
        
        let mappings = manager.to_modbus_mappings("test_table").unwrap();
        assert_eq!(mappings.len(), 1);
        
        let mapping = &mappings[0];
        assert_eq!(mapping.name, "test_point");
        assert_eq!(mapping.address, 1000);
        assert_eq!(mapping.scale, 0.1);
    }
} 