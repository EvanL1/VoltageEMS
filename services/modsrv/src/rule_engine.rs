//! 规则引擎模块
//!
//! 提供条件判断和动作执行的规则引擎

use crate::error::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// 规则定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub conditions: Vec<Condition>,
    pub actions: Vec<Action>,
}

/// 条件定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Condition {
    pub field: String,
    pub operator: ConditionOperator,
    pub value: Value,
}

/// 条件操作符
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConditionOperator {
    Equals,
    NotEquals,
    GreaterThan,
    LessThan,
    GreaterThanOrEqual,
    LessThanOrEqual,
    Contains,
    NotContains,
}

/// 动作定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    pub action_type: ActionType,
    pub parameters: HashMap<String, Value>,
}

/// 动作类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionType {
    SetValue,
    SendCommand,
    LogMessage,
    Notify,
}

/// 规则引擎
pub struct RuleEngine {
    rules: Vec<Rule>,
}

impl RuleEngine {
    /// 创建新的规则引擎
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    /// 添加规则
    pub fn add_rule(&mut self, rule: Rule) {
        self.rules.push(rule);
    }

    /// 评估所有规则
    pub fn evaluate(&self, context: &HashMap<String, Value>) -> Result<Vec<Action>> {
        let mut actions = Vec::new();

        for rule in &self.rules {
            if !rule.enabled {
                continue;
            }

            if self.evaluate_conditions(&rule.conditions, context) {
                actions.extend(rule.actions.clone());
            }
        }

        Ok(actions)
    }

    /// 评估条件组
    fn evaluate_conditions(
        &self,
        conditions: &[Condition],
        context: &HashMap<String, Value>,
    ) -> bool {
        // 所有条件都必须满足（AND逻辑）
        conditions
            .iter()
            .all(|condition| self.evaluate_condition(condition, context).unwrap_or(false))
    }

    /// 评估单个条件
    fn evaluate_condition(
        &self,
        condition: &Condition,
        context: &HashMap<String, Value>,
    ) -> Result<bool> {
        let field_value = context.get(&condition.field);

        if field_value.is_none() {
            return Ok(false);
        }

        let field_value = field_value.unwrap();

        match condition.operator {
            ConditionOperator::Equals => Ok(field_value == &condition.value),
            ConditionOperator::NotEquals => Ok(field_value != &condition.value),
            ConditionOperator::GreaterThan => {
                if let (Some(a), Some(b)) = (field_value.as_f64(), condition.value.as_f64()) {
                    Ok(a > b)
                } else {
                    Ok(false)
                }
            }
            ConditionOperator::LessThan => {
                if let (Some(a), Some(b)) = (field_value.as_f64(), condition.value.as_f64()) {
                    Ok(a < b)
                } else {
                    Ok(false)
                }
            }
            ConditionOperator::GreaterThanOrEqual => {
                if let (Some(a), Some(b)) = (field_value.as_f64(), condition.value.as_f64()) {
                    Ok(a >= b)
                } else {
                    Ok(false)
                }
            }
            ConditionOperator::LessThanOrEqual => {
                if let (Some(a), Some(b)) = (field_value.as_f64(), condition.value.as_f64()) {
                    Ok(a <= b)
                } else {
                    Ok(false)
                }
            }
            ConditionOperator::Contains => {
                if let (Some(a), Some(b)) = (field_value.as_str(), condition.value.as_str()) {
                    Ok(a.contains(b))
                } else {
                    Ok(false)
                }
            }
            ConditionOperator::NotContains => {
                if let (Some(a), Some(b)) = (field_value.as_str(), condition.value.as_str()) {
                    Ok(!a.contains(b))
                } else {
                    Ok(false)
                }
            }
        }
    }
}

impl Default for RuleEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rule_evaluation() {
        let mut engine = RuleEngine::new();

        let rule = Rule {
            id: "test_rule".to_string(),
            name: "Test Rule".to_string(),
            enabled: true,
            conditions: vec![Condition {
                field: "temperature".to_string(),
                operator: ConditionOperator::GreaterThan,
                value: Value::from(25.0),
            }],
            actions: vec![Action {
                action_type: ActionType::SetValue,
                parameters: HashMap::new(),
            }],
        };

        engine.add_rule(rule);

        let mut context = HashMap::new();
        context.insert("temperature".to_string(), Value::from(30.0));

        let actions = engine.evaluate(&context).unwrap();
        assert_eq!(actions.len(), 1);
    }
}
