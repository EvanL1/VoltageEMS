//! CalcEngine - Expression evaluator with built-in functions
//!
//! Provides formula evaluation with support for:
//! - Arithmetic: +, -, *, /, ^, %
//! - Comparison: <, >, <=, >=, ==, !=
//! - Logic: &&, ||, !
//! - Built-in functions: integrate, moving_avg, rate_of_change, scale, clamp, etc.

use crate::builtin_functions::{self, BuiltinFunctions};
use crate::error::{CalcError, Result};
use crate::state::StateStore;
use evalexpr::{ContextWithMutableFunctions, ContextWithMutableVariables, Value};
use regex::Regex;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock};

// Pre-compiled regex patterns for stateful function parsing (compiled once, used many times)
// Using expect() for compile-time constant patterns that cannot fail
static RE_INTEGRATE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"integrate\s*\(\s*(\w+)(?:\s*,\s*([0-9.]+))?\s*\)")
        .expect("RE_INTEGRATE: invalid regex pattern")
});
static RE_MOVING_AVG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"moving_avg\s*\(\s*(\w+)\s*,\s*(\d+)\s*\)")
        .expect("RE_MOVING_AVG: invalid regex pattern")
});
static RE_RATE_OF_CHANGE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"rate_of_change\s*\(\s*(\w+)\s*\)")
        .expect("RE_RATE_OF_CHANGE: invalid regex pattern")
});

/// CalcEngine - Formula evaluation engine
///
/// # Example
/// ```ignore
/// use voltage_calc::{CalcEngine, MemoryStateStore};
/// use std::sync::Arc;
///
/// let store = Arc::new(MemoryStateStore::new());
/// let engine = CalcEngine::new(store, "rule_1");
///
/// let mut vars = HashMap::new();
/// vars.insert("P", 1000.0);
///
/// // Simple arithmetic
/// let result = engine.evaluate_simple("P * 2", &vars)?;
///
/// // With stateful functions (async)
/// let energy = engine.evaluate("integrate(P)", &vars).await?;
/// ```
pub struct CalcEngine<S: StateStore> {
    /// Built-in function executor
    builtin: BuiltinFunctions<S>,
}

impl<S: StateStore> CalcEngine<S> {
    /// Create new CalcEngine
    ///
    /// # Arguments
    /// * `state_store` - State storage for stateful functions
    /// * `context` - Context identifier (e.g., rule_id, instance_id)
    pub fn new(state_store: Arc<S>, context: impl Into<String>) -> Self {
        Self {
            builtin: BuiltinFunctions::new(state_store, context),
        }
    }

    /// Evaluate a simple expression (no stateful functions)
    ///
    /// For expressions without integrate/moving_avg/rate_of_change,
    /// this is faster as it doesn't require async.
    ///
    /// Supported stateless functions: scale, clamp, abs, min, max, round, sign
    pub fn evaluate_simple(&self, formula: &str, variables: &HashMap<String, f64>) -> Result<f64> {
        let mut context = evalexpr::HashMapContext::new();

        // Add variables
        for (name, value) in variables {
            context
                .set_value(name.to_string(), Value::from(*value))
                .map_err(|e| {
                    CalcError::expression(format!("Failed to set variable {}: {}", name, e))
                })?;
        }

        // Add stateless built-in functions
        Self::register_stateless_functions(&mut context)?;

        // Evaluate
        let result = evalexpr::eval_with_context(formula, &context).map_err(|e| {
            CalcError::expression(format!("Failed to evaluate '{}': {}", formula, e))
        })?;

        Self::value_to_f64(result, formula)
    }

    /// Evaluate an expression with full function support (async)
    ///
    /// Supports all functions including stateful ones:
    /// - integrate(var) - Time integral
    /// - moving_avg(var, window) - Moving average
    /// - rate_of_change(var) - Rate of change dv/dt
    ///
    /// Note: Function parsing is done via preprocessing, not evalexpr native functions.
    /// This allows async execution of stateful functions.
    pub async fn evaluate(&self, formula: &str, variables: &HashMap<String, f64>) -> Result<f64> {
        // Check for stateful function calls
        let processed_formula = self.process_stateful_functions(formula, variables).await?;

        // Evaluate the processed formula
        self.evaluate_simple(&processed_formula, variables)
    }

    /// Process stateful functions in formula and replace with computed values
    ///
    /// Pattern: function_name(arg1, arg2, ...)
    async fn process_stateful_functions(
        &self,
        formula: &str,
        variables: &HashMap<String, f64>,
    ) -> Result<String> {
        let mut result = formula.to_string();

        // Process integrate(var) or integrate(var, factor)
        result = self.process_integrate(&result, variables).await?;

        // Process moving_avg(var, window)
        result = self.process_moving_avg(&result, variables).await?;

        // Process rate_of_change(var)
        result = self.process_rate_of_change(&result, variables).await?;

        Ok(result)
    }

    /// Process integrate function calls
    ///
    /// Optimized to O(n) by collecting all matches first, then replacing in reverse order
    async fn process_integrate(
        &self,
        formula: &str,
        variables: &HashMap<String, f64>,
    ) -> Result<String> {
        // Collect all matches with their ranges and parameters (single scan)
        let matches: Vec<_> = RE_INTEGRATE
            .captures_iter(formula)
            .filter_map(|caps| {
                let m = caps.get(0)?;
                let var_name = caps.get(1)?.as_str();
                let factor: f64 = caps
                    .get(2)
                    .and_then(|m| m.as_str().parse().ok())
                    .unwrap_or(1.0);
                Some((m.range(), var_name.to_string(), factor))
            })
            .collect();

        if matches.is_empty() {
            return Ok(formula.to_string());
        }

        let mut result = formula.to_string();

        // Process in reverse order to preserve indices
        for (range, var_name, factor) in matches.into_iter().rev() {
            let value = variables
                .get(&var_name)
                .copied()
                .ok_or_else(|| CalcError::variable_not_found(format!("integrate: {}", var_name)))?;

            let integrated = self.builtin.integrate(&var_name, value, factor).await?;
            result.replace_range(range, &integrated.to_string());
        }

        Ok(result)
    }

    /// Process moving_avg function calls
    ///
    /// Optimized to O(n) by collecting all matches first, then replacing in reverse order
    async fn process_moving_avg(
        &self,
        formula: &str,
        variables: &HashMap<String, f64>,
    ) -> Result<String> {
        // Collect all matches with their ranges and parameters (single scan)
        let matches: Vec<_> = RE_MOVING_AVG
            .captures_iter(formula)
            .filter_map(|caps| {
                let m = caps.get(0)?;
                let var_name = caps.get(1)?.as_str();
                let window: usize = caps.get(2)?.as_str().parse().ok()?;
                Some((m.range(), var_name.to_string(), window))
            })
            .collect();

        if matches.is_empty() {
            return Ok(formula.to_string());
        }

        let mut result = formula.to_string();

        // Process in reverse order to preserve indices
        for (range, var_name, window) in matches.into_iter().rev() {
            let value = variables.get(&var_name).copied().ok_or_else(|| {
                CalcError::variable_not_found(format!("moving_avg: {}", var_name))
            })?;

            let avg = self.builtin.moving_avg(&var_name, value, window).await?;
            result.replace_range(range, &avg.to_string());
        }

        Ok(result)
    }

    /// Process rate_of_change function calls
    ///
    /// Optimized to O(n) by collecting all matches first, then replacing in reverse order
    async fn process_rate_of_change(
        &self,
        formula: &str,
        variables: &HashMap<String, f64>,
    ) -> Result<String> {
        // Collect all matches with their ranges and parameters (single scan)
        let matches: Vec<_> = RE_RATE_OF_CHANGE
            .captures_iter(formula)
            .filter_map(|caps| {
                let m = caps.get(0)?;
                let var_name = caps.get(1)?.as_str();
                Some((m.range(), var_name.to_string()))
            })
            .collect();

        if matches.is_empty() {
            return Ok(formula.to_string());
        }

        let mut result = formula.to_string();

        // Process in reverse order to preserve indices
        for (range, var_name) in matches.into_iter().rev() {
            let value = variables.get(&var_name).copied().ok_or_else(|| {
                CalcError::variable_not_found(format!("rate_of_change: {}", var_name))
            })?;

            let rate = self.builtin.rate_of_change(&var_name, value).await?;
            result.replace_range(range, &rate.to_string());
        }

        Ok(result)
    }

    /// Register stateless functions with evalexpr context
    fn register_stateless_functions(context: &mut evalexpr::HashMapContext) -> Result<()> {
        use evalexpr::{EvalexprError, Function};

        // Helper to convert Value to f64 (handles both Int and Float)
        fn to_f64(value: &Value) -> std::result::Result<f64, EvalexprError> {
            match value {
                Value::Float(f) => Ok(*f),
                Value::Int(i) => Ok(*i as f64),
                _ => Err(EvalexprError::expected_number(value.clone())),
            }
        }

        // scale(value, factor)
        context
            .set_function(
                "scale".to_string(),
                Function::new(|args| {
                    let tuple = args.as_tuple()?;
                    let value = to_f64(&tuple[0])?;
                    let factor = to_f64(&tuple[1])?;
                    Ok(Value::Float(builtin_functions::scale(value, factor)))
                }),
            )
            .map_err(|e| CalcError::expression(format!("Failed to register scale: {}", e)))?;

        // clamp(value, min, max)
        context
            .set_function(
                "clamp".to_string(),
                Function::new(|args| {
                    let tuple = args.as_tuple()?;
                    let value = to_f64(&tuple[0])?;
                    let min = to_f64(&tuple[1])?;
                    let max = to_f64(&tuple[2])?;
                    Ok(Value::Float(builtin_functions::clamp(value, min, max)))
                }),
            )
            .map_err(|e| CalcError::expression(format!("Failed to register clamp: {}", e)))?;

        // abs(value)
        context
            .set_function(
                "abs".to_string(),
                Function::new(|args| {
                    let value = to_f64(args)?;
                    Ok(Value::Float(builtin_functions::abs(value)))
                }),
            )
            .map_err(|e| CalcError::expression(format!("Failed to register abs: {}", e)))?;

        // min(a, b)
        context
            .set_function(
                "min".to_string(),
                Function::new(|args| {
                    let tuple = args.as_tuple()?;
                    let a = to_f64(&tuple[0])?;
                    let b = to_f64(&tuple[1])?;
                    Ok(Value::Float(builtin_functions::min(a, b)))
                }),
            )
            .map_err(|e| CalcError::expression(format!("Failed to register min: {}", e)))?;

        // max(a, b)
        context
            .set_function(
                "max".to_string(),
                Function::new(|args| {
                    let tuple = args.as_tuple()?;
                    let a = to_f64(&tuple[0])?;
                    let b = to_f64(&tuple[1])?;
                    Ok(Value::Float(builtin_functions::max(a, b)))
                }),
            )
            .map_err(|e| CalcError::expression(format!("Failed to register max: {}", e)))?;

        // round(value, decimals)
        context
            .set_function(
                "round".to_string(),
                Function::new(|args| {
                    let tuple = args.as_tuple()?;
                    let value = to_f64(&tuple[0])?;
                    let decimals_i64 = tuple[1].as_int()?;
                    // Clamp to valid i32 range for decimal places
                    let decimals = decimals_i64.clamp(i32::MIN as i64, i32::MAX as i64) as i32;
                    Ok(Value::Float(builtin_functions::round(value, decimals)))
                }),
            )
            .map_err(|e| CalcError::expression(format!("Failed to register round: {}", e)))?;

        // sign(value)
        context
            .set_function(
                "sign".to_string(),
                Function::new(|args| {
                    let value = to_f64(args)?;
                    Ok(Value::Float(builtin_functions::sign(value)))
                }),
            )
            .map_err(|e| CalcError::expression(format!("Failed to register sign: {}", e)))?;

        // if(condition, then, else) - conditional expression
        // Note: evalexpr already has "if" built-in, but adding explicit support
        // The syntax is: if(condition, then_value, else_value)

        Ok(())
    }

    /// Convert evalexpr Value to f64
    fn value_to_f64(value: Value, formula: &str) -> Result<f64> {
        match value {
            Value::Float(f) => Ok(f),
            Value::Int(i) => Ok(i as f64),
            Value::Boolean(b) => Ok(if b { 1.0 } else { 0.0 }),
            _ => Err(CalcError::expression(format!(
                "Expression did not evaluate to a number: {}",
                formula
            ))),
        }
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
#[allow(clippy::approx_constant)]
mod tests {
    use super::*;
    use crate::state::MemoryStateStore;
    use std::sync::Arc;

    fn create_engine() -> CalcEngine<MemoryStateStore> {
        let store = Arc::new(MemoryStateStore::new());
        CalcEngine::new(store, "test")
    }

    #[test]
    fn test_basic_arithmetic() {
        let engine = create_engine();
        let mut vars = HashMap::new();
        vars.insert("a".to_string(), 10.0);
        vars.insert("b".to_string(), 5.0);

        assert_eq!(engine.evaluate_simple("a + b", &vars).unwrap(), 15.0);
        assert_eq!(engine.evaluate_simple("a * b", &vars).unwrap(), 50.0);
        assert_eq!(engine.evaluate_simple("a - b", &vars).unwrap(), 5.0);
        assert_eq!(engine.evaluate_simple("a / b", &vars).unwrap(), 2.0);
    }

    #[test]
    fn test_operator_precedence() {
        let engine = create_engine();
        let vars = HashMap::new();

        // 2 + 3 * 4 = 2 + 12 = 14
        assert_eq!(engine.evaluate_simple("2 + 3 * 4", &vars).unwrap(), 14.0);
        // (2 + 3) * 4 = 20
        assert_eq!(engine.evaluate_simple("(2 + 3) * 4", &vars).unwrap(), 20.0);
    }

    #[test]
    fn test_builtin_scale() {
        let engine = create_engine();
        let vars = HashMap::new();

        assert_eq!(
            engine.evaluate_simple("scale(100, 0.5)", &vars).unwrap(),
            50.0
        );
    }

    #[test]
    fn test_builtin_clamp() {
        let engine = create_engine();
        let vars = HashMap::new();

        assert_eq!(
            engine.evaluate_simple("clamp(50, 0, 100)", &vars).unwrap(),
            50.0
        );
        assert_eq!(
            engine.evaluate_simple("clamp(-10, 0, 100)", &vars).unwrap(),
            0.0
        );
        assert_eq!(
            engine.evaluate_simple("clamp(150, 0, 100)", &vars).unwrap(),
            100.0
        );
    }

    #[test]
    fn test_builtin_abs() {
        let engine = create_engine();
        let vars = HashMap::new();

        assert_eq!(engine.evaluate_simple("abs(-5)", &vars).unwrap(), 5.0);
        assert_eq!(engine.evaluate_simple("abs(5)", &vars).unwrap(), 5.0);
    }

    #[test]
    fn test_builtin_min_max() {
        let engine = create_engine();
        let vars = HashMap::new();

        assert_eq!(engine.evaluate_simple("min(10, 5)", &vars).unwrap(), 5.0);
        assert_eq!(engine.evaluate_simple("max(10, 5)", &vars).unwrap(), 10.0);
    }

    #[test]
    fn test_builtin_round() {
        let engine = create_engine();
        let vars = HashMap::new();

        assert_eq!(
            engine.evaluate_simple("round(3.14159, 2)", &vars).unwrap(),
            3.14
        );
    }

    #[test]
    fn test_builtin_sign() {
        let engine = create_engine();
        let vars = HashMap::new();

        assert_eq!(engine.evaluate_simple("sign(10)", &vars).unwrap(), 1.0);
        assert_eq!(engine.evaluate_simple("sign(-10)", &vars).unwrap(), -1.0);
        assert_eq!(engine.evaluate_simple("sign(0)", &vars).unwrap(), 0.0);
    }

    #[test]
    fn test_complex_expression() {
        let engine = create_engine();
        let mut vars = HashMap::new();
        vars.insert("P".to_string(), 1000.0);
        vars.insert("efficiency".to_string(), 0.95);

        // P * efficiency
        assert_eq!(
            engine.evaluate_simple("P * efficiency", &vars).unwrap(),
            950.0
        );

        // clamp(P * 1.1, 0, 1000) - limit power increase
        let result = engine
            .evaluate_simple("clamp(P * 1.1, 0, 1000)", &vars)
            .unwrap();
        assert_eq!(result, 1000.0);
    }

    #[tokio::test]
    async fn test_integrate_in_formula() {
        let store = Arc::new(MemoryStateStore::new());
        let engine = CalcEngine::new(store, "test");

        let mut vars = HashMap::new();
        vars.insert("P".to_string(), 1000.0);

        // First call returns 0
        let result = engine.evaluate("integrate(P)", &vars).await.unwrap();
        assert_eq!(result, 0.0);
    }

    #[tokio::test]
    async fn test_moving_avg_in_formula() {
        let store = Arc::new(MemoryStateStore::new());
        let engine = CalcEngine::new(store, "test");

        let mut vars = HashMap::new();
        vars.insert("T".to_string(), 25.0);

        // moving_avg(T, 10)
        let result = engine.evaluate("moving_avg(T, 10)", &vars).await.unwrap();
        assert_eq!(result, 25.0); // First value = average
    }

    #[tokio::test]
    async fn test_rate_of_change_in_formula() {
        let store = Arc::new(MemoryStateStore::new());
        let engine = CalcEngine::new(store, "test");

        let mut vars = HashMap::new();
        vars.insert("V".to_string(), 220.0);

        // rate_of_change(V)
        let result = engine.evaluate("rate_of_change(V)", &vars).await.unwrap();
        assert_eq!(result, 0.0); // First call returns 0
    }

    #[tokio::test]
    async fn test_mixed_formula() {
        let store = Arc::new(MemoryStateStore::new());
        let engine = CalcEngine::new(store, "test");

        let mut vars = HashMap::new();
        vars.insert("P".to_string(), 1000.0);

        // integrate(P) + P * 0.1
        let result = engine
            .evaluate("integrate(P) + P * 0.1", &vars)
            .await
            .unwrap();
        assert_eq!(result, 100.0); // 0 + 1000 * 0.1
    }
}
