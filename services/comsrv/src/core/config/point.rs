//! Basic Point Configuration
//!
//! This module defines the basic Point structure used throughout
//! the communication service for protocol-agnostic point handling.

use serde::{Deserialize, Serialize};

/// Generic point configuration for protocol adapters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Point {
    /// Point identifier
    pub id: String,

    /// Human-readable name
    pub name: String,

    /// Protocol-specific address (could be register, coil, etc.)
    pub address: String,

    /// Data type (bool, uint16, float32, etc.)
    pub data_type: String,

    /// Scale factor for value conversion
    #[serde(default = "default_scale")]
    pub scale: f64,

    /// Offset for value conversion  
    #[serde(default)]
    pub offset: f64,

    /// Engineering unit
    pub unit: Option<String>,

    /// Description
    pub description: Option<String>,
}

fn default_scale() -> f64 {
    1.0
}

impl Point {
    /// Create a new point with basic information
    pub fn new(id: String, name: String, address: String, data_type: String) -> Self {
        Self {
            id,
            name,
            address,
            data_type,
            scale: 1.0,
            offset: 0.0,
            unit: None,
            description: None,
        }
    }

    /// Convert raw value to engineering value
    pub fn to_engineering(&self, raw_value: f64) -> f64 {
        raw_value * self.scale + self.offset
    }

    /// Convert engineering value to raw value
    pub fn from_engineering(&self, eng_value: f64) -> f64 {
        (eng_value - self.offset) / self.scale
    }
}
