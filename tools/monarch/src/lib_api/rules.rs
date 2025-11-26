//! Rules API - Library Mode
//!
//! Direct library calls to rulesrv for rule chain management and execution

use crate::context::RulesrvContext;
use crate::lib_api::{LibApiError, Result};
use serde::{Deserialize, Serialize};
use voltage_config::rulesrv::RuleChain;

/// Rule summary for list operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleSummary {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub priority: u32,
}

/// Type alias for rule chain database row to avoid clippy::type_complexity warning
/// Fields: (id, name, description, enabled, priority, cooldown_ms, variables_json, nodes_json, start_node_id, flow_json)
type RuleChainDbRow = (
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

/// Rules service - provides rule chain management and execution operations
pub struct RulesService<'a> {
    ctx: &'a RulesrvContext,
}

impl<'a> RulesService<'a> {
    /// Create a new rules service from context
    pub fn new(ctx: &'a RulesrvContext) -> Self {
        Self { ctx }
    }

    /// List all rule chains
    ///
    /// Returns a list of all configured rule chains.
    pub async fn list(&self) -> Result<Vec<RuleSummary>> {
        // Query database for rule chains
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

    /// Get rule chain by ID
    ///
    /// Returns detailed information about a specific rule chain.
    pub async fn get(&self, rule_id: &str) -> Result<RuleChain> {
        use voltage_config::rulesrv::{FlowNode, Variable};

        // Query database for rule chain
        let row: Option<RuleChainDbRow> = sqlx::query_as(
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
        ) = row
            .ok_or_else(|| LibApiError::not_found(format!("Rule chain '{}' not found", rule_id)))?;

        // Deserialize parsed structures
        let variables: Vec<Variable> = serde_json::from_str(&variables_json)
            .map_err(|e| LibApiError::config(format!("Invalid variables JSON: {}", e)))?;
        let nodes: Vec<FlowNode> = serde_json::from_str(&nodes_json)
            .map_err(|e| LibApiError::config(format!("Invalid nodes JSON: {}", e)))?;

        let flow_json = flow_json_opt.and_then(|s| serde_json::from_str(&s).ok());

        Ok(RuleChain {
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

    /// Create a new rule chain
    ///
    /// Creates a new rule chain in the database.
    /// Public API - use monarch sync for CLI operations.
    #[allow(dead_code)]
    pub async fn create(&self, chain: RuleChain) -> Result<()> {
        let variables_json = serde_json::to_string(&chain.variables)
            .map_err(|e| LibApiError::config(format!("Failed to serialize variables: {}", e)))?;
        let nodes_json = serde_json::to_string(&chain.nodes)
            .map_err(|e| LibApiError::config(format!("Failed to serialize nodes: {}", e)))?;
        let flow_json = chain
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
        .bind(&chain.id)
        .bind(&chain.name)
        .bind(&chain.description)
        .bind(chain.enabled)
        .bind(chain.priority as i64)
        .bind(chain.cooldown_ms as i64)
        .bind(&variables_json)
        .bind(&nodes_json)
        .bind(&chain.start_node_id)
        .bind(flow_json)
        .execute(&self.ctx.sqlite_pool)
        .await?;

        Ok(())
    }

    /// Update an existing rule chain
    ///
    /// Updates a rule chain's configuration in the database.
    /// Public API - use monarch sync for CLI operations.
    #[allow(dead_code)]
    pub async fn update(&self, rule_id: &str, chain: RuleChain) -> Result<()> {
        let variables_json = serde_json::to_string(&chain.variables)
            .map_err(|e| LibApiError::config(format!("Failed to serialize variables: {}", e)))?;
        let nodes_json = serde_json::to_string(&chain.nodes)
            .map_err(|e| LibApiError::config(format!("Failed to serialize nodes: {}", e)))?;
        let flow_json = chain
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
        .bind(&chain.name)
        .bind(&chain.description)
        .bind(chain.enabled)
        .bind(chain.priority as i64)
        .bind(chain.cooldown_ms as i64)
        .bind(&variables_json)
        .bind(&nodes_json)
        .bind(&chain.start_node_id)
        .bind(flow_json)
        .bind(rule_id)
        .execute(&self.ctx.sqlite_pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(
                LibApiError::not_found(format!("Rule chain '{}' not found", rule_id)).into(),
            );
        }

        Ok(())
    }

    /// Delete a rule chain
    ///
    /// Removes a rule chain from the database.
    /// Public API - use monarch sync for CLI operations.
    #[allow(dead_code)]
    pub async fn delete(&self, rule_id: &str) -> Result<()> {
        let result = sqlx::query("DELETE FROM rules WHERE id = ?")
            .bind(rule_id)
            .execute(&self.ctx.sqlite_pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(
                LibApiError::not_found(format!("Rule chain '{}' not found", rule_id)).into(),
            );
        }

        Ok(())
    }

    /// Enable a rule chain
    ///
    /// Sets a rule chain's enabled flag to true.
    pub async fn enable(&self, rule_id: &str) -> Result<()> {
        let result = sqlx::query(
            "UPDATE rules SET enabled = 1, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
        )
        .bind(rule_id)
        .execute(&self.ctx.sqlite_pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(
                LibApiError::not_found(format!("Rule chain '{}' not found", rule_id)).into(),
            );
        }

        Ok(())
    }

    /// Disable a rule chain
    ///
    /// Sets a rule chain's enabled flag to false.
    pub async fn disable(&self, rule_id: &str) -> Result<()> {
        let result = sqlx::query(
            "UPDATE rules SET enabled = 0, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
        )
        .bind(rule_id)
        .execute(&self.ctx.sqlite_pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(
                LibApiError::not_found(format!("Rule chain '{}' not found", rule_id)).into(),
            );
        }

        Ok(())
    }

    /// Execute a rule chain manually
    ///
    /// Triggers immediate execution of a rule chain (even if disabled).
    pub async fn execute(&self, rule_id: &str) -> Result<String> {
        // Get rule chain from database
        let chain = self.get(rule_id).await?;

        // TODO: Execute rule chain using ChainExecutor
        // For now, return a placeholder message
        Ok(format!(
            "Rule chain '{}' execution triggered (not yet implemented in lib mode)",
            chain.name
        ))
    }
}

#[cfg(test)]
mod tests {
    // Note: Tests would require a full service context setup
    // For now, we'll skip unit tests and rely on integration tests
}
