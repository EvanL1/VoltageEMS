//! Condition Evaluator Module
//!
//! This module provides condition evaluation functionality for the rule engine.
//! It supports complex conditional logic with AND/OR operators and various comparison operations.

use crate::rule_engine::{
    AggregateField, AggregateFunction, ComparisonOperator, Condition, ConditionGroup,
    ExecutionContext, LogicalOperator, ModsrvField,
};
use anyhow::{anyhow, Context, Result};
use regex::Regex;
use serde_json::Value;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use tracing::{debug, trace};

/// Condition evaluator for processing rule conditions
pub struct ConditionEvaluator {
    /// Cache for compiled regex patterns
    regex_cache: HashMap<String, Regex>,

    /// RTDB for fetching data
    rtdb: Arc<dyn voltage_rtdb::Rtdb>,
}

impl ConditionEvaluator {
    /// Create a new condition evaluator
    pub fn new(rtdb: Arc<dyn voltage_rtdb::Rtdb>) -> Self {
        Self {
            regex_cache: HashMap::new(),
            rtdb,
        }
    }

    /// Evaluate a condition group
    pub fn evaluate_condition_group<'a>(
        &'a mut self,
        condition_group: &'a ConditionGroup,
        context: &'a ExecutionContext,
    ) -> Pin<Box<dyn Future<Output = Result<bool>> + Send + 'a>> {
        Box::pin(async move {
            match condition_group {
                ConditionGroup::Single(condition) => {
                    self.evaluate_condition(condition, context).await
                },
                ConditionGroup::Group { logic, rules } => {
                    self.evaluate_logical_group(*logic, rules, context).await
                },
            }
        })
    }

    /// Evaluate a logical group of conditions
    async fn evaluate_logical_group(
        &mut self,
        logic: LogicalOperator,
        conditions: &[ConditionGroup],
        context: &ExecutionContext,
    ) -> Result<bool> {
        if conditions.is_empty() {
            return Ok(true);
        }

        match logic {
            LogicalOperator::And => {
                // All conditions must be true
                for condition in conditions {
                    if !self.evaluate_condition_group(condition, context).await? {
                        trace!("AND condition failed, short-circuiting");
                        return Ok(false);
                    }
                }
                Ok(true)
            },
            LogicalOperator::Or => {
                // At least one condition must be true
                for condition in conditions {
                    if self.evaluate_condition_group(condition, context).await? {
                        trace!("OR condition succeeded, short-circuiting");
                        return Ok(true);
                    }
                }
                Ok(false)
            },
        }
    }

    /// Evaluate a single condition
    async fn evaluate_condition(
        &mut self,
        condition: &Condition,
        context: &ExecutionContext,
    ) -> Result<bool> {
        // Get the field value
        let field_value = self.get_field_value(&condition.field, context).await?;

        // Get the comparison value
        let compare_value = if let Some(ref value_ref) = condition.value_ref {
            // Dynamic value from another field
            self.get_field_value(value_ref, context).await?
        } else if let Some(ref value) = condition.value {
            // Static value
            value.clone()
        } else {
            return Err(anyhow!("Condition must have either value or value_ref"));
        };

        // Perform comparison
        let result = self.compare_values(&field_value, &compare_value, condition.operator)?;

        debug!(
            "Condition evaluation: {} {} {:?} = {}",
            condition.field,
            match condition.operator {
                ComparisonOperator::GreaterThan => ">",
                ComparisonOperator::LessThan => "<",
                ComparisonOperator::GreaterThanOrEqual => ">=",
                ComparisonOperator::LessThanOrEqual => "<=",
                ComparisonOperator::Equal => "==",
                ComparisonOperator::NotEqual => "!=",
                ComparisonOperator::InRange => "in",
                ComparisonOperator::NotInRange => "not_in",
                ComparisonOperator::Contains => "contains",
                ComparisonOperator::Matches => "matches",
            },
            compare_value,
            result
        );

        Ok(result)
    }

    /// Get field value from context or Redis
    async fn get_field_value(&mut self, field: &str, context: &ExecutionContext) -> Result<Value> {
        // Check if it's an aggregate function
        if let Some(aggregate_field) = AggregateField::parse(field) {
            return self.evaluate_aggregate(&aggregate_field, context).await;
        }

        // Check if it's a modsrv field reference
        if let Some(modsrv_field) = ModsrvField::parse(field) {
            let redis_key = modsrv_field.to_redis_key();
            return self.fetch_from_rtdb(&redis_key).await;
        }

        // First check context data for legacy fields
        if let Some(value) = context.data.get(field) {
            return Ok(value.clone());
        }

        // Then try to fetch from RTDB (legacy format)
        let value = self.fetch_from_rtdb(field).await?;
        Ok(value)
    }

    /// Evaluate aggregate function on modsrv instances
    ///
    /// New ID-based implementation:
    /// 1. SCAN inst:*:name to get all instance name keys
    /// 2. GET each name and apply regex filter  
    /// 3. Extract instance IDs from matching keys
    /// 4. Query data from inst:{id}:{M|A} hashes
    async fn evaluate_aggregate(
        &mut self,
        aggregate_field: &AggregateField,
        _context: &ExecutionContext,
    ) -> Result<Value> {
        // Step 1: SCAN all inst:*:name keys
        let name_pattern = "inst:*:name";
        let name_keys = self.rtdb.scan_match(name_pattern).await.context(format!(
            "Failed to scan keys with pattern: {}",
            name_pattern
        ))?;

        if name_keys.is_empty() {
            debug!("No instance name keys found");
            return Ok(Value::Number(serde_json::Number::from(0)));
        }

        // Step 2: Build regex from wildcard pattern
        let pattern_regex = self.wildcard_to_regex(&aggregate_field.instance_pattern)?;

        // Step 3: Filter instances by regex and extract IDs
        let mut matching_instance_ids = Vec::new();
        for key in &name_keys {
            // Extract instance_id from key format "inst:100:name"
            let parts: Vec<&str> = key.split(':').collect();
            if parts.len() != 3 || parts[0] != "inst" || parts[2] != "name" {
                trace!("Skipping invalid name key format: {}", key);
                continue;
            }

            let instance_id: u16 = match parts[1].parse() {
                Ok(id) => id,
                Err(_) => {
                    trace!("Failed to parse instance_id from key: {}", key);
                    continue;
                },
            };

            // GET instance name
            let name_bytes = match self.rtdb.get(key).await? {
                Some(bytes) => bytes,
                None => {
                    trace!("Instance name key exists but has no value: {}", key);
                    continue;
                },
            };

            let instance_name = String::from_utf8_lossy(&name_bytes);

            // Apply regex filter
            if pattern_regex.is_match(&instance_name) {
                matching_instance_ids.push(instance_id);
            }
        }

        if matching_instance_ids.is_empty() {
            debug!(
                "No instances match pattern: {}",
                aggregate_field.instance_pattern
            );
            return Ok(Value::Number(serde_json::Number::from(0)));
        }

        debug!(
            "Found {} instances matching pattern '{}': {:?}",
            matching_instance_ids.len(),
            aggregate_field.instance_pattern,
            matching_instance_ids
        );

        // Step 4: Query data from matching instances
        let mut values = Vec::new();
        for instance_id in &matching_instance_ids {
            let hash_key = format!("inst:{}:{}", instance_id, aggregate_field.point_type);

            let result = self
                .rtdb
                .hash_get(&hash_key, &aggregate_field.point_id.to_string())
                .await
                .context(format!(
                    "Failed to get hash field {}:{}",
                    hash_key, aggregate_field.point_id
                ))?;

            if let Some(val_bytes) = result {
                let val_str = String::from_utf8(val_bytes.to_vec())
                    .context("Failed to convert bytes to string")?;
                if let Ok(num) = val_str.parse::<f64>() {
                    values.push(num);
                }
            }
        }

        // Step 5: Apply aggregate function
        let result = match aggregate_field.function {
            AggregateFunction::Sum => values.iter().sum::<f64>(),
            AggregateFunction::Average => {
                if values.is_empty() {
                    0.0
                } else {
                    values.iter().sum::<f64>() / values.len() as f64
                }
            },
            AggregateFunction::Max => values.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b)),
            AggregateFunction::Min => values.iter().fold(f64::INFINITY, |a, &b| a.min(b)),
            AggregateFunction::Count => values.len() as f64,
        };

        Ok(Value::Number(
            serde_json::Number::from_f64(result).unwrap_or_else(|| serde_json::Number::from(0)),
        ))
    }

    /// Fetch value from RTDB
    async fn fetch_from_rtdb(&self, field: &str) -> Result<Value> {
        // Parse field format: "prefix:key:field" or "prefix:key"
        let parts: Vec<&str> = field.split(':').collect();

        let value = if parts.len() >= 3 {
            // Hash field access
            let key = parts[0..parts.len() - 1].join(":");
            let hash_field = parts[parts.len() - 1];

            let result = self
                .rtdb
                .hash_get(&key, hash_field)
                .await
                .context(format!("Failed to get hash field {}:{}", key, hash_field))?;

            result
                .and_then(|bytes| String::from_utf8(bytes.to_vec()).ok())
                .map(Value::String)
        } else {
            // Simple key access
            let result = self
                .rtdb
                .get(field)
                .await
                .context(format!("Failed to get key {}", field))?;

            result
                .and_then(|bytes| String::from_utf8(bytes.to_vec()).ok())
                .map(Value::String)
        };

        value.ok_or_else(|| anyhow!("Field not found in RTDB: {}", field))
    }

    /// Compare two values based on operator
    fn compare_values(
        &mut self,
        left: &Value,
        right: &Value,
        operator: ComparisonOperator,
    ) -> Result<bool> {
        match operator {
            ComparisonOperator::GreaterThan => self.compare_numeric(left, right, |a, b| a > b),
            ComparisonOperator::LessThan => self.compare_numeric(left, right, |a, b| a < b),
            ComparisonOperator::GreaterThanOrEqual => {
                self.compare_numeric(left, right, |a, b| a >= b)
            },
            ComparisonOperator::LessThanOrEqual => self.compare_numeric(left, right, |a, b| a <= b),
            ComparisonOperator::Equal => Ok(self.values_equal(left, right)),
            ComparisonOperator::NotEqual => Ok(!self.values_equal(left, right)),
            ComparisonOperator::InRange => self.check_in_array(left, right),
            ComparisonOperator::NotInRange => Ok(!self.check_in_array(left, right)?),
            ComparisonOperator::Contains => self.check_contains(left, right),
            ComparisonOperator::Matches => self.check_regex_match(left, right),
        }
    }

    /// Compare numeric values
    fn compare_numeric<F>(&self, left: &Value, right: &Value, op: F) -> Result<bool>
    where
        F: Fn(f64, f64) -> bool,
    {
        let left_num = self.to_number(left)?;
        let right_num = self.to_number(right)?;
        Ok(op(left_num, right_num))
    }

    /// Convert value to number
    fn to_number(&self, value: &Value) -> Result<f64> {
        match value {
            Value::Number(n) => n.as_f64().ok_or_else(|| anyhow!("Invalid number format")),
            Value::String(s) => s
                .parse::<f64>()
                .context(format!("Cannot parse '{}' as number", s)),
            Value::Bool(b) => Ok(if *b { 1.0 } else { 0.0 }),
            _ => Err(anyhow!("Cannot convert {:?} to number", value)),
        }
    }

    /// Check if two values are equal
    fn values_equal(&self, left: &Value, right: &Value) -> bool {
        match (left, right) {
            (Value::Number(l), Value::Number(r)) => {
                if let (Some(lf), Some(rf)) = (l.as_f64(), r.as_f64()) {
                    (lf - rf).abs() < f64::EPSILON
                } else {
                    l == r
                }
            },
            (Value::String(l), Value::String(r)) => l == r,
            (Value::Bool(l), Value::Bool(r)) => l == r,
            (Value::Null, Value::Null) => true,
            _ => {
                // Try to convert both to strings for comparison
                format!("{:?}", left) == format!("{:?}", right)
            },
        }
    }

    /// Check if value is in array
    fn check_in_array(&self, value: &Value, array: &Value) -> Result<bool> {
        match array {
            Value::Array(arr) => Ok(arr.iter().any(|item| self.values_equal(value, item))),
            _ => Err(anyhow!("Right operand must be an array for 'in' operator")),
        }
    }

    /// Check if string contains substring
    fn check_contains(&self, left: &Value, right: &Value) -> Result<bool> {
        let left_str = match left {
            Value::String(s) => s.clone(),
            _ => format!("{:?}", left),
        };

        let right_str = match right {
            Value::String(s) => s.clone(),
            _ => format!("{:?}", right),
        };

        Ok(left_str.contains(&right_str))
    }

    /// Check if value matches regex pattern
    fn check_regex_match(&mut self, value: &Value, pattern: &Value) -> Result<bool> {
        let value_str = match value {
            Value::String(s) => s.clone(),
            _ => format!("{:?}", value),
        };

        let pattern_str = match pattern {
            Value::String(s) => s.clone(),
            _ => return Err(anyhow!("Regex pattern must be a string")),
        };

        // Use cached regex if available
        let regex = if let Some(cached) = self.regex_cache.get(&pattern_str) {
            cached
        } else {
            let compiled = Regex::new(&pattern_str)
                .context(format!("Invalid regex pattern: {}", pattern_str))?;
            self.regex_cache.insert(pattern_str.clone(), compiled);
            self.regex_cache.get(&pattern_str).ok_or_else(|| {
                anyhow!(
                    "Failed to retrieve cached regex for pattern: {}",
                    pattern_str
                )
            })?
        };

        Ok(regex.is_match(&value_str))
    }

    /// Convert wildcard pattern to regex
    ///
    /// Transforms shell-style wildcards (* and ?) to regex patterns
    /// and caches the compiled regex for reuse.
    ///
    /// Examples:
    /// - "pv_*" → "^pv_.*$"
    /// - "battery_?" → "^battery_.$"
    /// - "inv.01" → "^inv\\.01$" (dots escaped)
    fn wildcard_to_regex(&mut self, pattern: &str) -> Result<Regex> {
        if !self.regex_cache.contains_key(pattern) {
            // Convert wildcard pattern to regex
            let regex_pattern = pattern
                .replace(".", "\\.")  // Escape dots
                .replace("*", ".*")   // * → .*
                .replace("?", "."); // ? → .

            let full_pattern = format!("^{}$", regex_pattern);
            let compiled = Regex::new(&full_pattern)
                .context(format!("Failed to compile regex from pattern: {}", pattern))?;

            self.regex_cache.insert(pattern.to_string(), compiled);
        }

        self.regex_cache
            .get(pattern)
            .cloned()
            .ok_or_else(|| anyhow!("Failed to retrieve cached regex for pattern: {}", pattern))
    }
}

/// Helper function to evaluate conditions with a new evaluator
#[allow(dead_code)]
pub async fn evaluate_conditions(
    condition_group: &ConditionGroup,
    context: &ExecutionContext,
    rtdb: Arc<dyn voltage_rtdb::Rtdb>,
) -> Result<bool> {
    let mut evaluator = ConditionEvaluator::new(rtdb);
    evaluator
        .evaluate_condition_group(condition_group, context)
        .await
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;
    use crate::rule_engine::Condition;

    fn create_test_context() -> ExecutionContext {
        let mut data = HashMap::new();
        data.insert("temperature".to_string(), serde_json::json!(75.5));
        data.insert("pressure".to_string(), serde_json::json!(100.0));
        data.insert("status".to_string(), Value::String("active".to_string()));
        data.insert(
            "flags".to_string(),
            Value::Array(vec![
                Value::String("flag1".to_string()),
                Value::String("flag2".to_string()),
            ]),
        );

        ExecutionContext {
            timestamp: chrono::Utc::now(),
            execution_id: "test-exec-1".to_string(),
            data,
            history: vec![],
            data_history: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn test_simple_condition() {
        let condition = Condition {
            field: "temperature".to_string(),
            operator: ComparisonOperator::GreaterThan,
            value: Some(serde_json::json!(70.0)),
            value_ref: None,
        };

        let context = create_test_context();
        let rtdb: Arc<dyn voltage_rtdb::Rtdb> = Arc::new(voltage_rtdb::MemoryRtdb::new());
        let mut evaluator = ConditionEvaluator::new(rtdb);

        let result = evaluator
            .evaluate_condition(&condition, &context)
            .await
            .unwrap();
        assert!(result);
    }

    #[tokio::test]
    async fn test_string_equality() {
        let condition = Condition {
            field: "status".to_string(),
            operator: ComparisonOperator::Equal,
            value: Some(Value::String("active".to_string())),
            value_ref: None,
        };

        let context = create_test_context();
        let rtdb: Arc<dyn voltage_rtdb::Rtdb> = Arc::new(voltage_rtdb::MemoryRtdb::new());
        let mut evaluator = ConditionEvaluator::new(rtdb);

        let result = evaluator
            .evaluate_condition(&condition, &context)
            .await
            .unwrap();
        assert!(result);
    }

    #[tokio::test]
    async fn test_in_operator() {
        let condition = Condition {
            field: "status".to_string(),
            operator: ComparisonOperator::InRange,
            value: Some(Value::Array(vec![
                Value::String("active".to_string()),
                Value::String("pending".to_string()),
            ])),
            value_ref: None,
        };

        let context = create_test_context();
        let rtdb: Arc<dyn voltage_rtdb::Rtdb> = Arc::new(voltage_rtdb::MemoryRtdb::new());
        let mut evaluator = ConditionEvaluator::new(rtdb);

        let result = evaluator
            .evaluate_condition(&condition, &context)
            .await
            .unwrap();
        assert!(result);
    }

    #[tokio::test]
    async fn test_and_logic() {
        let condition_group = ConditionGroup::Group {
            logic: LogicalOperator::And,
            rules: vec![
                ConditionGroup::Single(Condition {
                    field: "temperature".to_string(),
                    operator: ComparisonOperator::GreaterThan,
                    value: Some(serde_json::json!(70.0)),
                    value_ref: None,
                }),
                ConditionGroup::Single(Condition {
                    field: "pressure".to_string(),
                    operator: ComparisonOperator::LessThanOrEqual,
                    value: Some(serde_json::json!(100.0)),
                    value_ref: None,
                }),
            ],
        };

        let context = create_test_context();
        let rtdb: Arc<dyn voltage_rtdb::Rtdb> = Arc::new(voltage_rtdb::MemoryRtdb::new());
        let mut evaluator = ConditionEvaluator::new(rtdb);

        let result = evaluator
            .evaluate_condition_group(&condition_group, &context)
            .await
            .unwrap();
        assert!(result);
    }

    #[tokio::test]
    async fn test_or_logic() {
        let condition_group = ConditionGroup::Group {
            logic: LogicalOperator::Or,
            rules: vec![
                ConditionGroup::Single(Condition {
                    field: "temperature".to_string(),
                    operator: ComparisonOperator::GreaterThan,
                    value: Some(serde_json::json!(100.0)), // False
                    value_ref: None,
                }),
                ConditionGroup::Single(Condition {
                    field: "status".to_string(),
                    operator: ComparisonOperator::Equal,
                    value: Some(Value::String("active".to_string())), // True
                    value_ref: None,
                }),
            ],
        };

        let context = create_test_context();
        let rtdb: Arc<dyn voltage_rtdb::Rtdb> = Arc::new(voltage_rtdb::MemoryRtdb::new());
        let mut evaluator = ConditionEvaluator::new(rtdb);

        let result = evaluator
            .evaluate_condition_group(&condition_group, &context)
            .await
            .unwrap();
        assert!(result);
    }
}
