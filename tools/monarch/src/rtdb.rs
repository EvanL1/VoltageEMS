//! RTDB (Real-Time Database) management module
//!
//! Provides direct Redis operations for debugging and inspection

use anyhow::Result;
use clap::Subcommand;
use tracing::{info, warn};

#[cfg(feature = "lib-mode")]
use crate::context::ServiceContext;

#[cfg(feature = "lib-mode")]
use voltage_rtdb::{Bytes, Rtdb};

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

pub async fn handle_command(cmd: RtdbCommands, service_ctx: Option<&ServiceContext>) -> Result<()> {
    #[cfg(feature = "lib-mode")]
    {
        if service_ctx.is_none() {
            warn!("RTDB commands require offline mode (--offline flag)");
            warn!("Please run with --offline to use lib-mode RTDB access");
            return Ok(());
        }

        let ctx = service_ctx.expect("service_ctx checked above");

        // Use modsrv's RTDB (all services share same Redis)
        let modsrv = ctx.modsrv()?;
        let rtdb = &modsrv.rtdb;

        match cmd {
            RtdbCommands::Get { key, field } => {
                handle_get(&**rtdb, &key, field.as_deref()).await?;
            },
            RtdbCommands::Set { key, value, field } => {
                handle_set(&**rtdb, &key, &value, field.as_deref()).await?;
            },
            RtdbCommands::Scan { pattern, limit } => {
                handle_scan(&**rtdb, &pattern, limit).await?;
            },
            RtdbCommands::Del { keys, force } => {
                handle_del(&**rtdb, &keys, force).await?;
            },
            RtdbCommands::Inspect { key, full } => {
                handle_inspect(&**rtdb, &key, full).await?;
            },
            RtdbCommands::Patterns => {
                show_patterns();
            },
        }
    }

    #[cfg(not(feature = "lib-mode"))]
    {
        let _ = (cmd, service_ctx);
        warn!("RTDB commands are only available in lib-mode");
        warn!("Please rebuild monarch with --features lib-mode");
    }

    Ok(())
}

#[cfg(feature = "lib-mode")]
async fn handle_get(rtdb: &impl Rtdb, key: &str, field: Option<&str>) -> Result<()> {
    if let Some(field) = field {
        // HGET operation
        let value = rtdb.hash_get(key, field).await?;
        match value {
            Some(bytes) => {
                let value_str = String::from_utf8_lossy(&bytes);
                println!("Key: {} | Field: {}", key, field);
                println!("Value: {}", value_str);
            },
            None => {
                println!("Field '{}' not found in key '{}'", field, key);
            },
        }
    } else {
        // GET operation
        let value = rtdb.get(key).await?;
        match value {
            Some(bytes) => {
                let value_str = String::from_utf8_lossy(&bytes);
                println!("Key: {}", key);
                println!("Value: {}", value_str);
            },
            None => {
                println!("Key '{}' not found", key);
            },
        }
    }
    Ok(())
}

#[cfg(feature = "lib-mode")]
async fn handle_set(rtdb: &impl Rtdb, key: &str, value: &str, field: Option<&str>) -> Result<()> {
    let value_bytes = Bytes::from(value.to_string());

    if let Some(field) = field {
        // HSET operation
        rtdb.hash_set(key, field, value_bytes).await?;
        info!("Set Hash: {} | Field: {} = {}", key, field, value);
    } else {
        // SET operation
        rtdb.set(key, value_bytes).await?;
        info!("Set String: {} = {}", key, value);
    }

    println!("✓ Value set successfully");
    Ok(())
}

#[cfg(feature = "lib-mode")]
async fn handle_scan(rtdb: &impl Rtdb, pattern: &str, limit: usize) -> Result<()> {
    let keys = rtdb.scan_match(pattern).await?;

    let total = keys.len();
    let displayed = if limit > 0 && total > limit {
        limit
    } else {
        total
    };

    println!("=== Scan Results ===");
    println!("Pattern: {}", pattern);
    println!("Found: {} keys", total);

    if displayed > 0 {
        println!("\nKeys:");
        for (i, key) in keys.iter().take(displayed).enumerate() {
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

    Ok(())
}

#[cfg(feature = "lib-mode")]
async fn handle_del(rtdb: &impl Rtdb, keys: &[String], force: bool) -> Result<()> {
    if keys.is_empty() {
        println!("⚠ No keys specified");
        return Ok(());
    }

    // Show keys to be deleted
    println!("=== Delete Confirmation ===");
    println!("Keys to delete:");
    for key in keys {
        println!("  - {}", key);
    }

    if !force {
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

    // Delete keys
    let mut deleted = 0;
    for key in keys {
        if rtdb.del(key).await? {
            deleted += 1;
            info!("Deleted key: {}", key);
        }
    }

    println!("✓ Deleted {} of {} keys", deleted, keys.len());
    Ok(())
}

#[cfg(feature = "lib-mode")]
async fn handle_inspect(rtdb: &impl Rtdb, key: &str, full: bool) -> Result<()> {
    // Check if key exists
    if !rtdb.exists(key).await? {
        println!("⚠ Key '{}' does not exist", key);
        return Ok(());
    }

    println!("=== Key Inspection ===");
    println!("Key: {}", key);

    // Try to detect type by attempting different operations
    // Try Hash first (most common in VoltageEMS)
    let hash_data = rtdb.hash_get_all(key).await?;
    if !hash_data.is_empty() {
        println!("Type: Hash");
        println!("Fields: {}", hash_data.len());

        if full || hash_data.len() <= 20 {
            println!("\nContent:");
            let mut fields: Vec<_> = hash_data.into_iter().collect();
            fields.sort_by(|a, b| a.0.cmp(&b.0));

            for (field, value) in fields {
                let value_str = String::from_utf8_lossy(&value);
                println!("  {:<20} = {}", field, value_str);
            }
        } else {
            println!("\n(Use --full to show all {} fields)", hash_data.len());
            println!("Preview (first 10 fields):");
            let mut fields: Vec<_> = hash_data.into_iter().collect();
            fields.sort_by(|a, b| a.0.cmp(&b.0));

            for (field, value) in fields.iter().take(10) {
                let value_str = String::from_utf8_lossy(value);
                println!("  {:<20} = {}", field, value_str);
            }
        }
        return Ok(());
    }

    // Try String
    if let Some(value) = rtdb.get(key).await? {
        let value_str = String::from_utf8_lossy(&value);
        println!("Type: String");
        println!("Length: {} bytes", value.len());
        println!("\nValue:");
        println!("{}", value_str);
        return Ok(());
    }

    println!("Type: Unknown (or unsupported type)");
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

    println!("TODO Queues:");
    println!("  comsrv:<channel_id>:C:TODO  - Control commands queue");
    println!("  comsrv:<channel_id>:A:TODO  - Adjustment commands queue");
    println!();

    println!("Usage Examples:");
    println!("  monarch rtdb scan \"inst:*:M\"              - Scan all instance measurements");
    println!("  monarch rtdb get route:c2m                - Get C2M routing table");
    println!("  monarch rtdb inspect inst:1:M --full      - Inspect instance 1 measurements");
    println!("  monarch rtdb del test:* --force           - Delete all test keys");
}
