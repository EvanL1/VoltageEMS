//! Monarch - Unified Management Tool for VoltageEMS
//!
//! A powerful management tool that combines configuration synchronization,
//! service management, and operational control for all VoltageEMS services.

mod channels;
mod context;
mod core;
mod doctor;
mod logs;
mod models;
mod rtdb;
mod rules;
mod services;
mod shm;
mod utils;

// ===== Library Mode API =====
//
// Direct service library calls instead of HTTP API.
// Provides the same functionality as HTTP API but with:
// - No need for running services
// - 10x faster response time (no HTTP overhead)
// - Better error messages (direct access to internal errors)
// - Support for batch operations
pub mod lib_api {
    #[cfg(feature = "lib-mode")]
    pub mod channels;

    #[cfg(feature = "lib-mode")]
    pub mod models;

    #[cfg(feature = "lib-mode")]
    pub mod rules;

    /// Common result type for lib API operations
    pub type Result<T> = anyhow::Result<T>;

    /// Error type for lib API operations
    #[derive(Debug, thiserror::Error)]
    pub enum LibApiError {
        #[cfg(feature = "lib-mode")]
        #[error("Comsrv error: {0}")]
        Comsrv(#[from] comsrv::ComSrvError),

        #[cfg(feature = "lib-mode")]
        #[error("Modsrv error: {0}")]
        Modsrv(#[from] modsrv::ModSrvError),
        // rules errors are now handled by modsrv::ModSrvError
        #[error("Database error: {0}")]
        Database(#[from] sqlx::Error),

        #[error("Redis error: {0}")]
        Redis(String),

        #[error("Configuration error: {0}")]
        Config(String),

        #[error("Not found: {0}")]
        NotFound(String),

        #[error("Invalid input: {0}")]
        InvalidInput(String),
    }

    impl LibApiError {
        /// Create a not found error
        pub fn not_found(msg: impl Into<String>) -> Self {
            Self::NotFound(msg.into())
        }

        /// Create an invalid input error
        pub fn invalid_input(msg: impl Into<String>) -> Self {
            Self::InvalidInput(msg.into())
        }

        /// Create a config error
        pub fn config(msg: impl Into<String>) -> Self {
            Self::Config(msg.into())
        }
    }

    #[cfg(feature = "lib-mode")]
    impl From<voltage_rtdb::error::RtdbError> for LibApiError {
        fn from(err: voltage_rtdb::error::RtdbError) -> Self {
            Self::Redis(err.to_string())
        }
    }
}

use crate::context::{ServiceConfig, ServiceContext};
use crate::core::{schema, MonarchCore};
use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use colored::*;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(name = "monarch")]
#[command(about = "👑 Monarch - VoltageEMS Unified Management Tool")]
#[command(long_about = "👑 Monarch - VoltageEMS Unified Management Tool

Configuration Management:
  sync        Sync configuration to SQLite database (use --dry-run to validate only)
  status      Show current configuration status
  init        Initialize database schemas
  export      Export configuration from SQLite to YAML/CSV

Service Operations:
  channels    Manage communication channels and protocols
  models      Manage product templates and device instances
  rules       Manage and execute business rules
  services    Start, stop, and manage VoltageEMS services
  logs        Dynamically adjust log levels for running services

Examples:
  monarch sync                          # Sync all configurations
  monarch sync --dry-run                # Validate without syncing
  monarch channels list                 # List all channels
  monarch models products list          # List products
  monarch rules enable R001             # Enable a rule
  monarch services status               # Check service status
  monarch logs level all debug          # Switch all services to debug mode
  monarch logs get all                  # Show current log levels

Use 'monarch <command> --help' for more information on a specific command.")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Disable colored output
    #[arg(long, global = true)]
    no_color: bool,

    /// Configuration files path (default: auto-detect /opt/MonarchEdge/config or ./config)
    #[arg(short = 'c', long = "config-path", global = true)]
    config_path: Option<String>,

    /// Database files path (default: auto-detect /opt/MonarchEdge/data or ./data)
    #[arg(long = "db-path", global = true)]
    db_path: Option<String>,

    /// Force offline mode (use lib API instead of HTTP)
    #[arg(short = 'o', long, global = true)]
    offline: bool,

    /// Force online mode (use HTTP API only)
    #[arg(long, global = true)]
    online: bool,
}

#[derive(Subcommand)]
enum Commands {
    // === Configuration Management Commands ===
    /// Sync all configuration to SQLite database
    Sync {
        /// Validate only, don't write to database (dry run)
        #[arg(short = 'n', long)]
        dry_run: bool,

        /// Force sync without validation (ignored if --dry-run)
        #[arg(short, long)]
        force: bool,

        /// Show detailed progress for each item
        #[arg(short, long)]
        detailed: bool,

        /// Check database consistency (duplicates, references)
        #[arg(long)]
        check: bool,
    },

    /// Show current configuration status
    Status {
        /// Show detailed status
        #[arg(short, long)]
        detailed: bool,
    },

    /// Initialize database schema (migration-only, safe upgrade)
    Init {
        /// DEPRECATED: This option is disabled for safety. Database can only be upgraded, not reset.
        #[arg(short, long, hide = true)]
        force: bool,
    },

    /// Export configuration from SQLite to YAML/CSV
    Export {
        /// Output directory (default: config/)
        #[arg(short = 'O', long)]
        output: Option<String>,

        /// Show detailed export progress
        #[arg(short, long)]
        detailed: bool,
    },

    // === Service Management Commands ===
    /// Manage communication channels
    #[command(about = "Manage communication channels and protocols")]
    Channels {
        #[command(subcommand)]
        command: channels::ChannelCommands,
    },

    /// Manage models (products and instances)
    #[command(about = "Manage product templates and device instances")]
    Models {
        #[command(subcommand)]
        command: models::ModelCommands,
    },

    /// Manage business rules
    #[command(about = "Manage and execute business rules")]
    Rules {
        #[command(subcommand)]
        command: rules::RuleCommands,
    },

    /// Direct Redis RTDB operations
    #[command(about = "Direct Redis RTDB operations for debugging and inspection")]
    Rtdb {
        #[command(subcommand)]
        command: rtdb::RtdbCommands,
    },

    /// Manage Docker services
    #[command(about = "Start, stop, and manage VoltageEMS services")]
    Services {
        #[command(subcommand)]
        command: services::ServiceCommands,
    },

    /// Manage log levels
    #[command(about = "Dynamically adjust log levels for running services")]
    Logs {
        #[command(subcommand)]
        command: logs::LogCommands,
    },

    /// Shared memory operations (interactive REPL)
    #[command(about = "Zero-latency shared memory CLI (like mysql-cli)")]
    Shm {
        #[command(subcommand)]
        command: Option<shm::ShmCommands>,
    },

    /// System health check and diagnostics
    #[command(about = "Check system health and diagnose issues")]
    Doctor {
        /// Show detailed information (response times, etc.)
        #[arg(short, long)]
        verbose: bool,

        /// Output as JSON (for scripts)
        #[arg(long)]
        json: bool,
    },
}

fn print_banner() {
    println!();
    println!(
        "{}",
        "╔════════════════════════════════════════════════════╗".bright_blue()
    );
    println!(
        "{}",
        "║                                                    ║".bright_blue()
    );
    println!(
        "{}",
        "║               MONARCH CONFIG MANAGER               ║".bright_blue()
    );
    println!(
        "{}",
        "║                                                    ║".bright_blue()
    );
    println!(
        "{}",
        "║    Configuration Management for MonarchEdge        ║".bright_blue()
    );
    println!(
        "{}",
        "║                                                    ║".bright_blue()
    );
    println!(
        "{}",
        "╚════════════════════════════════════════════════════╝".bright_blue()
    );
    println!();
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Configure colored output
    if cli.no_color {
        colored::control::set_override(false);
    }

    // Initialize logging
    let log_level = if cli.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(log_level)
        .with_target(false)
        .init();

    // Use ServiceConfig::auto_detect() as baseline, then override with CLI args
    let mut service_config = ServiceConfig::auto_detect();

    // Override with CLI arguments if provided
    if let Some(config_path) = cli.config_path.as_deref() {
        service_config.config_path = PathBuf::from(config_path);
    }

    if let Some(db_path) = cli.db_path.as_deref() {
        service_config.db_path = PathBuf::from(db_path);
    }

    // Extract paths for use in configuration commands
    let config_path = &service_config.config_path;
    let db_path = &service_config.db_path;

    // Print banner for interactive commands
    if !cli.no_color {
        print_banner();
        println!(
            "{} Config: {}, DB: {}",
            "Using paths:".bright_cyan(),
            config_path.display(),
            db_path.display()
        );
        println!();
    }

    // Initialize ServiceContext for offline mode support
    #[cfg(feature = "lib-mode")]
    let service_ctx = {
        // Detect operating mode
        let use_offline = if cli.online {
            false // Explicitly online mode
        } else if cli.offline {
            true // Explicitly offline mode
        } else {
            // Auto-detect: try to initialize offline mode
            true
        };

        if use_offline {
            // Use the constructed service_config directly
            let mut ctx = ServiceContext::new(service_config.clone());

            // Initialize only modsrv (on-demand initialization)
            // SetAction command only needs modsrv, not comsrv
            match ctx.init_modsrv().await {
                Ok(_) => {
                    if cli.verbose {
                        println!(
                            "{} Offline mode initialized (modsrv only)",
                            "INFO".bright_green()
                        );
                    }
                    Some(ctx)
                },
                Err(e) => {
                    if cli.verbose {
                        println!("{} Failed to initialize modsrv: {}", "WARN".yellow(), e);
                        println!("{} Falling back to online mode", "INFO".bright_cyan());
                    }
                    None
                },
            }
        } else {
            if cli.verbose {
                println!("{} Using online mode (HTTP API)", "INFO".bright_cyan());
            }
            None
        }
    };

    #[cfg(not(feature = "lib-mode"))]
    let service_ctx: Option<ServiceContext> = None;

    match cli.command {
        // Configuration management commands
        Commands::Sync {
            dry_run,
            force,
            detailed,
            check,
        } => {
            if dry_run {
                println!(
                    "{}",
                    "Validating all configuration (dry run)...".bright_cyan()
                );
                validate_command(detailed, config_path, db_path, check).await?;
            } else {
                println!("{}", "Syncing all configuration...".bright_cyan());
                sync_command(force, detailed, config_path, db_path, check).await?;
            }
        },
        Commands::Status { detailed } => {
            println!("{}", "Configuration Status".bright_cyan());
            status_command(detailed, db_path).await?;
        },
        Commands::Init { force } => {
            println!("{}", "Initializing database schema...".bright_cyan());
            init_command(db_path, force).await?;
        },
        Commands::Export { output, detailed } => {
            println!(
                "{}",
                "Exporting configuration from database...".bright_cyan()
            );
            export_command(output, detailed, config_path, db_path).await?;
        },

        // Service management commands
        Commands::Channels { command } => {
            let base_url =
                std::env::var("COMSRV_URL").unwrap_or_else(|_| "http://localhost:6001".to_string());
            channels::handle_command(command, service_ctx.as_ref(), Some(&base_url)).await?;
        },
        Commands::Models { command } => {
            let base_url =
                std::env::var("MODSRV_URL").unwrap_or_else(|_| "http://localhost:6002".to_string());
            models::handle_command(command, service_ctx.as_ref(), Some(&base_url)).await?;
        },
        Commands::Rules { command } => {
            // rules merged into modsrv (port 6002)
            let base_url =
                std::env::var("RULES_URL").unwrap_or_else(|_| "http://localhost:6002".to_string());
            rules::handle_command(command, service_ctx.as_ref(), Some(&base_url)).await?;
        },
        Commands::Rtdb { command } => {
            rtdb::handle_command(command, service_ctx.as_ref()).await?;
        },
        Commands::Services { command } => {
            services::handle_command(command, service_ctx.as_ref()).await?;
        },
        Commands::Logs { command } => {
            logs::handle_command(command).await?;
        },
        Commands::Shm { command } => {
            // Shm command doesn't need async or service context
            shm::handle_command(command)?;
        },
        Commands::Doctor { verbose, json } => {
            doctor::run_doctor(config_path, db_path, verbose, json).await?;
        },
    }

    Ok(())
}

async fn sync_command(
    force: bool,
    detailed: bool,
    config_path: &Path,
    db_path: &Path,
    check: bool,
) -> Result<()> {
    // Sync order: global config → channels/points → products/instances/rules
    let configs = ["global", "comsrv", "modsrv"];

    println!();

    for (idx, cfg) in configs.iter().enumerate() {
        print!(
            "{} [{}/{}] Syncing {}... ",
            "-".bright_cyan(),
            idx + 1,
            configs.len(),
            cfg.bright_yellow()
        );

        let core = MonarchCore::readwrite(db_path, config_path, cfg).await?;

        // Validate first unless forced
        if !force {
            match core.validate(cfg).await {
                Ok(result) if !result.is_valid => {
                    println!("{}", "FAIL".red());
                    for error in &result.errors {
                        eprintln!("   {} {}", "ERROR".red(), error);
                    }
                    eprintln!("   {} Use --force to skip validation", "HINT".bright_blue());
                    std::process::exit(1);
                },
                Err(e) => {
                    println!("{}", "FAIL".red());
                    eprintln!("   {} {}", "ERROR".red(), e);
                    std::process::exit(1);
                },
                _ => {},
            }
        }

        // Perform sync
        match core.sync(cfg).await {
            Ok(result) => {
                if result.errors.is_empty() {
                    println!("{}", "OK".green());
                } else {
                    println!("{} ({} errors)", "WARN".yellow(), result.errors.len());
                }

                if detailed {
                    println!("     {} items synced", result.items_synced);
                    if result.items_deleted > 0 {
                        println!("     {} items deleted", result.items_deleted);
                    }
                    for error in &result.errors {
                        println!("     {} {}: {}", "!".red(), error.item, error.error);
                    }
                }
            },
            Err(e) => {
                println!("{}", "FAIL".red());
                eprintln!("   {} {}", "ERROR".red(), e);
                std::process::exit(1);
            },
        }
    }

    // Run database consistency checks if requested
    if check {
        println!();
        run_db_checks(db_path).await?;
    }

    println!("\n{} Configuration synced successfully!", "DONE".green());
    Ok(())
}

async fn validate_command(
    detailed: bool,
    config_path: &Path,
    db_path: &Path,
    check: bool,
) -> Result<()> {
    let configs = ["global", "comsrv", "modsrv"];
    let mut all_valid = true;

    println!();

    let core = MonarchCore::new(config_path);

    for cfg in configs {
        print!(
            "{} Validating {}... ",
            "-".bright_cyan(),
            cfg.bright_yellow()
        );

        match core.validate(cfg).await {
            Ok(result) => {
                if result.is_valid {
                    println!("{}", "OK".green());
                    if detailed && !result.warnings.is_empty() {
                        for warning in &result.warnings {
                            println!("   {} {}", "WARN".yellow(), warning);
                        }
                    }
                } else {
                    println!("{}", "FAIL".red());
                    for error in &result.errors {
                        eprintln!("   {} {}", "ERROR".red(), error);
                    }
                    all_valid = false;
                }
            },
            Err(e) => {
                println!("{}", "FAIL".red());
                eprintln!("   {} {}", "ERROR".red(), e);
                all_valid = false;
            },
        }
    }

    // Run database consistency checks if requested
    if check {
        println!();
        let check_failed = run_db_checks(db_path).await?;
        if check_failed {
            all_valid = false;
        }
    }

    if !all_valid {
        println!("\n{} Validation failed", "ERROR".red());
        std::process::exit(1);
    }

    println!("\n{} All configurations valid!", "SUCCESS".green());
    Ok(())
}

async fn status_command(detailed: bool, db_path: &Path) -> Result<()> {
    let db_file = db_path.join("voltage.db");

    println!();
    println!("{}", "=".repeat(50).bright_blue());
    println!("{:^50}", "VoltageEMS Configuration Status".bright_yellow());
    println!("{}", "=".repeat(50).bright_blue());
    println!();

    print!("{} Database: ", "-".bright_cyan());

    if db_file.exists() {
        match utils::check_database_status(&db_file).await {
            Ok(status) => {
                println!("{} {}", "OK".green(), db_file.display());

                if detailed {
                    let sync_time = status.last_sync.unwrap_or_else(|| "never".to_string());
                    println!(
                        "   {} Last sync: {}",
                        "-".bright_blue(),
                        sync_time.bright_white()
                    );
                    if let Some(count) = status.item_count {
                        println!("   {} Items: {}", "-".bright_blue(), count);
                    }
                }
            },
            Err(_) => {
                println!("{} Not initialized", "WARN".yellow());
                println!("   {} Run 'monarch init' first", "HINT".bright_blue());
            },
        }
    } else {
        println!("{} Not found", "ERROR".red());
        println!(
            "   {} Run 'monarch init' to create database",
            "HINT".bright_blue()
        );
    }

    println!();
    println!("{}", "=".repeat(50).bright_blue());
    Ok(())
}

async fn init_command(db_path: &Path, force: bool) -> Result<()> {
    let db_file = db_path.join("voltage.db");

    println!();

    // --force is disabled for safety (migration-only policy)
    if force {
        eprintln!(
            "{} --force is disabled for safety.",
            "WARNING".bright_yellow()
        );
        eprintln!("   Database can only be upgraded, not reset.");
        eprintln!(
            "   If you really need to reset, manually delete: {}",
            db_file.display()
        );
        return Ok(());
    }

    // Check if database already exists
    if db_file.exists() {
        println!(
            "{} Database already exists: {}",
            "INFO".bright_cyan(),
            db_file.display()
        );
        println!(
            "{} Running safe schema upgrade (CREATE TABLE IF NOT EXISTS)...",
            "INFO".bright_blue()
        );
        // Continue to run schema init - it uses IF NOT EXISTS so it's safe
    }

    // Initialize all tables
    print!(
        "{} Creating database schema in {}... ",
        "-".bright_cyan(),
        db_file.display().to_string().bright_white()
    );

    match schema::init_database(&db_file).await {
        Ok(_) => println!("{}", "OK".green()),
        Err(e) => {
            println!("{}", "FAIL".red());
            eprintln!("   {} Failed to initialize database: {}", "ERROR".red(), e);
            std::process::exit(1);
        },
    }

    println!(
        "\n{} Database initialized: {}",
        "DONE".green(),
        db_file.display()
    );
    Ok(())
}

async fn export_command(
    output: Option<String>,
    detailed: bool,
    config_path: &Path,
    db_path: &Path,
) -> Result<()> {
    let configs = ["global", "comsrv", "modsrv"];
    let output_base = output
        .map(PathBuf::from)
        .unwrap_or_else(|| config_path.to_path_buf());

    println!();

    for cfg in configs {
        print!(
            "{} Exporting {}... ",
            "-".bright_cyan(),
            cfg.bright_yellow()
        );

        let output_dir = output_base.join(cfg);

        if detailed {
            println!();
            println!("   {} Output: {}", "-".bright_blue(), output_dir.display());
        }

        let core = MonarchCore::readwrite(db_path, config_path, cfg).await?;

        let output_path = output_dir
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid output path"))?;

        match core.export(cfg, output_path).await {
            Ok(_) => println!("{}", "OK".green()),
            Err(e) => {
                println!("{}", "FAIL".red());
                eprintln!("   {} {}", "ERROR".red(), e);
                std::process::exit(1);
            },
        }
    }

    println!(
        "\n{} Export completed: {}",
        "DONE".green(),
        output_base.display()
    );
    Ok(())
}

/// Run database consistency checks (duplicates, references)
/// Returns true if any errors were found
async fn run_db_checks(db_path: &Path) -> Result<bool> {
    use sqlx::SqlitePool;

    println!("{}", "Checking database consistency...".bright_cyan());

    let db_file = db_path.join("voltage.db");
    let pool = SqlitePool::connect(&format!("sqlite:{}", db_file.display()))
        .await
        .context("Failed to connect to database")?;

    let mut has_errors = false;

    // Check for duplicate IDs
    print!("  Checking channel IDs... ");
    has_errors |= check_duplicates(&pool, "channels", "channel_id").await?;

    print!("  Checking instance IDs... ");
    has_errors |= check_duplicates(&pool, "instances", "instance_id").await?;

    print!("  Checking rule IDs... ");
    has_errors |= check_duplicates(&pool, "rules", "id").await?;

    // Check point tables
    for table in [
        "telemetry_points",
        "signal_points",
        "control_points",
        "adjustment_points",
    ] {
        print!("  Checking {} table... ", table.replace("_", " "));
        has_errors |= check_point_duplicates(&pool, table).await?;
    }

    if has_errors {
        println!("\n{} Database consistency issues found", "ERROR".red());
    } else {
        println!("\n{} Database consistency OK", "OK".green());
    }

    Ok(has_errors)
}

async fn check_duplicates(pool: &sqlx::SqlitePool, table: &str, id_column: &str) -> Result<bool> {
    let query = format!(
        "SELECT {}, COUNT(*) as count FROM {} GROUP BY {} HAVING count > 1",
        id_column, table, id_column
    );

    let rows: Vec<(String, i64)> = sqlx::query_as(&query).fetch_all(pool).await?;

    if rows.is_empty() {
        println!("{}", "OK".green());
        Ok(false)
    } else {
        println!("{}", "FAIL".red());
        for (id, count) in rows {
            eprintln!(
                "    {} {} '{}' appears {} times",
                "ERROR".red(),
                id_column,
                id,
                count
            );
        }
        Ok(true)
    }
}

async fn check_point_duplicates(pool: &sqlx::SqlitePool, table: &str) -> Result<bool> {
    let query = format!(
        "SELECT channel_id, point_id, COUNT(*) as count FROM {} GROUP BY channel_id, point_id HAVING count > 1",
        table
    );

    let rows: Vec<(i32, i64, i64)> = sqlx::query_as(&query).fetch_all(pool).await?;

    if rows.is_empty() {
        println!("{}", "OK".green());
        Ok(false)
    } else {
        println!("{}", "FAIL".red());
        for (channel_id, point_id, count) in rows {
            eprintln!(
                "    {} (channel_id={}, point_id={}) appears {} times",
                "ERROR".red(),
                channel_id,
                point_id,
                count
            );
        }
        Ok(true)
    }
}
