//! Test rules schema macro generation

#![allow(clippy::disallowed_methods)] // Integration test - unwrap is acceptable

use voltage_config::rules::{
    RULES_TABLE, RULE_HISTORY_TABLE, SERVICE_CONFIG_TABLE, SYNC_METADATA_TABLE,
};

#[test]
fn test_service_config_table_generation() {
    println!("=== SERVICE_CONFIG_TABLE ===");
    println!("{}\n", SERVICE_CONFIG_TABLE);

    assert!(SERVICE_CONFIG_TABLE.contains("CREATE TABLE IF NOT EXISTS service_config"));
    assert!(SERVICE_CONFIG_TABLE.contains("service_name TEXT NOT NULL"));
    assert!(SERVICE_CONFIG_TABLE.contains("key TEXT NOT NULL"));
    assert!(SERVICE_CONFIG_TABLE.contains("PRIMARY KEY (service_name, key)"));
    assert!(SERVICE_CONFIG_TABLE.contains("value TEXT NOT NULL"));
    assert!(SERVICE_CONFIG_TABLE.contains("type TEXT NOT NULL DEFAULT 'string'"));
    assert!(SERVICE_CONFIG_TABLE.contains("description TEXT"));
    assert!(SERVICE_CONFIG_TABLE.contains("updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP"));
}

#[test]
fn test_sync_metadata_table_generation() {
    println!("=== SYNC_METADATA_TABLE ===");
    println!("{}\n", SYNC_METADATA_TABLE);

    assert!(SYNC_METADATA_TABLE.contains("CREATE TABLE IF NOT EXISTS sync_metadata"));
    assert!(SYNC_METADATA_TABLE.contains("service TEXT PRIMARY KEY"));
    assert!(SYNC_METADATA_TABLE.contains("last_sync TEXT NOT NULL"));
    assert!(SYNC_METADATA_TABLE.contains("version TEXT"));
}

#[test]
fn test_rules_table_generation() {
    println!("=== RULES_TABLE ===");
    println!("{}\n", RULES_TABLE);

    assert!(RULES_TABLE.contains("CREATE TABLE IF NOT EXISTS rules"));
    assert!(RULES_TABLE.contains("id INTEGER PRIMARY KEY"));
    assert!(RULES_TABLE.contains("name TEXT NOT NULL"));
    assert!(RULES_TABLE.contains("description TEXT"));
    assert!(RULES_TABLE.contains("nodes_json TEXT NOT NULL"));
    assert!(
        RULES_TABLE.contains("enabled BOOLEAN NOT NULL DEFAULT TRUE")
            || RULES_TABLE.contains("enabled BOOLEAN DEFAULT TRUE")
    );
    assert!(
        RULES_TABLE.contains("priority INTEGER NOT NULL DEFAULT 0")
            || RULES_TABLE.contains("priority INTEGER DEFAULT 0")
    );
    assert!(
        RULES_TABLE.contains("created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP")
            || RULES_TABLE.contains("created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP")
    );
    assert!(
        RULES_TABLE.contains("updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP")
            || RULES_TABLE.contains("updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP")
    );
}

#[test]
fn test_rule_history_table_generation() {
    println!("=== RULE_HISTORY_TABLE ===");
    println!("{}\n", RULE_HISTORY_TABLE);

    assert!(RULE_HISTORY_TABLE.contains("CREATE TABLE IF NOT EXISTS rule_history"));
    assert!(RULE_HISTORY_TABLE.contains("id INTEGER PRIMARY KEY AUTOINCREMENT"));
    assert!(RULE_HISTORY_TABLE.contains("rule_id INTEGER NOT NULL REFERENCES rules(id)"));
    assert!(RULE_HISTORY_TABLE.contains("triggered_at TEXT NOT NULL"));
    assert!(RULE_HISTORY_TABLE.contains("execution_result TEXT"));
    assert!(RULE_HISTORY_TABLE.contains("error TEXT"));
}
