//! Forward Calculation Module

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde::{Deserialize, Serialize};

use super::telemetry::TelemetryType;
use crate::utils::Result;

/// Calculation operation types
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CalculationOperation {
    Add,
    Subtract,
    Multiply,
    Divide,
    Average,
    Sum,
}

/// Source point configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourcePointConfig {
    pub point_id: String,
    pub variable_name: String,
    pub scale_factor: Option<f64>,
    pub offset: Option<f64>,
    pub required: bool,
    pub default_value: Option<f64>,
}

impl SourcePointConfig {
    pub fn new(point_id: &str, variable_name: &str) -> Self {
        Self {
            point_id: point_id.to_string(),
            variable_name: variable_name.to_string(),
            scale_factor: None,
            offset: None,
            required: true,
            default_value: None,
        }
    }

    pub fn process_value(&self, raw_value: f64) -> f64 {
        let scaled = raw_value * self.scale_factor.unwrap_or(1.0);
        scaled + self.offset.unwrap_or(0.0)
    }
}

/// Target point configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetPointConfig {
    pub point_id: String,
    pub name: String,
    pub telemetry_type: TelemetryType,
    pub unit: Option<String>,
}

impl TargetPointConfig {
    pub fn new(point_id: &str, name: &str, telemetry_type: TelemetryType) -> Self {
        Self {
            point_id: point_id.to_string(),
            name: name.to_string(),
            telemetry_type,
            unit: None,
        }
    }
}

/// Forward calculation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForwardCalculationConfig {
    pub id: String,
    pub name: String,
    pub source_points: Vec<SourcePointConfig>,
    pub target_point: TargetPointConfig,
    pub operation: CalculationOperation,
    pub enabled: bool,
    pub priority: i32,
}

impl ForwardCalculationConfig {
    pub fn new(
        id: &str,
        name: &str,
        source_points: Vec<SourcePointConfig>,
        target_point: TargetPointConfig,
        operation: CalculationOperation,
    ) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            source_points,
            target_point,
            operation,
            enabled: true,
            priority: 0,
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.id.is_empty() {
            return Err(crate::utils::ComSrvError::InvalidParameter(
                "Calculation ID cannot be empty".to_string(),
            ));
        }
        if self.source_points.is_empty() {
            return Err(crate::utils::ComSrvError::InvalidParameter(
                "At least one source point is required".to_string(),
            ));
        }
        Ok(())
    }
}

/// Forward calculation engine
#[derive(Clone)]
pub struct ForwardCalculationEngine {
    calculations: Arc<RwLock<HashMap<String, ForwardCalculationConfig>>>,
    point_values: Arc<RwLock<HashMap<String, f64>>>,
    channel_id: String,
}

impl ForwardCalculationEngine {
    pub fn new(channel_id: String) -> Self {
        Self {
            calculations: Arc::new(RwLock::new(HashMap::new())),
            point_values: Arc::new(RwLock::new(HashMap::new())),
            channel_id,
        }
    }

    pub async fn load_calculations(&self, configs: Vec<ForwardCalculationConfig>) -> Result<()> {
        let mut calculations = self.calculations.write().await;
        calculations.clear();
        
        for config in configs {
            config.validate()?;
            calculations.insert(config.id.clone(), config);
        }
        
        Ok(())
    }

    pub async fn update_point_value(&self, point_id: &str, value: f64) {
        self.point_values.write().await.insert(point_id.to_string(), value);
    }

    pub async fn get_calculation_config(&self, calc_id: &str) -> Option<ForwardCalculationConfig> {
        self.calculations.read().await.get(calc_id).cloned()
    }

    pub async fn execute_calculation(&self, calc_id: &str) -> Result<Option<f64>> {
        let config = self.get_calculation_config(calc_id).await.ok_or_else(|| {
            crate::utils::ComSrvError::NotFound(format!("Calculation not found: {}", calc_id))
        })?;

        if !config.enabled {
            return Ok(None);
        }

        let point_values = self.point_values.read().await;
        let mut source_values = Vec::new();
        
        for source in &config.source_points {
            if let Some(&raw_value) = point_values.get(&source.point_id) {
                let processed_value = source.process_value(raw_value);
                source_values.push(processed_value);
            } else if source.required {
                if let Some(default) = source.default_value {
                    source_values.push(default);
                } else {
                    return Err(crate::utils::ComSrvError::PointNotFound(format!(
                        "Required source point {} not available",
                        source.point_id
                    )));
                }
            }
        }

        let result = match &config.operation {
            CalculationOperation::Add => {
                if source_values.len() >= 2 {
                    source_values[0] + source_values[1]
                } else {
                    return Err(crate::utils::ComSrvError::InvalidOperation(
                        "Add operation requires at least 2 values".to_string(),
                    ));
                }
            }
            CalculationOperation::Subtract => {
                if source_values.len() >= 2 {
                    source_values[0] - source_values[1]
                } else {
                    return Err(crate::utils::ComSrvError::InvalidOperation(
                        "Subtract operation requires at least 2 values".to_string(),
                    ));
                }
            }
            CalculationOperation::Multiply => {
                if source_values.len() >= 2 {
                    source_values[0] * source_values[1]
                } else {
                    return Err(crate::utils::ComSrvError::InvalidOperation(
                        "Multiply operation requires at least 2 values".to_string(),
                    ));
                }
            }
            CalculationOperation::Divide => {
                if source_values.len() >= 2 {
                    if source_values[1] == 0.0 {
                        return Err(crate::utils::ComSrvError::InvalidOperation(
                            "Division by zero".to_string(),
                        ));
                    }
                    source_values[0] / source_values[1]
                } else {
                    return Err(crate::utils::ComSrvError::InvalidOperation(
                        "Divide operation requires at least 2 values".to_string(),
                    ));
                }
            }
            CalculationOperation::Average => {
                if source_values.is_empty() {
                    0.0
                } else {
                    source_values.iter().sum::<f64>() / source_values.len() as f64
                }
            }
            CalculationOperation::Sum => source_values.iter().sum(),
        };

        Ok(Some(result))
    }

    pub fn channel_id(&self) -> &str {
        &self.channel_id
    }

    pub async fn calculate_all(&self) -> Result<HashMap<String, f64>> {
        let calculations = self.calculations.read().await;
        let mut results = HashMap::new();

        for config in calculations.values() {
            if config.enabled {
                if let Ok(Some(result)) = self.execute_calculation(&config.id).await {
                    results.insert(config.target_point.point_id.clone(), result);
                    self.point_values.write().await.insert(config.target_point.point_id.clone(), result);
                }
            }
        }

        Ok(results)
    }
}

/// Forward calculation manager
#[derive(Clone)]
pub struct ForwardCalculationManager {
    engines: Arc<RwLock<HashMap<String, ForwardCalculationEngine>>>,
}

impl ForwardCalculationManager {
    pub fn new() -> Self {
        Self {
            engines: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn get_engine(&self, channel_id: &str) -> ForwardCalculationEngine {
        let mut engines = self.engines.write().await;
        engines
            .entry(channel_id.to_string())
            .or_insert_with(|| ForwardCalculationEngine::new(channel_id.to_string()))
            .clone()
    }

    pub async fn get_active_channels(&self) -> Vec<String> {
        self.engines.read().await.keys().cloned().collect()
    }
}

impl Default for ForwardCalculationManager {
    fn default() -> Self {
        Self::new()
    }
}
