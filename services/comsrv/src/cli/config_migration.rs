//! Configuration Migration Tool
//! 
//! This tool helps migrate old configuration formats to the new plugin-based format.

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use tracing::{info, warn};

/// Old configuration format (before plugin system)
#[derive(Debug, Deserialize)]
struct OldChannelConfig {
    pub id: u16,
    pub name: String,
    pub description: Option<String>,
    pub protocol: String,
    pub parameters: HashMap<String, serde_yaml::Value>,
    #[serde(default)]
    pub logging: OldLoggingConfig,
}

#[derive(Debug, Deserialize, Default)]
struct OldLoggingConfig {
    pub enabled: bool,
    pub level: Option<String>,
    pub log_dir: Option<String>,
}

/// Configuration migration tool
pub struct ConfigMigration {
    dry_run: bool,
}

impl ConfigMigration {
    pub fn new(dry_run: bool) -> Self {
        Self { dry_run }
    }
    
    /// Migrate a configuration file
    pub async fn migrate_file(&self, input_path: &Path, output_path: &Path) -> Result<()> {
        info!("Migrating configuration from {:?} to {output_path:?}", input_path);
        
        // Read old configuration
        let content = std::fs::read_to_string(input_path)?;
        let old_config: serde_yaml::Value = serde_yaml::from_str(&content)?;
        
        // Migrate the configuration
        let new_config = self.migrate_config(old_config)?;
        
        // Write new configuration
        if self.dry_run {
            info!("Dry run mode - would write configuration to {output_path:?}");
            println!("Migrated configuration:");
            println!("{}", serde_yaml::to_string(&new_config)?);
        } else {
            let yaml_str = serde_yaml::to_string(&new_config)?;
            std::fs::write(output_path, yaml_str)?;
            info!("Configuration migrated successfully");
        }
        
        Ok(())
    }
    
    /// Migrate configuration structure
    fn migrate_config(&self, mut config: serde_yaml::Value) -> Result<serde_yaml::Value> {
        // Migrate channels if present
        if let Some(channels) = config.get_mut("channels") {
            if let Some(channels_array) = channels.as_sequence_mut() {
                for channel in channels_array {
                    self.migrate_channel(channel)?;
                }
            }
        }
        
        Ok(config)
    }
    
    /// Migrate a single channel configuration
    fn migrate_channel(&self, channel: &mut serde_yaml::Value) -> Result<()> {
        let channel_map = channel.as_mapping_mut()
            .ok_or_else(|| anyhow!("Channel must be a mapping"))?;
        
        // Get protocol type
        let protocol = channel_map.get(&serde_yaml::Value::String("protocol".to_string()))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Channel missing protocol field"))?;
        
        // Migrate based on protocol type
        match protocol {
            "ModbusTcp" | "modbus_tcp" => self.migrate_modbus_tcp(channel_map)?,
            "ModbusRtu" | "modbus_rtu" => self.migrate_modbus_rtu(channel_map)?,
            "IEC104" | "iec104" | "iec60870-5-104" => self.migrate_iec104(channel_map)?,
            "CAN" | "can" => self.migrate_can(channel_map)?,
            "Virtual" | "virtual" => self.migrate_virtual(channel_map)?,
            _ => warn!("Unknown protocol type: {protocol}"),
        }
        
        // Normalize protocol name
        if let Some(protocol_value) = channel_map.get_mut(&serde_yaml::Value::String("protocol".to_string())) {
            match protocol {
                "ModbusTcp" => *protocol_value = serde_yaml::Value::String("modbus_tcp".to_string()),
                "ModbusRtu" => *protocol_value = serde_yaml::Value::String("modbus_rtu".to_string()),
                "IEC104" => *protocol_value = serde_yaml::Value::String("iec104".to_string()),
                "CAN" => *protocol_value = serde_yaml::Value::String("can".to_string()),
                "Virtual" => *protocol_value = serde_yaml::Value::String("virtual".to_string()),
                _ => {}
            }
        }
        
        // Migrate logging configuration
        if let Some(logging) = channel_map.get_mut(&serde_yaml::Value::String("logging".to_string())) {
            self.migrate_logging(logging)?;
        }
        
        Ok(())
    }
    
    /// Migrate Modbus TCP configuration
    fn migrate_modbus_tcp(&self, channel: &mut serde_yaml::Mapping) -> Result<()> {
        // Create protocol_params if not exists
        let protocol_params_key = serde_yaml::Value::String("protocol_params".to_string());
        if !channel.contains_key(&protocol_params_key) {
            channel.insert(protocol_params_key.clone(), serde_yaml::Value::Mapping(serde_yaml::Mapping::new()));
        }
        
        let protocol_params = channel.get_mut(&protocol_params_key)
            .and_then(|v| v.as_mapping_mut())
            .ok_or_else(|| anyhow!("protocol_params must be a mapping"))?;
        
        // Migrate parameters
        let params_key = serde_yaml::Value::String("parameters".to_string());
        if let Some(params) = channel.get(&params_key) {
            if let Some(params_map) = params.as_mapping() {
                // Copy relevant parameters
                for (key, value) in params_map {
                    if let Some(key_str) = key.as_str() {
                        match key_str {
                            "address" => {
                                // Split address into host:port
                                if let Some(addr_str) = value.as_str() {
                                    if let Some((host, port)) = addr_str.split_once(':') {
                                        protocol_params.insert(
                                            serde_yaml::Value::String("host".to_string()),
                                            serde_yaml::Value::String(host.to_string())
                                        );
                                        if let Ok(port_num) = port.parse::<u16>() {
                                            protocol_params.insert(
                                                serde_yaml::Value::String("port".to_string()),
                                                serde_yaml::Value::Number(port_num.into())
                                            );
                                        }
                                    }
                                }
                            }
                            "host" | "port" | "slave_id" | "timeout" | "max_retries" | "poll_rate" => {
                                protocol_params.insert(key.clone(), value.clone());
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        
        // Remove old parameters key
        channel.remove(&params_key);
        
        Ok(())
    }
    
    /// Migrate Modbus RTU configuration
    fn migrate_modbus_rtu(&self, channel: &mut serde_yaml::Mapping) -> Result<()> {
        // Similar to Modbus TCP but with serial parameters
        let protocol_params_key = serde_yaml::Value::String("protocol_params".to_string());
        if !channel.contains_key(&protocol_params_key) {
            channel.insert(protocol_params_key.clone(), serde_yaml::Value::Mapping(serde_yaml::Mapping::new()));
        }
        
        let protocol_params = channel.get_mut(&protocol_params_key)
            .and_then(|v| v.as_mapping_mut())
            .ok_or_else(|| anyhow!("protocol_params must be a mapping"))?;
        
        // Migrate parameters
        let params_key = serde_yaml::Value::String("parameters".to_string());
        if let Some(params) = channel.get(&params_key) {
            if let Some(params_map) = params.as_mapping() {
                for (key, value) in params_map {
                    if let Some(key_str) = key.as_str() {
                        match key_str {
                            "port" | "baud_rate" | "data_bits" | "stop_bits" | "parity" | "slave_id" => {
                                protocol_params.insert(key.clone(), value.clone());
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        
        channel.remove(&params_key);
        Ok(())
    }
    
    /// Migrate IEC104 configuration
    fn migrate_iec104(&self, channel: &mut serde_yaml::Mapping) -> Result<()> {
        let protocol_params_key = serde_yaml::Value::String("protocol_params".to_string());
        if !channel.contains_key(&protocol_params_key) {
            channel.insert(protocol_params_key.clone(), serde_yaml::Value::Mapping(serde_yaml::Mapping::new()));
        }
        
        let protocol_params = channel.get_mut(&protocol_params_key)
            .and_then(|v| v.as_mapping_mut())
            .ok_or_else(|| anyhow!("protocol_params must be a mapping"))?;
        
        // Migrate parameters
        let params_key = serde_yaml::Value::String("parameters".to_string());
        if let Some(params) = channel.get(&params_key) {
            if let Some(params_map) = params.as_mapping() {
                for (key, value) in params_map {
                    protocol_params.insert(key.clone(), value.clone());
                }
            }
        }
        
        channel.remove(&params_key);
        Ok(())
    }
    
    /// Migrate CAN configuration
    fn migrate_can(&self, channel: &mut serde_yaml::Mapping) -> Result<()> {
        let protocol_params_key = serde_yaml::Value::String("protocol_params".to_string());
        if !channel.contains_key(&protocol_params_key) {
            channel.insert(protocol_params_key.clone(), serde_yaml::Value::Mapping(serde_yaml::Mapping::new()));
        }
        
        let protocol_params = channel.get_mut(&protocol_params_key)
            .and_then(|v| v.as_mapping_mut())
            .ok_or_else(|| anyhow!("protocol_params must be a mapping"))?;
        
        // Migrate parameters
        let params_key = serde_yaml::Value::String("parameters".to_string());
        if let Some(params) = channel.get(&params_key) {
            if let Some(params_map) = params.as_mapping() {
                for (key, value) in params_map {
                    protocol_params.insert(key.clone(), value.clone());
                }
            }
        }
        
        channel.remove(&params_key);
        Ok(())
    }
    
    /// Migrate Virtual protocol configuration
    fn migrate_virtual(&self, channel: &mut serde_yaml::Mapping) -> Result<()> {
        // Virtual protocol has minimal configuration
        let protocol_params_key = serde_yaml::Value::String("protocol_params".to_string());
        if !channel.contains_key(&protocol_params_key) {
            channel.insert(protocol_params_key.clone(), serde_yaml::Value::Mapping(serde_yaml::Mapping::new()));
        }
        
        // Remove old parameters if any
        let params_key = serde_yaml::Value::String("parameters".to_string());
        channel.remove(&params_key);
        
        Ok(())
    }
    
    /// Migrate logging configuration
    fn migrate_logging(&self, logging: &mut serde_yaml::Value) -> Result<()> {
        if let Some(logging_map) = logging.as_mapping_mut() {
            // Rename log_dir to log_directory if present
            let log_dir_key = serde_yaml::Value::String("log_dir".to_string());
            let log_directory_key = serde_yaml::Value::String("log_directory".to_string());
            
            if let Some(log_dir) = logging_map.remove(&log_dir_key) {
                logging_map.insert(log_directory_key, log_dir);
            }
        }
        
        Ok(())
    }
}

/// CLI command handler for config migration
pub async fn handle_migrate_command(input: &str, output: &str, dry_run: bool) -> Result<()> {
    let migration = ConfigMigration::new(dry_run);
    let input_path = Path::new(input);
    let output_path = Path::new(output);
    
    if !input_path.exists() {
        return Err(anyhow!("Input file does not exist: {input_path:?}"));
    }
    
    migration.migrate_file(input_path, output_path).await?;
    
    Ok(())
}