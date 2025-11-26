//! Configuration validation and inspection module
//!
//! Provides tools for validating, checking, and linting configuration files

use anyhow::{Context, Result};
use clap::Subcommand;
use colored::Colorize;
use sqlx::SqlitePool;
use std::path::Path;
use tracing::info;

use crate::core::MonarchCore;

#[derive(Subcommand)]
pub enum ConfigCommands {
    /// Validate configuration files
    #[command(about = "Validate configuration files without syncing")]
    Validate {
        /// Service name: global, comsrv, modsrv, rulesrv, or all
        service: String,
        /// Show detailed validation output
        #[arg(short, long)]
        detailed: bool,
    },

    /// Check configuration consistency
    #[command(about = "Check configuration for issues")]
    Check {
        /// Service name
        service: String,
        /// Check type
        #[command(subcommand)]
        check_type: CheckType,
    },
}

#[derive(Subcommand)]
pub enum CheckType {
    /// Check for duplicate IDs
    #[command(about = "Detect duplicate IDs in configuration files")]
    Duplicates,

    /// Check cross-references
    #[command(about = "Validate cross-references (routing table consistency)")]
    References,

    /// Check CSV headers
    #[command(about = "Validate CSV file headers")]
    Headers,

    /// Run all checks
    #[command(about = "Run all available checks")]
    All,
}

pub async fn handle_command(cmd: ConfigCommands, config_path: &Path) -> Result<()> {
    match cmd {
        ConfigCommands::Validate { service, detailed } => {
            handle_validate(&service, detailed, config_path).await?;
        },
        ConfigCommands::Check {
            service,
            check_type,
        } => {
            handle_check(&service, check_type).await?;
        },
    }
    Ok(())
}

async fn handle_validate(service: &str, detailed: bool, config_path: &Path) -> Result<()> {
    // Determine which services to validate
    let services = match service {
        "all" => vec!["global", "comsrv", "modsrv", "rulesrv"],
        s if ["comsrv", "modsrv", "rulesrv", "global"].contains(&s) => vec![s],
        _ => {
            eprintln!("{} Unknown service: {}", "ERROR".red(), service.red());
            eprintln!(
                "Valid services: {}",
                "global, comsrv, modsrv, rulesrv, all".green()
            );
            std::process::exit(1);
        },
    };

    let mut all_valid = true;
    println!();

    let core = MonarchCore::new(config_path);

    for svc in services {
        print!(
            "{} Validating {} configuration... ",
            "-".bright_cyan(),
            svc.bright_yellow()
        );

        match core.validate(svc).await {
            Ok(result) => {
                if result.is_valid {
                    println!("{} Valid", "OK".green());
                    if detailed && !result.warnings.is_empty() {
                        for warning in &result.warnings {
                            println!("   {} {}", "WARNING".yellow(), warning);
                        }
                    }
                } else {
                    println!("{} Invalid", "FAIL".red());
                    for error in &result.errors {
                        eprintln!("   {} {}", "ERROR".red(), error);
                    }
                    all_valid = false;
                }
            },
            Err(e) => {
                println!("{} Invalid", "FAIL".red());
                eprintln!("   {} {}", "ERROR".red(), e);
                all_valid = false;
            },
        }
    }

    println!();
    if all_valid {
        println!("{} All configurations are valid", "SUCCESS".green().bold());
    } else {
        eprintln!("{} Some configurations have errors", "FAILURE".red().bold());
        std::process::exit(1);
    }

    Ok(())
}

async fn handle_check(service: &str, check_type: CheckType) -> Result<()> {
    match check_type {
        CheckType::Duplicates => {
            check_duplicates(service).await?;
        },
        CheckType::References => {
            check_references(service).await?;
        },
        CheckType::Headers => {
            check_headers(service).await?;
        },
        CheckType::All => {
            println!("=== Running All Checks for {} ===\n", service);
            check_duplicates(service).await?;
            println!();
            check_references(service).await?;
            println!();
            check_headers(service).await?;
        },
    }
    Ok(())
}

async fn check_duplicates(service: &str) -> Result<()> {
    info!("Checking for duplicate IDs in {}", service);

    println!("=== Duplicate ID Check ===");
    println!("Service: {}\n", service.bright_yellow());

    // Connect to SQLite database
    let db_path =
        std::env::var("VOLTAGE_DB_PATH").unwrap_or_else(|_| "data/voltage.db".to_string());
    let pool = SqlitePool::connect(&format!("sqlite:{}", db_path))
        .await
        .context("Failed to connect to database")?;

    let mut has_duplicates = false;

    // Check based on service type
    match service {
        "comsrv" => {
            has_duplicates |= check_channel_duplicates(&pool).await?;
            has_duplicates |= check_point_duplicates(&pool, "comsrv").await?;
        },
        "modsrv" => {
            has_duplicates |= check_instance_duplicates(&pool).await?;
            has_duplicates |= check_point_duplicates(&pool, "modsrv").await?;
        },
        "rulesrv" => {
            has_duplicates |= check_rule_duplicates(&pool).await?;
        },
        "all" => {
            println!("{}", "Checking all services...\n".bright_cyan());
            has_duplicates |= check_channel_duplicates(&pool).await?;
            has_duplicates |= check_instance_duplicates(&pool).await?;
            has_duplicates |= check_rule_duplicates(&pool).await?;
            has_duplicates |= check_point_duplicates(&pool, "all").await?;
        },
        _ => {
            eprintln!("{} Unknown service: {}", "ERROR".red(), service.red());
            return Ok(());
        },
    }

    println!();
    if has_duplicates {
        eprintln!("{} Duplicate IDs found", "FAILURE".red().bold());
        std::process::exit(1);
    } else {
        println!("{} No duplicate IDs found", "SUCCESS".green().bold());
    }

    Ok(())
}

/// Check for duplicate channel IDs in channels table
async fn check_channel_duplicates(pool: &SqlitePool) -> Result<bool> {
    print!("Checking channel IDs... ");

    let duplicates: Vec<(i32, i64)> = sqlx::query_as(
        "SELECT channel_id, COUNT(*) as count
         FROM channels
         GROUP BY channel_id
         HAVING count > 1",
    )
    .fetch_all(pool)
    .await?;

    if duplicates.is_empty() {
        println!("{}", "OK".green());
        Ok(false)
    } else {
        println!("{}", "FAIL".red());
        for (channel_id, count) in duplicates {
            eprintln!(
                "  {} Channel ID {} appears {} times",
                "ERROR".red(),
                channel_id,
                count
            );
        }
        Ok(true)
    }
}

/// Check for duplicate instance IDs in instances table
async fn check_instance_duplicates(pool: &SqlitePool) -> Result<bool> {
    print!("Checking instance IDs... ");

    let duplicates: Vec<(i32, i64)> = sqlx::query_as(
        "SELECT instance_id, COUNT(*) as count
         FROM instances
         GROUP BY instance_id
         HAVING count > 1",
    )
    .fetch_all(pool)
    .await?;

    if duplicates.is_empty() {
        println!("{}", "OK".green());
        Ok(false)
    } else {
        println!("{}", "FAIL".red());
        for (instance_id, count) in duplicates {
            eprintln!(
                "  {} Instance ID {} appears {} times",
                "ERROR".red(),
                instance_id,
                count
            );
        }
        Ok(true)
    }
}

/// Check for duplicate rule IDs in rules table
async fn check_rule_duplicates(pool: &SqlitePool) -> Result<bool> {
    print!("Checking rule IDs... ");

    let duplicates: Vec<(String, i64)> = sqlx::query_as(
        "SELECT id, COUNT(*) as count
         FROM rules
         GROUP BY id
         HAVING count > 1",
    )
    .fetch_all(pool)
    .await?;

    if duplicates.is_empty() {
        println!("{}", "OK".green());
        Ok(false)
    } else {
        println!("{}", "FAIL".red());
        for (rule_id, count) in duplicates {
            eprintln!(
                "  {} Rule ID '{}' appears {} times",
                "ERROR".red(),
                rule_id,
                count
            );
        }
        Ok(true)
    }
}

/// Check for duplicate point IDs across all point tables
async fn check_point_duplicates(pool: &SqlitePool, _service: &str) -> Result<bool> {
    let tables = vec![
        "telemetry_points",
        "signal_points",
        "control_points",
        "adjustment_points",
    ];

    let mut has_duplicates = false;

    for table in tables {
        print!("Checking {} table... ", table.replace("_", " "));

        // Point tables have PRIMARY KEY (channel_id, point_id)
        // Check for duplicate (channel_id, point_id) combinations
        let query = format!(
            "SELECT channel_id, point_id, COUNT(*) as count
             FROM {}
             GROUP BY channel_id, point_id
             HAVING count > 1",
            table
        );

        let duplicates: Vec<(i32, i64, i64)> = sqlx::query_as(&query).fetch_all(pool).await?;

        if duplicates.is_empty() {
            println!("{}", "OK".green());
        } else {
            println!("{}", "FAIL".red());
            for (channel_id, point_id, count) in duplicates {
                eprintln!(
                    "  {} (channel_id={}, point_id={}) appears {} times in {}",
                    "ERROR".red(),
                    channel_id,
                    point_id,
                    count,
                    table
                );
            }
            has_duplicates = true;
        }
    }

    Ok(has_duplicates)
}

async fn check_references(service: &str) -> Result<()> {
    info!("Checking cross-references in {}", service);

    println!("=== Cross-Reference Check ===");
    println!("Service: {}", service);
    println!("\n✓ Reference validation not yet implemented");
    println!("  Will validate routing table references to channels and instances");

    Ok(())
}

async fn check_headers(service: &str) -> Result<()> {
    info!("Checking CSV headers in {}", service);

    println!("=== CSV Header Check ===");
    println!("Service: {}", service);
    println!("\n✓ Header validation not yet implemented");
    println!("  Will verify CSV headers match expected schema");

    Ok(())
}
