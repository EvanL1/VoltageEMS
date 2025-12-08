//! Shared calculation engine types for VoltageEMS services
//!
//! This module provides unified calculation and formula evaluation types
//! used primarily by modsrv but available to all services.

use crate::common::ComparisonOperator;
use crate::modsrv::InstanceRedisKeys;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[cfg(feature = "schema")]
use schemars::JsonSchema;

// ============================================================================
// Model Point Reference Types (for calculation configuration)
// ============================================================================

/// Model point type (M = Measurement, A = Action)
///
/// This enum is used in calculation configurations to distinguish between
/// measurement points (M) and action points (A) at the model layer.
/// It's different from protocol-level PointType (T/S/C/A).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub enum ModelPointType {
    /// Measurement point (测量点)
    M,
    /// Action point (动作点)
    A,
}

impl ModelPointType {
    /// Convert to string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::M => "M",
            Self::A => "A",
        }
    }
}

impl std::fmt::Display for ModelPointType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for ModelPointType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "M" => Ok(Self::M),
            "A" => Ok(Self::A),
            _ => Err(format!(
                "Invalid model point type: '{}', expected 'M' or 'A'",
                s
            )),
        }
    }
}

/// Unified point reference with short field names (inst, type, id)
///
/// Used in YAML configuration and database for calculation sources and outputs.
/// Example: `{ inst: 1, type: M, id: 10 }` → Redis key: `inst:1:M:10`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct PointRef {
    /// Instance ID
    pub inst: u16,
    /// Point type (M = Measurement, A = Action)
    #[serde(rename = "type")]
    pub type_: ModelPointType,
    /// Point ID
    pub id: u32,
}

impl PointRef {
    /// Create a new measurement point reference
    pub fn measurement(inst: u16, id: u32) -> Self {
        Self {
            inst,
            type_: ModelPointType::M,
            id,
        }
    }

    /// Create a new action point reference
    pub fn action(inst: u16, id: u32) -> Self {
        Self {
            inst,
            type_: ModelPointType::A,
            id,
        }
    }

    /// Convert to Redis key: inst:{inst}:{type}:{id}
    pub fn to_redis_key(&self) -> String {
        match self.type_ {
            ModelPointType::M => InstanceRedisKeys::measurement(self.inst, self.id),
            ModelPointType::A => InstanceRedisKeys::action(self.inst, self.id),
        }
    }
}

// ============================================================================
// Calculation Configuration Types (for YAML and SQLite persistence)
// ============================================================================

/// Calculation definition for YAML configuration
///
/// This is the user-facing configuration format with short field names.
/// It gets stored in SQLite and loaded at service startup.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct CalculationConfig {
    /// Unique calculation name
    pub name: String,
    /// Optional description
    #[serde(default)]
    pub description: Option<String>,
    /// Calculation type and parameters
    #[serde(rename = "type")]
    pub calculation_type: CalculationType,
    /// Output point reference
    pub output: PointRef,
    /// Whether calculation is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// YAML file root structure for calculations
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct CalculationsFile {
    /// List of calculation definitions
    pub calculations: Vec<CalculationConfig>,
}

// ============================================================================
// Calculation Types
// ============================================================================

/// Main calculation type definition
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CalculationType {
    /// Simple mathematical expression
    Expression {
        /// Mathematical formula (e.g., "P1 + P2 * 0.5")
        formula: String,
        /// Variable to Redis key mapping
        variables: HashMap<String, String>,
    },
    /// Statistical aggregation
    Aggregation {
        /// Type of aggregation operation
        operation: AggregationType,
        /// Source Redis keys to aggregate
        source_keys: Vec<String>,
        /// Optional time window for aggregation
        #[serde(default)]
        time_window: Option<TimeWindow>,
    },
    /// Time-series analysis
    TimeSeries {
        /// Type of time-series operation
        operation: TimeSeriesOperation,
        /// Source Redis key
        source_key: String,
        /// Operation-specific parameters
        #[serde(default)]
        parameters: HashMap<String, f64>,
    },
    /// Energy-specific calculations
    Energy {
        /// Type of energy calculation
        operation: EnergyCalculation,
        /// Input Redis key mappings
        inputs: HashMap<String, String>,
        /// Additional parameters
        #[serde(default)]
        parameters: HashMap<String, f64>,
    },
    /// Conditional logic
    Conditional {
        /// Condition to evaluate
        condition: ConditionExpression,
        /// Calculation if true
        if_true: Box<CalculationType>,
        /// Calculation if false
        if_false: Box<CalculationType>,
    },
    /// Direct value assignment
    Constant {
        /// Constant value
        value: serde_json::Value,
    },
}

// ============================================================================
// Aggregation Types
// ============================================================================

/// Statistical aggregation operations
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum AggregationType {
    /// Sum of all values
    Sum,
    /// Arithmetic mean
    Average,
    /// Minimum value
    Min,
    /// Maximum value
    Max,
    /// Count of non-null values
    Count,
    /// Standard deviation
    StandardDeviation,
    /// Median value
    Median,
    /// Percentile (0-100)
    Percentile {
        #[serde(default = "default_percentile")]
        value: f64,
    },
    /// Weighted average
    WeightedAverage {
        /// Weights for each source value
        weights: Vec<f64>,
    },
    /// Root mean square
    RootMeanSquare,
    /// Geometric mean
    GeometricMean,
    /// Harmonic mean
    HarmonicMean,
}

fn default_percentile() -> f64 {
    50.0 // Default to median
}

impl AggregationType {
    /// Check if this aggregation requires all values to be present
    pub fn requires_complete_data(&self) -> bool {
        matches!(
            self,
            Self::StandardDeviation | Self::Median | Self::Percentile { .. }
        )
    }

    /// Get minimum number of values required
    pub fn min_values_required(&self) -> usize {
        match self {
            Self::StandardDeviation => 2,
            Self::Average | Self::GeometricMean | Self::HarmonicMean => 1,
            _ => 0,
        }
    }
}

// ============================================================================
// Time Window Definitions
// ============================================================================

/// Time window for calculations
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct TimeWindow {
    /// Duration in seconds
    pub duration_seconds: u64,
    /// Sliding window (true) or tumbling window (false)
    #[serde(default)]
    pub sliding: bool,
    /// Step size for sliding windows (seconds)
    #[serde(default)]
    pub step_seconds: Option<u64>,
}

impl TimeWindow {
    /// Create a tumbling window
    pub fn tumbling(duration_seconds: u64) -> Self {
        Self {
            duration_seconds,
            sliding: false,
            step_seconds: None,
        }
    }

    /// Create a sliding window
    pub fn sliding(duration_seconds: u64, step_seconds: u64) -> Self {
        Self {
            duration_seconds,
            sliding: true,
            step_seconds: Some(step_seconds),
        }
    }
}

// ============================================================================
// Time Series Operations
// ============================================================================

/// Time-series analysis operations
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum TimeSeriesOperation {
    /// Simple moving average
    MovingAverage,
    /// Exponential moving average
    ExponentialMovingAverage,
    /// Rate of change (derivative)
    RateOfChange,
    /// First derivative
    Derivative,
    /// Integration over time
    Integral,
    /// Linear trend analysis
    Trend,
    /// Time-series forecasting
    Forecast,
    /// Anomaly detection
    AnomalyDetection,
    /// Fourier transform
    FourierTransform,
    /// Autocorrelation
    Autocorrelation,
    /// Seasonal decomposition
    SeasonalDecomposition,
}

impl TimeSeriesOperation {
    /// Get default parameters for this operation
    pub fn default_parameters(&self) -> HashMap<String, f64> {
        let mut params = HashMap::new();
        match self {
            Self::MovingAverage => {
                params.insert("window_size".to_string(), 10.0);
            },
            Self::ExponentialMovingAverage => {
                params.insert("alpha".to_string(), 0.3);
            },
            Self::Forecast => {
                params.insert("horizon".to_string(), 24.0);
            },
            Self::AnomalyDetection => {
                params.insert("threshold".to_string(), 3.0);
            },
            _ => {},
        }
        params
    }
}

// ============================================================================
// Energy Calculations
// ============================================================================

/// Energy-specific calculation types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum EnergyCalculation {
    /// Power balance calculation (P_total = P_pv + P_battery - P_load)
    PowerBalance,
    /// State of charge calculation
    StateOfCharge,
    /// Energy efficiency calculation (η = P_out / P_in)
    EnergyEfficiency,
    /// Load forecasting
    LoadForecast,
    /// Optimal power dispatch
    OptimalDispatch,
    /// Cost optimization
    CostOptimization,
    /// Peak shaving calculation
    PeakShaving,
    /// Carbon emissions calculation
    CarbonEmissions,
    /// Grid frequency regulation
    FrequencyRegulation,
    /// Voltage regulation
    VoltageRegulation,
    /// Power factor correction
    PowerFactorCorrection,
    /// Energy storage optimization
    StorageOptimization,
}

impl EnergyCalculation {
    /// Get required inputs for this calculation
    pub fn required_inputs(&self) -> Vec<&'static str> {
        match self {
            Self::PowerBalance => vec!["pv_power", "battery_power", "load_power"],
            Self::StateOfCharge => vec!["battery_current", "battery_capacity"],
            Self::EnergyEfficiency => vec!["input_power", "output_power"],
            Self::LoadForecast => vec!["historical_load"],
            Self::OptimalDispatch => vec!["available_sources", "load_demand"],
            Self::CostOptimization => vec!["energy_prices", "load_profile"],
            Self::PeakShaving => vec!["load_profile", "peak_threshold"],
            Self::CarbonEmissions => vec!["energy_sources", "emission_factors"],
            Self::FrequencyRegulation => vec!["grid_frequency", "target_frequency"],
            Self::VoltageRegulation => vec!["voltage", "target_voltage"],
            Self::PowerFactorCorrection => vec!["active_power", "reactive_power"],
            Self::StorageOptimization => vec!["storage_capacity", "charge_rate", "discharge_rate"],
        }
    }
}

// ============================================================================
// Conditional Expressions
// ============================================================================

/// Condition expression for conditional calculations
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ConditionExpression {
    /// Simple comparison
    Comparison {
        left: String,
        operator: ComparisonOperator,
        right: ConditionValue,
    },
    /// Logical AND
    And {
        conditions: Vec<ConditionExpression>,
    },
    /// Logical OR
    Or {
        conditions: Vec<ConditionExpression>,
    },
    /// Logical NOT
    Not { condition: Box<ConditionExpression> },
    /// Range check
    InRange {
        value: String,
        min: f64,
        max: f64,
        #[serde(default = "default_true")]
        inclusive: bool,
    },
}

fn default_true() -> bool {
    true
}

/// Value types for conditions
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(untagged)]
pub enum ConditionValue {
    Number(f64),
    String(String),
    Boolean(bool),
    Variable(String),
}

// ============================================================================
// Calculation Triggers
// ============================================================================

/// Trigger mechanism for calculation execution
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CalculationTrigger {
    /// Periodic execution
    Interval {
        /// Interval in milliseconds
        milliseconds: u64,
        /// Optional initial delay
        #[serde(default)]
        initial_delay_ms: Option<u64>,
    },
    /// Triggered by data changes
    DataChange {
        /// Redis keys to watch
        watch_keys: Vec<String>,
        /// Debounce time in milliseconds
        #[serde(default)]
        debounce_ms: Option<u64>,
    },
    /// Manual trigger via API
    Manual,
    /// Chained from another calculation
    Chain {
        /// Parent calculation ID
        parent_calculation_id: String,
        /// Only trigger on successful parent
        #[serde(default = "default_true")]
        on_success_only: bool,
    },
    /// Cron expression trigger
    Cron {
        /// Cron expression string
        expression: String,
    },
    /// Event-based trigger
    Event {
        /// Event type to watch for
        event_type: String,
        /// Optional event filter
        #[serde(default)]
        filter: Option<HashMap<String, String>>,
    },
}

// ============================================================================
// Calculation Results
// ============================================================================

/// Result of a calculation execution
#[derive(Debug, Clone, Serialize, Deserialize)]
// JsonSchema not supported for chrono::DateTime
pub struct CalculationResult {
    /// Calculation ID
    pub calculation_id: String,
    /// Execution timestamp
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Calculated value
    pub value: serde_json::Value,
    /// Execution status
    pub status: CalculationStatus,
    /// Error message if failed
    #[serde(default)]
    pub error: Option<String>,
    /// Execution time in milliseconds
    pub execution_time_ms: u64,
    /// Quality code of the result
    #[serde(default)]
    pub quality: crate::protocols::QualityCode,
    /// Additional metadata
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Calculation execution status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum CalculationStatus {
    Success,
    Error,
    Timeout,
    PartialData,
    Skipped,
    InvalidInput,
}

// ============================================================================
// Calculation Definitions
// ============================================================================

/// Complete calculation definition
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct CalculationDefinition {
    /// Unique calculation ID
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Description
    #[serde(default)]
    pub description: Option<String>,
    /// Calculation type and parameters
    pub calculation_type: CalculationType,
    /// Redis key for storing result
    pub output_key: String,
    /// Execution trigger
    pub trigger: CalculationTrigger,
    /// Whether calculation is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Priority (lower = higher priority)
    #[serde(default)]
    pub priority: Option<u32>,
    /// Tags for categorization
    #[serde(default)]
    pub tags: Vec<String>,
    /// Additional metadata
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl CalculationDefinition {
    /// Create a new calculation definition
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        calculation_type: CalculationType,
        output_key: impl Into<String>,
        trigger: CalculationTrigger,
    ) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: None,
            calculation_type,
            output_key: output_key.into(),
            trigger,
            enabled: true,
            priority: None,
            tags: Vec::new(),
            metadata: HashMap::new(),
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;

    #[test]
    fn test_aggregation_requirements() {
        assert!(AggregationType::StandardDeviation.requires_complete_data());
        assert!(!AggregationType::Sum.requires_complete_data());
        assert_eq!(AggregationType::StandardDeviation.min_values_required(), 2);
    }

    #[test]
    fn test_time_window_creation() {
        let tumbling = TimeWindow::tumbling(60);
        assert!(!tumbling.sliding);
        assert_eq!(tumbling.duration_seconds, 60);

        let sliding = TimeWindow::sliding(300, 10);
        assert!(sliding.sliding);
        assert_eq!(sliding.step_seconds, Some(10));
    }

    #[test]
    fn test_energy_calculation_inputs() {
        let inputs = EnergyCalculation::PowerBalance.required_inputs();
        assert!(inputs.contains(&"pv_power"));
        assert!(inputs.contains(&"battery_power"));
        assert!(inputs.contains(&"load_power"));
    }

    #[test]
    fn test_calculation_definition_creation() {
        let def = CalculationDefinition::new(
            "calc1",
            "Test Calculation",
            CalculationType::Constant {
                value: serde_json::json!(42),
            },
            "result:calc1",
            CalculationTrigger::Manual,
        );
        assert_eq!(def.id, "calc1");
        assert!(def.enabled);
    }

    // ========================================================================
    // ModelPointType, PointRef, CalculationConfig serialization tests
    // ========================================================================

    #[test]
    fn test_model_point_type_json_serde() {
        // Test JSON serialization/deserialization
        assert_eq!(serde_json::to_string(&ModelPointType::M).unwrap(), "\"M\"");
        assert_eq!(serde_json::to_string(&ModelPointType::A).unwrap(), "\"A\"");

        // Test deserialization
        assert_eq!(
            serde_json::from_str::<ModelPointType>("\"M\"").unwrap(),
            ModelPointType::M
        );
        assert_eq!(
            serde_json::from_str::<ModelPointType>("\"A\"").unwrap(),
            ModelPointType::A
        );
    }

    #[test]
    fn test_model_point_type_from_str() {
        assert_eq!("M".parse::<ModelPointType>().unwrap(), ModelPointType::M);
        assert_eq!("A".parse::<ModelPointType>().unwrap(), ModelPointType::A);
        assert!("X".parse::<ModelPointType>().is_err());
    }

    #[test]
    fn test_model_point_type_display() {
        assert_eq!(ModelPointType::M.to_string(), "M");
        assert_eq!(ModelPointType::A.to_string(), "A");
        assert_eq!(ModelPointType::M.as_str(), "M");
    }

    #[test]
    fn test_point_ref_yaml_roundtrip() {
        // Test YAML format: { inst: 1, type: M, id: 10 }
        let yaml = r#"{ inst: 1, type: M, id: 10 }"#;
        let point: PointRef = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(point.inst, 1);
        assert_eq!(point.type_, ModelPointType::M);
        assert_eq!(point.id, 10);

        // Test action point
        let yaml_a = r#"{ inst: 5, type: A, id: 20 }"#;
        let point_a: PointRef = serde_yaml::from_str(yaml_a).unwrap();
        assert_eq!(point_a.type_, ModelPointType::A);
    }

    #[test]
    fn test_point_ref_constructors() {
        let m = PointRef::measurement(1, 10);
        assert_eq!(m.type_, ModelPointType::M);

        let a = PointRef::action(2, 20);
        assert_eq!(a.type_, ModelPointType::A);
    }

    #[test]
    fn test_point_ref_to_redis_key() {
        let m = PointRef::measurement(1, 10);
        assert_eq!(m.to_redis_key(), "inst:1:M:10");

        let a = PointRef::action(2, 20);
        assert_eq!(a.to_redis_key(), "inst:2:A:20");
    }

    #[test]
    fn test_calculation_config_yaml_roundtrip() {
        let yaml = r#"
name: test_calc
description: "Test calculation"
type:
  type: expression
  formula: "a + b"
  variables:
    a: "inst:1:M:1"
    b: "inst:1:M:2"
output: { inst: 100, type: M, id: 1 }
enabled: true
"#;
        let config: CalculationConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.name, "test_calc");
        assert_eq!(config.description, Some("Test calculation".to_string()));
        assert!(config.enabled);
        assert_eq!(config.output.inst, 100);
        assert_eq!(config.output.type_, ModelPointType::M);
        assert_eq!(config.output.id, 1);

        // Verify calculation type
        if let CalculationType::Expression { formula, variables } = &config.calculation_type {
            assert_eq!(formula, "a + b");
            assert_eq!(variables.get("a"), Some(&"inst:1:M:1".to_string()));
        } else {
            panic!("Expected Expression type");
        }
    }

    #[test]
    fn test_calculation_config_default_enabled() {
        // enabled defaults to true when not specified
        let yaml = r#"
name: minimal_calc
type:
  type: constant
  value: 42
output: { inst: 1, type: M, id: 1 }
"#;
        let config: CalculationConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(config.enabled);
        assert!(config.description.is_none());
    }

    #[test]
    fn test_calculations_file_yaml_roundtrip() {
        let yaml = r#"
calculations:
  - name: calc1
    type:
      type: constant
      value: 1
    output: { inst: 1, type: M, id: 1 }
  - name: calc2
    type:
      type: constant
      value: 2
    output: { inst: 1, type: M, id: 2 }
    enabled: false
"#;
        let file: CalculationsFile = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(file.calculations.len(), 2);
        assert_eq!(file.calculations[0].name, "calc1");
        assert!(file.calculations[0].enabled);
        assert_eq!(file.calculations[1].name, "calc2");
        assert!(!file.calculations[1].enabled);
    }

    #[test]
    fn test_calculation_config_aggregation_type() {
        let yaml = r#"
name: avg_calc
type:
  type: aggregation
  operation: average
  source_keys:
    - "inst:1:M:1"
    - "inst:2:M:1"
output: { inst: 100, type: M, id: 10 }
"#;
        let config: CalculationConfig = serde_yaml::from_str(yaml).unwrap();
        if let CalculationType::Aggregation {
            operation,
            source_keys,
            ..
        } = &config.calculation_type
        {
            assert!(matches!(operation, AggregationType::Average));
            assert_eq!(source_keys.len(), 2);
        } else {
            panic!("Expected Aggregation type");
        }
    }

    #[test]
    fn test_calculation_config_energy_type() {
        let yaml = r#"
name: energy_calc
type:
  type: energy
  operation: power_balance
  inputs:
    pv_power: "inst:1:M:1"
    battery_power: "inst:1:M:2"
    load_power: "inst:1:M:3"
  parameters:
    interval_seconds: 1.0
output: { inst: 100, type: M, id: 20 }
"#;
        let config: CalculationConfig = serde_yaml::from_str(yaml).unwrap();
        if let CalculationType::Energy {
            inputs, parameters, ..
        } = &config.calculation_type
        {
            assert!(inputs.contains_key("pv_power"));
            assert!(inputs.contains_key("battery_power"));
            assert_eq!(parameters.get("interval_seconds"), Some(&1.0));
        } else {
            panic!("Expected Energy type");
        }
    }
}
