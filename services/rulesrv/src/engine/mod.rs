pub mod executor;
pub mod simple_engine;

pub use executor::RuleExecutor;
pub use simple_engine::{
    ActionConfig, ActionType, ComparisonOperator, Condition, ConditionGroup, LogicOperator,
    RuleAction, /* RuleExecutionResult, */ SimpleRule, SimpleRuleEngine,
};
