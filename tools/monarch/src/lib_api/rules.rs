//! Rules API - Library Mode
//!
//! Direct library calls for rule management and execution
//! Note: rulesrv has been merged into modsrv (port 6003)

use crate::context::ModsrvContext;
use crate::lib_api::{LibApiError, Result};
use serde::{Deserialize, Serialize};
use voltage_config::rulesrv::Rule;

/// Rule summary for list operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleSummary {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub priority: u32,
}

/// Type alias for rule database row to avoid clippy::type_complexity warning
/// Fields: (id, name, description, enabled, priority, cooldown_ms, variables_json, nodes_json, start_node_id, flow_json)
type RuleDbRow = (
    String,
    String,
    Option<String>,
    i64,
    i64,
    i64,
    String,
    String,
    String,
    Option<String>,
);

/// Rules service - provides rule management and execution operations
/// Uses ModsrvContext since rulesrv has been merged into modsrv
pub struct RulesService<'a> {
    ctx: &'a ModsrvContext,
}

impl<'a> RulesService<'a> {
    /// Create a new rules service from modsrv context
    /// (rulesrv has been merged into modsrv)
    pub fn new(ctx: &'a ModsrvContext) -> Self {
        Self { ctx }
    }

    /// List all rules
    ///
    /// Returns a list of all configured rules.
    pub async fn list(&self) -> Result<Vec<RuleSummary>> {
        // Query database for rules
        let db_rules: Vec<(String, String, i64, i64)> = sqlx::query_as(
            "SELECT id, name, enabled, priority FROM rules ORDER BY priority DESC, id",
        )
        .fetch_all(&self.ctx.sqlite_pool)
        .await?;

        let summaries: Vec<RuleSummary> = db_rules
            .into_iter()
            .map(|(id, name, enabled, priority)| RuleSummary {
                id,
                name,
                enabled: enabled != 0,
                priority: priority as u32,
            })
            .collect();

        Ok(summaries)
    }

    /// Get rule by ID
    ///
    /// Returns detailed information about a specific rule.
    pub async fn get(&self, rule_id: &str) -> Result<Rule> {
        use voltage_config::rulesrv::{FlowNode, Variable};

        // Query database for rule
        let row: Option<RuleDbRow> = sqlx::query_as(
            "SELECT id, name, description, enabled, priority, cooldown_ms,
                    variables_json, nodes_json, start_node_id, flow_json
             FROM rules WHERE id = ?",
        )
        .bind(rule_id)
        .fetch_optional(&self.ctx.sqlite_pool)
        .await?;

        let (
            id,
            name,
            description,
            enabled,
            priority,
            cooldown_ms,
            variables_json,
            nodes_json,
            start_node_id,
            flow_json_opt,
        ) = row.ok_or_else(|| LibApiError::not_found(format!("Rule '{}' not found", rule_id)))?;

        // Deserialize parsed structures
        let variables: Vec<Variable> = serde_json::from_str(&variables_json)
            .map_err(|e| LibApiError::config(format!("Invalid variables JSON: {}", e)))?;
        let nodes: Vec<FlowNode> = serde_json::from_str(&nodes_json)
            .map_err(|e| LibApiError::config(format!("Invalid nodes JSON: {}", e)))?;

        let flow_json = flow_json_opt.and_then(|s| serde_json::from_str(&s).ok());

        Ok(Rule {
            id,
            name,
            description,
            enabled: enabled != 0,
            priority: priority as u32,
            cooldown_ms: cooldown_ms as u64,
            variables,
            nodes,
            start_node_id,
            flow_json,
        })
    }

    /// Create a new rule
    ///
    /// Creates a new rule in the database.
    /// Public API - use monarch sync for CLI operations.
    #[allow(dead_code)]
    pub async fn create(&self, rule: Rule) -> Result<()> {
        let variables_json = serde_json::to_string(&rule.variables)
            .map_err(|e| LibApiError::config(format!("Failed to serialize variables: {}", e)))?;
        let nodes_json = serde_json::to_string(&rule.nodes)
            .map_err(|e| LibApiError::config(format!("Failed to serialize nodes: {}", e)))?;
        let flow_json = rule
            .flow_json
            .map(|v| serde_json::to_string(&v))
            .transpose()
            .map_err(|e| LibApiError::config(format!("Failed to serialize flow_json: {}", e)))?;

        // Insert into database
        sqlx::query(
            "INSERT INTO rules (id, name, description, enabled, priority, cooldown_ms,
                                      variables_json, nodes_json, start_node_id, flow_json)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&rule.id)
        .bind(&rule.name)
        .bind(&rule.description)
        .bind(rule.enabled)
        .bind(rule.priority as i64)
        .bind(rule.cooldown_ms as i64)
        .bind(&variables_json)
        .bind(&nodes_json)
        .bind(&rule.start_node_id)
        .bind(flow_json)
        .execute(&self.ctx.sqlite_pool)
        .await?;

        Ok(())
    }

    /// Update an existing rule
    ///
    /// Updates a rule's configuration in the database.
    /// Public API - use monarch sync for CLI operations.
    #[allow(dead_code)]
    pub async fn update(&self, rule_id: &str, rule: Rule) -> Result<()> {
        let variables_json = serde_json::to_string(&rule.variables)
            .map_err(|e| LibApiError::config(format!("Failed to serialize variables: {}", e)))?;
        let nodes_json = serde_json::to_string(&rule.nodes)
            .map_err(|e| LibApiError::config(format!("Failed to serialize nodes: {}", e)))?;
        let flow_json = rule
            .flow_json
            .map(|v| serde_json::to_string(&v))
            .transpose()
            .map_err(|e| LibApiError::config(format!("Failed to serialize flow_json: {}", e)))?;

        let result = sqlx::query(
            "UPDATE rules SET name = ?, description = ?, enabled = ?, priority = ?,
                    cooldown_ms = ?, variables_json = ?, nodes_json = ?, start_node_id = ?,
                    flow_json = ?, updated_at = CURRENT_TIMESTAMP
             WHERE id = ?",
        )
        .bind(&rule.name)
        .bind(&rule.description)
        .bind(rule.enabled)
        .bind(rule.priority as i64)
        .bind(rule.cooldown_ms as i64)
        .bind(&variables_json)
        .bind(&nodes_json)
        .bind(&rule.start_node_id)
        .bind(flow_json)
        .bind(rule_id)
        .execute(&self.ctx.sqlite_pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(LibApiError::not_found(format!("Rule '{}' not found", rule_id)).into());
        }

        Ok(())
    }

    /// Delete a rule
    ///
    /// Removes a rule from the database.
    /// Public API - use monarch sync for CLI operations.
    #[allow(dead_code)]
    pub async fn delete(&self, rule_id: &str) -> Result<()> {
        let result = sqlx::query("DELETE FROM rules WHERE id = ?")
            .bind(rule_id)
            .execute(&self.ctx.sqlite_pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(LibApiError::not_found(format!("Rule '{}' not found", rule_id)).into());
        }

        Ok(())
    }

    /// Enable a rule
    ///
    /// Sets a rule's enabled flag to true.
    pub async fn enable(&self, rule_id: &str) -> Result<()> {
        let result = sqlx::query(
            "UPDATE rules SET enabled = 1, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
        )
        .bind(rule_id)
        .execute(&self.ctx.sqlite_pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(LibApiError::not_found(format!("Rule '{}' not found", rule_id)).into());
        }

        Ok(())
    }

    /// Disable a rule
    ///
    /// Sets a rule's enabled flag to false.
    pub async fn disable(&self, rule_id: &str) -> Result<()> {
        let result = sqlx::query(
            "UPDATE rules SET enabled = 0, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
        )
        .bind(rule_id)
        .execute(&self.ctx.sqlite_pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(LibApiError::not_found(format!("Rule '{}' not found", rule_id)).into());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    // Note: Tests would require a full service context setup
    // For now, we'll skip unit tests and rely on integration tests
}
