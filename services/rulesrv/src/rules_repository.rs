use crate::app::AppState;
use anyhow::{anyhow, Context, Result};
use common::sqlite::SqliteClient;
use serde_json::{Map, Value};
use sqlx::{sqlite::SqliteRow, Row};
use std::sync::Arc;

fn sqlite_client(state: &AppState) -> Result<Arc<SqliteClient>> {
    state
        .sqlite_client
        .clone()
        .ok_or_else(|| anyhow!("SQLite client not configured for rule management"))
}

pub async fn list_rules(state: &AppState) -> Result<Vec<Value>> {
    let sqlite = sqlite_client(state)?;
    let rows = sqlx::query(
        r#"
        SELECT id, name, description, flow_json, enabled, priority
        FROM rules
        ORDER BY priority DESC, id ASC
        "#,
    )
    .fetch_all(sqlite.pool())
    .await
    .context("Failed to query rules from SQLite")?;

    let mut rules = Vec::with_capacity(rows.len());
    for row in rows {
        rules.push(hydrate_rule(row)?);
    }
    Ok(rules)
}

pub async fn get_rule(state: &AppState, id: &str) -> Result<Value> {
    let sqlite = sqlite_client(state)?;
    let row = sqlx::query(
        r#"
        SELECT id, name, description, flow_json, enabled, priority
        FROM rules
        WHERE id = ?
        "#,
    )
    .bind(id)
    .fetch_optional(sqlite.pool())
    .await
    .context("Failed to query rule from SQLite")?;

    match row {
        Some(row) => hydrate_rule(row),
        None => Err(anyhow!("Rule {} not found", id)),
    }
}

pub async fn upsert_rule(state: &AppState, rule_id: &str, rule: &Value) -> Result<()> {
    let sqlite = sqlite_client(state)?;

    let mut stored_rule = rule.clone();
    if !stored_rule.is_object() {
        return Err(anyhow!("Rule payload must be a JSON object"));
    }

    if let Value::Object(obj) = &mut stored_rule {
        obj.insert("id".to_string(), Value::String(rule_id.to_string()));
    }

    let name = stored_rule
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or(rule_id)
        .to_string();

    let description = stored_rule
        .get("description")
        .and_then(Value::as_str)
        .map(|s| s.to_string());

    let enabled = stored_rule
        .get("enabled")
        .and_then(Value::as_bool)
        .unwrap_or(true);

    let priority = stored_rule
        .get("priority")
        .and_then(Value::as_i64)
        .unwrap_or(0);

    let flow_json_value = stored_rule
        .get("flow_json")
        .cloned()
        .unwrap_or_else(|| stored_rule.clone());

    let flow_json =
        serde_json::to_string(&flow_json_value).context("Failed to serialize rule flow_json")?;

    sqlx::query(
        r#"
        INSERT INTO rules (id, name, description, flow_json, enabled, priority)
        VALUES (?, ?, ?, ?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET
            name = excluded.name,
            description = excluded.description,
            flow_json = excluded.flow_json,
            enabled = excluded.enabled,
            priority = excluded.priority,
            updated_at = CURRENT_TIMESTAMP
        "#,
    )
    .bind(rule_id)
    .bind(&name)
    .bind(description)
    .bind(&flow_json)
    .bind(enabled)
    .bind(priority)
    .execute(sqlite.pool())
    .await
    .context("Failed to upsert rule into SQLite")?;

    Ok(())
}

pub async fn delete_rule(state: &AppState, id: &str) -> Result<()> {
    let sqlite = sqlite_client(state)?;
    let result = sqlx::query("DELETE FROM rules WHERE id = ?")
        .bind(id)
        .execute(sqlite.pool())
        .await
        .context("Failed to delete rule from SQLite")?;

    if result.rows_affected() == 0 {
        return Err(anyhow!("Rule {} not found", id));
    }

    Ok(())
}

pub async fn set_rule_enabled(state: &AppState, id: &str, enabled: bool) -> Result<()> {
    let sqlite = sqlite_client(state)?;

    // Use transaction to prevent race conditions between SELECT and UPDATE
    let mut tx = sqlite
        .pool()
        .begin()
        .await
        .context("Failed to begin transaction")?;

    let row = sqlx::query("SELECT flow_json FROM rules WHERE id = ?")
        .bind(id)
        .fetch_optional(&mut *tx)
        .await
        .context("Failed to fetch rule for enable/disable")?;

    let flow_json = match row {
        Some(row) => {
            let stored: Option<String> = row.try_get("flow_json")?;
            stored.unwrap_or_else(|| "{}".to_string())
        },
        None => {
            let _ = tx.rollback().await;
            return Err(anyhow!("Rule {} not found", id));
        },
    };

    let mut flow_json_value =
        serde_json::from_str::<Value>(&flow_json).unwrap_or_else(|_| Value::Object(Map::new()));

    if !flow_json_value.is_object() {
        flow_json_value = Value::Object(Map::new());
    }

    if let Some(obj) = flow_json_value.as_object_mut() {
        obj.insert("enabled".to_string(), Value::Bool(enabled));
    }

    let updated_flow_json =
        serde_json::to_string(&flow_json_value).context("Failed to serialize updated rule JSON")?;

    sqlx::query(
        r#"
        UPDATE rules
        SET enabled = ?, flow_json = ?, updated_at = CURRENT_TIMESTAMP
        WHERE id = ?
        "#,
    )
    .bind(enabled)
    .bind(&updated_flow_json)
    .bind(id)
    .execute(&mut *tx)
    .await
    .context("Failed to update rule enabled flag")?;

    // Commit transaction
    tx.commit().await.context("Failed to commit transaction")?;

    Ok(())
}

fn hydrate_rule(row: SqliteRow) -> Result<Value> {
    let id: String = row.try_get("id")?;
    let name: String = row.try_get("name")?;
    let description: Option<String> = row.try_get("description")?;
    let flow_json_str: Option<String> = row.try_get("flow_json")?;
    let enabled: i64 = row.try_get("enabled")?;
    let priority: i64 = row.try_get("priority")?;

    let mut rule_value = flow_json_str
        .as_deref()
        .and_then(|s| serde_json::from_str::<Value>(s).ok())
        .unwrap_or_else(|| Value::Object(Map::new()));

    if !rule_value.is_object() {
        rule_value = Value::Object(Map::new());
    }

    if let Some(obj) = rule_value.as_object_mut() {
        obj.insert("id".to_string(), Value::String(id));
        obj.insert("name".to_string(), Value::String(name));

        match description {
            Some(desc) => {
                obj.insert("description".to_string(), Value::String(desc));
            },
            None => {
                obj.remove("description");
            },
        }

        obj.insert("enabled".to_string(), Value::Bool(enabled != 0));
        obj.insert("priority".to_string(), Value::from(priority));
    }

    Ok(rule_value)
}
