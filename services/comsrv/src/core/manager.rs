
use std::collections::HashMap;
use anyhow::anyhow;
use serde_json::Value;

use crate::core::config::app::AppConfig;
// These imports are from old architecture, commented out for now
// use crate::core::config::point::Point;
// use crate::core::protocols::adapter::Adapter;
// use crate::core::protocols::modbus::adapter::ModbusAdapter;

/// Manages all protocol adapters and provides a unified interface for interacting with them.
/// Note: This is legacy code and needs to be refactored for the new architecture
pub struct Manager {
    // adapters: HashMap<&'static str, Box<dyn Adapter>>,
    // points: HashMap<String, Point>,
}

impl Manager {
    /// Creates a new `Manager` and initializes all protocol adapters based on the provided configuration.
    /// Note: This is legacy code and needs to be refactored for the new architecture
    pub async fn new(_conf: AppConfig) -> anyhow::Result<Self> {
        // TODO: Refactor this for the new protocol factory architecture
        Ok(Self {
            // adapters: HashMap::new(),
            // points: HashMap::new(),
        })
    }

    /// Legacy method - needs refactoring
    pub async fn read(&self, _point_key: &str) -> anyhow::Result<Value> {
        // TODO: Implement using new protocol factory
        Err(anyhow!("Legacy Manager::read not implemented"))
    }

    /// Legacy method - needs refactoring
    pub async fn write(&self, _point_key: &str, _value: Value) -> anyhow::Result<()> {
        // TODO: Implement using new protocol factory
        Err(anyhow!("Legacy Manager::write not implemented"))
    }
}

/// Determines the protocol from a point key.
/// Assumes the format "protocol:point_name".
fn protocol_of(key: &str) -> &str {
    key.splitn(2, ':').next().unwrap_or("default")
}
