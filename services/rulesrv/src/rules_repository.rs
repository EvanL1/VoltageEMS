//! Rules Repository - SQLite persistence for rule chains
//!
//! Stores Vue Flow rule chains with parsed execution structure

use crate::app::AppState;
use crate::error::{Result, RuleSrvError};
use crate::parser::parse_flow_json;
use common::sqlite::SqliteClient;
use serde_json::Value;
use sqlx::{sqlite::SqliteRow, Row};
use std::sync::Arc;
use voltage_config::rulesrv::RuleChain;

fn sqlite_client(state: &AppState) -> Result<Arc<SqliteClient>> {
    state
        .sqlite_client
        .clone()
        .ok_or_else(|| RuleSrvError::DatabaseError("SQLite client not configured".to_string()))
}

/// List all rule chains (returns flow_json for frontend display)
pub async fn list_rules(state: &AppState) -> Result<Vec<Value>> {
    let sqlite = sqlite_client(state)?;
    let rows = sqlx::query(
        r#"
        SELECT id, name, description, flow_json, enabled, priority, cooldown_ms
        FROM rules
        ORDER BY priority DESC, id ASC
        "#,
    )
    .fetch_all(sqlite.pool())
    .await
    .map_err(|e| RuleSrvError::DatabaseError(e.to_string()))?;

    let mut rules = Vec::with_capacity(rows.len());
    for row in rows {
        rules.push(hydrate_rule_json(row)?);
    }
    Ok(rules)
}

/// Get a single rule chain by ID (returns flow_json for frontend display)
pub async fn get_rule(state: &AppState, id: &str) -> Result<Value> {
    let sqlite = sqlite_client(state)?;
    let row = sqlx::query(
        r#"
        SELECT id, name, description, flow_json, enabled, priority, cooldown_ms
        FROM rules
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_optional(sqlite.pool())
    .await
    .map_err(|e| RuleSrvError::DatabaseError(e.to_string()))?;

    match row {
        Some(row) => hydrate_rule_json(row),
        None => Err(RuleSrvError::RuleNotFound(id.to_string())),
    }
}

/// Get a rule chain for execution (returns parsed RuleChain struct)
pub async fn get_rule_chain(state: &AppState, id: &str) -> Result<RuleChain> {
    let sqlite = sqlite_client(state)?;
    let row = sqlx::query(
        r#"
        SELECT id, name, description, enabled, priority, cooldown_ms,
               variables_json, nodes_json, start_node_id, flow_json
        FROM rules
        WHERE id = ? AND enabled = 1
        "#,
    )
    .bind(id)
    .fetch_optional(sqlite.pool())
    .await
    .map_err(|e| RuleSrvError::DatabaseError(e.to_string()))?;

    match row {
        Some(row) => hydrate_rule_chain(row),
        None => Err(RuleSrvError::RuleNotFound(id.to_string())),
    }
}

/// Load all enabled rule chains for execution
pub async fn load_enabled_rule_chains(state: &AppState) -> Result<Vec<RuleChain>> {
    let sqlite = sqlite_client(state)?;
    let rows = sqlx::query(
        r#"
        SELECT id, name, description, enabled, priority, cooldown_ms,
               variables_json, nodes_json, start_node_id, flow_json
        FROM rules
        WHERE enabled = 1
        ORDER BY priority DESC, id ASC
        "#,
    )
    .fetch_all(sqlite.pool())
    .await
    .map_err(|e| RuleSrvError::DatabaseError(e.to_string()))?;

    let mut chains = Vec::with_capacity(rows.len());
    for row in rows {
        chains.push(hydrate_rule_chain(row)?);
    }
    Ok(chains)
}

/// Load all rule chains (including disabled) for hot reload
pub async fn load_all_chains(state: &AppState) -> anyhow::Result<Vec<RuleChain>> {
    let sqlite = sqlite_client(state)?;
    let rows = sqlx::query(
        r#"
        SELECT id, name, description, enabled, priority, cooldown_ms,
               variables_json, nodes_json, start_node_id, flow_json
        FROM rules
        ORDER BY priority DESC, id ASC
        "#,
    )
    .fetch_all(sqlite.pool())
    .await
    .map_err(|e| RuleSrvError::DatabaseError(e.to_string()))?;

    let mut chains = Vec::with_capacity(rows.len());
    for row in rows {
        chains.push(hydrate_rule_chain(row)?);
    }
    Ok(chains)
}

/// Upsert a rule chain - parses flow_json and stores both parsed and raw data
pub async fn upsert_rule(state: &AppState, rule_id: &str, rule: &Value) -> Result<()> {
    let sqlite = sqlite_client(state)?;

    // Extract metadata from the incoming JSON
    let name = rule
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or(rule_id)
        .to_string();

    let description = rule
        .get("description")
        .and_then(Value::as_str)
        .map(|s| s.to_string());

    let enabled = rule.get("enabled").and_then(Value::as_bool).unwrap_or(true);

    let priority = rule.get("priority").and_then(Value::as_i64).unwrap_or(0) as u32;

    let cooldown_ms = rule.get("cooldown_ms").and_then(Value::as_i64).unwrap_or(0) as u64;

    // Get the flow_json - either from "flow_json" field or the entire rule object
    let flow_json_value = rule
        .get("flow_json")
        .cloned()
        .unwrap_or_else(|| rule.clone());

    // Parse the Vue Flow JSON into execution structure
    let parsed = parse_flow_json(&flow_json_value)?;

    // Serialize parsed structures
    let variables_json = serde_json::to_string(&parsed.variables)
        .map_err(|e| RuleSrvError::SerializationError(e.to_string()))?;
    let nodes_json = serde_json::to_string(&parsed.nodes)
        .map_err(|e| RuleSrvError::SerializationError(e.to_string()))?;
    let flow_json_str = serde_json::to_string(&flow_json_value)
        .map_err(|e| RuleSrvError::SerializationError(e.to_string()))?;

    sqlx::query(
        r#"
        INSERT INTO rules (id, name, description, flow_json, enabled, priority, cooldown_ms,
                                 variables_json, nodes_json, start_node_id)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET
            name = excluded.name,
            description = excluded.description,
            flow_json = excluded.flow_json,
            enabled = excluded.enabled,
            priority = excluded.priority,
            cooldown_ms = excluded.cooldown_ms,
            variables_json = excluded.variables_json,
            nodes_json = excluded.nodes_json,
            start_node_id = excluded.start_node_id,
            updated_at = CURRENT_TIMESTAMP
        "#,
    )
    .bind(rule_id)
    .bind(&name)
    .bind(description)
    .bind(&flow_json_str)
    .bind(enabled)
    .bind(priority as i64)
    .bind(cooldown_ms as i64)
    .bind(&variables_json)
    .bind(&nodes_json)
    .bind(&parsed.start_node_id)
    .execute(sqlite.pool())
    .await
    .map_err(|e| RuleSrvError::DatabaseError(e.to_string()))?;

    Ok(())
}

/// Delete a rule chain
pub async fn delete_rule(state: &AppState, id: &str) -> Result<()> {
    let sqlite = sqlite_client(state)?;
    let result = sqlx::query("DELETE FROM rules WHERE id = ?")
        .bind(id)
        .execute(sqlite.pool())
        .await
        .map_err(|e| RuleSrvError::DatabaseError(e.to_string()))?;

    if result.rows_affected() == 0 {
        return Err(RuleSrvError::RuleNotFound(id.to_string()));
    }

    Ok(())
}

/// Enable or disable a rule chain
pub async fn set_rule_enabled(state: &AppState, id: &str, enabled: bool) -> Result<()> {
    let sqlite = sqlite_client(state)?;

    let result = sqlx::query(
        r#"
        UPDATE rules
        SET enabled = ?, updated_at = CURRENT_TIMESTAMP
        WHERE id = ?
        "#,
    )
    .bind(enabled)
    .bind(id)
    .execute(sqlite.pool())
    .await
    .map_err(|e| RuleSrvError::DatabaseError(e.to_string()))?;

    if result.rows_affected() == 0 {
        return Err(RuleSrvError::RuleNotFound(id.to_string()));
    }

    Ok(())
}

/// Hydrate a row into flow_json Value (for frontend display)
fn hydrate_rule_json(row: SqliteRow) -> Result<Value> {
    let id: String = row
        .try_get("id")
        .map_err(|e| RuleSrvError::DatabaseError(e.to_string()))?;
    let name: String = row
        .try_get("name")
        .map_err(|e| RuleSrvError::DatabaseError(e.to_string()))?;
    let description: Option<String> = row
        .try_get("description")
        .map_err(|e| RuleSrvError::DatabaseError(e.to_string()))?;
    let flow_json_str: Option<String> = row
        .try_get("flow_json")
        .map_err(|e| RuleSrvError::DatabaseError(e.to_string()))?;
    let enabled: i64 = row
        .try_get("enabled")
        .map_err(|e| RuleSrvError::DatabaseError(e.to_string()))?;
    let priority: i64 = row
        .try_get("priority")
        .map_err(|e| RuleSrvError::DatabaseError(e.to_string()))?;
    let cooldown_ms: i64 = row
        .try_get("cooldown_ms")
        .map_err(|e| RuleSrvError::DatabaseError(e.to_string()))?;

    // Parse flow_json and merge with metadata
    let mut rule_value = flow_json_str
        .as_deref()
        .and_then(|s| serde_json::from_str::<Value>(s).ok())
        .unwrap_or_else(|| serde_json::json!({}));

    if let Some(obj) = rule_value.as_object_mut() {
        obj.insert("id".to_string(), Value::String(id));
        obj.insert("name".to_string(), Value::String(name));
        if let Some(desc) = description {
            obj.insert("description".to_string(), Value::String(desc));
        }
        obj.insert("enabled".to_string(), Value::Bool(enabled != 0));
        obj.insert("priority".to_string(), Value::from(priority));
        obj.insert("cooldown_ms".to_string(), Value::from(cooldown_ms));
    }

    Ok(rule_value)
}

/// Hydrate a row into RuleChain struct (for execution)
fn hydrate_rule_chain(row: SqliteRow) -> Result<RuleChain> {
    use voltage_config::rulesrv::{FlowNode, Variable};

    let id: String = row
        .try_get("id")
        .map_err(|e| RuleSrvError::DatabaseError(e.to_string()))?;
    let name: String = row
        .try_get("name")
        .map_err(|e| RuleSrvError::DatabaseError(e.to_string()))?;
    let description: Option<String> = row
        .try_get("description")
        .map_err(|e| RuleSrvError::DatabaseError(e.to_string()))?;
    let enabled: i64 = row
        .try_get("enabled")
        .map_err(|e| RuleSrvError::DatabaseError(e.to_string()))?;
    let priority: i64 = row
        .try_get("priority")
        .map_err(|e| RuleSrvError::DatabaseError(e.to_string()))?;
    let cooldown_ms: i64 = row
        .try_get("cooldown_ms")
        .map_err(|e| RuleSrvError::DatabaseError(e.to_string()))?;
    let variables_json: String = row
        .try_get("variables_json")
        .map_err(|e| RuleSrvError::DatabaseError(e.to_string()))?;
    let nodes_json: String = row
        .try_get("nodes_json")
        .map_err(|e| RuleSrvError::DatabaseError(e.to_string()))?;
    let start_node_id: String = row
        .try_get("start_node_id")
        .map_err(|e| RuleSrvError::DatabaseError(e.to_string()))?;
    let flow_json_str: Option<String> = row
        .try_get("flow_json")
        .map_err(|e| RuleSrvError::DatabaseError(e.to_string()))?;

    // Deserialize parsed structures
    let variables: Vec<Variable> = serde_json::from_str(&variables_json)
        .map_err(|e| RuleSrvError::SerializationError(format!("variables: {}", e)))?;
    let nodes: Vec<FlowNode> = serde_json::from_str(&nodes_json)
        .map_err(|e| RuleSrvError::SerializationError(format!("nodes: {}", e)))?;

    let flow_json = flow_json_str.and_then(|s| serde_json::from_str(&s).ok());

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
