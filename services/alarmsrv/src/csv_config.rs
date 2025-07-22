//! CSV-based alarm configuration support
//!
//! This module provides support for simple CSV-based alarm configuration,
//! similar to the approach used by comsrv and modsrv for point configuration.

use anyhow::{anyhow, Result};
use glob::Pattern;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::path::Path;
use tracing::{info, warn};

/// CSV alarm configuration entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsvAlarmConfig {
    /// Unique alarm ID (number)
    pub alarm_id: u32,
    /// Module pattern for matching (supports wildcards)
    pub module_pattern: String,
    /// Point name to monitor
    pub point_name: String,
    /// Comparison operator (>, <, =, >=, <=, !=)
    pub operator: String,
    /// Threshold value
    pub threshold: f64,
    /// Alarm level (warning, critical, major, minor)
    pub level: String,
    /// Alarm name/description
    pub name: String,
    /// Enabled status (1=enabled, 0=disabled)
    #[serde(default = "default_enabled")]
    pub enabled: u8,
}

/// Default enabled value (1) for backward compatibility
fn default_enabled() -> u8 {
    1
}

impl CsvAlarmConfig {
    /// Check if module matches the pattern
    pub fn matches_module(&self, module_id: &str) -> bool {
        match Pattern::new(&self.module_pattern) {
            Ok(pattern) => pattern.matches(module_id),
            Err(_) => {
                // Fallback to simple string match if pattern is invalid
                self.module_pattern == "*" || self.module_pattern == module_id
            }
        }
    }

    /// Check if value satisfies the alarm condition
    pub fn check_condition(&self, value: f64) -> bool {
        match self.operator.as_str() {
            ">" => value > self.threshold,
            "<" => value < self.threshold,
            ">=" => value >= self.threshold,
            "<=" => value <= self.threshold,
            "=" => (value - self.threshold).abs() < f64::EPSILON,
            "!=" => (value - self.threshold).abs() >= f64::EPSILON,
            _ => {
                warn!("Unknown operator: {}", self.operator);
                false
            }
        }
    }

    /// Convert level string to AlarmLevel enum
    pub fn get_alarm_level(&self) -> crate::domain::AlarmLevel {
        match self.level.to_lowercase().as_str() {
            "critical" => crate::domain::AlarmLevel::Critical,
            "major" => crate::domain::AlarmLevel::Major,
            "warning" => crate::domain::AlarmLevel::Warning,
            "minor" => crate::domain::AlarmLevel::Minor,
            _ => {
                warn!("Unknown alarm level: {}, defaulting to Warning", self.level);
                crate::domain::AlarmLevel::Warning
            }
        }
    }
}

/// CSV alarm configuration loader
pub struct CsvAlarmConfigLoader {
    /// Path to the CSV configuration file
    config_path: String,
}

impl CsvAlarmConfigLoader {
    /// Create new loader with configuration path
    pub fn new(config_path: String) -> Self {
        Self { config_path }
    }

    /// Load alarm configurations from CSV file
    pub fn load(&self) -> Result<Vec<CsvAlarmConfig>> {
        let path = Path::new(&self.config_path);
        
        if !path.exists() {
            warn!("CSV alarm configuration file not found: {}", self.config_path);
            return Ok(Vec::new());
        }

        let file = File::open(path)
            .map_err(|e| anyhow!("Failed to open CSV file {}: {}", self.config_path, e))?;

        let mut reader = csv::Reader::from_reader(file);
        let mut configs = Vec::new();

        for (line_num, result) in reader.deserialize().enumerate() {
            match result {
                Ok(config) => {
                    let alarm_config: CsvAlarmConfig = config;
                    
                    // Validate configuration
                    if let Err(e) = self.validate_config(&alarm_config) {
                        warn!("Invalid alarm config at line {}: {}", line_num + 2, e);
                        continue;
                    }
                    
                    configs.push(alarm_config);
                }
                Err(e) => {
                    warn!("Failed to parse CSV line {}: {}", line_num + 2, e);
                }
            }
        }

        info!("Loaded {} CSV alarm configurations from {}", configs.len(), self.config_path);
        Ok(configs)
    }

    /// Validate alarm configuration
    fn validate_config(&self, config: &CsvAlarmConfig) -> Result<()> {
        // Validate operator
        let valid_operators = ["=", ">", "<", ">=", "<=", "!="];
        if !valid_operators.contains(&config.operator.as_str()) {
            return Err(anyhow!("Invalid operator: {}", config.operator));
        }

        // Validate level
        let valid_levels = ["critical", "major", "warning", "minor"];
        if !valid_levels.contains(&config.level.to_lowercase().as_str()) {
            return Err(anyhow!("Invalid alarm level: {}", config.level));
        }

        // Validate module pattern
        if let Err(e) = Pattern::new(&config.module_pattern) {
            return Err(anyhow!("Invalid module pattern {}: {}", config.module_pattern, e));
        }

        // Validate enabled field (must be 0 or 1)
        if config.enabled != 0 && config.enabled != 1 {
            return Err(anyhow!("Invalid enabled value: {} (must be 0 or 1)", config.enabled));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_device_matching() {
        let config = CsvAlarmConfig {
            alarm_id: 1001,
            device_pattern: "*".to_string(),
            point_name: "temperature".to_string(),
            operator: ">".to_string(),
            threshold: 80.0,
            level: "warning".to_string(),
            name: "Temperature warning".to_string(),
        };

        assert!(config.matches_device("device_001"));
        assert!(config.matches_device("any_device"));

        let config_specific = CsvAlarmConfig {
            device_pattern: "device_*".to_string(),
            ..config
        };

        assert!(config_specific.matches_device("device_001"));
        assert!(!config_specific.matches_device("sensor_001"));
    }

    #[test]
    fn test_condition_checking() {
        let config = CsvAlarmConfig {
            alarm_id: 1001,
            device_pattern: "*".to_string(),
            point_name: "temperature".to_string(),
            operator: ">".to_string(),
            threshold: 80.0,
            level: "warning".to_string(),
            name: "Temperature warning".to_string(),
        };

        assert!(config.check_condition(85.0));
        assert!(!config.check_condition(75.0));
        assert!(!config.check_condition(80.0));
    }

    #[test]
    fn test_csv_loading() {
        let csv_content = r#"alarm_id,device_pattern,point_name,operator,threshold,level,name
1001,*,temperature,>,80.0,warning,Temperature Warning
1002,device_*,pressure,<,10.0,critical,Low Pressure
"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(csv_content.as_bytes()).unwrap();
        
        let loader = CsvAlarmConfigLoader::new(temp_file.path().to_string_lossy().to_string());
        let configs = loader.load().unwrap();

        assert_eq!(configs.len(), 2);
        assert_eq!(configs[0].alarm_id, 1001);
        assert_eq!(configs[0].point_name, "temperature");
        assert_eq!(configs[1].alarm_id, 1002);
        assert_eq!(configs[1].operator, "<");
    }
}