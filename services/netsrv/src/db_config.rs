/// netsrv runtime configuration stored in the shared SQLite database.
use sqlx::SqlitePool;
use tracing::info;

use crate::models::NetConfig;

const DEFAULTS: &[(&str, &str, &str)] = &[
    ("product_sn",                    "MonarchHub",          "Product serial number"),
    ("device_sn",                     "auto",                "Device SN or 'auto' to read from hardware"),
    ("broker_host",                   "localhost",           "MQTT broker hostname"),
    ("broker_port",                   "8883",                "MQTT broker port"),
    ("broker_keepalive_secs",         "120",                 "MQTT keepalive (seconds)"),
    ("client_id",                     "auto",                "MQTT client ID ('auto' = use device_sn)"),
    ("ssl_enabled",                   "false",               "Enable TLS for MQTT"),
    ("reconnect_delay_secs",          "10",                  "Seconds between reconnect attempts"),
    ("reconnect_max_attempts",        "50",                  "Maximum reconnect attempts (0 = unlimited)"),
    ("report_interval_secs",          "50",                  "Data upload interval (seconds)"),
    ("report_batch_size",             "50",                  "Max entries per MQTT message"),
    ("system_monitor_enabled",        "true",                "Collect and upload system metrics"),
    ("system_monitor_interval_secs",  "10",                  "System metrics collection interval (s)"),
    ("subscribe_patterns",            r#"["inst:*:M","inst:*:A"]"#, "JSON array of Redis key patterns"),
    ("exclude_patterns",              "[]",                  "JSON array of regex exclude patterns"),
    ("alarmsrv_url",                  "http://localhost:6007", "alarmsrv base URL for call-alarm"),
];

pub async fn create_config_table(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS netsrv_config (
            key         TEXT PRIMARY KEY,
            value       TEXT NOT NULL,
            description TEXT,
            updated_at  TEXT DEFAULT (datetime('now'))
        )",
    )
    .execute(pool)
    .await?;

    for (key, value, desc) in DEFAULTS {
        sqlx::query(
            "INSERT OR IGNORE INTO netsrv_config (key, value, description) VALUES (?, ?, ?)",
        )
        .bind(key)
        .bind(value)
        .bind(desc)
        .execute(pool)
        .await?;
    }

    info!("netsrv_config table ready");
    Ok(())
}

pub async fn load_config(pool: &SqlitePool) -> anyhow::Result<NetConfig> {
    use std::collections::HashMap;

    let rows: Vec<(String, String)> =
        sqlx::query_as("SELECT key, value FROM netsrv_config")
            .fetch_all(pool)
            .await?;
    let map: HashMap<String, String> = rows.into_iter().collect();

    let get = |k: &str, d: &str| map.get(k).cloned().unwrap_or_else(|| d.to_string());

    Ok(NetConfig {
        product_sn: get("product_sn", "MonarchHub"),
        device_sn: get("device_sn", "auto"),
        broker_host: get("broker_host", "localhost"),
        broker_port: get("broker_port", "8883").parse().unwrap_or(8883),
        broker_keepalive_secs: get("broker_keepalive_secs", "120").parse().unwrap_or(120),
        client_id: get("client_id", "auto"),
        ssl_enabled: get("ssl_enabled", "false") == "true",
        reconnect_delay_secs: get("reconnect_delay_secs", "10").parse().unwrap_or(10),
        reconnect_max_attempts: get("reconnect_max_attempts", "50").parse().unwrap_or(50),
        report_interval_secs: get("report_interval_secs", "50").parse().unwrap_or(50),
        report_batch_size: get("report_batch_size", "50").parse().unwrap_or(50),
        system_monitor_enabled: get("system_monitor_enabled", "true") == "true",
        system_monitor_interval_secs: get("system_monitor_interval_secs", "10")
            .parse()
            .unwrap_or(10),
        subscribe_patterns: serde_json::from_str(&get(
            "subscribe_patterns",
            r#"["inst:*:M","inst:*:A"]"#,
        ))
        .unwrap_or_else(|_| vec!["inst:*:M".to_string(), "inst:*:A".to_string()]),
        exclude_patterns: serde_json::from_str(&get("exclude_patterns", "[]"))
            .unwrap_or_default(),
        alarmsrv_url: get("alarmsrv_url", "http://localhost:6007"),
    })
}

pub async fn save_config(pool: &SqlitePool, cfg: &NetConfig) -> anyhow::Result<()> {
    let pairs: Vec<(&str, String)> = vec![
        ("product_sn", cfg.product_sn.clone()),
        ("device_sn", cfg.device_sn.clone()),
        ("broker_host", cfg.broker_host.clone()),
        ("broker_port", cfg.broker_port.to_string()),
        ("broker_keepalive_secs", cfg.broker_keepalive_secs.to_string()),
        ("client_id", cfg.client_id.clone()),
        ("ssl_enabled", cfg.ssl_enabled.to_string()),
        ("reconnect_delay_secs", cfg.reconnect_delay_secs.to_string()),
        ("reconnect_max_attempts", cfg.reconnect_max_attempts.to_string()),
        ("report_interval_secs", cfg.report_interval_secs.to_string()),
        ("report_batch_size", cfg.report_batch_size.to_string()),
        ("system_monitor_enabled", cfg.system_monitor_enabled.to_string()),
        (
            "system_monitor_interval_secs",
            cfg.system_monitor_interval_secs.to_string(),
        ),
        (
            "subscribe_patterns",
            serde_json::to_string(&cfg.subscribe_patterns)?,
        ),
        (
            "exclude_patterns",
            serde_json::to_string(&cfg.exclude_patterns)?,
        ),
        ("alarmsrv_url", cfg.alarmsrv_url.clone()),
    ];

    for (key, value) in pairs {
        sqlx::query(
            "INSERT INTO netsrv_config (key, value)
             VALUES (?, ?)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value,
                                            updated_at = datetime('now')",
        )
        .bind(key)
        .bind(value)
        .execute(pool)
        .await?;
    }
    Ok(())
}
