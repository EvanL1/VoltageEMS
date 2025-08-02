use crate::error::Result;
use async_trait::async_trait;
use serde_json::Value;

/// Action handler trait for control operations
#[async_trait]
#[allow(dead_code)]
pub trait ActionHandler: Send + Sync {
    /// Get the name of this action handler
    fn name(&self) -> &str;

    /// Get the type of this action handler
    fn handler_type(&self) -> String;

    /// Check if this handler can handle the given action type
    fn can_handle(&self, action_type: &str) -> bool;

    /// Execute an action
    async fn execute_action(&self, action_type: &str, config: &Value) -> Result<String>;
}
