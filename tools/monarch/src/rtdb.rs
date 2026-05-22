//! RTDB (Real-Time Database) management module
//!
//! Provides direct Redis operations for debugging and inspection

use anyhow::Result;
use clap::Subcommand;
use common::redis::RedisClient;
use std::sync::Arc;
use tracing::info;
use voltage_rtdb::{Bytes, RedisRtdb, Rtdb};

#[derive(Subcommand)]
pub enum RtdbCommands {
    /// Get value by key (String or Hash field)
    #[command(about = "Get value from Redis (supports String and Hash types)")]
    Get {
        /// Redis key
        key: String,
        /// Hash field (optional, for HGET)
        #[arg(short, long)]
        field: Option<String>,
    },

    /// Set value for key (String or Hash field)
    #[command(about = "Set value in Redis (supports String and Hash types)")]
    Set {
        /// Redis key
        key: String,
        /// Value to set
        value: String,
        /// Hash field (optional, for HSET)
        #[arg(short, long)]
        field: Option<String>,
    },

    /// Scan keys matching pattern
    #[command(about = "Scan Redis keys matching glob pattern")]
    Scan {
        /// Glob pattern (e.g., \"inst:*:M\", \"route:*\")
        pattern: String,
        /// Limit results (0 = unlimited)
        #[arg(short, long, default_value = "100")]
        limit: usize,
    },

    /// Delete key(s)
    #[command(about = "Delete Redis key(s)")]
    Del {
        /// Redis key(s) to delete
        keys: Vec<String>,
        /// Force deletion without confirmation
        #[arg(short, long)]
        force: bool,
    },

    /// Inspect key type and content
    #[command(about = "Inspect Redis key type and show content preview")]
    Inspect {
        /// Redis key
        key: String,
        /// Show full content for Hash/List/Set
        #[arg(short, long)]
        full: bool,
    },

    /// List common key patterns
    #[command(about = "Show common Redis key patterns used in VoltageEMS")]
    Patterns,
}

pub async fn handle_command(cmd: RtdbCommands, redis_url: &str, json: bool) -> Result<()> {
    let redis_client = Arc::new(RedisClient::new(redis_url).await?);
    let rtdb = RedisRtdb::from_client(redis_client);

    match cmd {
        RtdbCommands::Get { key, field } => handle_get(&rtdb, &key, field.as_deref(), json).await,
        RtdbCommands::Set { key, value, field } => {
            handle_set(&rtdb, &key, &value, field.as_deref(), json).await
        },
        RtdbCommands::Scan { pattern, limit } => handle_scan(&rtdb, &pattern, limit, json).await,
        RtdbCommands::Del { keys, force } => handle_del(&rtdb, &keys, force, json).await,
        RtdbCommands::Inspect { key, full } => handle_inspect(&rtdb, &key, full, json).await,
        RtdbCommands::Patterns => {
            if json {
                eprintln!("warning: --json is not supported for 'patterns'");
            }
            show_patterns();
            Ok(())
        },
    }
}

async fn handle_get(rtdb: &impl Rtdb, key: &str, field: Option<&str>, json: bool) -> Result<()> {
    if let Some(field) = field {
        let value = rtdb.hash_get(key, field).await?;
        let value_str = value
            .as_ref()
            .map(|b| String::from_utf8_lossy(b).into_owned());
        if json {
            crate::output::print_success(serde_json::json!({
                "key": key, "field": field, "value": value_str,
            }));
        } else {
            match value_str {
                Some(v) => {
                    println!("Key: {} | Field: {}", key, field);
                    println!("Value: {}", v);
                },
                None => println!("Field '{}' not found in key '{}'", field, key),
            }
        }
    } else {
        let value = rtdb.get(key).await?;
        let value_str = value
            .as_ref()
            .map(|b| String::from_utf8_lossy(b).into_owned());
        if json {
            crate::output::print_success(serde_json::json!({
                "key": key, "value": value_str,
            }));
        } else {
            match value_str {
                Some(v) => {
                    println!("Key: {}", key);
                    println!("Value: {}", v);
                },
                None => println!("Key '{}' not found", key),
            }
        }
    }
    Ok(())
}

async fn handle_set(
    rtdb: &impl Rtdb,
    key: &str,
    value: &str,
    field: Option<&str>,
    json: bool,
) -> Result<()> {
    let value_bytes = Bytes::from(value.to_string());

    if let Some(field) = field {
        rtdb.hash_set(key, field, value_bytes).await?;
        info!("Set Hash: {} | Field: {} = {}", key, field, value);
    } else {
        rtdb.set(key, value_bytes).await?;
        info!("Set String: {} = {}", key, value);
    }

    if json {
        crate::output::print_ok();
    } else {
        println!("✓ Value set successfully");
    }
    Ok(())
}

async fn handle_scan(rtdb: &impl Rtdb, pattern: &str, limit: usize, json: bool) -> Result<()> {
    let keys = rtdb.scan_match(pattern).await?;
    let total = keys.len();
    let displayed = if limit > 0 && total > limit {
        limit
    } else {
        total
    };
    let visible_keys: Vec<&str> = keys.iter().take(displayed).map(|s| s.as_str()).collect();

    if json {
        crate::output::print_success(serde_json::json!({
            "pattern": pattern,
            "total": total,
            "keys": visible_keys,
        }));
    } else {
        println!("=== Scan Results ===");
        println!("Pattern: {}", pattern);
        println!("Found: {} keys", total);

        if displayed > 0 {
            println!("\nKeys:");
            for (i, key) in visible_keys.iter().enumerate() {
                println!("  {}. {}", i + 1, key);
            }
            if total > displayed {
                println!(
                    "\n... and {} more keys (use --limit 0 to show all)",
                    total - displayed
                );
            }
        } else {
            println!("\n⚠ No keys found matching pattern");
        }
    }

    Ok(())
}

async fn handle_del(rtdb: &impl Rtdb, keys: &[String], force: bool, json: bool) -> Result<()> {
    if keys.is_empty() {
        if json {
            crate::output::print_success(serde_json::json!({"deleted": 0, "total": 0}));
        } else {
            println!("⚠ No keys specified");
        }
        return Ok(());
    }

    // In json mode, skip interactive confirmation (agents can't prompt)
    if !force && !json {
        println!("=== Delete Confirmation ===");
        println!("Keys to delete:");
        for key in keys {
            println!("  - {}", key);
        }
        println!(
            "\nAre you sure you want to delete {} key(s)? [y/N]",
            keys.len()
        );
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("✗ Deletion cancelled");
            return Ok(());
        }
    }

    let mut deleted = 0;
    for key in keys {
        if rtdb.del(key).await? {
            deleted += 1;
            info!("Deleted key: {}", key);
        }
    }

    if json {
        crate::output::print_success(serde_json::json!({
            "deleted": deleted, "total": keys.len(),
        }));
    } else {
        println!("✓ Deleted {} of {} keys", deleted, keys.len());
    }
    Ok(())
}

async fn handle_inspect(rtdb: &impl Rtdb, key: &str, full: bool, json: bool) -> Result<()> {
    if !rtdb.exists(key).await? {
        if json {
            crate::output::print_success(serde_json::json!({
                "key": key, "exists": false,
            }));
        } else {
            println!("⚠ Key '{}' does not exist", key);
        }
        return Ok(());
    }

    // Try Hash first (most common in VoltageEMS)
    let hash_data = rtdb.hash_get_all(key).await?;
    if !hash_data.is_empty() {
        let mut fields: Vec<_> = hash_data.into_iter().collect();
        fields.sort_by(|a, b| a.0.cmp(&b.0));

        if json {
            let field_map: serde_json::Map<String, serde_json::Value> = fields
                .into_iter()
                .map(|(k, v)| {
                    (
                        k,
                        serde_json::Value::String(String::from_utf8_lossy(&v).into_owned()),
                    )
                })
                .collect();
            crate::output::print_success(serde_json::json!({
                "key": key, "exists": true, "type": "hash",
                "field_count": field_map.len(), "fields": field_map,
            }));
        } else {
            println!("=== Key Inspection ===");
            println!("Key: {}", key);
            println!("Type: Hash");
            println!("Fields: {}", fields.len());

            if full || fields.len() <= 20 {
                println!("\nContent:");
                for (field, value) in &fields {
                    println!("  {:<20} = {}", field, String::from_utf8_lossy(value));
                }
            } else {
                println!("\n(Use --full to show all {} fields)", fields.len());
                println!("Preview (first 10 fields):");
                for (field, value) in fields.iter().take(10) {
                    println!("  {:<20} = {}", field, String::from_utf8_lossy(value));
                }
            }
        }
        return Ok(());
    }

    // Try String
    if let Some(value) = rtdb.get(key).await? {
        let value_str = String::from_utf8_lossy(&value).into_owned();
        if json {
            crate::output::print_success(serde_json::json!({
                "key": key, "exists": true, "type": "string",
                "length": value.len(), "value": value_str,
            }));
        } else {
            println!("=== Key Inspection ===");
            println!("Key: {}", key);
            println!("Type: String");
            println!("Length: {} bytes", value.len());
            println!("\nValue:");
            println!("{}", value_str);
        }
        return Ok(());
    }

    if json {
        crate::output::print_success(serde_json::json!({
            "key": key, "exists": true, "type": "unknown",
        }));
    } else {
        println!("=== Key Inspection ===");
        println!("Key: {}", key);
        println!("Type: Unknown (or unsupported type)");
    }
    Ok(())
}

fn show_patterns() {
    println!("=== Common Redis Key Patterns in VoltageEMS ===\n");

    println!("Instance Data:");
    println!("  inst:<id>:M              - Instance measurements hash");
    println!("  inst:<id>:A              - Instance actions hash");
    println!("  inst:<id>:name           - Instance name mapping");
    println!();

    println!("Channel Data:");
    println!("  comsrv:<channel_id>:T    - Channel telemetry hash");
    println!("  comsrv:<channel_id>:S    - Channel signals hash");
    println!("  comsrv:<channel_id>:C    - Channel controls hash");
    println!("  comsrv:<channel_id>:A    - Channel adjustments hash");
    println!();

    println!("Routing Tables:");
    println!("  route:c2m                - Channel-to-Model routing hash");
    println!("  route:m2c                - Model-to-Channel routing hash");
    println!("  route:c2c                - Channel-to-Channel routing hash");
    println!();

    println!("Usage Examples:");
    println!("  monarch rtdb scan \"inst:*:M\"              - Scan all instance measurements");
    println!("  monarch rtdb get route:c2m                - Get C2M routing table");
    println!("  monarch rtdb inspect inst:1:M --full      - Inspect instance 1 measurements");
    println!("  monarch rtdb del test:* --force           - Delete all test keys");
}
