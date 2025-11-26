//! Expression Evaluator
//!
//! Mathematical expression evaluation using evalexpr library.
//! Supports proper operator precedence, parentheses, and variables.

use crate::error::{ModelError, Result};
use evalexpr::ContextWithMutableVariables;
use std::collections::HashMap;

/// Expression evaluator for mathematical formulas
pub struct ExpressionEvaluator;

impl ExpressionEvaluator {
    /// Create new expression evaluator
    pub fn new() -> Self {
        Self
    }

    /// Evaluate a formula with given variables
    ///
    /// # Arguments
    /// * `formula` - Mathematical expression (e.g., "a + b * 2")
    /// * `variables` - Variable name to value mapping
    ///
    /// # Returns
    /// Evaluation result as f64
    ///
    /// # Examples
    /// ```
    /// use voltage_model::ExpressionEvaluator;
    /// use std::collections::HashMap;
    ///
    /// let evaluator = ExpressionEvaluator::new();
    /// let mut vars = HashMap::new();
    /// vars.insert("a".to_string(), 10.0);
    /// vars.insert("b".to_string(), 5.0);
    ///
    /// let result = evaluator.evaluate("a + b * 2", &vars).unwrap();
    /// assert_eq!(result, 20.0); // 10 + 5*2 = 20
    /// ```
    pub fn evaluate(&self, formula: &str, variables: &HashMap<String, f64>) -> Result<f64> {
        let mut context = evalexpr::HashMapContext::new();

        // Add variables to context
        for (name, value) in variables {
            context
                .set_value(name.to_string(), evalexpr::Value::from(*value))
                .map_err(|e| {
                    ModelError::expression(format!("Failed to set variable {}: {}", name, e))
                })?;
        }

        // Evaluate expression with context
        let result = evalexpr::eval_with_context(formula, &context).map_err(|e| {
            ModelError::expression(format!("Failed to evaluate '{}': {}", formula, e))
        })?;

        // Convert result to f64
        match result {
            evalexpr::Value::Float(f) => Ok(f),
            evalexpr::Value::Int(i) => Ok(i as f64),
            _ => Err(ModelError::expression(format!(
                "Expression did not evaluate to a number: {}",
                formula
            ))),
        }
    }
}

impl Default for ExpressionEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_operations() {
        let evaluator = ExpressionEvaluator::new();
        let mut vars = HashMap::new();
        vars.insert("a".to_string(), 10.0);
        vars.insert("b".to_string(), 5.0);

        assert_eq!(evaluator.evaluate("a + b", &vars).unwrap(), 15.0);
        assert_eq!(evaluator.evaluate("a * b", &vars).unwrap(), 50.0);
        assert_eq!(evaluator.evaluate("a - b", &vars).unwrap(), 5.0);
        assert_eq!(evaluator.evaluate("a / b", &vars).unwrap(), 2.0);
    }

    #[test]
    fn test_operator_precedence() {
        let evaluator = ExpressionEvaluator::new();
        let vars = HashMap::new();

        // Multiplication before addition
        assert_eq!(evaluator.evaluate("2 + 3 * 4", &vars).unwrap(), 14.0);
        // Division before subtraction
        assert_eq!(evaluator.evaluate("10 - 6 / 2", &vars).unwrap(), 7.0);
        // Parentheses override
        assert_eq!(evaluator.evaluate("(2 + 3) * 4", &vars).unwrap(), 20.0);
    }

    #[test]
    fn test_power_operator() {
        let evaluator = ExpressionEvaluator::new();
        let vars = HashMap::new();

        assert_eq!(evaluator.evaluate("2 ^ 3", &vars).unwrap(), 8.0);
        assert_eq!(evaluator.evaluate("2 + 3 ^ 2", &vars).unwrap(), 11.0);
    }

    #[test]
    fn test_complex_expressions() {
        let evaluator = ExpressionEvaluator::new();
        let mut vars = HashMap::new();
        vars.insert("a".to_string(), 10.0);
        vars.insert("b".to_string(), 5.0);
        vars.insert("c".to_string(), 2.0);

        // ((a + b) * c) - (a / b) = (15 * 2) - 2 = 28
        assert_eq!(
            evaluator
                .evaluate("((a + b) * c) - (a / b)", &vars)
                .unwrap(),
            28.0
        );
    }

    #[test]
    fn test_error_handling() {
        let evaluator = ExpressionEvaluator::new();
        let vars = HashMap::new();

        // Division by zero
        assert!(evaluator.evaluate("10 / 0", &vars).is_err());
        // Unknown variable
        assert!(evaluator.evaluate("unknown_var + 1", &vars).is_err());
        // Invalid syntax
        assert!(evaluator.evaluate("2 + + 3", &vars).is_err());
    }

    #[test]
    fn test_floating_point() {
        let evaluator = ExpressionEvaluator::new();
        let mut vars = HashMap::new();
        vars.insert("a".to_string(), 0.1);
        vars.insert("b".to_string(), 0.2);

        let result = evaluator.evaluate("a + b", &vars).unwrap();
        assert!((result - 0.3).abs() < 0.0001);
    }
}
