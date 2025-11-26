//! Rules Repository - SQLite persistence for rules
//!
//! Stores Vue Flow rules with parsed execution structure

use crate::error::{Result, RuleError};
use crate::parser::parse_flow_json;
use serde_json::Value;
use sqlx::{sqlite::SqliteRow, Row, SqlitePool};
use voltage_config::rulesrv::Rule;

/// List all rules (returns flow_json for frontend display)
pub async fn list_rules(pool: &SqlitePool) -> Result<Vec<Value>> {
    let rows = sqlx::query(
        r#"
        SELECT id, name, description, flow_json, enabled, priority, cooldown_ms
        FROM rules
        ORDER BY priority DESC, id ASC
        "#,
    )
    .fetch_all(pool)
    .await?;

    let mut rules = Vec::with_capacity(rows.len());
    for row in rows {
        rules.push(hydrate_rule_json(row)?);
    }
    Ok(rules)
}

/// Get a single rule by ID (returns flow_json for frontend display)
pub async fn get_rule(pool: &SqlitePool, id: &str) -> Result<Value> {
    let row = sqlx::query(
        r#"
        SELECT id, name, description, flow_json, enabled, priority, cooldown_ms
        FROM rules
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    match row {
        Some(row) => hydrate_rule_json(row),
        None => Err(RuleError::NotFound(id.to_string())),
    }
}

/// Get a rule for execution (returns parsed Rule struct)
pub async fn get_rule_for_execution(pool: &SqlitePool, id: &str) -> Result<Rule> {
    let row = sqlx::query(
        r#"
        SELECT id, name, description, enabled, priority, cooldown_ms,
               variables_json, nodes_json, start_node_id, flow_json
        FROM rules
        WHERE id = ? AND enabled = 1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    match row {
        Some(row) => hydrate_rule(row),
        None => Err(RuleError::NotFound(id.to_string())),
    }
}

/// Load all enabled rules for execution
pub async fn load_enabled_rules(pool: &SqlitePool) -> Result<Vec<Rule>> {
    let rows = sqlx::query(
        r#"
        SELECT id, name, description, enabled, priority, cooldown_ms,
               variables_json, nodes_json, start_node_id, flow_json
        FROM rules
        WHERE enabled = 1
        ORDER BY priority DESC, id ASC
        "#,
    )
    .fetch_all(pool)
    .await?;

    let mut rules = Vec::with_capacity(rows.len());
    for row in rows {
        rules.push(hydrate_rule(row)?);
    }
    Ok(rules)
}

/// Load all rules (including disabled) for hot reload
pub async fn load_all_rules(pool: &SqlitePool) -> Result<Vec<Rule>> {
    let rows = sqlx::query(
        r#"
        SELECT id, name, description, enabled, priority, cooldown_ms,
               variables_json, nodes_json, start_node_id, flow_json
        FROM rules
        ORDER BY priority DESC, id ASC
        "#,
    )
    .fetch_all(pool)
    .await?;

    let mut rules = Vec::with_capacity(rows.len());
    for row in rows {
        rules.push(hydrate_rule(row)?);
    }
    Ok(rules)
}

/// Upsert a rule - parses flow_json and stores both parsed and raw data
pub async fn upsert_rule(pool: &SqlitePool, rule_id: &str, rule: &Value) -> Result<()> {
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
    let variables_json = serde_json::to_string(&parsed.variables)?;
    let nodes_json = serde_json::to_string(&parsed.nodes)?;
    let flow_json_str = serde_json::to_string(&flow_json_value)?;

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
    .execute(pool)
    .await?;

    Ok(())
}

/// Delete a rule
pub async fn delete_rule(pool: &SqlitePool, id: &str) -> Result<()> {
    let result = sqlx::query("DELETE FROM rules WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(RuleError::NotFound(id.to_string()));
    }

    Ok(())
}

/// Enable or disable a rule
pub async fn set_rule_enabled(pool: &SqlitePool, id: &str, enabled: bool) -> Result<()> {
    let result = sqlx::query(
        r#"
        UPDATE rules
        SET enabled = ?, updated_at = CURRENT_TIMESTAMP
        WHERE id = ?
        "#,
    )
    .bind(enabled)
    .bind(id)
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(RuleError::NotFound(id.to_string()));
    }

    Ok(())
}

/// Hydrate a row into flow_json Value (for frontend display)
fn hydrate_rule_json(row: SqliteRow) -> Result<Value> {
    let id: String = row.try_get("id")?;
    let name: String = row.try_get("name")?;
    let description: Option<String> = row.try_get("description")?;
    let flow_json_str: Option<String> = row.try_get("flow_json")?;
    let enabled: i64 = row.try_get("enabled")?;
    let priority: i64 = row.try_get("priority")?;
    let cooldown_ms: i64 = row.try_get("cooldown_ms")?;

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

/// Hydrate a row into Rule struct (for execution)
fn hydrate_rule(row: SqliteRow) -> Result<Rule> {
    use voltage_config::rulesrv::{FlowNode, Variable};

    let id: String = row.try_get("id")?;
    let name: String = row.try_get("name")?;
    let description: Option<String> = row.try_get("description")?;
    let enabled: i64 = row.try_get("enabled")?;
    let priority: i64 = row.try_get("priority")?;
    let cooldown_ms: i64 = row.try_get("cooldown_ms")?;
    let variables_json: String = row.try_get("variables_json")?;
    let nodes_json: String = row.try_get("nodes_json")?;
    let start_node_id: String = row.try_get("start_node_id")?;
    let flow_json_str: Option<String> = row.try_get("flow_json")?;

    // Deserialize parsed structures
    let variables: Vec<Variable> = serde_json::from_str(&variables_json)
        .map_err(|e| RuleError::SerializationError(format!("variables: {}", e)))?;
    let nodes: Vec<FlowNode> = serde_json::from_str(&nodes_json)
        .map_err(|e| RuleError::SerializationError(format!("nodes: {}", e)))?;

    let flow_json = flow_json_str.and_then(|s| serde_json::from_str(&s).ok());

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
