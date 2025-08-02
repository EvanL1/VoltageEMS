//! Point mapping management module
//!
//! Handles the mapping between ModSrv model point names and underlying comsrv channel/point IDs

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::error::{ModelSrvError, Result};

/// Single point mapping information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointMapping {
    /// Channel ID
    pub channel: u16,
    /// Point ID
    pub point: u32,
    /// Point type: "m"(measurement), "s"(signal), "c"(control), "a"(adjustment)
    #[serde(rename = "type")]
    pub point_type: String,
}

/// Model mapping configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMappingConfig {
    /// Monitoring point mappings
    pub monitoring: HashMap<String, PointMapping>,
    /// Control point mappings
    pub control: HashMap<String, PointMapping>,
}

/// Mapping manager
pub struct MappingManager {
    /// model_id -> ModelMappingConfig
    mappings: HashMap<String, ModelMappingConfig>,
}

impl MappingManager {
    /// Create new mapping manager
    pub fn new() -> Self {
        Self {
            mappings: HashMap::new(),
        }
    }

    /// Load mapping configuration from file
    pub async fn load_from_file<P: AsRef<Path>>(&mut self, model_id: &str, path: P) -> Result<()> {
        let path = path.as_ref();
        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| ModelSrvError::io(format!("Failed to read mapping file: {}", e)))?;

        let config: ModelMappingConfig = serde_json::from_str(&content).map_err(|e| {
            ModelSrvError::format(format!("Failed to parse mapping configuration: {}", e))
        })?;

        self.mappings.insert(model_id.to_string(), config);
        Ok(())
    }

    /// Load mapping configuration
    #[allow(dead_code)]
    pub fn load_mappings(&mut self, model_id: &str, config: ModelMappingConfig) {
        self.mappings.insert(model_id.to_string(), config);
    }

    /// Get monitoring point mapping
    #[allow(dead_code)]
    pub fn get_monitoring_mapping(
        &self,
        model_id: &str,
        point_name: &str,
    ) -> Option<&PointMapping> {
        self.mappings.get(model_id)?.monitoring.get(point_name)
    }

    /// Get control point mapping
    #[allow(dead_code)]
    pub fn get_control_mapping(&self, model_id: &str, control_name: &str) -> Option<&PointMapping> {
        self.mappings.get(model_id)?.control.get(control_name)
    }

    /// Find point name by channel and point (reverse lookup)
    #[allow(dead_code)]
    pub fn find_point_name(
        &self,
        model_id: &str,
        channel: u16,
        point: u32,
        is_control: bool,
    ) -> Option<String> {
        let model_mapping = self.mappings.get(model_id)?;

        let points = if is_control {
            &model_mapping.control
        } else {
            &model_mapping.monitoring
        };

        for (name, mapping) in points {
            if mapping.channel == channel && mapping.point == point {
                return Some(name.clone());
            }
        }

        None
    }

    /// Get all monitoring point mappings for a model
    #[allow(dead_code)]
    pub fn get_all_monitoring_mappings(
        &self,
        model_id: &str,
    ) -> Option<&HashMap<String, PointMapping>> {
        self.mappings.get(model_id).map(|m| &m.monitoring)
    }

    /// Get all control point mappings for a model
    #[allow(dead_code)]
    pub fn get_all_control_mappings(
        &self,
        model_id: &str,
    ) -> Option<&HashMap<String, PointMapping>> {
        self.mappings.get(model_id).map(|m| &m.control)
    }

    /// Batch load all mapping files from a directory
    pub async fn load_directory<P: AsRef<Path>>(&mut self, dir: P) -> Result<()> {
        let dir = dir.as_ref();
        let mut entries = tokio::fs::read_dir(dir)
            .await
            .map_err(|e| ModelSrvError::io(format!("Failed to read mapping directory: {}", e)))?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| ModelSrvError::io(format!("Failed to read directory entry: {}", e)))?
        {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Some(model_id) = path.file_stem().and_then(|s| s.to_str()) {
                    match self.load_from_file(model_id, &path).await {
                        Ok(_) => tracing::info!("Loaded mapping configuration: {}", model_id),
                        Err(e) => tracing::warn!(
                            "Failed to load mapping configuration {}: {}",
                            model_id,
                            e
                        ),
                    }
                }
            }
        }

        Ok(())
    }

    /// Get all mappings (for loading to Redis)
    pub fn get_all_mappings(&self) -> &HashMap<String, ModelMappingConfig> {
        &self.mappings
    }
}

impl Default for MappingManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mapping_lookup() {
        let mut manager = MappingManager::new();

        let config = ModelMappingConfig {
            monitoring: HashMap::from([(
                "voltage_a".to_string(),
                PointMapping {
                    channel: 1001,
                    point: 10001,
                    point_type: "m".to_string(),
                },
            )]),
            control: HashMap::from([(
                "main_switch".to_string(),
                PointMapping {
                    channel: 1001,
                    point: 30001,
                    point_type: "c".to_string(),
                },
            )]),
        };

        manager.load_mappings("test_model", config);

        // Test monitoring point mapping
        let mapping = manager
            .get_monitoring_mapping("test_model", "voltage_a")
            .unwrap();
        assert_eq!(mapping.channel, 1001);
        assert_eq!(mapping.point, 10001);

        // Test control point mapping
        let mapping = manager
            .get_control_mapping("test_model", "main_switch")
            .unwrap();
        assert_eq!(mapping.channel, 1001);
        assert_eq!(mapping.point, 30001);

        // Test reverse lookup
        let name = manager
            .find_point_name("test_model", 1001, 10001, false)
            .unwrap();
        assert_eq!(name, "voltage_a");
    }
}
