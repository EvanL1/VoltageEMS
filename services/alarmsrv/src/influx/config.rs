//! InfluxDB configuration for alarm historical storage

use serde::{Deserialize, Serialize};

/// InfluxDB 3.2 configuration for alarm storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfluxDBConfig {
    /// Enable InfluxDB historical storage
    pub enabled: bool,
    /// InfluxDB server URL
    pub url: String,
    /// Database/bucket name
    pub database: String,
    /// Access token (optional for InfluxDB 3.2)
    pub token: Option<String>,
    /// Organization name (optional)
    pub organization: Option<String>,
    /// Batch size for bulk writes
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,
    /// Flush interval in seconds
    #[serde(default = "default_flush_interval")]
    pub flush_interval_seconds: u64,
}

fn default_batch_size() -> usize {
    500
}

fn default_flush_interval() -> u64 {
    30
}

impl Default for InfluxDBConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            url: "http://localhost:8086".to_string(),
            database: "alarmsrv_history".to_string(),
            token: None,
            organization: None,
            batch_size: 500,
            flush_interval_seconds: 30,
        }
    }
}

impl InfluxDBConfig {
    /// Validate configuration
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.enabled {
            if self.url.is_empty() {
                return Err(anyhow::anyhow!("InfluxDB URL cannot be empty"));
            }
            if self.database.is_empty() {
                return Err(anyhow::anyhow!("InfluxDB database name cannot be empty"));
            }
            if self.batch_size == 0 {
                return Err(anyhow::anyhow!("Batch size must be greater than 0"));
            }
            if self.flush_interval_seconds == 0 {
                return Err(anyhow::anyhow!("Flush interval must be greater than 0"));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = InfluxDBConfig::default();
        assert!(config.enabled);
        assert_eq!(config.url, "http://localhost:8086");
        assert_eq!(config.database, "alarmsrv_history");
        assert_eq!(config.batch_size, 500);
        assert_eq!(config.flush_interval_seconds, 30);
    }

    #[test]
    fn test_config_validation() {
        let mut config = InfluxDBConfig::default();
        assert!(config.validate().is_ok());

        config.url = String::new();
        assert!(config.validate().is_err());

        config.url = "http://localhost:8086".to_string();
        config.database = String::new();
        assert!(config.validate().is_err());

        config.database = "test_db".to_string();
        config.batch_size = 0;
        assert!(config.validate().is_err());

        config.batch_size = 100;
        config.flush_interval_seconds = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_disabled_config_validation() {
        let mut config = InfluxDBConfig::default();
        config.enabled = false;
        config.url = String::new(); // Invalid URL
        config.database = String::new(); // Invalid database
        
        // Should still validate when disabled
        assert!(config.validate().is_ok());
    }
}