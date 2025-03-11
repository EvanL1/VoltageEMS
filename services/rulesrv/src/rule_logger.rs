//! Rule Execution Logger Module
//!
//! Logs rule execution history:
//! - Rule trigger events with condition evaluation results
//! - Action execution details and results
//! - Execution performance metrics
//! - Error tracking

use std::collections::HashMap;
use tracing::warn;

use crate::rule_engine::ExecutionResult;

/// Log rule execution result to file
pub fn log_rule_execution(result: &ExecutionResult) {
    let rule_id = &result.rule_id;

    // Build execution summary
    let success_count = result.actions_executed.iter().filter(|a| a.success).count();
    let total_actions = result.actions_executed.len();

    let conditions_status = if result.conditions_met {
        "MET"
    } else {
        "NOT_MET"
    };

    // Build action details
    let mut action_lines = Vec::new();
    for action_result in &result.actions_executed {
        let status = if action_result.success {
            "SUCCESS"
        } else {
            "FAILED"
        };

        // Format result as string
        let result_str = action_result
            .result
            .as_ref()
            .map(|v| v.to_string())
            .unwrap_or_else(|| "null".to_string());

        let detail = if let Some(error) = &action_result.error {
            format!(
                "- {}: {} [{}] error: {}",
                action_result.action_type, result_str, status, error
            )
        } else {
            format!(
                "- {}: {} [{}]",
                action_result.action_type, result_str, status
            )
        };

        action_lines.push(detail);
    }

    // Build metadata if present
    let metadata = extract_metadata_from_actions(&result.actions_executed);
    let metadata_str = if !metadata.is_empty() {
        let mut parts: Vec<String> = metadata
            .iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect();
        parts.sort();
        format!(", {}", parts.join(", "))
    } else {
        String::new()
    };

    let error_info = if let Some(error) = &result.error {
        format!("\n  Error: {}", error)
    } else {
        String::new()
    };

    let message = format!(
        "TRIGGERED | execution_id={}, duration_ms={}{}
  Conditions: {}
  Actions executed: {}/{} success
{}{}",
        result.timestamp.format("%Y%m%d_%H%M%S").to_string() + "_" + rule_id,
        result.duration_ms,
        metadata_str,
        conditions_status,
        success_count,
        total_actions,
        action_lines.join("\n"),
        error_info
    );

    if let Err(e) = common::logging::write_to_rule_log(rule_id, &message) {
        warn!("Failed to write rule execution log for {}: {}", rule_id, e);
    }
}

/// Log rule evaluation that didn't trigger
pub fn log_rule_evaluation(rule_id: &str, reason: &str, duration_ms: u64) {
    let message = format!("EVALUATED | reason={}, duration_ms={}", reason, duration_ms);

    if let Err(e) = common::logging::write_to_rule_log(rule_id, &message) {
        warn!("Failed to write rule evaluation log for {}: {}", rule_id, e);
    }
}

/// Log rule execution error
pub fn log_rule_error(rule_id: &str, error: &str, duration_ms: u64) {
    let message = format!(
        "ERROR | error={}, duration_ms={}",
        error.replace('\n', " "),
        duration_ms
    );

    if let Err(e) = common::logging::write_to_rule_log(rule_id, &message) {
        warn!("Failed to write rule error log for {}: {}", rule_id, e);
    }
}

/// Extract metadata from action results
fn extract_metadata_from_actions(
    actions: &[crate::rule_engine::ActionResult],
) -> HashMap<String, String> {
    let mut metadata = HashMap::new();

    for action in actions {
        // Parse action result for metadata if it's a string
        if let Some(result_value) = &action.result {
            if let Some(result_str) = result_value.as_str() {
                if let Some((key, value)) = parse_action_metadata(result_str) {
                    metadata.insert(key, value);
                }
            }
        }
    }

    metadata
}

/// Parse action result string for metadata (e.g., "instance=battery_01")
fn parse_action_metadata(result: &str) -> Option<(String, String)> {
    if let Some((key, value)) = result.split_once('=') {
        Some((key.trim().to_string(), value.trim().to_string()))
    } else {
        None
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;
    use crate::rule_engine::{ActionResult, ExecutionResult};

    #[test]
    fn test_log_rule_execution() {
        let result = ExecutionResult {
            rule_id: "test_rule".to_string(),
            timestamp: chrono::Utc::now(),
            conditions_met: true,
            actions_executed: vec![
                ActionResult {
                    action_type: "set_value".to_string(),
                    result: Some(serde_json::Value::String(
                        "instance=battery_01, point=10, value=1".to_string(),
                    )),
                    success: true,
                    error: None,
                },
                ActionResult {
                    action_type: "log_message".to_string(),
                    result: Some(serde_json::Value::String("message=Test log".to_string())),
                    success: true,
                    error: None,
                },
            ],
            duration_ms: 125,
            error: None,
        };

        // Should not panic
        log_rule_execution(&result);
    }

    #[test]
    fn test_log_rule_evaluation() {
        // Should not panic
        log_rule_evaluation("test_rule", "conditions_not_met", 10);
    }

    #[test]
    fn test_log_rule_error() {
        // Should not panic
        log_rule_error("test_rule", "Redis connection failed", 50);
    }

    #[test]
    fn test_extract_metadata_from_actions() {
        let actions = vec![
            ActionResult {
                action_type: "set_value".to_string(),
                result: Some(serde_json::Value::String("instance=battery_01".to_string())),
                success: true,
                error: None,
            },
            ActionResult {
                action_type: "log_message".to_string(),
                result: Some(serde_json::Value::String("no metadata here".to_string())),
                success: true,
                error: None,
            },
        ];

        let metadata = extract_metadata_from_actions(&actions);
        assert_eq!(metadata.get("instance"), Some(&"battery_01".to_string()));
    }

    #[test]
    fn test_parse_action_metadata() {
        assert_eq!(
            parse_action_metadata("instance=battery_01"),
            Some(("instance".to_string(), "battery_01".to_string()))
        );
        assert_eq!(parse_action_metadata("no equals sign"), None);
    }
}
