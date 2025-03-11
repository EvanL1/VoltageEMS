//! Virtual Point Calculation Module
//!
//! This module provides calculation capabilities for virtual measurement points
//! based on physical measurement points within the same model.

use anyhow::{anyhow, Context, Result};
use bytes::Bytes;
use chrono::{DateTime, Datelike, Timelike, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info};
use voltage_config::modsrv::RedisKeys;
use voltage_rtdb::Rtdb;

use crate::instance_logger;
use crate::time_series::{TimeSeriesCalculator, TimeSeriesFunction};

// Security limits for expression evaluation
const MAX_EXPRESSION_LENGTH: usize = 256;
const MAX_NESTING_DEPTH: usize = 10;
const MAX_OPERATIONS: usize = 50;
#[allow(dead_code)]
const MAX_RECURSION_DEPTH: usize = 20;

/// Expression validator for security checks
///
/// Critical security component that validates user-provided expressions to prevent:
/// - Code injection attacks
/// - Stack overflow from deeply nested expressions
/// - Denial of service from overly complex expressions
///
/// IMPORTANT: Keep this even if currently unused - it's a security safeguard
#[allow(dead_code)]
pub struct ExpressionValidator {
    max_length: usize,
    max_depth: usize,
    max_operations: usize,
}

#[allow(dead_code)]
impl ExpressionValidator {
    /// Create a new validator with default security limits
    pub fn new() -> Self {
        Self {
            max_length: MAX_EXPRESSION_LENGTH,
            max_depth: MAX_NESTING_DEPTH,
            max_operations: MAX_OPERATIONS,
        }
    }

    /// Validate an expression for security risks
    pub fn validate(&self, expr: &str) -> Result<()> {
        // 1. Check expression length
        if expr.len() > self.max_length {
            return Err(anyhow!(
                "Expression too long: {} > {}. Maximum allowed length is {} characters.",
                expr.len(),
                self.max_length,
                self.max_length
            ));
        }

        // 2. Check nesting depth
        let depth = self.calculate_nesting_depth(expr)?;
        if depth > self.max_depth {
            return Err(anyhow!(
                "Expression nesting too deep: {} > {}. Maximum allowed depth is {}.",
                depth,
                self.max_depth,
                self.max_depth
            ));
        }

        // 3. Count operations
        let ops = self.count_operations(expr);
        if ops > self.max_operations {
            return Err(anyhow!(
                "Too many operations: {} > {}. Maximum allowed operations is {}.",
                ops,
                self.max_operations,
                self.max_operations
            ));
        }

        // 4. Check for dangerous characters and patterns
        self.check_safe_characters(expr)?;

        Ok(())
    }

    /// Calculate the maximum nesting depth of parentheses
    fn calculate_nesting_depth(&self, expr: &str) -> Result<usize> {
        let mut max_depth = 0;
        let mut current_depth = 0;

        for ch in expr.chars() {
            match ch {
                '(' => {
                    current_depth += 1;
                    max_depth = max_depth.max(current_depth);
                },
                ')' => {
                    if current_depth == 0 {
                        return Err(anyhow!(
                            "Mismatched parentheses: too many closing parentheses"
                        ));
                    }
                    current_depth -= 1;
                },
                _ => {},
            }
        }

        if current_depth != 0 {
            return Err(anyhow!(
                "Mismatched parentheses: {} unclosed",
                current_depth
            ));
        }

        Ok(max_depth)
    }

    /// Count the number of operations in the expression
    fn count_operations(&self, expr: &str) -> usize {
        expr.chars()
            .filter(|&ch| matches!(ch, '+' | '-' | '*' | '/'))
            .count()
    }

    /// Check that the expression only contains safe characters
    fn check_safe_characters(&self, expr: &str) -> Result<()> {
        // Allow: letters, numbers, underscore, operators, parentheses, dots, spaces
        let allowed_pattern = regex::Regex::new(r"^[a-zA-Z0-9_+\-*/().\s]+$")?;

        if !allowed_pattern.is_match(expr) {
            // Find the first invalid character for better error message
            for ch in expr.chars() {
                if !matches!(ch, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_' | '+' | '-' | '*' | '/' | '(' | ')' | '.' | ' ')
                {
                    return Err(anyhow!(
                        "Invalid character '{}' in expression. Only alphanumeric, operators (+,-,*,/), parentheses, and dots are allowed.",
                        ch
                    ));
                }
            }
            return Err(anyhow!("Expression contains invalid characters"));
        }

        // Check for dangerous patterns
        if expr.contains(";;") || expr.contains("&&") || expr.contains("||") {
            return Err(anyhow!("Expression contains dangerous operators"));
        }

        // Check for command injection patterns
        if expr.contains("$") || expr.contains("`") || expr.contains("\\") {
            return Err(anyhow!(
                "Expression contains potential injection characters"
            ));
        }

        Ok(())
    }
}

impl Default for ExpressionValidator {
    fn default() -> Self {
        Self::new()
    }
}

/// Virtual point calculation type
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum CalcType {
    #[default]
    Expression,
    TimeSeries,
}

/// Virtual point definition
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VirtualPointDef {
    pub desc: String,
    pub calc: String,
    #[serde(default)]
    pub calc_type: CalcType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_series_config: Option<TimeSeriesConfig>,
}

/// Time series configuration for virtual points
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSeriesConfig {
    pub function: String, // "delta", "moving_avg", "peak", "valley", "integrate"
    pub source: String,   // Source point ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schedule: Option<String>, // Cron expression for reset
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window: Option<u32>, // Window size for moving average (minutes)
}

/// Virtual point calculator for advanced calculations
///
/// This component is temporarily disabled during system migration but will be
/// re-enabled for real-time virtual point calculations. It provides:
/// - Expression evaluation with safety checks
/// - Conditional calculations
/// - Aggregate functions (sum, avg, min, max)
/// - Time-series accumulation
#[allow(dead_code)]
pub struct VirtualCalculator<R: Rtdb> {
    rtdb: Arc<R>,
    time_series_calculator: Arc<RwLock<TimeSeriesCalculator<R>>>,
}

#[allow(dead_code)]
impl<R: Rtdb> VirtualCalculator<R> {
    /// Create new virtual calculator
    pub fn new(rtdb: Arc<R>) -> Self {
        let time_series_calculator = TimeSeriesCalculator::new(rtdb.clone());
        Self {
            rtdb,
            time_series_calculator: Arc::new(RwLock::new(time_series_calculator)),
        }
    }

    /// Calculate virtual point value
    pub async fn calculate(
        &self,
        model_id: &str,
        point_name: &str,
        calc_expr: &str,
        model_data: &HashMap<String, f64>,
    ) -> Result<f64> {
        debug!(
            "Calculating virtual point {} for model {} with expression: {}",
            point_name, model_id, calc_expr
        );

        // Parse and execute different calculation types
        if calc_expr.starts_with("accumulate(") {
            self.calc_accumulate(model_id, point_name, calc_expr, model_data)
                .await
        } else if calc_expr.contains("?") && calc_expr.contains(":") {
            self.calc_conditional(calc_expr, model_data)
        } else if calc_expr.starts_with("sum(")
            || calc_expr.starts_with("avg(")
            || calc_expr.starts_with("max(")
            || calc_expr.starts_with("min(")
        {
            self.calc_aggregate(calc_expr, model_data)
        } else {
            self.calc_expression(calc_expr, model_data)
        }
    }

    /// Calculate accumulation (e.g., daily energy)
    async fn calc_accumulate(
        &self,
        model_id: &str,
        point_name: &str,
        calc_expr: &str,
        model_data: &HashMap<String, f64>,
    ) -> Result<f64> {
        // Parse: accumulate(source_point, condition, options)
        // Example: accumulate(battery_power, when > 0, reset_daily)

        let parts = self.parse_accumulate_expr(calc_expr)?;
        let source_point = &parts.source;
        let condition = &parts.condition;
        let reset_period = &parts.reset_period;

        // Get source value from model data
        let source_value = model_data
            .get(source_point)
            .ok_or_else(|| anyhow!("Source point {} not found in model data", source_point))?;

        // Check if value meets condition
        if !self.evaluate_condition(*source_value, condition)? {
            // Condition not met, return current accumulated value
            return self.get_accumulated_value(model_id, point_name).await;
        }

        // Check if reset is needed
        if self
            .should_reset(model_id, point_name, reset_period)
            .await?
        {
            self.reset_accumulator(model_id, point_name).await?;
        }

        // Calculate increment (convert power to energy: W * seconds / 3600 = Wh)
        let increment = if condition.contains("abs") {
            source_value.abs() / 3600.0
        } else {
            source_value / 3600.0
        };

        // Add to accumulator using get-modify-set pattern
        let accum_key = format!("modsrv:{}:accum:{}", model_id, point_name);

        // Get current value
        let current: f64 = match self.rtdb.get(&accum_key).await? {
            Some(bytes) => String::from_utf8_lossy(&bytes).parse().unwrap_or(0.0),
            None => 0.0,
        };

        // Calculate new value
        let new_value = current + increment;

        // Set new value
        self.rtdb
            .set(&accum_key, Bytes::from(new_value.to_string()))
            .await?;

        Ok(new_value)
    }

    /// Calculate conditional expression
    fn calc_conditional(&self, calc_expr: &str, model_data: &HashMap<String, f64>) -> Result<f64> {
        // Parse: condition ? true_value : false_value
        // Example: battery_soc > 90 ? 1.0 : 0.0

        let parts: Vec<&str> = calc_expr.split("?").collect();
        if parts.len() != 2 {
            return Err(anyhow!("Invalid conditional expression"));
        }

        let condition = parts[0].trim();
        let values: Vec<&str> = parts[1].split(":").collect();
        if values.len() != 2 {
            return Err(anyhow!("Invalid conditional values"));
        }

        let true_val = self.evaluate_value(values[0].trim(), model_data)?;
        let false_val = self.evaluate_value(values[1].trim(), model_data)?;

        // Evaluate condition
        if self.evaluate_expression_condition(condition, model_data)? {
            Ok(true_val)
        } else {
            Ok(false_val)
        }
    }

    /// Calculate aggregate function
    fn calc_aggregate(&self, calc_expr: &str, model_data: &HashMap<String, f64>) -> Result<f64> {
        // Parse: function(point1, point2, ...)
        // Example: sum(load1, load2, load3)

        let (func_name, points) = self.parse_aggregate_expr(calc_expr)?;

        // Collect values
        let mut values = Vec::new();
        for point in points {
            let value = model_data
                .get(&point)
                .ok_or_else(|| anyhow!("Point {} not found in model data", point))?;
            values.push(*value);
        }

        // Calculate based on function
        match func_name.as_str() {
            "sum" => Ok(values.iter().sum()),
            "avg" => Ok(values.iter().sum::<f64>() / values.len() as f64),
            "max" => Ok(values.iter().cloned().fold(f64::NEG_INFINITY, f64::max)),
            "min" => Ok(values.iter().cloned().fold(f64::INFINITY, f64::min)),
            _ => Err(anyhow!("Unknown aggregate function: {}", func_name)),
        }
    }

    /// Calculate mathematical expression
    fn calc_expression(&self, calc_expr: &str, model_data: &HashMap<String, f64>) -> Result<f64> {
        // Simple expression evaluation
        // Example: battery_voltage * battery_current / 1000

        // Validate expression for security before processing
        let validator = ExpressionValidator::new();
        validator
            .validate(calc_expr)
            .context("Expression validation failed")?;

        let mut expr = calc_expr.to_string();

        // Replace point names with values
        for (point_name, value) in model_data {
            // Use word boundaries to avoid partial replacements
            let pattern = format!(r"\b{}\b", regex::escape(point_name));
            expr = regex::Regex::new(&pattern)?
                .replace_all(&expr, value.to_string().as_str())
                .to_string();
        }

        // Evaluate the expression
        self.evaluate_math_expr(&expr)
    }

    /// Parse accumulate expression (static method for testing)
    pub(crate) fn parse_accumulate_expr_static(expr: &str) -> Result<AccumulateParams> {
        // Remove "accumulate(" and ")"
        let content = expr
            .trim_start_matches("accumulate(")
            .trim_end_matches(")")
            .trim();

        let parts: Vec<&str> = content.split(",").map(|s| s.trim()).collect();
        if parts.len() < 2 {
            return Err(anyhow!("Invalid accumulate expression"));
        }

        Ok(AccumulateParams {
            source: parts[0].to_string(),
            condition: parts.get(1).unwrap_or(&"").to_string(),
            reset_period: parts.get(2).unwrap_or(&"never").to_string(),
        })
    }

    /// Parse accumulate expression (instance method wrapper)
    fn parse_accumulate_expr(&self, expr: &str) -> Result<AccumulateParams> {
        Self::parse_accumulate_expr_static(expr)
    }

    /// Parse aggregate expression
    fn parse_aggregate_expr(&self, expr: &str) -> Result<(String, Vec<String>)> {
        let open_paren = expr
            .find('(')
            .ok_or_else(|| anyhow!("Invalid aggregate expression"))?;
        let close_paren = expr
            .rfind(')')
            .ok_or_else(|| anyhow!("Invalid aggregate expression"))?;

        let func_name = expr[..open_paren].trim().to_string();
        let points_str = &expr[open_paren + 1..close_paren];

        let points: Vec<String> = points_str
            .split(',')
            .map(|s| s.trim().to_string())
            .collect();

        Ok((func_name, points))
    }

    /// Evaluate condition for accumulation
    fn evaluate_condition(&self, value: f64, condition: &str) -> Result<bool> {
        if condition.contains("when > 0") {
            Ok(value > 0.0)
        } else if condition.contains("when < 0") {
            Ok(value < 0.0)
        } else if condition.contains("when >= 0") {
            Ok(value >= 0.0)
        } else if condition.contains("when <= 0") {
            Ok(value <= 0.0)
        } else if condition.is_empty() || condition == "always" {
            Ok(true)
        } else {
            Err(anyhow!("Unknown condition: {}", condition))
        }
    }

    /// Evaluate expression condition
    fn evaluate_expression_condition(
        &self,
        condition: &str,
        model_data: &HashMap<String, f64>,
    ) -> Result<bool> {
        // Parse conditions like "battery_soc > 90"
        let operators = vec![">", "<", ">=", "<=", "==", "!="];

        for op in operators {
            if let Some(pos) = condition.find(op) {
                let left = condition[..pos].trim();
                let right = condition[pos + op.len()..].trim();

                let left_val = self.evaluate_value(left, model_data)?;
                let right_val = self.evaluate_value(right, model_data)?;

                return Ok(match op {
                    ">" => left_val > right_val,
                    "<" => left_val < right_val,
                    ">=" => left_val >= right_val,
                    "<=" => left_val <= right_val,
                    "==" => (left_val - right_val).abs() < 1e-6,
                    "!=" => (left_val - right_val).abs() >= 1e-6,
                    _ => false,
                });
            }
        }

        Err(anyhow!("Invalid condition expression: {}", condition))
    }

    /// Evaluate a value (can be a number or point name)
    fn evaluate_value(&self, value_str: &str, model_data: &HashMap<String, f64>) -> Result<f64> {
        // Try to parse as number first
        if let Ok(val) = value_str.parse::<f64>() {
            return Ok(val);
        }

        // Try to get from model data
        model_data
            .get(value_str)
            .copied()
            .ok_or_else(|| anyhow!("Value {} not found", value_str))
    }

    /// Simple math expression evaluator (static method for testing)
    pub fn evaluate_math_expr_static(expr: &str) -> Result<f64> {
        // This is a simplified evaluator for basic operations
        // In production, consider using a proper expression parser library

        // Remove whitespace
        let expr = expr.replace(" ", "");

        // Try to parse as a simple number
        if let Ok(val) = expr.parse::<f64>() {
            return Ok(val);
        }

        // Handle parentheses first
        if expr.contains('(') {
            return Self::evaluate_with_parentheses_safe_static(&expr, 0);
        }

        // Handle operations in order: *, /, +, -
        if let Some(pos) = expr.rfind('+') {
            let left = Self::evaluate_math_expr_static(&expr[..pos])?;
            let right = Self::evaluate_math_expr_static(&expr[pos + 1..])?;
            return Ok(left + right);
        }

        if let Some(pos) = expr.rfind('-') {
            // Check if it's a negative number
            if pos > 0 {
                let left = Self::evaluate_math_expr_static(&expr[..pos])?;
                let right = Self::evaluate_math_expr_static(&expr[pos + 1..])?;
                return Ok(left - right);
            }
        }

        if let Some(pos) = expr.rfind('*') {
            let left = Self::evaluate_math_expr_static(&expr[..pos])?;
            let right = Self::evaluate_math_expr_static(&expr[pos + 1..])?;
            return Ok(left * right);
        }

        if let Some(pos) = expr.rfind('/') {
            let left = Self::evaluate_math_expr_static(&expr[..pos])?;
            let right = Self::evaluate_math_expr_static(&expr[pos + 1..])?;
            if right.abs() < 1e-10 {
                return Err(anyhow!("Division by zero"));
            }
            return Ok(left / right);
        }

        // Try to parse as number again (for negative numbers)
        expr.parse::<f64>()
            .context(format!("Failed to evaluate expression: {}", expr))
    }

    /// Simple math expression evaluator (instance method wrapper)
    fn evaluate_math_expr(&self, expr: &str) -> Result<f64> {
        Self::evaluate_math_expr_static(expr)
    }

    /// Evaluate expression with parentheses (static method with recursion depth protection)
    fn evaluate_with_parentheses_safe_static(expr: &str, depth: usize) -> Result<f64> {
        // Check recursion depth to prevent stack overflow
        if depth > MAX_RECURSION_DEPTH {
            return Err(anyhow!(
                "Maximum recursion depth ({}) exceeded. Expression too complex.",
                MAX_RECURSION_DEPTH
            ));
        }

        let mut result = expr.to_string();

        while let Some(start) = result.rfind('(') {
            let end = result[start..]
                .find(')')
                .map(|p| start + p)
                .ok_or_else(|| anyhow!("Mismatched parentheses"))?;

            let inner = &result[start + 1..end];
            // Pass incremented depth to track recursion
            let inner_result = Self::evaluate_math_expr_with_depth_static(inner, depth + 1)?;

            result = format!("{}{}{}", &result[..start], inner_result, &result[end + 1..]);
        }

        Self::evaluate_math_expr_with_depth_static(&result, depth + 1)
    }

    /// Evaluate expression with parentheses (instance method wrapper)
    fn evaluate_with_parentheses_safe(&self, expr: &str, depth: usize) -> Result<f64> {
        Self::evaluate_with_parentheses_safe_static(expr, depth)
    }

    /// Helper method for evaluate_math_expr with depth tracking (static)
    fn evaluate_math_expr_with_depth_static(expr: &str, depth: usize) -> Result<f64> {
        // Check recursion depth
        if depth > MAX_RECURSION_DEPTH {
            return Err(anyhow!(
                "Maximum recursion depth ({}) exceeded",
                MAX_RECURSION_DEPTH
            ));
        }

        // Remove whitespace
        let expr = expr.replace(" ", "");

        // Try to parse as a simple number
        if let Ok(val) = expr.parse::<f64>() {
            return Ok(val);
        }

        // Handle parentheses first
        if expr.contains('(') {
            return Self::evaluate_with_parentheses_safe_static(&expr, depth + 1);
        }

        // Handle operations in order: *, /, +, -
        if let Some(pos) = expr.rfind('+') {
            let left = Self::evaluate_math_expr_with_depth_static(&expr[..pos], depth + 1)?;
            let right = Self::evaluate_math_expr_with_depth_static(&expr[pos + 1..], depth + 1)?;
            return Ok(left + right);
        }

        if let Some(pos) = expr.rfind('-') {
            // Check if it's a negative number
            if pos > 0 {
                let left = Self::evaluate_math_expr_with_depth_static(&expr[..pos], depth + 1)?;
                let right =
                    Self::evaluate_math_expr_with_depth_static(&expr[pos + 1..], depth + 1)?;
                return Ok(left - right);
            }
        }

        if let Some(pos) = expr.rfind('*') {
            let left = Self::evaluate_math_expr_with_depth_static(&expr[..pos], depth + 1)?;
            let right = Self::evaluate_math_expr_with_depth_static(&expr[pos + 1..], depth + 1)?;
            return Ok(left * right);
        }

        if let Some(pos) = expr.rfind('/') {
            let left = Self::evaluate_math_expr_with_depth_static(&expr[..pos], depth + 1)?;
            let right = Self::evaluate_math_expr_with_depth_static(&expr[pos + 1..], depth + 1)?;
            if right.abs() < 1e-10 {
                return Err(anyhow!("Division by zero"));
            }
            return Ok(left / right);
        }

        // Try to parse as number again (for negative numbers)
        expr.parse::<f64>()
            .context(format!("Failed to evaluate expression: {}", expr))
    }

    /// Helper method for evaluate_math_expr with depth tracking (instance wrapper)
    fn evaluate_math_expr_with_depth(&self, expr: &str, depth: usize) -> Result<f64> {
        Self::evaluate_math_expr_with_depth_static(expr, depth)
    }

    /// Check if accumulator should be reset
    async fn should_reset(
        &self,
        model_id: &str,
        point_name: &str,
        reset_period: &str,
    ) -> Result<bool> {
        let reset_key = format!("modsrv:{}:reset:{}", model_id, point_name);

        // Get last reset time
        let last_reset: Option<i64> = match self.rtdb.get(&reset_key).await? {
            Some(bytes) => Some(String::from_utf8_lossy(&bytes).parse()?),
            None => None,
        };

        let now = Utc::now();

        if let Some(last_reset_ts) = last_reset {
            let last_reset_time = DateTime::<Utc>::from_timestamp(last_reset_ts, 0)
                .ok_or_else(|| anyhow!("Invalid timestamp"))?;

            match reset_period {
                "reset_daily" => {
                    // Reset if day changed
                    Ok(now.date_naive() != last_reset_time.date_naive())
                },
                "reset_hourly" => {
                    // Reset if hour changed
                    Ok(now.hour() != last_reset_time.hour()
                        || now.date_naive() != last_reset_time.date_naive())
                },
                "reset_monthly" => {
                    // Reset if month changed
                    Ok(now.month() != last_reset_time.month()
                        || now.year() != last_reset_time.year())
                },
                _ => Ok(false),
            }
        } else {
            // No previous reset, should reset now
            Ok(true)
        }
    }

    /// Reset accumulator
    async fn reset_accumulator(&self, model_id: &str, point_name: &str) -> Result<()> {
        let accum_key = format!("modsrv:{}:accum:{}", model_id, point_name);
        let reset_key = format!("modsrv:{}:reset:{}", model_id, point_name);

        // Reset accumulator to 0
        self.rtdb.set(&accum_key, Bytes::from("0.0")).await?;

        // Update reset timestamp
        self.rtdb
            .set(&reset_key, Bytes::from(Utc::now().timestamp().to_string()))
            .await?;

        info!("Reset accumulator for {}/{}", model_id, point_name);
        Ok(())
    }

    /// Calculate time series virtual point
    pub async fn calculate_time_series(
        &self,
        model_id: &str,
        point_id: &str,
        config: &TimeSeriesConfig,
        model_data: &HashMap<String, f64>,
    ) -> Result<f64> {
        debug!(
            "Calculating time series point {} for model {} with function: {}",
            point_id, model_id, config.function
        );

        // Get source value
        let source_value = model_data
            .get(&config.source)
            .ok_or_else(|| anyhow!("Source point {} not found in model data", config.source))?;

        // Create time series function based on config
        let ts_function = match config.function.as_str() {
            "delta" => {
                let schedule = config
                    .schedule
                    .as_ref()
                    .ok_or_else(|| anyhow!("Delta function requires schedule"))?;
                TimeSeriesFunction::Delta {
                    schedule: schedule.clone(),
                }
            },
            "moving_avg" => {
                let window = config
                    .window
                    .ok_or_else(|| anyhow!("Moving average requires window size"))?;
                TimeSeriesFunction::MovingAverage {
                    window_minutes: window,
                }
            },
            "peak" => {
                let schedule = config
                    .schedule
                    .as_ref()
                    .ok_or_else(|| anyhow!("Peak function requires schedule"))?;
                TimeSeriesFunction::Peak {
                    schedule: schedule.clone(),
                }
            },
            "valley" => {
                let schedule = config
                    .schedule
                    .as_ref()
                    .ok_or_else(|| anyhow!("Valley function requires schedule"))?;
                TimeSeriesFunction::Valley {
                    schedule: schedule.clone(),
                }
            },
            "integrate" => TimeSeriesFunction::Integration {
                reset_schedule: config.schedule.clone(),
            },
            _ => return Err(anyhow!("Unknown time series function: {}", config.function)),
        };

        // Calculate using time series calculator
        let mut calculator = self.time_series_calculator.write().await;
        calculator
            .calculate(model_id, point_id, *source_value, &ts_function)
            .await
    }

    /// Get current accumulated value
    async fn get_accumulated_value(&self, model_id: &str, point_name: &str) -> Result<f64> {
        let accum_key = format!("modsrv:{}:accum:{}", model_id, point_name);
        let value: f64 = match self.rtdb.get(&accum_key).await? {
            Some(bytes) => String::from_utf8_lossy(&bytes).parse().unwrap_or(0.0),
            None => 0.0,
        };
        Ok(value)
    }
}

/// Parameters for accumulate calculation
#[allow(dead_code)]
pub(crate) struct AccumulateParams {
    source: String,
    condition: String,
    reset_period: String,
}

/// Start real-time virtual point calculation task
///
/// This function spawns a background task that periodically calculates all virtual points
/// for all instances. It reads measurement data from Redis and writes calculated virtual
/// point values back to Redis.
///
/// @param rtdb: Arc<dyn Rtdb> - RTDB for data access
/// @param pool: SqlitePool - Database connection for querying instances and products
/// @param polling_interval_ms: u64 - Interval between calculations in milliseconds
/// @return tokio::task::JoinHandle - Handle to the background task
pub fn start_realtime_virtual_calculation<R: Rtdb + 'static>(
    rtdb: Arc<R>,
    pool: sqlx::SqlitePool,
    polling_interval_ms: u64,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let calculator = VirtualCalculator::new(rtdb);
        let mut interval =
            tokio::time::interval(std::time::Duration::from_millis(polling_interval_ms));

        // Create instance logger tracker registry
        let tracker_registry = instance_logger::create_tracker_registry();

        info!(
            "Virtual point calculation task started with {}ms interval",
            polling_interval_ms
        );

        loop {
            interval.tick().await;

            // Query all instances with their products
            let instances_result = sqlx::query_as::<_, (i32, String, String)>(
                r#"
                SELECT instance_id, instance_name, product_name
                FROM instances
                "#,
            )
            .fetch_all(&pool)
            .await;

            let instances = match instances_result {
                Ok(instances) => instances,
                Err(e) => {
                    tracing::error!("Failed to query instances for virtual calculation: {}", e);
                    continue;
                },
            };

            for (instance_id, instance_name, product_name) in instances {
                // Get virtual points for this product
                let virtual_points_result =
                    sqlx::query_as::<_, (u32, String, String, Option<String>)>(
                        r#"
                    SELECT measurement_id, name, unit, description
                    FROM measurement_points
                    WHERE product_name = ?
                    "#,
                    )
                    .bind(&product_name)
                    .fetch_all(&pool)
                    .await;

                let virtual_points = match virtual_points_result {
                    Ok(points) => points,
                    Err(e) => {
                        tracing::warn!(
                            "Failed to query virtual points for product {}: {}",
                            product_name,
                            e
                        );
                        continue;
                    },
                };

                if virtual_points.is_empty() {
                    continue; // No virtual points for this instance
                }

                // Get all measurement data for this instance from Redis
                let redis_key = RedisKeys::measurement_hash(instance_id as u16);
                let model_data_bytes_result = calculator.rtdb.hash_get_all(&redis_key).await;

                let model_data_str = match model_data_bytes_result {
                    Ok(data_bytes) => data_bytes
                        .into_iter()
                        .map(|(k, v)| (k, String::from_utf8_lossy(&v).to_string()))
                        .collect::<HashMap<String, String>>(),
                    Err(e) => {
                        tracing::warn!(
                            "Failed to get measurement data for instance {}: {}",
                            instance_name,
                            e
                        );
                        continue;
                    },
                };

                // Query measurement_points for this product to get measurement_id → name mapping
                let measurement_map_result = sqlx::query_as::<_, (i32, String)>(
                    r#"
                    SELECT measurement_id, name
                    FROM measurement_points
                    WHERE product_name = ?
                    "#,
                )
                .bind(&product_name)
                .fetch_all(&pool)
                .await;

                let measurement_map: HashMap<u32, String> = match measurement_map_result {
                    Ok(rows) => rows
                        .into_iter()
                        .map(|(measurement_id, name)| (measurement_id as u32, name))
                        .collect(),
                    Err(e) => {
                        tracing::warn!(
                            "Failed to query measurement names for product {}: {}",
                            product_name,
                            e
                        );
                        continue;
                    },
                };

                // Convert String values to f64 AND map index to name
                // Design decision: Only use friendly names. Numeric IDs completely removed from code.
                let mut model_data: HashMap<String, f64> = HashMap::new();
                for (key, value) in model_data_str {
                    if let Ok(val) = value.parse::<f64>() {
                        // Extract index from Redis key (e.g., "1" from key or "modsrv:instance:M:1")
                        let index_str = key.split(':').next_back().unwrap_or(&key);

                        // Try to parse index and map to name
                        if let Ok(index) = index_str.parse::<u32>() {
                            if let Some(name) = measurement_map.get(&index) {
                                // Keep friendly names (e.g., "PV_VOLTAGE")
                                model_data.insert(name.clone(), val);
                                tracing::trace!(
                                    "Instance {}: Loaded point {} ({}) = {}",
                                    instance_name,
                                    index,
                                    name,
                                    val
                                );
                            } else {
                                tracing::warn!(
                                    "Instance {}: Point index {} has no name defined in product {}, skipping",
                                    instance_name,
                                    index,
                                    product_name
                                );
                            }
                        } else {
                            // Keep non-numeric keys (used internally) unchanged
                            model_data.insert(index_str.to_string(), val);
                        }
                    }
                }

                // Check if snapshot is due and log instance data
                let should_snapshot =
                    match instance_logger::should_snapshot(&tracker_registry, &instance_name).await
                    {
                        Ok(true) => true,
                        Ok(false) => false,
                        Err(e) => {
                            tracing::warn!(
                                "Failed to check snapshot status for instance {}: {}",
                                instance_name,
                                e
                            );
                            false
                        },
                    };

                if should_snapshot {
                    // Collect A-point data from Redis
                    let action_key = RedisKeys::action_hash(instance_id as u16);
                    let action_data_bytes = calculator
                        .rtdb
                        .hash_get_all(&action_key)
                        .await
                        .unwrap_or_default();
                    let action_data: HashMap<String, String> = action_data_bytes
                        .into_iter()
                        .map(|(k, v)| (k, String::from_utf8_lossy(&v).to_string()))
                        .collect();

                    // Log snapshot with both M and A data
                    if let Err(e) = instance_logger::log_snapshot(
                        &tracker_registry,
                        &instance_name,
                        &model_data,
                        &action_data,
                    )
                    .await
                    {
                        tracing::warn!(
                            "Failed to log snapshot for instance {}: {}",
                            instance_name,
                            e
                        );
                    }
                }

                // Calculate each virtual point
                for (index, name, _unit, expression_opt) in virtual_points {
                    let expression = match expression_opt {
                        Some(expr) if !expr.is_empty() => expr,
                        _ => {
                            tracing::debug!("Virtual point {} has no expression, skipping", index);
                            continue;
                        },
                    };

                    // Calculate virtual point value
                    match calculator
                        .calculate(&instance_name, &index.to_string(), &expression, &model_data)
                        .await
                    {
                        Ok(value) => {
                            // Write calculated value back to Redis using unified measurement hash
                            let hash_key = RedisKeys::measurement_hash(instance_id as u16);
                            let field = index.to_string();
                            if let Err(e) = calculator
                                .rtdb
                                .hash_set(&hash_key, &field, Bytes::from(value.to_string()))
                                .await
                            {
                                tracing::error!(
                                    "Failed to write virtual point {} for instance {}: {}",
                                    index,
                                    instance_name,
                                    e
                                );
                            } else {
                                // Update local cache so subsequent virtual expressions can reuse it
                                // Design decision: Only use friendly names for consistency with physical points
                                model_data.insert(name.clone(), value);

                                tracing::debug!(
                                    "Calculated virtual point {} ({}) for instance {} = {}",
                                    index,
                                    name,
                                    instance_name,
                                    value
                                );
                            }
                        },
                        Err(e) => {
                            tracing::warn!(
                                "Failed to calculate virtual point {} for instance {}: {}",
                                index,
                                instance_name,
                                e
                            );
                        },
                    }
                }
            }
        }
    })
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;

    #[test]
    fn test_evaluate_math_expr() {
        // Test basic arithmetic operations
        use voltage_rtdb::MemoryRtdb;
        assert_eq!(
            VirtualCalculator::<MemoryRtdb>::evaluate_math_expr_static("2+3").unwrap(),
            5.0
        );
        assert_eq!(
            VirtualCalculator::<MemoryRtdb>::evaluate_math_expr_static("10-3").unwrap(),
            7.0
        );
        assert_eq!(
            VirtualCalculator::<MemoryRtdb>::evaluate_math_expr_static("4*5").unwrap(),
            20.0
        );
        assert_eq!(
            VirtualCalculator::<MemoryRtdb>::evaluate_math_expr_static("15/3").unwrap(),
            5.0
        );

        // Test operator precedence
        assert_eq!(
            VirtualCalculator::<MemoryRtdb>::evaluate_math_expr_static("2+3*4").unwrap(),
            14.0
        );

        // Test parentheses
        assert_eq!(
            VirtualCalculator::<MemoryRtdb>::evaluate_math_expr_static("(2+3)*4").unwrap(),
            20.0
        );
    }

    #[test]
    fn test_expression_validator() {
        let validator = ExpressionValidator::new();

        // Valid expressions
        assert!(validator.validate("a + b * c").is_ok());
        assert!(validator.validate("(x + y) / 2").is_ok());
        assert!(validator
            .validate("battery_voltage * battery_current / 1000")
            .is_ok());

        // Invalid: too long
        let long_expr = "a".repeat(300);
        assert!(validator.validate(&long_expr).is_err());

        // Invalid: too deeply nested
        let nested = "(".repeat(15) + "1" + &")".repeat(15);
        assert!(validator.validate(&nested).is_err());

        // Invalid: dangerous characters
        assert!(validator.validate("a && b").is_err());
        assert!(validator.validate("a; rm -rf /").is_err());
        assert!(validator.validate("a || b").is_err());
        assert!(validator.validate("$HOME").is_err());
        assert!(validator.validate("`cmd`").is_err());

        // Invalid: mismatched parentheses
        assert!(validator.validate("(a + b))").is_err());
        assert!(validator.validate("((a + b)").is_err());
    }

    #[test]
    fn test_parse_accumulate_expr() {
        // Test parsing accumulate expression
        use voltage_rtdb::MemoryRtdb;
        let params = VirtualCalculator::<MemoryRtdb>::parse_accumulate_expr_static(
            "accumulate(battery_power, when > 0, reset_daily)",
        )
        .unwrap();
        assert_eq!(params.source, "battery_power");
        assert_eq!(params.condition, "when > 0");
        assert_eq!(params.reset_period, "reset_daily");
    }
}
