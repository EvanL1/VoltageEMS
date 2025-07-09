//! Simplified point mapping structure
//!
//! This module provides a simplified point mapping that only contains
//! essential fields: point_id and telemetry type. Protocol-specific
//! details are handled by each protocol implementation.

use crate::core::config::types::protocol::TelemetryType;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Simplified point mapping - only essential fields
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimplePointMapping {
    /// Point ID (must match four-remote table)
    pub point_id: String,

    /// Telemetry type (YC/YX/YK/YT)
    pub telemetry_type: TelemetryType,
}

/// Protocol mapping table using simplified structure
#[derive(Debug, Clone, Default)]
pub struct SimpleMappingTable {
    /// All points indexed by point_id
    pub points: HashMap<String, SimplePointMapping>,

    /// Points grouped by telemetry type for quick access
    pub by_type: HashMap<TelemetryType, Vec<String>>,
}

impl SimpleMappingTable {
    /// Create a new empty mapping table
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a point to the mapping table
    pub fn add_point(&mut self, mapping: SimplePointMapping) {
        let point_id = mapping.point_id.clone();
        let telemetry_type = mapping.telemetry_type.clone();

        // Add to main index
        self.points.insert(point_id.clone(), mapping);

        // Add to type index
        self.by_type
            .entry(telemetry_type)
            .or_insert_with(Vec::new)
            .push(point_id);
    }

    /// Get a point by ID
    pub fn get_point(&self, point_id: &str) -> Option<&SimplePointMapping> {
        self.points.get(point_id)
    }

    /// Get all points of a specific telemetry type
    pub fn get_points_by_type(&self, telemetry_type: &TelemetryType) -> Vec<&SimplePointMapping> {
        self.by_type
            .get(telemetry_type)
            .map(|ids| ids.iter().filter_map(|id| self.points.get(id)).collect())
            .unwrap_or_default()
    }

    /// Get total point count
    pub fn len(&self) -> usize {
        self.points.len()
    }

    /// Check if table is empty
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }
}

/// Protocol-specific mapping extension
/// Each protocol can extend this with their specific addressing needs
pub trait ProtocolMapping {
    /// Get the simple point mapping
    fn get_simple_mapping(&self) -> &SimplePointMapping;

    /// Get protocol-specific address as string
    fn get_address_string(&self) -> String;
}

/// Example: Modbus-specific mapping that extends the simple mapping
#[derive(Debug, Clone)]
pub struct ModbusMapping {
    /// Base mapping with point_id and telemetry_type
    pub base: SimplePointMapping,

    /// Modbus-specific fields
    pub slave_id: u8,
    pub function_code: u8,
    pub register_address: u16,
}

impl ProtocolMapping for ModbusMapping {
    fn get_simple_mapping(&self) -> &SimplePointMapping {
        &self.base
    }

    fn get_address_string(&self) -> String {
        format!(
            "{}:{}:{}",
            self.slave_id, self.function_code, self.register_address
        )
    }
}
