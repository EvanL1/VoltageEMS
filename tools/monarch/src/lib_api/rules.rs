//! Rules API - Library Mode
//!
//! Direct library calls to rulesrv for rule management and execution

use crate::context::RulesrvContext;
use crate::lib_api::{LibApiError, Result};
use rulesrv::rule_engine::Rule;
use serde::{Deserialize, Serialize};

/// Rule summary for list operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleSummary {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub priority: i32,
}

/// Rules service - provides rule management and execution operations
pub struct RulesService<'a> {
    ctx: &'a RulesrvContext,
}

impl<'a> RulesService<'a> {
    /// Create a new rules service from context
    pub fn new(ctx: &'a RulesrvContext) -> Self {
        Self { ctx }
    }

    /// List all rules
    ///
    /// Returns a list of all configured rules.
    pub async fn list(&self) -> Result<Vec<RuleSummary>> {
        // Query database for rules
        let db_rules: Vec<(String, String, bool, i32)> = sqlx::query_as(
            "SELECT id, name, enabled, priority FROM rules ORDER BY priority DESC, id",
        )
        .fetch_all(&self.ctx.sqlite_pool)
        .await?;

        let summaries: Vec<RuleSummary> = db_rules
            .into_iter()
            .map(|(id, name, enabled, priority)| RuleSummary {
                id,
                name,
                enabled,
                priority,
            })
            .collect();

        Ok(summaries)
    }

    /// Get rule by ID
    ///
    /// Returns detailed information about a specific rule.
    pub async fn get(&self, rule_id: &str) -> Result<Rule> {
        // Query database for rule
        let rule_row: Option<(String, String, Option<String>)> =
            sqlx::query_as("SELECT id, name, flow_json FROM rules WHERE id = ?")
                .bind(rule_id)
                .fetch_optional(&self.ctx.sqlite_pool)
                .await?;

        let (id, name, flow_json_opt) = rule_row
            .ok_or_else(|| LibApiError::not_found(format!("Rule '{}' not found", rule_id)))?;

        let flow_json = flow_json_opt.unwrap_or_else(|| "{}".to_string());

        // Parse flow_json as complete Rule object
        let mut rule: Rule = serde_json::from_str(&flow_json)
            .map_err(|e| LibApiError::config(format!("Invalid rule configuration: {}", e)))?;

        // Override id and name from database columns
        rule.id = id;
        rule.name = name;

        Ok(rule)
    }

    /// Create a new rule
    ///
    /// Creates a new rule in the database and rule cache.
    /// Public API - use monarch sync for CLI operations.
    #[allow(dead_code)]
    pub async fn create(&self, rule: Rule) -> Result<()> {
        // Serialize entire Rule object to JSON
        let flow_json = serde_json::to_string(&rule)
            .map_err(|e| LibApiError::config(format!("Failed to serialize rule: {}", e)))?;

        // Insert into database
        sqlx::query(
            "INSERT INTO rules (id, name, flow_json, enabled, priority, description, created_at)
             VALUES (?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)",
        )
        .bind(&rule.id)
        .bind(&rule.name)
        .bind(&flow_json)
        .bind(rule.enabled)
        .bind(rule.priority)
        .bind(&rule.description)
        .execute(&self.ctx.sqlite_pool)
        .await?;

        // TODO: Trigger reload to update cache
        // For now, the cache will be updated on next service restart

        Ok(())
    }

    /// Update an existing rule
    ///
    /// Updates a rule's configuration in the database.
    /// Public API - use monarch sync for CLI operations.
    #[allow(dead_code)]
    pub async fn update(&self, rule_id: &str, rule: Rule) -> Result<()> {
        // Serialize entire Rule object to JSON
        let flow_json = serde_json::to_string(&rule)
            .map_err(|e| LibApiError::config(format!("Failed to serialize rule: {}", e)))?;

        // Update database
        let result = sqlx::query(
            "UPDATE rules SET name = ?, flow_json = ?, enabled = ?, priority = ?, description = ?
             WHERE id = ?",
        )
        .bind(&rule.name)
        .bind(&flow_json)
        .bind(rule.enabled)
        .bind(rule.priority)
        .bind(&rule.description)
        .bind(rule_id)
        .execute(&self.ctx.sqlite_pool)
        .await?;

        if result.rows_affected() == 0 {
            return Err(LibApiError::not_found(format!("Rule '{}' not found", rule_id)).into());
        }

        // TODO: Trigger reload to update cache

        Ok(())
    }

    /// Delete a rule
    ///
    /// Removes a rule from the database and cache.
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

        // TODO: Trigger reload to update cache

        Ok(())
    }

    /// Enable a rule
    ///
    /// Sets a rule's enabled flag to true.
    pub async fn enable(&self, rule_id: &str) -> Result<()> {
        let result = sqlx::query("UPDATE rules SET enabled = 1 WHERE id = ?")
            .bind(rule_id)
            .execute(&self.ctx.sqlite_pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(LibApiError::not_found(format!("Rule '{}' not found", rule_id)).into());
        }

        // TODO: Trigger reload to update cache

        Ok(())
    }

    /// Disable a rule
    ///
    /// Sets a rule's enabled flag to false.
    pub async fn disable(&self, rule_id: &str) -> Result<()> {
        let result = sqlx::query("UPDATE rules SET enabled = 0 WHERE id = ?")
            .bind(rule_id)
            .execute(&self.ctx.sqlite_pool)
            .await?;

        if result.rows_affected() == 0 {
            return Err(LibApiError::not_found(format!("Rule '{}' not found", rule_id)).into());
        }

        // TODO: Trigger reload to update cache

        Ok(())
    }

    /// Execute a rule manually
    ///
    /// Triggers immediate execution of a rule (even if disabled).
    pub async fn execute(&self, rule_id: &str) -> Result<String> {
        // Get rule from database
        let rule = self.get(rule_id).await?;

        // TODO: Execute rule using rule engine
        // For now, return a placeholder message
        Ok(format!(
            "Rule '{}' execution triggered (not yet implemented in lib mode)",
            rule.name
        ))
    }
}

#[cfg(test)]
mod tests {
    // Note: Tests would require a full service context setup
    // For now, we'll skip unit tests and rely on integration tests
}
