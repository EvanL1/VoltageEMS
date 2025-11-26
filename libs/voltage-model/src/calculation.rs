//! Calculation Engine
//!
//! Core orchestrator for all calculation types.
//! Provides pure calculation logic without external IO dependencies.

use crate::energy::EnergyCalculator;
use crate::error::{ModelError, Result};
use crate::expression::ExpressionEvaluator;
use crate::statistics::StatisticsProcessor;
use crate::timeseries::TimeSeriesProcessor;
use std::collections::HashMap;
use std::sync::Arc;
use voltage_config::calculations::{
    AggregationType, CalculationResult, CalculationStatus, CalculationType, EnergyCalculation,
    TimeSeriesOperation,
};
use voltage_config::protocols::QualityCode;

/// Calculation Engine Configuration
#[derive(Debug, Clone, Default)]
pub struct CalculationEngineConfig {
    /// Allow execution of custom Lua scripts (default: false for security)
    pub allow_custom_scripts: bool,
    /// List of pre-approved script hashes (SHA256)
    pub script_whitelist: Vec<String>,
}

/// Calculation Engine - Core orchestrator for all calculation types
///
/// This engine provides pure calculation logic without IO dependencies.
/// Service layers (like modsrv) handle data fetching and result persistence.
pub struct CalculationEngine {
    expression_evaluator: Arc<ExpressionEvaluator>,
    statistics_processor: Arc<StatisticsProcessor>,
    time_series_processor: Arc<TimeSeriesProcessor>,
    energy_calculator: Arc<EnergyCalculator>,
    config: CalculationEngineConfig,
}

impl CalculationEngine {
    /// Create new calculation engine with default config
    pub fn new() -> Self {
        Self::with_config(CalculationEngineConfig::default())
    }

    /// Create new calculation engine with custom config
    pub fn with_config(config: CalculationEngineConfig) -> Self {
        Self {
            expression_evaluator: Arc::new(ExpressionEvaluator::new()),
            statistics_processor: Arc::new(StatisticsProcessor::new()),
            time_series_processor: Arc::new(TimeSeriesProcessor::new()),
            energy_calculator: Arc::new(EnergyCalculator::new()),
            config,
        }
    }

    /// Execute a calculation based on its type
    ///
    /// # Arguments
    /// * `calculation_type` - The type of calculation to execute
    /// * `values` - Pre-fetched values for the calculation
    ///
    /// # Returns
    /// CalculationResult with value, status, and metadata
    pub fn execute(
        &self,
        calculation_id: &str,
        calculation_type: &CalculationType,
        values: &CalculationValues,
    ) -> CalculationResult {
        let start_time = std::time::Instant::now();

        let result = match calculation_type {
            CalculationType::Expression { formula, .. } => {
                self.execute_expression(formula, &values.variables)
            },
            CalculationType::Aggregation { operation, .. } => {
                self.execute_aggregation(operation, &values.array_values)
            },
            CalculationType::TimeSeries {
                operation,
                parameters,
                ..
            } => self.execute_timeseries(operation, &values.time_series, parameters),
            CalculationType::Energy { operation, .. } => {
                self.execute_energy(operation, &values.variables)
            },
            CalculationType::LuaScript { script, .. } => {
                self.execute_lua_script(script, &values.array_string_values)
            },
            #[allow(unreachable_patterns)]
            _ => Err(ModelError::calculation("Calculation type not implemented")),
        };

        let execution_time_ms = start_time.elapsed().as_millis() as u64;

        match result {
            Ok(value) => CalculationResult {
                calculation_id: calculation_id.to_string(),
                timestamp: chrono::Utc::now(),
                value,
                status: CalculationStatus::Success,
                error: None,
                execution_time_ms,
                metadata: HashMap::new(),
                quality: QualityCode::Good,
            },
            Err(e) => CalculationResult {
                calculation_id: calculation_id.to_string(),
                timestamp: chrono::Utc::now(),
                value: serde_json::Value::Null,
                status: CalculationStatus::Error,
                error: Some(e.to_string()),
                execution_time_ms,
                metadata: HashMap::new(),
                quality: QualityCode::Bad,
            },
        }
    }

    /// Execute mathematical expression
    pub fn execute_expression(
        &self,
        formula: &str,
        values: &HashMap<String, f64>,
    ) -> Result<serde_json::Value> {
        let result = self.expression_evaluator.evaluate(formula, values)?;
        Ok(serde_json::json!(result))
    }

    /// Execute aggregation operation
    pub fn execute_aggregation(
        &self,
        operation: &AggregationType,
        values: &[f64],
    ) -> Result<serde_json::Value> {
        if values.is_empty() {
            return Err(ModelError::calculation("No data available for aggregation"));
        }
        let result = self.statistics_processor.aggregate(operation, values)?;
        Ok(serde_json::json!(result))
    }

    /// Execute time-series operation
    pub fn execute_timeseries(
        &self,
        operation: &TimeSeriesOperation,
        series: &[(f64, f64)],
        parameters: &HashMap<String, f64>,
    ) -> Result<serde_json::Value> {
        match operation {
            TimeSeriesOperation::MovingAverage => {
                let window_size = *parameters.get("window_size").unwrap_or(&5.0) as usize;
                let result = self
                    .time_series_processor
                    .calculate_moving_average(series, window_size);
                Ok(serde_json::json!(result))
            },
            TimeSeriesOperation::RateOfChange => {
                let result = self.time_series_processor.calculate_rate_of_change(series);
                Ok(serde_json::json!(result))
            },
            #[allow(unreachable_patterns)]
            _ => Err(ModelError::calculation(
                "Time series operation not implemented",
            )),
        }
    }

    /// Execute energy-specific calculation
    pub fn execute_energy(
        &self,
        operation: &EnergyCalculation,
        values: &HashMap<String, f64>,
    ) -> Result<serde_json::Value> {
        self.energy_calculator.calculate(operation, values)
    }

    /// Execute custom Lua script (security-gated)
    pub fn execute_lua_script(
        &self,
        script: &str,
        _inputs: &[String],
    ) -> Result<serde_json::Value> {
        // Security check: Verify if custom scripts are allowed
        if !self.config.allow_custom_scripts {
            use sha2::{Digest, Sha256};
            let mut hasher = Sha256::new();
            hasher.update(script.as_bytes());
            let script_hash = format!("{:x}", hasher.finalize());

            if !self.config.script_whitelist.contains(&script_hash) {
                return Err(ModelError::calculation(format!(
                    "Custom Lua scripts are disabled. Script hash {} is not in whitelist.",
                    script_hash
                )));
            }
        }

        // TODO: Implement Lua script execution
        Err(ModelError::calculation(
            "Lua script execution not yet implemented",
        ))
    }

    /// Get the expression evaluator for direct use
    pub fn expression_evaluator(&self) -> &ExpressionEvaluator {
        &self.expression_evaluator
    }

    /// Get the statistics processor for direct use
    pub fn statistics_processor(&self) -> &StatisticsProcessor {
        &self.statistics_processor
    }

    /// Get the time series processor for direct use
    pub fn time_series_processor(&self) -> &TimeSeriesProcessor {
        &self.time_series_processor
    }

    /// Get the energy calculator for direct use
    pub fn energy_calculator(&self) -> &EnergyCalculator {
        &self.energy_calculator
    }
}

impl Default for CalculationEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Pre-fetched values for calculation execution
///
/// Service layer fetches data from Redis/database and provides it here.
/// This allows the calculation engine to remain pure and testable.
#[derive(Debug, Clone, Default)]
pub struct CalculationValues {
    /// Variable name to f64 value mapping (for expressions, energy calculations)
    pub variables: HashMap<String, f64>,
    /// Array of f64 values (for aggregations)
    pub array_values: Vec<f64>,
    /// Time series data as (timestamp, value) pairs
    pub time_series: Vec<(f64, f64)>,
    /// Array of string values (for Lua scripts)
    pub array_string_values: Vec<String>,
}

impl CalculationValues {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_variables(mut self, variables: HashMap<String, f64>) -> Self {
        self.variables = variables;
        self
    }

    pub fn with_array_values(mut self, values: Vec<f64>) -> Self {
        self.array_values = values;
        self
    }

    pub fn with_time_series(mut self, series: Vec<(f64, f64)>) -> Self {
        self.time_series = series;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execute_expression() {
        let engine = CalculationEngine::new();
        let mut vars = HashMap::new();
        vars.insert("a".to_string(), 10.0);
        vars.insert("b".to_string(), 5.0);

        let result = engine.execute_expression("a + b * 2", &vars).unwrap();
        assert_eq!(result.as_f64().unwrap(), 20.0);
    }

    #[test]
    fn test_execute_aggregation() {
        let engine = CalculationEngine::new();
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];

        let result = engine
            .execute_aggregation(&AggregationType::Sum, &values)
            .unwrap();
        assert_eq!(result.as_f64().unwrap(), 15.0);

        let result = engine
            .execute_aggregation(&AggregationType::Average, &values)
            .unwrap();
        assert_eq!(result.as_f64().unwrap(), 3.0);
    }

    #[test]
    fn test_execute_timeseries() {
        let engine = CalculationEngine::new();
        let series = vec![
            (1.0, 10.0),
            (2.0, 20.0),
            (3.0, 30.0),
            (4.0, 40.0),
            (5.0, 50.0),
        ];
        let mut params = HashMap::new();
        params.insert("window_size".to_string(), 3.0);

        let result = engine
            .execute_timeseries(&TimeSeriesOperation::MovingAverage, &series, &params)
            .unwrap();
        let arr = result.as_array().unwrap();
        assert_eq!(arr.len(), 3);
        assert_eq!(arr[0].as_f64().unwrap(), 20.0); // (10+20+30)/3
    }

    #[test]
    fn test_execute_energy() {
        let engine = CalculationEngine::new();
        let mut values = HashMap::new();
        values.insert("input_power".to_string(), 1000.0);
        values.insert("output_power".to_string(), 950.0);

        let result = engine
            .execute_energy(&EnergyCalculation::EnergyEfficiency, &values)
            .unwrap();
        assert_eq!(result["efficiency_percent"].as_f64().unwrap(), 95.0);
    }

    #[test]
    fn test_execute_full_calculation() {
        let engine = CalculationEngine::new();

        let calc_type = CalculationType::Expression {
            formula: "a + b".to_string(),
            variables: HashMap::new(), // Original config mapping (not used in execute)
        };

        let mut vars = HashMap::new();
        vars.insert("a".to_string(), 10.0);
        vars.insert("b".to_string(), 5.0);
        let values = CalculationValues::new().with_variables(vars);

        let result = engine.execute("test_calc", &calc_type, &values);
        assert_eq!(result.status, CalculationStatus::Success);
        assert_eq!(result.value.as_f64().unwrap(), 15.0);
    }

    #[test]
    fn test_lua_script_security() {
        let engine = CalculationEngine::new(); // allow_custom_scripts = false

        let result = engine.execute_lua_script("return 1", &[]);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("disabled"));
    }
}
