//! Calculation Engine Module
//!
//! This module provides advanced calculation and formula evaluation capabilities
//! for processing complex mathematical operations on comsrv data.

#![allow(clippy::disallowed_methods)] // json! macro used in multiple functions

use anyhow::{anyhow, Context, Result};
use common::redis::RedisClient;
use evalexpr::ContextWithMutableVariables; // For expression evaluation
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

// Import shared calculation types from voltage-config
use voltage_config::calculations::{
    AggregationType, CalculationDefinition, CalculationResult, CalculationStatus,
    CalculationTrigger, CalculationType, EnergyCalculation, TimeSeriesOperation, TimeWindow,
};
use voltage_config::protocols::QualityCode;

// Type aliases for backward compatibility
pub type CalculationDef = CalculationDefinition;

/// Calculation Engine Configuration (reserved for future use)
#[derive(Debug, Clone, Default)]
pub struct CalculationEngineConfig;

/// Calculation Engine
pub struct CalculationEngine {
    redis_client: Option<Arc<RedisClient>>, // Optional for testing
    calculations: Arc<RwLock<HashMap<String, CalculationDef>>>,
    expression_evaluator: Arc<ExpressionEvaluator>,
    statistics_processor: Arc<StatisticsProcessor>,
    time_series_processor: Arc<TimeSeriesProcessor>,
    energy_calculator: Arc<EnergyCalculator>,
}

impl CalculationEngine {
    /// Create new calculation engine
    pub fn new(redis_client: Arc<RedisClient>) -> Self {
        Self::with_redis(Some(redis_client))
    }

    /// Create calculation engine with optional Redis client (for testing)
    pub fn with_redis(redis_client: Option<Arc<RedisClient>>) -> Self {
        Self {
            redis_client,
            calculations: Arc::new(RwLock::new(HashMap::new())),
            expression_evaluator: Arc::new(ExpressionEvaluator::new()),
            statistics_processor: Arc::new(StatisticsProcessor::new()),
            time_series_processor: Arc::new(TimeSeriesProcessor::new()),
            energy_calculator: Arc::new(EnergyCalculator::new()),
        }
    }

    /// Register a calculation
    pub async fn register_calculation(&self, calc: CalculationDef) -> Result<()> {
        let mut calculations = self.calculations.write().await;
        debug!("Calc registered: {}", calc.id);
        calculations.insert(calc.id.clone(), calc);
        Ok(())
    }

    /// List all registered calculations
    pub async fn list_calculations(&self) -> Vec<CalculationDef> {
        let calculations = self.calculations.read().await;
        calculations.values().cloned().collect()
    }

    /// Get a specific calculation by ID
    pub async fn get_calculation(&self, id: &str) -> Option<CalculationDef> {
        let calculations = self.calculations.read().await;
        calculations.get(id).cloned()
    }

    /// Delete a calculation by ID
    pub async fn delete_calculation(&self, id: &str) -> Result<()> {
        let mut calculations = self.calculations.write().await;
        if calculations.remove(id).is_some() {
            debug!("Calc deleted: {}", id);
            Ok(())
        } else {
            Err(anyhow!("Calculation not found: {}", id))
        }
    }

    /// Execute a calculation
    pub async fn execute_calculation(&self, calculation_id: &str) -> Result<CalculationResult> {
        let calculations = self.calculations.read().await;
        let calc = calculations
            .get(calculation_id)
            .ok_or_else(|| anyhow!("Calculation not found: {}", calculation_id))?
            .clone();

        if !calc.enabled {
            return Err(anyhow!("Calculation is disabled: {}", calculation_id));
        }

        let start_time = std::time::Instant::now();

        let result = match &calc.calculation_type {
            CalculationType::Expression { formula, variables } => {
                self.execute_expression(formula, variables).await
            },
            CalculationType::Aggregation {
                operation,
                source_keys,
                time_window,
            } => {
                self.execute_aggregation(operation, source_keys, time_window.as_ref())
                    .await
            },
            CalculationType::TimeSeries {
                operation,
                source_key,
                parameters,
            } => {
                self.execute_timeseries(operation, source_key, parameters)
                    .await
            },
            CalculationType::Energy {
                operation, inputs, ..
            } => self.execute_energy_calculation(operation, inputs).await,
            // Handle calculation types not yet implemented
            _ => {
                warn!("Unsupported calc type");
                Err(anyhow!("Calculation type not yet implemented"))
            },
        };

        let execution_time_ms = start_time.elapsed().as_millis() as u64;

        match result {
            Ok(value) => {
                // Store result in Redis (if available)
                if let Some(redis) = &self.redis_client {
                    redis.set(&calc.output_key, value.to_string()).await?;
                }

                // Also store with timestamp (using raw command as ZADD not in RedisClient)
                // TODO: Add ZADD to RedisClient if needed frequently

                Ok(CalculationResult {
                    calculation_id: calculation_id.to_string(),
                    timestamp: chrono::Utc::now(),
                    value,
                    status: CalculationStatus::Success,
                    error: None,
                    execution_time_ms,
                    metadata: HashMap::new(),
                    quality: QualityCode::Good,
                })
            },
            Err(e) => {
                error!("Calc failed: {}", e);
                Ok(CalculationResult {
                    calculation_id: calculation_id.to_string(),
                    timestamp: chrono::Utc::now(),
                    value: serde_json::Value::Null,
                    status: CalculationStatus::Error,
                    error: Some(e.to_string()),
                    execution_time_ms,
                    metadata: HashMap::new(),
                    quality: QualityCode::Bad,
                })
            },
        }
    }

    /// Execute mathematical expression
    async fn execute_expression(
        &self,
        formula: &str,
        variables: &HashMap<String, String>,
    ) -> Result<serde_json::Value> {
        // Fetch variable values from Redis
        let mut values = HashMap::new();
        for (var_name, redis_key) in variables {
            let redis = self
                .redis_client
                .as_ref()
                .ok_or_else(|| anyhow!("RedisClient required for reading point data"))?;
            let value: Option<String> = redis.get(redis_key).await?;

            if let Some(val_str) = value {
                let val: f64 = val_str
                    .parse()
                    .context(format!("Failed to parse value for {}", var_name))?;
                values.insert(var_name.clone(), val);
            } else {
                warn!("Var {} not in Redis", var_name);
                values.insert(var_name.clone(), 0.0);
            }
        }

        let result = self.expression_evaluator.evaluate(formula, &values)?;
        Ok(serde_json::json!(result))
    }

    /// Execute aggregation operation
    async fn execute_aggregation(
        &self,
        operation: &AggregationType,
        source_keys: &[String],
        _time_window: Option<&TimeWindow>,
    ) -> Result<serde_json::Value> {
        // Fetch values from Redis
        let mut values = Vec::new();
        for key in source_keys {
            let redis = self
                .redis_client
                .as_ref()
                .ok_or_else(|| anyhow!("RedisClient required for reading point data"))?;
            let value: Option<String> = redis.get(key).await?;

            if let Some(val_str) = value {
                if let Ok(val) = val_str.parse::<f64>() {
                    values.push(val);
                }
            }
        }

        if values.is_empty() {
            return Err(anyhow!("No data available for aggregation"));
        }

        let result = self.statistics_processor.aggregate(operation, &values)?;
        Ok(serde_json::json!(result))
    }

    /// Execute time-series operation
    async fn execute_timeseries(
        &self,
        operation: &TimeSeriesOperation,
        source_key: &str,
        parameters: &HashMap<String, f64>,
    ) -> Result<serde_json::Value> {
        // Fetch historical data from sorted set
        let _history_key = format!("{}:history", source_key);
        let now = chrono::Utc::now().timestamp_millis();
        let window = parameters.get("window_seconds").unwrap_or(&3600.0) * 1000.0;
        let _start = now - window as i64;

        // TODO: Add ZRANGEBYSCORE to RedisClient if needed frequently
        // For now, just return empty result as placeholder
        let data: Vec<(String, f64)> = Vec::new();

        let mut time_series = Vec::new();
        for i in (0..data.len()).step_by(2) {
            if let Ok(value) = data[i].0.parse::<f64>() {
                time_series.push((data[i].1, value));
            }
        }

        match operation {
            TimeSeriesOperation::MovingAverage => {
                let window_size = *parameters.get("window_size").unwrap_or(&5.0) as usize;
                let result = self.calculate_moving_average(&time_series, window_size);
                Ok(serde_json::json!(result))
            },
            TimeSeriesOperation::RateOfChange => {
                let result = self.calculate_rate_of_change(&time_series);
                Ok(serde_json::json!(result))
            },
            _ => Err(anyhow!("Time series operation not yet implemented")),
        }
    }

    /// Execute energy-specific calculation
    async fn execute_energy_calculation(
        &self,
        operation: &EnergyCalculation,
        inputs: &HashMap<String, String>,
    ) -> Result<serde_json::Value> {
        // Fetch input values from Redis using internal client
        let mut values = HashMap::new();
        for (name, key) in inputs {
            let redis = self
                .redis_client
                .as_ref()
                .ok_or_else(|| anyhow!("RedisClient required for reading point data"))?;
            let value: Option<String> = redis.get(key).await?;
            if let Some(val_str) = value {
                if let Ok(val) = val_str.parse::<f64>() {
                    values.insert(name.clone(), val);
                }
            }
        }

        self.energy_calculator.calculate(operation, &values)
    }

    /// Calculate moving average (delegates to TimeSeriesProcessor)
    fn calculate_moving_average(&self, series: &[(f64, f64)], window: usize) -> Vec<f64> {
        self.time_series_processor
            .calculate_moving_average(series, window)
    }

    /// Calculate rate of change (delegates to TimeSeriesProcessor)
    fn calculate_rate_of_change(&self, series: &[(f64, f64)]) -> Vec<f64> {
        self.time_series_processor.calculate_rate_of_change(series)
    }

    /// Execute an ad-hoc expression with provided variable values (no Redis IO, no persistence)
    pub async fn execute_expression_values(
        &self,
        formula: &str,
        values: &std::collections::HashMap<String, f64>,
    ) -> Result<serde_json::Value> {
        let result = self.expression_evaluator.evaluate(formula, values)?;
        Ok(serde_json::json!(result))
    }

    /// Execute an ad-hoc aggregation on provided values (no Redis IO, no persistence)
    pub fn execute_aggregation_values(
        &self,
        operation: &AggregationType,
        values: &[f64],
    ) -> Result<serde_json::Value> {
        let result = self.statistics_processor.aggregate(operation, values)?;
        Ok(serde_json::json!(result))
    }

    /// Execute an ad-hoc energy calculation on provided inputs (no Redis IO, no persistence)
    pub fn execute_energy_values(
        &self,
        operation: &EnergyCalculation,
        inputs: &std::collections::HashMap<String, f64>,
    ) -> Result<serde_json::Value> {
        self.energy_calculator.calculate(operation, inputs)
    }

    /// Load all enabled calculation definitions from SQLite database
    ///
    /// This method reads from the `calculations` table (populated by Monarch from YAML)
    /// and registers each calculation with the engine.
    ///
    /// # Table Schema
    /// ```sql
    /// calculations(calculation_name, calculation_type, output_inst, output_type, output_id, enabled)
    /// ```
    ///
    /// # Returns
    /// Number of calculations loaded successfully
    pub async fn load_from_sqlite(&self, pool: &SqlitePool) -> Result<usize> {
        let rows = sqlx::query_as::<_, (String, Option<String>, String, i64, String, i64)>(
            r#"
            SELECT calculation_name, description, calculation_type,
                   output_inst, output_type, output_id
            FROM calculations
            WHERE enabled = TRUE
            "#,
        )
        .fetch_all(pool)
        .await
        .context("Failed to load calculations from SQLite")?;

        let mut count = 0;
        for (name, description, type_json, inst, type_, id) in rows {
            // Parse the JSON-serialized CalculationType
            let calc_type: CalculationType = match serde_json::from_str(&type_json) {
                Ok(ct) => ct,
                Err(e) => {
                    error!("Parse calc_type '{}': {}", name, e);
                    continue;
                },
            };

            // Generate Redis key: inst:{inst}:{type}:{id}
            let output_key = format!("inst:{}:{}:{}", inst, type_, id);

            // Create calculation definition
            let calc = CalculationDef {
                id: name.clone(),
                name: name.clone(),
                description,
                calculation_type: calc_type,
                output_key,
                trigger: CalculationTrigger::Manual, // API trigger only (no scheduled execution)
                enabled: true,
                priority: None,
                tags: Vec::new(),
                metadata: HashMap::new(),
            };

            // Register with the engine
            if let Err(e) = self.register_calculation(calc).await {
                warn!("Register calc '{}': {}", name, e);
                continue;
            }

            count += 1;
        }

        info!("{} calcs loaded", count);
        Ok(count)
    }
}

/// Expression evaluator for mathematical formulas
struct ExpressionEvaluator;

impl ExpressionEvaluator {
    fn new() -> Self {
        Self
    }

    fn evaluate(&self, formula: &str, variables: &HashMap<String, f64>) -> Result<f64> {
        // Use evalexpr for proper expression evaluation with correct operator precedence
        let mut context = evalexpr::HashMapContext::new();

        // Add variables to context
        for (name, value) in variables {
            context
                .set_value(name.to_string(), evalexpr::Value::from(*value))
                .map_err(|e| anyhow!("Failed to set variable {}: {}", name, e))?;
        }

        // Evaluate expression with context
        let result = evalexpr::eval_with_context(formula, &context)
            .context(format!("Failed to evaluate expression: {}", formula))?;

        // Convert result to f64
        match result {
            evalexpr::Value::Float(f) => Ok(f),
            evalexpr::Value::Int(i) => Ok(i as f64),
            _ => Err(anyhow!(
                "Expression did not evaluate to a number: {}",
                formula
            )),
        }
    }
}

/// Statistics processor for aggregations
struct StatisticsProcessor;

impl StatisticsProcessor {
    fn new() -> Self {
        Self
    }

    fn aggregate(&self, operation: &AggregationType, values: &[f64]) -> Result<f64> {
        if values.is_empty() {
            return Err(anyhow!("Cannot aggregate empty dataset"));
        }

        match operation {
            AggregationType::Sum => Ok(values.iter().sum()),
            AggregationType::Average => {
                if values.is_empty() {
                    Err(anyhow!("Cannot calculate average of empty dataset"))
                } else {
                    Ok(values.iter().sum::<f64>() / values.len() as f64)
                }
            },
            AggregationType::Min => {
                if values.is_empty() {
                    Err(anyhow!("Cannot find minimum of empty dataset"))
                } else {
                    Ok(values.iter().cloned().fold(f64::INFINITY, f64::min))
                }
            },
            AggregationType::Max => {
                if values.is_empty() {
                    Err(anyhow!("Cannot find maximum of empty dataset"))
                } else {
                    Ok(values.iter().cloned().fold(f64::NEG_INFINITY, f64::max))
                }
            },
            AggregationType::Count => Ok(values.len() as f64),
            AggregationType::StandardDeviation => {
                let mean = values.iter().sum::<f64>() / values.len() as f64;
                let variance =
                    values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64;
                Ok(variance.sqrt())
            },
            AggregationType::Median => {
                let mut sorted = values.to_vec();
                sorted.sort_by(|a, b| match a.partial_cmp(b) {
                    Some(ordering) => ordering,
                    None => {
                        tracing::error!("NaN compare: {} vs {}", a, b);
                        std::cmp::Ordering::Equal
                    },
                });
                let mid = sorted.len() / 2;
                if sorted.len().is_multiple_of(2) {
                    Ok((sorted[mid - 1] + sorted[mid]) / 2.0)
                } else {
                    Ok(sorted[mid])
                }
            },
            AggregationType::Percentile { value } => {
                if *value < 0.0 || *value > 100.0 {
                    return Err(anyhow!("Percentile must be between 0 and 100"));
                }
                let mut sorted = values.to_vec();
                sorted.sort_by(|a, b| match a.partial_cmp(b) {
                    Some(ordering) => ordering,
                    None => {
                        tracing::error!("NaN compare: {} vs {}", a, b);
                        std::cmp::Ordering::Equal
                    },
                });
                let index = (value / 100.0 * (sorted.len() - 1) as f64).round() as usize;
                Ok(sorted[index])
            },
            AggregationType::WeightedAverage { weights } => {
                if weights.len() != values.len() {
                    return Err(anyhow!("Weights and values must have same length"));
                }
                let weighted_sum: f64 = values.iter().zip(weights.iter()).map(|(v, w)| v * w).sum();
                let weight_sum: f64 = weights.iter().sum();
                if weight_sum == 0.0 {
                    return Err(anyhow!("Sum of weights cannot be zero"));
                }
                Ok(weighted_sum / weight_sum)
            },
            // Handle new aggregation types not yet implemented
            _ => {
                warn!("Unsupported agg: {:?}", operation);
                Err(anyhow!("Aggregation type not yet implemented"))
            },
        }
    }
}

/// Time-series processor for temporal data analysis
struct TimeSeriesProcessor;

impl TimeSeriesProcessor {
    fn new() -> Self {
        Self
    }

    /// Calculate moving average over a sliding window
    fn calculate_moving_average(&self, series: &[(f64, f64)], window: usize) -> Vec<f64> {
        if series.len() < window {
            return vec![];
        }

        let mut results = Vec::new();
        for i in window..=series.len() {
            let sum: f64 = series[i - window..i].iter().map(|(_, v)| v).sum();
            results.push(sum / window as f64);
        }
        results
    }

    /// Calculate rate of change between consecutive points
    fn calculate_rate_of_change(&self, series: &[(f64, f64)]) -> Vec<f64> {
        if series.len() < 2 {
            return vec![];
        }

        let mut results = Vec::new();
        for i in 1..series.len() {
            let dt = series[i].0 - series[i - 1].0;
            let dv = series[i].1 - series[i - 1].1;
            if dt > 0.0 {
                results.push(dv / dt);
            }
        }
        results
    }
}

/// Energy-specific calculator
struct EnergyCalculator;

impl EnergyCalculator {
    fn new() -> Self {
        Self
    }

    fn calculate(
        &self,
        operation: &EnergyCalculation,
        values: &HashMap<String, f64>,
    ) -> Result<serde_json::Value> {
        match operation {
            EnergyCalculation::PowerBalance => {
                let pv = values.get("pv_power").unwrap_or(&0.0);
                let battery = values.get("battery_power").unwrap_or(&0.0);
                let load = values.get("load_power").unwrap_or(&0.0);
                let grid = values.get("grid_power").unwrap_or(&0.0);

                // Power balance: Sources (pv + battery) - Consumption (load + grid)
                let balance = pv + battery - load - grid;
                Ok(serde_json::json!({
                    "power_balance": balance,
                    "is_balanced": balance.abs() < 0.001,
                    "components": {
                        "pv": pv,
                        "battery": battery,
                        "load": load,
                        "grid": grid
                    }
                }))
            },
            EnergyCalculation::StateOfCharge => {
                let current = values.get("battery_current").unwrap_or(&0.0);
                let voltage = values.get("battery_voltage").unwrap_or(&0.0);
                let capacity = values.get("battery_capacity").unwrap_or(&100.0);
                let previous_soc = values.get("previous_soc").unwrap_or(&50.0);
                let dt = values.get("time_delta").unwrap_or(&1.0); // seconds

                // Coulomb counting method
                let charge_change = current * dt / 3600.0; // Convert to Ah
                let soc_change = (charge_change / capacity) * 100.0;
                let new_soc = (previous_soc + soc_change).clamp(0.0, 100.0);

                Ok(serde_json::json!({
                    "soc": new_soc,
                    "soc_change": soc_change,
                    "energy_stored": new_soc * capacity * voltage / 100.0,
                    "power": current * voltage
                }))
            },
            EnergyCalculation::EnergyEfficiency => {
                let input_power = values.get("input_power").unwrap_or(&0.0);
                let output_power = values.get("output_power").unwrap_or(&0.0);

                let efficiency = if *input_power > 0.0 {
                    (output_power / input_power * 100.0).min(100.0)
                } else {
                    0.0
                };

                let losses = input_power - output_power;

                Ok(serde_json::json!({
                    "efficiency_percent": efficiency,
                    "losses_watts": losses,
                    "input_power": input_power,
                    "output_power": output_power
                }))
            },
            EnergyCalculation::LoadForecast => {
                // Simple load forecast based on historical average
                let current_load = values.get("current_load").unwrap_or(&0.0);
                let avg_load = values.get("avg_load_24h").unwrap_or(&0.0);
                let peak_load = values.get("peak_load_24h").unwrap_or(&0.0);
                let hour_of_day = values.get("hour_of_day").unwrap_or(&12.0);

                // Simple time-based forecast
                let hour_factor = 1.0 + 0.3 * ((hour_of_day - 12.0).abs() / 12.0 - 0.5);
                let forecast = avg_load * hour_factor;

                Ok(serde_json::json!({
                    "forecast_load": forecast,
                    "current_load": current_load,
                    "confidence": 0.75,
                    "peak_probability": forecast / peak_load
                }))
            },
            EnergyCalculation::OptimalDispatch => {
                let pv_available = values.get("pv_available").unwrap_or(&0.0);
                let battery_available = values.get("battery_available").unwrap_or(&0.0);
                let grid_price = values.get("grid_price").unwrap_or(&0.1);
                let load_demand = values.get("load_demand").unwrap_or(&0.0);
                let battery_soc = values.get("battery_soc").unwrap_or(&50.0);

                // Simple dispatch logic
                let pv_dispatch = pv_available.min(*load_demand);
                let mut battery_dispatch = 0.0;
                let mut grid_dispatch = 0.0;

                let remaining = load_demand - pv_dispatch;

                if remaining > 0.0 {
                    // Use battery if SOC > 20% and grid price is high
                    if *battery_soc > 20.0 && *grid_price > 0.15 {
                        battery_dispatch = remaining.min(*battery_available);
                        grid_dispatch = remaining - battery_dispatch;
                    } else {
                        grid_dispatch = remaining;
                    }
                }

                Ok(serde_json::json!({
                    "dispatch": {
                        "pv": pv_dispatch,
                        "battery": battery_dispatch,
                        "grid": grid_dispatch
                    },
                    "total_cost": grid_dispatch * grid_price,
                    "renewable_ratio": (pv_dispatch + battery_dispatch) / load_demand
                }))
            },
            EnergyCalculation::CostOptimization => {
                let energy_consumed = values.get("energy_consumed").unwrap_or(&0.0);
                let peak_demand = values.get("peak_demand").unwrap_or(&0.0);
                let energy_rate = values.get("energy_rate").unwrap_or(&0.1);
                let demand_rate = values.get("demand_rate").unwrap_or(&10.0);
                let solar_generated = values.get("solar_generated").unwrap_or(&0.0);
                let solar_credit_rate = values.get("solar_credit_rate").unwrap_or(&0.08);

                let energy_cost = energy_consumed * energy_rate;
                let demand_cost = peak_demand * demand_rate;
                let solar_credit = solar_generated * solar_credit_rate;
                let total_cost = energy_cost + demand_cost - solar_credit;

                Ok(serde_json::json!({
                    "energy_cost": energy_cost,
                    "demand_cost": demand_cost,
                    "solar_credit": solar_credit,
                    "total_cost": total_cost,
                    "cost_per_kwh": if *energy_consumed > 0.0 {
                        total_cost / energy_consumed
                    } else {
                        0.0
                    }
                }))
            },
            // Handle new energy calculation variants not yet implemented
            _ => {
                warn!("Unsupported energy calc: {:?}", operation);
                Err(anyhow!("Energy calculation type not yet implemented"))
            },
        }
    }
}

/// Helper function to create calculation from JSON
pub fn calculation_from_json(json: serde_json::Value) -> Result<CalculationDef> {
    serde_json::from_value(json).context("Failed to parse calculation definition")
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;

    #[test]
    fn test_expression_evaluator() {
        let evaluator = ExpressionEvaluator::new();
        let mut vars = HashMap::new();
        vars.insert("a".to_string(), 10.0);
        vars.insert("b".to_string(), 5.0);

        let result = evaluator.evaluate("a + b", &vars).unwrap();
        assert_eq!(result, 15.0);

        let result = evaluator.evaluate("a * b", &vars).unwrap();
        assert_eq!(result, 50.0);
    }

    #[test]
    fn test_statistics_processor() {
        let processor = StatisticsProcessor::new();
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];

        let sum = processor.aggregate(&AggregationType::Sum, &values).unwrap();
        assert_eq!(sum, 15.0);

        let avg = processor
            .aggregate(&AggregationType::Average, &values)
            .unwrap();
        assert_eq!(avg, 3.0);

        let min = processor.aggregate(&AggregationType::Min, &values).unwrap();
        assert_eq!(min, 1.0);

        let max = processor.aggregate(&AggregationType::Max, &values).unwrap();
        assert_eq!(max, 5.0);
    }

    #[test]
    fn test_energy_calculator() {
        let calculator = EnergyCalculator::new();
        let mut values = HashMap::new();
        values.insert("pv_power".to_string(), 5000.0);
        values.insert("battery_power".to_string(), 2000.0);
        values.insert("load_power".to_string(), 6000.0);
        values.insert("grid_power".to_string(), -1000.0);

        let result = calculator
            .calculate(&EnergyCalculation::PowerBalance, &values)
            .unwrap();
        let balance = result["power_balance"].as_f64().unwrap();
        // Power balance: pv + battery - load - grid
        // = 5000 + 2000 - 6000 - (-1000) = 2000
        // Positive balance means surplus power (which would be exported to grid)
        assert_eq!(balance, 2000.0);
    }

    #[test]
    fn test_expression_operator_precedence() {
        let evaluator = ExpressionEvaluator::new();
        let vars = HashMap::new();

        // Test multiplication before addition: 2 + 3 * 4 = 14 (not 20)
        let result = evaluator.evaluate("2 + 3 * 4", &vars).unwrap();
        assert_eq!(result, 14.0, "Multiplication should happen before addition");

        // Test division before subtraction: 10 - 6 / 2 = 7 (not 2)
        let result = evaluator.evaluate("10 - 6 / 2", &vars).unwrap();
        assert_eq!(result, 7.0, "Division should happen before subtraction");

        // Test parentheses override: (2 + 3) * 4 = 20
        let result = evaluator.evaluate("(2 + 3) * 4", &vars).unwrap();
        assert_eq!(result, 20.0, "Parentheses should override precedence");

        // Test complex expression: 2 + 3 * 4 - 6 / 2 = 11
        let result = evaluator.evaluate("2 + 3 * 4 - 6 / 2", &vars).unwrap();
        assert_eq!(
            result, 11.0,
            "Complex expression should respect all precedence rules"
        );
    }

    #[test]
    fn test_expression_with_variables() {
        let evaluator = ExpressionEvaluator::new();
        let mut vars = HashMap::new();
        vars.insert("P1".to_string(), 10.0);
        vars.insert("P2".to_string(), 5.0);
        vars.insert("factor".to_string(), 0.5);

        // Test: P1 + P2 * factor = 10 + 5 * 0.5 = 12.5
        let result = evaluator.evaluate("P1 + P2 * factor", &vars).unwrap();
        assert_eq!(
            result, 12.5,
            "Variables with operators should respect precedence"
        );

        // Test: (P1 + P2) * factor = (10 + 5) * 0.5 = 7.5
        let result = evaluator.evaluate("(P1 + P2) * factor", &vars).unwrap();
        assert_eq!(result, 7.5, "Parentheses should work with variables");
    }

    #[test]
    fn test_expression_power_operator() {
        let evaluator = ExpressionEvaluator::new();
        let vars = HashMap::new();

        // Test exponentiation: 2 ^ 3 = 8
        let result = evaluator.evaluate("2 ^ 3", &vars).unwrap();
        assert_eq!(result, 8.0, "Power operator should work");

        // Test power precedence: 2 + 3 ^ 2 = 11 (not 25)
        let result = evaluator.evaluate("2 + 3 ^ 2", &vars).unwrap();
        assert_eq!(
            result, 11.0,
            "Power should have higher precedence than addition"
        );
    }

    #[test]
    fn test_expression_error_handling() {
        let evaluator = ExpressionEvaluator::new();
        let vars = HashMap::new();

        // Test division by zero
        let result = evaluator.evaluate("10 / 0", &vars);
        assert!(result.is_err(), "Division by zero should return error");

        // Test unknown variable
        let result = evaluator.evaluate("unknown_var + 1", &vars);
        assert!(result.is_err(), "Unknown variable should return error");

        // Test invalid syntax
        let result = evaluator.evaluate("2 + + 3", &vars);
        assert!(result.is_err(), "Invalid syntax should return error");
    }

    // ===== CalculationEngine Integration Tests =====

    #[tokio::test]
    async fn test_calculation_engine_new() {
        // Create engine without Redis for testing structure
        let engine = CalculationEngine::with_redis(None);

        // Verify engine was created successfully
        assert!(engine.redis_client.is_none());
    }

    #[tokio::test]
    async fn test_register_calculation_success() {
        let engine = CalculationEngine::with_redis(None);

        let mut variables = HashMap::new();
        variables.insert("P1".to_string(), "modsrv:instance1:M:1".to_string());
        variables.insert("P2".to_string(), "modsrv:instance1:M:2".to_string());

        let calc = CalculationDef {
            id: "calc_001".to_string(),
            name: "Test Calculation".to_string(),
            description: Some("Test description".to_string()),
            calculation_type: CalculationType::Expression {
                formula: "P1 + P2".to_string(),
                variables,
            },
            output_key: "modsrv:calc:result".to_string(),
            trigger: voltage_config::calculations::CalculationTrigger::DataChange {
                watch_keys: vec![
                    "modsrv:instance1:M:1".to_string(),
                    "modsrv:instance1:M:2".to_string(),
                ],
                debounce_ms: None,
            },
            enabled: true,
            priority: None,
            tags: vec![],
            metadata: HashMap::new(),
        };

        let result = engine.register_calculation(calc).await;
        assert!(result.is_ok());

        // Verify calculation was registered
        let calculations = engine.calculations.read().await;
        assert!(calculations.contains_key("calc_001"));
    }

    #[tokio::test]
    async fn test_register_calculation_duplicate() {
        let engine = CalculationEngine::with_redis(None);

        let mut variables = HashMap::new();
        variables.insert("P1".to_string(), "modsrv:instance1:M:1".to_string());
        variables.insert("P2".to_string(), "modsrv:instance1:M:2".to_string());

        let calc = CalculationDef {
            id: "calc_001".to_string(),
            name: "Test Calculation".to_string(),
            description: None,
            calculation_type: CalculationType::Expression {
                formula: "P1 + P2".to_string(),
                variables,
            },
            output_key: "modsrv:calc:result".to_string(),
            trigger: voltage_config::calculations::CalculationTrigger::DataChange {
                watch_keys: vec![
                    "modsrv:instance1:M:1".to_string(),
                    "modsrv:instance1:M:2".to_string(),
                ],
                debounce_ms: None,
            },
            enabled: true,
            priority: None,
            tags: vec![],
            metadata: HashMap::new(),
        };

        // Register first time - should succeed
        assert!(engine.register_calculation(calc.clone()).await.is_ok());

        // Register again - should still succeed (update existing)
        assert!(engine.register_calculation(calc).await.is_ok());
    }

    #[test]
    fn test_calculation_from_json_valid() {
        // CalculationType uses #[serde(tag = "type")] so it needs internal tagging
        let json = serde_json::json!({
            "id": "calc_test",
            "name": "JSON Test",
            "description": "Test from JSON",
            "calculation_type": {
                "type": "expression",
                "formula": "a + b",
                "variables": {
                    "a": "modsrv:test:M:1",
                    "b": "modsrv:test:M:2"
                }
            },
            "output_key": "modsrv:calc:result",
            "trigger": {
                "type": "interval",
                "milliseconds": 1000
            },
            "enabled": true,
            "metadata": {}
        });

        let result = calculation_from_json(json);
        assert!(result.is_ok(), "Failed to parse JSON: {:?}", result.err());

        let calc = result.unwrap();
        assert_eq!(calc.id, "calc_test");
        assert_eq!(calc.name, "JSON Test");
        assert!(calc.enabled);
    }

    #[test]
    fn test_calculation_from_json_invalid() {
        let json = serde_json::json!({
            "id": "calc_test",
            "name": "Incomplete Test"
            // Missing required fields
        });

        let result = calculation_from_json(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_expression_evaluator_basic_operations() {
        let evaluator = ExpressionEvaluator::new();
        let mut vars = HashMap::new();
        vars.insert("x".to_string(), 10.0);
        vars.insert("y".to_string(), 3.0);

        // Test modulo operation
        let result = evaluator.evaluate("x % y", &vars).unwrap();
        assert_eq!(result, 1.0);

        // Test negative numbers
        let result = evaluator.evaluate("-x + y", &vars).unwrap();
        assert_eq!(result, -7.0);

        // Test power operation
        let result = evaluator.evaluate("y ^ 2", &vars).unwrap();
        assert_eq!(result, 9.0);
    }

    #[test]
    fn test_statistics_aggregation_edge_cases() {
        let processor = StatisticsProcessor::new();

        // Test empty array
        let empty: Vec<f64> = vec![];
        let result = processor.aggregate(&AggregationType::Sum, &empty);
        assert!(result.is_err());

        // Test single value
        let single = vec![42.0];
        assert_eq!(
            processor.aggregate(&AggregationType::Sum, &single).unwrap(),
            42.0
        );
        assert_eq!(
            processor
                .aggregate(&AggregationType::Average, &single)
                .unwrap(),
            42.0
        );
        assert_eq!(
            processor.aggregate(&AggregationType::Min, &single).unwrap(),
            42.0
        );
        assert_eq!(
            processor.aggregate(&AggregationType::Max, &single).unwrap(),
            42.0
        );

        // Test negative values
        let negatives = vec![-10.0, -5.0, -15.0];
        assert_eq!(
            processor
                .aggregate(&AggregationType::Sum, &negatives)
                .unwrap(),
            -30.0
        );
        assert_eq!(
            processor
                .aggregate(&AggregationType::Min, &negatives)
                .unwrap(),
            -15.0
        );
        assert_eq!(
            processor
                .aggregate(&AggregationType::Max, &negatives)
                .unwrap(),
            -5.0
        );
    }

    #[test]
    fn test_energy_calculator_power_balance() {
        let calculator = EnergyCalculator::new();

        // Test perfect balance
        let mut values = HashMap::new();
        values.insert("pv_power".to_string(), 1000.0);
        values.insert("battery_power".to_string(), 0.0);
        values.insert("load_power".to_string(), 1000.0);
        values.insert("grid_power".to_string(), 0.0);

        let result = calculator
            .calculate(&EnergyCalculation::PowerBalance, &values)
            .unwrap();
        let balance = result["power_balance"].as_f64().unwrap();
        assert_eq!(balance, 0.0, "Perfect balance should equal zero");

        // Test surplus (negative grid power means export)
        values.insert("pv_power".to_string(), 2000.0);
        values.insert("load_power".to_string(), 1000.0);
        values.insert("battery_power".to_string(), 500.0);

        let result = calculator
            .calculate(&EnergyCalculation::PowerBalance, &values)
            .unwrap();
        let balance = result["power_balance"].as_f64().unwrap();
        assert!(balance != 0.0, "Surplus should create imbalance");
    }

    #[test]
    fn test_expression_complex_nested() {
        let evaluator = ExpressionEvaluator::new();
        let mut vars = HashMap::new();
        vars.insert("a".to_string(), 10.0);
        vars.insert("b".to_string(), 5.0);
        vars.insert("c".to_string(), 2.0);

        // Test: ((a + b) * c) - (a / b) = (15 * 2) - 2 = 28
        let result = evaluator
            .evaluate("((a + b) * c) - (a / b)", &vars)
            .unwrap();
        assert_eq!(
            result, 28.0,
            "Complex nested expression should evaluate correctly"
        );

        // Test: (a ^ 2 + b ^ 2) * c = (100 + 25) * 2 = 250
        let result = evaluator.evaluate("(a ^ 2 + b ^ 2) * c", &vars).unwrap();
        assert_eq!(
            result, 250.0,
            "Power operations should work in nested expressions"
        );

        // Test: Complex precedence: a * b + c ^ 2 - a / b = 50 + 4 - 2 = 52
        let result = evaluator.evaluate("a * b + c ^ 2 - a / b", &vars).unwrap();
        assert_eq!(
            result, 52.0,
            "All operators should respect correct precedence"
        );
    }

    // ========================================================================
    // Ad-hoc calculation tests (no Redis IO during execution)
    // ========================================================================

    #[tokio::test]
    async fn test_execute_expression_values() {
        let engine = CalculationEngine::with_redis(None);

        let mut values = HashMap::new();
        values.insert("x".to_string(), 10.0);
        values.insert("y".to_string(), 3.0);

        let result = engine.execute_expression_values("x * y + 5", &values).await;
        assert!(result.is_ok());

        let json_result = result.unwrap();
        assert_eq!(json_result.as_f64().unwrap(), 35.0);
    }

    #[tokio::test]
    async fn test_execute_aggregation_values() {
        let engine = CalculationEngine::with_redis(None);

        let values = vec![10.0, 20.0, 30.0, 40.0, 50.0];

        // Test Sum
        let result = engine
            .execute_aggregation_values(&AggregationType::Sum, &values)
            .unwrap();
        assert_eq!(result.as_f64().unwrap(), 150.0);

        // Test Average
        let result = engine
            .execute_aggregation_values(&AggregationType::Average, &values)
            .unwrap();
        assert_eq!(result.as_f64().unwrap(), 30.0);

        // Test Min
        let result = engine
            .execute_aggregation_values(&AggregationType::Min, &values)
            .unwrap();
        assert_eq!(result.as_f64().unwrap(), 10.0);

        // Test Max
        let result = engine
            .execute_aggregation_values(&AggregationType::Max, &values)
            .unwrap();
        assert_eq!(result.as_f64().unwrap(), 50.0);
    }

    #[tokio::test]
    async fn test_execute_energy_values() {
        let engine = CalculationEngine::with_redis(None);

        let mut inputs = HashMap::new();
        inputs.insert("pv_power".to_string(), 1500.0);
        inputs.insert("battery_power".to_string(), 200.0);
        inputs.insert("load_power".to_string(), 1200.0);
        inputs.insert("grid_power".to_string(), 100.0);

        let result = engine
            .execute_energy_values(&EnergyCalculation::PowerBalance, &inputs)
            .unwrap();

        eprintln!("Result: {:?}", result);
        let balance = result["power_balance"].as_f64().unwrap();
        eprintln!("Balance: {}", balance);
        // pv + battery - load - grid = 1500 + 200 - 1200 - 100 = 400
        assert!((balance - 400.0).abs() < 0.01);
    }

    // ========================================================================
    // Advanced Statistics Aggregation Tests
    // ========================================================================

    #[test]
    fn test_aggregation_count() {
        let processor = StatisticsProcessor::new();
        let values = vec![10.0, 20.0, 30.0, 40.0, 50.0];

        let result = processor
            .aggregate(&AggregationType::Count, &values)
            .unwrap();
        assert_eq!(result, 5.0, "Count should return number of elements");

        // Empty array
        let empty: Vec<f64> = vec![];
        let result = processor.aggregate(&AggregationType::Count, &empty);
        assert!(result.is_err(), "Count on empty array should fail");
    }

    #[test]
    fn test_aggregation_median_odd() {
        let processor = StatisticsProcessor::new();

        // Odd number of elements
        let values = vec![1.0, 3.0, 5.0, 7.0, 9.0];
        let result = processor
            .aggregate(&AggregationType::Median, &values)
            .unwrap();
        assert_eq!(result, 5.0, "Median of odd elements should be middle value");

        // Unsorted input
        let values = vec![9.0, 1.0, 5.0, 3.0, 7.0];
        let result = processor
            .aggregate(&AggregationType::Median, &values)
            .unwrap();
        assert_eq!(result, 5.0, "Median should work on unsorted data");
    }

    #[test]
    fn test_aggregation_median_even() {
        let processor = StatisticsProcessor::new();

        // Even number of elements
        let values = vec![1.0, 2.0, 3.0, 4.0];
        let result = processor
            .aggregate(&AggregationType::Median, &values)
            .unwrap();
        assert_eq!(
            result, 2.5,
            "Median of even elements should be average of middle two"
        );

        // Two elements
        let values = vec![10.0, 20.0];
        let result = processor
            .aggregate(&AggregationType::Median, &values)
            .unwrap();
        assert_eq!(result, 15.0);
    }

    #[test]
    fn test_aggregation_standard_deviation() {
        let processor = StatisticsProcessor::new();

        // Known standard deviation
        let values = vec![2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0];
        let result = processor
            .aggregate(&AggregationType::StandardDeviation, &values)
            .unwrap();
        // Mean = 5.0, variance = 4.0, std_dev = 2.0
        assert!(
            (result - 2.0).abs() < 0.01,
            "Standard deviation calculation"
        );

        // Zero variance (all same values)
        let values = vec![5.0, 5.0, 5.0, 5.0];
        let result = processor
            .aggregate(&AggregationType::StandardDeviation, &values)
            .unwrap();
        assert_eq!(result, 0.0, "Standard deviation of identical values is 0");
    }

    #[test]
    fn test_aggregation_percentile() {
        let processor = StatisticsProcessor::new();
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0];

        // 25th percentile
        let result = processor
            .aggregate(&AggregationType::Percentile { value: 25.0 }, &values)
            .unwrap();
        assert!(
            (result - 3.0).abs() < 1.0,
            "25th percentile should be around 3"
        );

        // 50th percentile (median)
        let result = processor
            .aggregate(&AggregationType::Percentile { value: 50.0 }, &values)
            .unwrap();
        assert!(
            (result - 5.5).abs() < 1.0,
            "50th percentile should be around median"
        );

        // 95th percentile
        let result = processor
            .aggregate(&AggregationType::Percentile { value: 95.0 }, &values)
            .unwrap();
        assert!(
            (result - 10.0).abs() < 1.0,
            "95th percentile should be near max"
        );
    }

    #[test]
    fn test_percentile_invalid_values() {
        let processor = StatisticsProcessor::new();
        let values = vec![1.0, 2.0, 3.0];

        // Invalid percentiles
        let result = processor.aggregate(&AggregationType::Percentile { value: -1.0 }, &values);
        assert!(result.is_err(), "Negative percentile should return error");

        let result = processor.aggregate(&AggregationType::Percentile { value: 101.0 }, &values);
        assert!(result.is_err(), "Percentile > 100 should return error");
    }

    #[test]
    fn test_aggregation_weighted_average() {
        let processor = StatisticsProcessor::new();
        let values = vec![10.0, 20.0, 30.0];
        let weights = vec![1.0, 2.0, 3.0];

        let result = processor
            .aggregate(&AggregationType::WeightedAverage { weights }, &values)
            .unwrap();
        // (10*1 + 20*2 + 30*3) / (1+2+3) = (10+40+90) / 6 = 140/6 = 23.333...
        assert!(
            (result - 23.333).abs() < 0.01,
            "Weighted average calculation"
        );
    }

    #[test]
    fn test_weighted_average_error_cases() {
        let processor = StatisticsProcessor::new();
        let values = vec![10.0, 20.0, 30.0];

        // Mismatched lengths
        let weights_short = vec![1.0, 2.0];
        let result = processor.aggregate(
            &AggregationType::WeightedAverage {
                weights: weights_short,
            },
            &values,
        );
        assert!(
            result.is_err(),
            "Weighted average with mismatched lengths should fail"
        );

        // Zero sum of weights
        let weights_zero = vec![0.0, 0.0, 0.0];
        let result = processor.aggregate(
            &AggregationType::WeightedAverage {
                weights: weights_zero,
            },
            &values,
        );
        assert!(
            result.is_err(),
            "Weighted average with zero weight sum should fail"
        );
    }

    // ========================================================================
    // Energy Calculation Tests (SOC, Efficiency)
    // ========================================================================

    #[test]
    fn test_state_of_charge_calculation() {
        let calculator = EnergyCalculator::new();

        let mut values = HashMap::new();
        values.insert("battery_current".to_string(), 10.0); // 10A charging
        values.insert("battery_voltage".to_string(), 48.0); // 48V
        values.insert("battery_capacity".to_string(), 100.0); // 100Ah
        values.insert("previous_soc".to_string(), 50.0); // 50% SOC
        values.insert("time_delta".to_string(), 3600.0); // 1 hour in seconds

        let result = calculator
            .calculate(&EnergyCalculation::StateOfCharge, &values)
            .unwrap();

        let soc = result["soc"].as_f64().unwrap();
        // After 1 hour at 10A into 100Ah battery: 50% + (10*1/100)*100 = 60%
        assert!(
            (soc - 60.0).abs() < 0.1,
            "SOC should increase by 10% after 1 hour charging at 10A"
        );

        let power = result["power"].as_f64().unwrap();
        assert_eq!(power, 480.0, "Power should be current * voltage");
    }

    #[test]
    fn test_soc_clamping() {
        let calculator = EnergyCalculator::new();

        // Test upper limit clamping
        let mut values = HashMap::new();
        values.insert("battery_current".to_string(), 50.0); // Large charging current
        values.insert("battery_voltage".to_string(), 48.0);
        values.insert("battery_capacity".to_string(), 100.0);
        values.insert("previous_soc".to_string(), 95.0); // Already high SOC
        values.insert("time_delta".to_string(), 3600.0); // 1 hour

        let result = calculator
            .calculate(&EnergyCalculation::StateOfCharge, &values)
            .unwrap();

        let soc = result["soc"].as_f64().unwrap();
        assert_eq!(soc, 100.0, "SOC should be clamped to 100%");

        // Test lower limit clamping
        values.insert("battery_current".to_string(), -50.0); // Large discharge
        values.insert("previous_soc".to_string(), 5.0); // Low SOC

        let result = calculator
            .calculate(&EnergyCalculation::StateOfCharge, &values)
            .unwrap();

        let soc = result["soc"].as_f64().unwrap();
        assert_eq!(soc, 0.0, "SOC should be clamped to 0%");
    }

    #[test]
    fn test_soc_with_defaults() {
        let calculator = EnergyCalculator::new();

        // Test with missing values (should use defaults)
        let values = HashMap::new(); // Empty - all defaults

        let result = calculator
            .calculate(&EnergyCalculation::StateOfCharge, &values)
            .unwrap();

        let soc = result["soc"].as_f64().unwrap();
        // With all defaults: current=0, previous_soc=50, no change
        assert_eq!(
            soc, 50.0,
            "SOC should remain at default 50% with no current"
        );
    }

    #[test]
    fn test_energy_efficiency_calculation() {
        let calculator = EnergyCalculator::new();

        let mut values = HashMap::new();
        values.insert("input_power".to_string(), 1000.0); // 1000W input
        values.insert("output_power".to_string(), 950.0); // 950W output

        let result = calculator
            .calculate(&EnergyCalculation::EnergyEfficiency, &values)
            .unwrap();

        let efficiency = result["efficiency_percent"].as_f64().unwrap();
        assert_eq!(efficiency, 95.0, "Efficiency should be 95%");

        let losses = result["losses_watts"].as_f64().unwrap();
        assert_eq!(losses, 50.0, "Losses should be 50W");
    }

    #[test]
    fn test_efficiency_zero_input() {
        let calculator = EnergyCalculator::new();

        let mut values = HashMap::new();
        values.insert("input_power".to_string(), 0.0); // No input
        values.insert("output_power".to_string(), 100.0); // Some output (invalid)

        let result = calculator
            .calculate(&EnergyCalculation::EnergyEfficiency, &values)
            .unwrap();

        let efficiency = result["efficiency_percent"].as_f64().unwrap();
        assert_eq!(
            efficiency, 0.0,
            "Efficiency should be 0 when input power is 0"
        );
    }

    #[test]
    fn test_efficiency_edge_cases() {
        let calculator = EnergyCalculator::new();

        // Test efficiency capped at 100%
        let mut values = HashMap::new();
        values.insert("input_power".to_string(), 1000.0);
        values.insert("output_power".to_string(), 1100.0); // Invalid: output > input

        let result = calculator
            .calculate(&EnergyCalculation::EnergyEfficiency, &values)
            .unwrap();

        let efficiency = result["efficiency_percent"].as_f64().unwrap();
        assert_eq!(efficiency, 100.0, "Efficiency should be capped at 100% max");

        // Test negative losses
        let losses = result["losses_watts"].as_f64().unwrap();
        assert!(
            losses < 0.0,
            "Losses can be negative when output > input (error condition)"
        );
    }

    // ========================================================================
    // Time Series Processing Tests
    // ========================================================================

    #[test]
    fn test_time_series_moving_average() {
        let processor = TimeSeriesProcessor::new();

        // Time series data: (timestamp, value)
        let series = vec![
            (1.0, 10.0),
            (2.0, 20.0),
            (3.0, 30.0),
            (4.0, 40.0),
            (5.0, 50.0),
        ];

        // Window size 3: averages [10,20,30], [20,30,40], [30,40,50]
        let result = processor.calculate_moving_average(&series, 3);
        assert_eq!(result.len(), 3, "Should have 3 moving average values");
        assert_eq!(result[0], 20.0, "First average: (10+20+30)/3 = 20");
        assert_eq!(result[1], 30.0, "Second average: (20+30+40)/3 = 30");
        assert_eq!(result[2], 40.0, "Third average: (30+40+50)/3 = 40");
    }

    #[test]
    fn test_moving_average_insufficient_data() {
        let processor = TimeSeriesProcessor::new();

        // Too few data points for window
        let series = vec![(1.0, 10.0), (2.0, 20.0)];
        let result = processor.calculate_moving_average(&series, 5);
        assert!(
            result.is_empty(),
            "Should return empty for insufficient data"
        );
    }

    #[test]
    fn test_moving_average_edge_cases() {
        let processor = TimeSeriesProcessor::new();

        // Window size = 1 (trivial case)
        let series = vec![(1.0, 10.0), (2.0, 20.0), (3.0, 30.0)];
        let result = processor.calculate_moving_average(&series, 1);
        assert_eq!(result.len(), 3);
        assert_eq!(
            result,
            vec![10.0, 20.0, 30.0],
            "Window of 1 returns original values"
        );

        // Window size = data length (single average)
        let result = processor.calculate_moving_average(&series, 3);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], 20.0, "Full window average: (10+20+30)/3 = 20");
    }

    #[test]
    fn test_rate_of_change_calculation() {
        let processor = TimeSeriesProcessor::new();

        // Constant rate of change
        let series = vec![(0.0, 0.0), (1.0, 10.0), (2.0, 20.0), (3.0, 30.0)];

        let result = processor.calculate_rate_of_change(&series);
        assert_eq!(result.len(), 3, "Should have 3 rate values");
        assert_eq!(result[0], 10.0, "Rate 0->1: (10-0)/(1-0) = 10");
        assert_eq!(result[1], 10.0, "Rate 1->2: (20-10)/(2-1) = 10");
        assert_eq!(result[2], 10.0, "Rate 2->3: (30-20)/(3-2) = 10");
    }

    #[test]
    fn test_rate_of_change_varying() {
        let processor = TimeSeriesProcessor::new();

        // Varying rate of change
        let series = vec![
            (0.0, 0.0),
            (2.0, 10.0), // +5/s
            (4.0, 30.0), // +10/s
            (5.0, 35.0), // +5/s
        ];

        let result = processor.calculate_rate_of_change(&series);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0], 5.0, "Rate: 10/2 = 5");
        assert_eq!(result[1], 10.0, "Rate: 20/2 = 10");
        assert_eq!(result[2], 5.0, "Rate: 5/1 = 5");
    }

    #[test]
    fn test_rate_of_change_edge_cases() {
        let processor = TimeSeriesProcessor::new();

        // Insufficient data
        let series = vec![(1.0, 10.0)];
        let result = processor.calculate_rate_of_change(&series);
        assert!(result.is_empty(), "Single point has no rate of change");

        // Zero time delta (should be skipped)
        let series = vec![
            (1.0, 10.0),
            (1.0, 20.0), // Same timestamp - invalid
            (2.0, 30.0),
        ];
        let result = processor.calculate_rate_of_change(&series);
        assert_eq!(result.len(), 1, "Should skip zero time delta");
        assert_eq!(result[0], 10.0, "Only valid rate: (30-20)/(2-1) = 10");
    }

    // ========================================================================
    // Additional Edge Cases and Error Handling Tests
    // ========================================================================

    #[test]
    fn test_aggregation_with_negative_values() {
        let processor = StatisticsProcessor::new();
        let values = vec![-10.0, -5.0, 0.0, 5.0, 10.0];

        assert_eq!(
            processor.aggregate(&AggregationType::Sum, &values).unwrap(),
            0.0
        );
        assert_eq!(
            processor
                .aggregate(&AggregationType::Average, &values)
                .unwrap(),
            0.0
        );
        assert_eq!(
            processor.aggregate(&AggregationType::Min, &values).unwrap(),
            -10.0
        );
        assert_eq!(
            processor.aggregate(&AggregationType::Max, &values).unwrap(),
            10.0
        );
    }

    #[test]
    fn test_expression_with_floating_point_precision() {
        let evaluator = ExpressionEvaluator::new();
        let mut vars = HashMap::new();
        vars.insert("a".to_string(), 0.1);
        vars.insert("b".to_string(), 0.2);

        let result = evaluator.evaluate("a + b", &vars).unwrap();
        // 0.1 + 0.2 might not be exactly 0.3 due to floating point
        assert!((result - 0.3).abs() < 0.0001);
    }

    #[test]
    fn test_expression_very_large_numbers() {
        let evaluator = ExpressionEvaluator::new();
        let mut vars = HashMap::new();
        vars.insert("x".to_string(), 1e15);
        vars.insert("y".to_string(), 1e15);

        let result = evaluator.evaluate("x + y", &vars).unwrap();
        assert_eq!(result, 2e15);
    }

    #[test]
    fn test_aggregation_single_value_all_operations() {
        let processor = StatisticsProcessor::new();
        let values = vec![42.0];

        assert_eq!(
            processor.aggregate(&AggregationType::Sum, &values).unwrap(),
            42.0
        );
        assert_eq!(
            processor
                .aggregate(&AggregationType::Average, &values)
                .unwrap(),
            42.0
        );
        assert_eq!(
            processor.aggregate(&AggregationType::Min, &values).unwrap(),
            42.0
        );
        assert_eq!(
            processor.aggregate(&AggregationType::Max, &values).unwrap(),
            42.0
        );
        assert_eq!(
            processor
                .aggregate(&AggregationType::Median, &values)
                .unwrap(),
            42.0
        );
        assert_eq!(
            processor
                .aggregate(&AggregationType::Count, &values)
                .unwrap(),
            1.0
        );
    }

    #[test]
    fn test_moving_average_large_window() {
        let processor = TimeSeriesProcessor::new();
        let series: Vec<(f64, f64)> = (0..100).map(|i| (i as f64, (i * 2) as f64)).collect();

        // Large window
        let result = processor.calculate_moving_average(&series, 50);
        assert_eq!(result.len(), 51); // 100 - 50 + 1

        // First window average
        let expected_first = (0..50).map(|i| i * 2).sum::<i32>() as f64 / 50.0;
        assert!((result[0] - expected_first).abs() < 0.1);
    }

    #[test]
    fn test_energy_calculations_zero_values() {
        let calculator = EnergyCalculator::new();

        // All zero inputs
        let mut values = HashMap::new();
        values.insert("pv_power".to_string(), 0.0);
        values.insert("battery_power".to_string(), 0.0);
        values.insert("load_power".to_string(), 0.0);
        values.insert("grid_power".to_string(), 0.0);

        let result = calculator
            .calculate(&EnergyCalculation::PowerBalance, &values)
            .unwrap();

        let balance = result["power_balance"].as_f64().unwrap();
        assert_eq!(balance, 0.0, "All zeros should result in zero balance");
        assert!(result["is_balanced"].as_bool().unwrap());
    }

    #[test]
    fn test_soc_discharge_scenario() {
        let calculator = EnergyCalculator::new();

        let mut values = HashMap::new();
        values.insert("battery_current".to_string(), -10.0); // Discharging
        values.insert("battery_voltage".to_string(), 48.0);
        values.insert("battery_capacity".to_string(), 100.0);
        values.insert("previous_soc".to_string(), 50.0);
        values.insert("time_delta".to_string(), 3600.0); // 1 hour

        let result = calculator
            .calculate(&EnergyCalculation::StateOfCharge, &values)
            .unwrap();

        let soc = result["soc"].as_f64().unwrap();
        // 50% - 10% = 40%
        assert!(
            (soc - 40.0).abs() < 0.1,
            "SOC should decrease during discharge"
        );

        let power = result["power"].as_f64().unwrap();
        assert_eq!(power, -480.0, "Power should be negative during discharge");
    }

    #[test]
    fn test_calculation_from_json_missing_fields() {
        let json = serde_json::json!({
            "id": "incomplete",
            "name": "Incomplete Calculation"
            // Missing required fields
        });

        let result = calculation_from_json(json);
        assert!(result.is_err(), "Incomplete JSON should fail");
    }

    #[test]
    fn test_expression_parentheses_and_precedence() {
        let evaluator = ExpressionEvaluator::new();
        let mut vars = HashMap::new();
        vars.insert("x".to_string(), 2.0);
        vars.insert("y".to_string(), 3.0);
        vars.insert("z".to_string(), 4.0);

        // Without parentheses: 2 + 3 * 4 = 14
        let result = evaluator.evaluate("x + y * z", &vars).unwrap();
        assert_eq!(result, 14.0);

        // With parentheses: (2 + 3) * 4 = 20
        let result = evaluator.evaluate("(x + y) * z", &vars).unwrap();
        assert_eq!(result, 20.0);
    }

    #[test]
    fn test_percentile_boundary_values() {
        let processor = StatisticsProcessor::new();
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];

        // 0th percentile (minimum)
        let result = processor
            .aggregate(&AggregationType::Percentile { value: 0.0 }, &values)
            .unwrap();
        assert_eq!(result, 1.0);

        // 100th percentile (maximum)
        let result = processor
            .aggregate(&AggregationType::Percentile { value: 100.0 }, &values)
            .unwrap();
        assert_eq!(result, 5.0);
    }

    #[test]
    fn test_standard_deviation_large_spread() {
        let processor = StatisticsProcessor::new();

        // Large spread
        let values = vec![1.0, 100.0, 1.0, 100.0, 1.0, 100.0];
        let result = processor
            .aggregate(&AggregationType::StandardDeviation, &values)
            .unwrap();
        assert!(result > 40.0, "Large spread should have high std dev");
    }

    #[test]
    fn test_weighted_average_equal_weights() {
        let processor = StatisticsProcessor::new();
        let values = vec![10.0, 20.0, 30.0];
        let weights = vec![1.0, 1.0, 1.0]; // Equal weights

        let result = processor
            .aggregate(&AggregationType::WeightedAverage { weights }, &values)
            .unwrap();

        // Equal weights should give same result as simple average
        assert_eq!(result, 20.0);
    }

    #[test]
    fn test_time_series_negative_rates() {
        let processor = TimeSeriesProcessor::new();

        // Decreasing values (negative rate)
        let series = vec![(0.0, 100.0), (1.0, 80.0), (2.0, 60.0), (3.0, 40.0)];

        let result = processor.calculate_rate_of_change(&series);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0], -20.0, "Should handle negative rates");
        assert_eq!(result[1], -20.0);
        assert_eq!(result[2], -20.0);
    }

    // ========================================================================
    // SQLite Persistence Tests (load_from_sqlite)
    // ========================================================================

    /// Helper: Create in-memory SQLite pool and initialize calculations table
    async fn create_test_sqlite_pool() -> sqlx::SqlitePool {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:")
            .await
            .expect("Failed to create in-memory SQLite pool");

        // Initialize calculations table
        sqlx::query(voltage_config::modsrv::CALCULATIONS_TABLE)
            .execute(&pool)
            .await
            .expect("Failed to create calculations table");

        pool
    }

    #[tokio::test]
    async fn test_load_from_sqlite_empty_db() {
        let pool = create_test_sqlite_pool().await;
        let engine = CalculationEngine::with_redis(None);

        let count = engine.load_from_sqlite(&pool).await.unwrap();
        assert_eq!(count, 0, "Empty database should load 0 calculations");
    }

    #[tokio::test]
    async fn test_load_from_sqlite_with_data() {
        let pool = create_test_sqlite_pool().await;

        // Insert a valid calculation
        let calc_type_json = serde_json::json!({
            "type": "expression",
            "formula": "a + b",
            "variables": {
                "a": "inst:1:M:1",
                "b": "inst:1:M:2"
            }
        });

        sqlx::query(
            "INSERT INTO calculations (calculation_name, description, calculation_type, output_inst, output_type, output_id, enabled)
             VALUES (?, ?, ?, ?, ?, ?, ?)"
        )
        .bind("test_expr_calc")
        .bind("Test expression calculation")
        .bind(calc_type_json.to_string())
        .bind(100_i64)
        .bind("M")
        .bind(1_i64)
        .bind(true)
        .execute(&pool)
        .await
        .expect("Failed to insert test calculation");

        let engine = CalculationEngine::with_redis(None);
        let count = engine.load_from_sqlite(&pool).await.unwrap();

        assert_eq!(count, 1, "Should load 1 calculation");

        // Verify calculation was registered
        let calculations = engine.calculations.read().await;
        assert!(
            calculations.contains_key("test_expr_calc"),
            "Calculation should be registered"
        );

        let calc = calculations.get("test_expr_calc").unwrap();
        assert_eq!(calc.output_key, "inst:100:M:1");
    }

    #[tokio::test]
    async fn test_load_from_sqlite_disabled_ignored() {
        let pool = create_test_sqlite_pool().await;

        // Insert a disabled calculation
        let calc_type_json = serde_json::json!({
            "type": "constant",
            "value": 42
        });

        sqlx::query(
            "INSERT INTO calculations (calculation_name, calculation_type, output_inst, output_type, output_id, enabled)
             VALUES (?, ?, ?, ?, ?, ?)"
        )
        .bind("disabled_calc")
        .bind(calc_type_json.to_string())
        .bind(100_i64)
        .bind("M")
        .bind(1_i64)
        .bind(false) // disabled
        .execute(&pool)
        .await
        .expect("Failed to insert disabled calculation");

        let engine = CalculationEngine::with_redis(None);
        let count = engine.load_from_sqlite(&pool).await.unwrap();

        assert_eq!(count, 0, "Disabled calculations should not be loaded");
    }

    #[tokio::test]
    async fn test_load_from_sqlite_multiple_calculations() {
        let pool = create_test_sqlite_pool().await;

        // Insert multiple calculations
        let types = [
            (
                "calc_expr",
                r#"{"type":"expression","formula":"x * 2","variables":{"x":"inst:1:M:1"}}"#,
            ),
            ("calc_const", r#"{"type":"constant","value":100}"#),
            (
                "calc_agg",
                r#"{"type":"aggregation","operation":"sum","source_keys":["inst:1:M:1","inst:2:M:1"]}"#,
            ),
        ];

        for (name, type_json) in types {
            sqlx::query(
                "INSERT INTO calculations (calculation_name, calculation_type, output_inst, output_type, output_id, enabled)
                 VALUES (?, ?, ?, ?, ?, TRUE)"
            )
            .bind(name)
            .bind(type_json)
            .bind(100_i64)
            .bind("M")
            .bind(name.len() as i64) // Use different point IDs
            .execute(&pool)
            .await
            .expect("Failed to insert calculation");
        }

        let engine = CalculationEngine::with_redis(None);
        let count = engine.load_from_sqlite(&pool).await.unwrap();

        assert_eq!(count, 3, "Should load 3 calculations");

        let calculations = engine.calculations.read().await;
        assert!(calculations.contains_key("calc_expr"));
        assert!(calculations.contains_key("calc_const"));
        assert!(calculations.contains_key("calc_agg"));
    }

    #[tokio::test]
    async fn test_load_from_sqlite_invalid_json_skipped() {
        let pool = create_test_sqlite_pool().await;

        // Insert a calculation with invalid JSON
        sqlx::query(
            "INSERT INTO calculations (calculation_name, calculation_type, output_inst, output_type, output_id, enabled)
             VALUES (?, ?, ?, ?, ?, TRUE)"
        )
        .bind("invalid_calc")
        .bind("not valid json {{{")
        .bind(100_i64)
        .bind("M")
        .bind(1_i64)
        .execute(&pool)
        .await
        .expect("Failed to insert invalid calculation");

        // Also insert a valid one
        let valid_json = r#"{"type":"constant","value":42}"#;
        sqlx::query(
            "INSERT INTO calculations (calculation_name, calculation_type, output_inst, output_type, output_id, enabled)
             VALUES (?, ?, ?, ?, ?, TRUE)"
        )
        .bind("valid_calc")
        .bind(valid_json)
        .bind(100_i64)
        .bind("M")
        .bind(2_i64)
        .execute(&pool)
        .await
        .expect("Failed to insert valid calculation");

        let engine = CalculationEngine::with_redis(None);
        let count = engine.load_from_sqlite(&pool).await.unwrap();

        // Invalid JSON should be skipped, valid one should be loaded
        assert_eq!(count, 1, "Should load only valid calculation");

        let calculations = engine.calculations.read().await;
        assert!(calculations.contains_key("valid_calc"));
        assert!(!calculations.contains_key("invalid_calc"));
    }

    #[tokio::test]
    async fn test_load_from_sqlite_output_key_format() {
        let pool = create_test_sqlite_pool().await;

        // Test both M (measurement) and A (action) output types
        let calc_json = r#"{"type":"constant","value":1}"#;

        sqlx::query(
            "INSERT INTO calculations (calculation_name, calculation_type, output_inst, output_type, output_id, enabled)
             VALUES (?, ?, ?, ?, ?, TRUE)"
        )
        .bind("measurement_output")
        .bind(calc_json)
        .bind(5_i64)
        .bind("M")
        .bind(10_i64)
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO calculations (calculation_name, calculation_type, output_inst, output_type, output_id, enabled)
             VALUES (?, ?, ?, ?, ?, TRUE)"
        )
        .bind("action_output")
        .bind(calc_json)
        .bind(6_i64)
        .bind("A")
        .bind(20_i64)
        .execute(&pool)
        .await
        .unwrap();

        let engine = CalculationEngine::with_redis(None);
        engine.load_from_sqlite(&pool).await.unwrap();

        let calculations = engine.calculations.read().await;

        let m_calc = calculations.get("measurement_output").unwrap();
        assert_eq!(
            m_calc.output_key, "inst:5:M:10",
            "Measurement output key format"
        );

        let a_calc = calculations.get("action_output").unwrap();
        assert_eq!(a_calc.output_key, "inst:6:A:20", "Action output key format");
    }
}
