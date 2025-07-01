
use std::collections::HashMap;
use anyhow::anyhow;
use serde_json::Value;

use crate::core::config::app::AppConfig;
use crate::core::config::point::Point;
use crate::core::config::ConfigManager;
use std::sync::Arc;
use crate::core::protocols::common::combase::adapter::Adapter;
use crate::core::protocols::modbus::adapter::ModbusAdapter;

/// Manages all protocol adapters and provides a unified interface for interacting with them.
pub struct AdapterManager {
    adapters: HashMap<&'static str, Box<dyn Adapter>>,
    points: HashMap<String, Point>,
}

impl AdapterManager {
    /// Creates a new `AdapterManager` and initializes all protocol adapters based on the provided configuration.
    pub async fn new(conf: AppConfig) -> anyhow::Result<Self> {
        let mut adapters: HashMap<&'static str, Box<dyn Adapter>> = HashMap::new();

        if let Some(cfg) = conf.modbus {
            let mut adapter = ModbusAdapter::new(cfg);
            adapter.init().await?;
            adapters.insert("modbus", Box::new(adapter));
        }
        // Initialize other protocol adapters here...

        Ok(Self {
            adapters,
            points: conf.point_map,
        })
    }

    /// Creates a new `AdapterManager` from a ConfigManager instance
    pub async fn from_config_manager(config_manager: Arc<ConfigManager>) -> anyhow::Result<Self> {
        let mut adapters: HashMap<&'static str, Box<dyn Adapter>> = HashMap::new();
        let mut points: HashMap<String, Point> = HashMap::new();

        // Get the configuration
        let config = config_manager.config();
        
        // Initialize adapters based on configured channels
        for channel in &config.channels {
            match channel.protocol.as_str() {
                "modbus" => {
                    // For now, create a simple placeholder for Modbus
                    // This should be replaced with proper channel-based adapter creation
                    // let adapter = ModbusAdapter::from_channel_config(channel)?;
                    // adapters.insert("modbus", Box::new(adapter));
                    tracing::info!("Modbus channel configured: {}", channel.name);
                }
                _ => {
                    tracing::warn!("Unsupported protocol: {}", channel.protocol);
                }
            }
            
            // Load points from the channel configuration
            // This is where CSV point mappings would be loaded
            // For now, we'll just log the configuration
            tracing::info!("Channel {} has {} combined points", 
                channel.name, channel.combined_points.len());
        }

        Ok(Self {
            adapters,
            points,
        })
    }

    /// Reads a value from a point, automatically routing the request to the correct protocol adapter.
    pub async fn read(&self, point_key: &str) -> anyhow::Result<Value> {
        let p = self.points.get(point_key).ok_or_else(|| anyhow!("Point not found: {}", point_key))?;
        let protocol = protocol_of(point_key);
        let adapter = self.adapters.get(protocol).ok_or_else(|| anyhow!("Adapter not found for protocol: {}", protocol))?;
        adapter.read(p).await
    }

    /// Writes a value to a point, automatically routing the request to the correct protocol adapter.
    pub async fn write(&self, point_key: &str, value: Value) -> anyhow::Result<()> {
        let p = self.points.get(point_key).ok_or_else(|| anyhow!("Point not found: {}", point_key))?;
        let protocol = protocol_of(point_key);
        let adapter = self.adapters.get(protocol).ok_or_else(|| anyhow!("Adapter not found for protocol: {}", protocol))?;
        adapter.write(p, value).await
    }
}

/// Determines the protocol from a point key.
/// Assumes the format "protocol:point_name".
fn protocol_of(key: &str) -> &str {
    key.splitn(2, ':').next().unwrap_or("default")
}
