//! Simplified Configuration Manager
//! 
//! This is a streamlined version of ConfigManager that uses types from the types module
//! and delegates CSV loading to the unified loader.

use crate::utils::error::{ComSrvError, Result};
use super::types::{
    AppConfig, ChannelConfig, ServiceConfig, 
    CombinedPoint,
};
use super::unified_loader::UnifiedCsvLoader;
use figment::{
    providers::{Format, Yaml, Toml, Json, Env},
    Figment,
};
use std::path::Path;
use std::sync::Arc;
use tracing::{info, warn};

/// Simplified configuration manager
pub struct ConfigManager {
    /// The loaded application configuration
    config: AppConfig,
    /// Figment instance for reloading
    figment: Figment,
}

impl ConfigManager {
    /// Load configuration from file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        info!("Loading configuration from: {}", path.display());
        
        // Determine file format from extension
        let extension = path.extension()
            .and_then(|ext| ext.to_str())
            .ok_or_else(|| ComSrvError::ConfigError(
                "Configuration file must have an extension".to_string()
            ))?;
        
        // Build Figment with appropriate provider
        let mut figment = Figment::new();
        
        match extension {
            "yaml" | "yml" => {
                figment = figment.merge(Yaml::file(path));
            }
            "toml" => {
                figment = figment.merge(Toml::file(path));
            }
            "json" => {
                figment = figment.merge(Json::file(path));
            }
            _ => {
                return Err(ComSrvError::ConfigError(
                    format!("Unsupported configuration format: {}", extension)
                ));
            }
        }
        
        // Add environment variable support
        figment = figment.merge(
            Env::prefixed("COMSRV_")
                .split("_")
                .lowercase(false)
        );
        
        // Extract configuration
        let mut config: AppConfig = figment.extract()
            .map_err(|e| ComSrvError::ConfigError(
                format!("Failed to parse configuration: {}", e)
            ))?;
        
        // Load CSV tables for each channel if configured
        let config_dir = path.parent();
        if let Some(config_dir) = config_dir {
            for channel in &mut config.channels {
                if let Some(ref table_config) = channel.table_config {
                    match UnifiedCsvLoader::load_channel_tables(table_config, config_dir) {
                        Ok(points) => {
                            info!("Loaded {} points for channel {} ({})", 
                                  points.len(), channel.id, channel.name);
                            channel.combined_points = points;
                        }
                        Err(e) => {
                            warn!("Failed to load CSV tables for channel {}: {}", channel.id, e);
                        }
                    }
                }
            }
        }
        
        Ok(ConfigManager {
            config,
            figment,
        })
    }
    
    /// Reload configuration from the same source
    pub fn reload(&mut self) -> Result<()> {
        info!("Reloading configuration");
        
        let mut config: AppConfig = self.figment.extract()
            .map_err(|e| ComSrvError::ConfigError(
                format!("Failed to reload configuration: {}", e)
            ))?;
        
        // Preserve loaded CSV data during reload
        for (idx, channel) in config.channels.iter_mut().enumerate() {
            if idx < self.config.channels.len() {
                channel.combined_points = self.config.channels[idx].combined_points.clone();
            }
        }
        
        self.config = config;
        Ok(())
    }
    
    /// Get the loaded configuration
    pub fn config(&self) -> &AppConfig {
        &self.config
    }
    
    /// Get service configuration
    pub fn service(&self) -> &ServiceConfig {
        &self.config.service
    }
    
    /// Get all channels
    pub fn channels(&self) -> &[ChannelConfig] {
        &self.config.channels
    }
    
    /// Get a channel by ID
    pub fn get_channel(&self, channel_id: u16) -> Option<&ChannelConfig> {
        self.config.channels.iter()
            .find(|ch| ch.id == channel_id)
    }
    
    /// Get a channel by name
    pub fn get_channel_by_name(&self, name: &str) -> Option<&ChannelConfig> {
        self.config.channels.iter()
            .find(|ch| ch.name == name)
    }
    
    /// Get combined points for a channel
    pub fn get_channel_points(&self, channel_id: u16) -> Vec<&CombinedPoint> {
        self.get_channel(channel_id)
            .map(|ch| ch.combined_points.iter().collect())
            .unwrap_or_default()
    }
    
    /// Find a point by ID across all channels
    pub fn find_point(&self, point_id: u32) -> Option<(&ChannelConfig, &CombinedPoint)> {
        for channel in &self.config.channels {
            if let Some(point) = channel.combined_points.iter()
                .find(|p| p.point_id == point_id) {
                return Some((channel, point));
            }
        }
        None
    }
    
    /// Get all points of a specific telemetry type
    pub fn get_points_by_type(&self, telemetry_type: &str) -> Vec<(u16, &CombinedPoint)> {
        let mut points = Vec::new();
        for channel in &self.config.channels {
            for point in &channel.combined_points {
                if point.telemetry_type == telemetry_type {
                    points.push((channel.id, point));
                }
            }
        }
        points
    }
    
    /// Validate configuration
    pub fn validate(&self) -> Result<()> {
        // Validate service configuration
        if self.config.service.name.is_empty() {
            return Err(ComSrvError::ConfigError(
                "Service name cannot be empty".to_string()
            ));
        }
        
        // Validate channels
        if self.config.channels.is_empty() {
            warn!("No channels configured");
        }
        
        let mut channel_ids = std::collections::HashSet::new();
        for channel in &self.config.channels {
            if !channel_ids.insert(channel.id) {
                return Err(ComSrvError::ConfigError(
                    format!("Duplicate channel ID: {}", channel.id)
                ));
            }
            
            if channel.name.is_empty() {
                return Err(ComSrvError::ConfigError(
                    format!("Channel {} has empty name", channel.id)
                ));
            }
        }
        
        // Validate points
        let mut point_ids = std::collections::HashSet::new();
        for channel in &self.config.channels {
            for point in &channel.combined_points {
                if !point_ids.insert(point.point_id) {
                    warn!("Duplicate point ID {} found", point.point_id);
                }
            }
        }
        
        info!("Configuration validation passed");
        Ok(())
    }
    
    /// Convert to Arc for sharing across threads
    pub fn into_arc(self) -> Arc<Self> {
        Arc::new(self)
    }
}

// Backward compatibility helpers
impl ConfigManager {
    /// Get channels (compatibility method for old code)
    pub fn get_channels(&self) -> &Vec<ChannelConfig> {
        &self.config.channels
    }
    
    /// Get channel count
    pub fn channel_count(&self) -> usize {
        self.config.channels.len()
    }
    
    /// Get total point count
    pub fn point_count(&self) -> usize {
        self.config.channels.iter()
            .map(|ch| ch.combined_points.len())
            .sum()
    }
    
    /// Check if Redis is enabled
    pub fn is_redis_enabled(&self) -> bool {
        self.config.service.redis.enabled
    }
    
    /// Check if API is enabled
    pub fn is_api_enabled(&self) -> bool {
        self.config.service.api.enabled
    }
    
    /// Get Redis URL
    pub fn redis_url(&self) -> &str {
        &self.config.service.redis.url
    }
    
    /// Get API bind address
    pub fn api_bind_address(&self) -> &str {
        &self.config.service.api.bind_address
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_load_yaml_config() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join("test_config.yml");
        
        std::fs::write(&config_path, r#"
version: "2.0"
service:
  name: test-service
channels: []
"#).unwrap();
        
        let config_manager = ConfigManager::from_file(&config_path).unwrap();
        assert_eq!(config_manager.config().service.name, "test-service");
    }
    
    #[test]
    fn test_validation() {
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join("test_config.yml");
        
        std::fs::write(&config_path, r#"
version: "2.0"
service:
  name: ""
channels: []
"#).unwrap();
        
        let config_manager = ConfigManager::from_file(&config_path).unwrap();
        assert!(config_manager.validate().is_err());
    }
}