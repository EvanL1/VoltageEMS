//! Tests for modsrv schema migration

#![allow(clippy::disallowed_methods)] // Integration test - unwrap is acceptable

use voltage_config::modsrv::*;

#[test]
fn test_products_table() {
    let sql = PRODUCTS_TABLE;

    assert!(sql.contains("CREATE TABLE"));
    assert!(sql.contains("products"));
    assert!(sql.contains("product_name TEXT PRIMARY KEY"));
    assert!(sql.contains("parent_name TEXT"));
    assert!(sql.contains("created_at"));
    assert!(sql.contains("DEFAULT CURRENT_TIMESTAMP"));

    println!("PRODUCTS_TABLE:\n{}", sql);
}

#[test]
fn test_instances_table() {
    let sql = INSTANCES_TABLE;

    assert!(sql.contains("CREATE TABLE"));
    assert!(sql.contains("instances"));
    assert!(sql.contains("instance_id"));
    assert!(sql.contains("PRIMARY KEY"));
    assert!(sql.contains("instance_name TEXT NOT NULL UNIQUE"));
    assert!(sql.contains("product_name TEXT NOT NULL"));
    assert!(sql.contains("REFERENCES products"));
    assert!(sql.contains("properties TEXT"));

    println!("INSTANCES_TABLE:\n{}", sql);
}

#[test]
fn test_measurement_routing_table() {
    let sql = MEASUREMENT_ROUTING_TABLE;

    assert!(sql.contains("CREATE TABLE"));
    assert!(sql.contains("measurement_routing"));
    assert!(sql.contains("routing_id INTEGER PRIMARY KEY AUTOINCREMENT"));
    assert!(sql.contains("instance_id"));
    assert!(sql.contains("channel_id"));
    assert!(sql.contains("channel_type"));
    assert!(sql.contains("measurement_id"));
    assert!(sql.contains("channel_point_id"));

    // Check constraints
    assert!(sql.contains("UNIQUE(instance_id, measurement_id)"));
    assert!(sql.contains("CHECK(channel_type IN ('T','S'))"));
    assert!(sql.contains("REFERENCES instances"));
    assert!(sql.contains("ON DELETE CASCADE"));

    println!("MEASUREMENT_ROUTING_TABLE:\n{}", sql);
}

#[test]
fn test_action_routing_table() {
    let sql = ACTION_ROUTING_TABLE;

    assert!(sql.contains("CREATE TABLE"));
    assert!(sql.contains("action_routing"));
    assert!(sql.contains("routing_id INTEGER PRIMARY KEY AUTOINCREMENT"));
    assert!(sql.contains("instance_id"));
    assert!(sql.contains("action_id"));
    assert!(sql.contains("channel_id"));
    assert!(sql.contains("channel_point_id"));

    // Check constraints
    assert!(sql.contains("UNIQUE(instance_id, action_id)"));
    assert!(sql.contains("CHECK(channel_type IN ('C','A'))"));
    assert!(sql.contains("REFERENCES instances"));
    assert!(sql.contains("ON DELETE CASCADE"));

    println!("ACTION_ROUTING_TABLE:\n{}", sql);
}

#[test]
fn test_measurement_points_table() {
    let sql = MEASUREMENT_POINTS_TABLE;

    assert!(sql.contains("CREATE TABLE"));
    assert!(sql.contains("measurement_points"));
    assert!(sql.contains("product_name TEXT NOT NULL"));
    assert!(sql.contains("measurement_id"));
    assert!(sql.contains("name TEXT NOT NULL"));
    assert!(sql.contains("unit TEXT"));
    assert!(sql.contains("description TEXT"));

    // Check composite primary key
    assert!(sql.contains("PRIMARY KEY (product_name, measurement_id)"));
    assert!(sql.contains("REFERENCES products"));
    assert!(sql.contains("ON DELETE CASCADE"));

    println!("MEASUREMENT_POINTS_TABLE:\n{}", sql);
}

#[test]
fn test_property_templates_table() {
    let sql = PROPERTY_TEMPLATES_TABLE;

    assert!(sql.contains("CREATE TABLE"));
    assert!(sql.contains("property_templates"));
    assert!(sql.contains("product_name TEXT NOT NULL"));
    assert!(sql.contains("property_id"));
    assert!(sql.contains("name TEXT NOT NULL"));

    // Check composite primary key
    assert!(sql.contains("PRIMARY KEY (product_name, property_id)"));
    assert!(sql.contains("REFERENCES products"));
    assert!(sql.contains("ON DELETE CASCADE"));

    println!("PROPERTY_TEMPLATES_TABLE:\n{}", sql);
}

#[test]
fn test_action_points_table() {
    let sql = ACTION_POINTS_TABLE;

    assert!(sql.contains("CREATE TABLE"));
    assert!(sql.contains("action_points"));
    assert!(sql.contains("product_name TEXT NOT NULL"));
    assert!(sql.contains("action_id"));
    assert!(sql.contains("name TEXT NOT NULL"));

    // Check composite primary key
    assert!(sql.contains("PRIMARY KEY (product_name, action_id)"));
    assert!(sql.contains("REFERENCES products"));
    assert!(sql.contains("ON DELETE CASCADE"));

    println!("ACTION_POINTS_TABLE:\n{}", sql);
}

#[test]
fn test_all_modsrv_tables_have_if_not_exists() {
    assert!(SERVICE_CONFIG_TABLE.contains("IF NOT EXISTS"));
    assert!(SYNC_METADATA_TABLE.contains("IF NOT EXISTS"));
    assert!(PRODUCTS_TABLE.contains("IF NOT EXISTS"));
    assert!(INSTANCES_TABLE.contains("IF NOT EXISTS"));
    assert!(MEASUREMENT_ROUTING_TABLE.contains("IF NOT EXISTS"));
    assert!(ACTION_ROUTING_TABLE.contains("IF NOT EXISTS"));
    assert!(MEASUREMENT_POINTS_TABLE.contains("IF NOT EXISTS"));
    assert!(PROPERTY_TEMPLATES_TABLE.contains("IF NOT EXISTS"));
    assert!(ACTION_POINTS_TABLE.contains("IF NOT EXISTS"));
}
