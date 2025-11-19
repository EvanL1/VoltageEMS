//! Monarch - Unified Management Tool for VoltageEMS
//!
//! A powerful management tool that combines configuration synchronization,
//! service management, and operational control for all VoltageEMS services.

mod channels;
mod config;
mod context;
mod core;
mod models;
mod rtdb;
mod rules;
mod services;
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
        /// Error type for improved service initialization errors
        #[allow(dead_code)]
        #[error("Service not initialized: {0}")]
        ServiceNotInitialized(String),

        #[cfg(feature = "lib-mode")]
        #[error("Comsrv error: {0}")]
        Comsrv(#[from] comsrv::ComSrvError),

        #[cfg(feature = "lib-mode")]
        #[error("Modsrv error: {0}")]
        Modsrv(#[from] modsrv::ModSrvError),

        #[cfg(feature = "lib-mode")]
        #[error("Rulesrv error: {0}")]
        Rulesrv(#[from] rulesrv::RuleSrvError),

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
use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::*;
use indicatif::{ProgressBar, ProgressStyle};
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(name = "monarch")]
#[command(about = "👑 Monarch - VoltageEMS Unified Management Tool")]
#[command(long_about = "👑 Monarch - VoltageEMS Unified Management Tool

Configuration Management:
  sync        Sync configuration to SQLite database
  validate    Validate configuration without syncing
  status      Show current configuration status
  init        Initialize database schemas
  export      Export configuration from SQLite to YAML/CSV
  diff        Compare SQLite configuration with YAML/CSV files

Service Operations:
  channels    Manage communication channels and protocols
  models      Manage product templates and device instances
  rules       Manage and execute business rules
  services    Start, stop, and manage VoltageEMS services

Examples:
  monarch sync all                      # Sync all configurations
  monarch channels list                 # List all channels
  monarch models products list          # List products
  monarch rules enable R001             # Enable a rule
  monarch services status               # Check service status

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
    /// Sync configuration to SQLite database
    Sync {
        /// Service name: global, comsrv, modsrv, rulesrv, or all
        service: String,

        /// Force sync without validation
        #[arg(short, long)]
        force: bool,

        /// Show detailed progress for each item
        #[arg(short, long)]
        detailed: bool,
    },

    /// Validate configuration without syncing
    Validate {
        /// Service name: global, comsrv, modsrv, rulesrv, or all
        service: String,

        /// Hide detailed validation output (default shows details)
        #[arg(short = 'b', long = "brief")]
        brief: bool,
    },

    /// Show current configuration status
    Status {
        /// Show detailed status for each service
        #[arg(short, long)]
        detailed: bool,
    },

    /// Initialize database schemas
    Init {
        /// Service name: global, comsrv, modsrv, rulesrv, or all
        service: String,
    },

    /// Export configuration from SQLite to YAML/CSV
    Export {
        /// Service name: global, comsrv, modsrv, rulesrv, or all
        service: String,

        /// Output directory (default: config/{service})
        #[arg(short, long)]
        output: Option<String>,

        /// Show detailed export progress
        #[arg(short, long)]
        detailed: bool,
    },

    /// Compare SQLite configuration with YAML/CSV files
    Diff {
        /// Service name: global, comsrv, modsrv, rulesrv, or all
        service: String,

        /// Show detailed differences
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

    /// Configuration validation and inspection
    #[command(about = "Configuration file validation and inspection utilities")]
    Config {
        #[command(subcommand)]
        command: config::ConfigCommands,
    },

    /// Manage Docker services
    #[command(about = "Start, stop, and manage VoltageEMS services")]
    Services {
        #[command(subcommand)]
        command: services::ServiceCommands,
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
            // SetAction command only needs modsrv, not comsrv or rulesrv
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
            service,
            force,
            detailed,
        } => {
            println!(
                "{} {}",
                "Starting sync for:".bright_cyan(),
                service.bright_yellow()
            );
            sync_command(&service, force, detailed, config_path, db_path).await?;
        },
        Commands::Validate { service, brief } => {
            println!(
                "{} {}",
                "Validating configuration for:".bright_cyan(),
                service.bright_yellow()
            );
            // Default to detailed (true) unless --brief is specified
            validate_command(&service, !brief, config_path).await?;
        },
        Commands::Status { detailed } => {
            println!("{}", "Configuration Status".bright_cyan());
            status_command(detailed, config_path, db_path).await?;
        },
        Commands::Init { service } => {
            println!(
                "{} {}",
                "Initializing database schemas for:".bright_cyan(),
                service.bright_yellow()
            );
            init_command(&service, db_path).await?;
        },
        Commands::Export {
            service,
            output,
            detailed,
        } => {
            println!(
                "{} {}",
                "Exporting configuration from database for:".bright_cyan(),
                service.bright_yellow()
            );
            export_command(&service, output, detailed, config_path, db_path).await?;
        },
        Commands::Diff { service, detailed } => {
            println!(
                "{} {}",
                "Comparing database with files for:".bright_cyan(),
                service.bright_yellow()
            );
            diff_command(&service, detailed, config_path, db_path).await?;
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
            let base_url = std::env::var("RULESRV_URL")
                .unwrap_or_else(|_| "http://localhost:6003".to_string());
            rules::handle_command(command, service_ctx.as_ref(), Some(&base_url)).await?;
        },
        Commands::Rtdb { command } => {
            rtdb::handle_command(command, service_ctx.as_ref()).await?;
        },
        Commands::Config { command } => {
            config::handle_command(command, config_path).await?;
        },
        Commands::Services { command } => {
            services::handle_command(command, service_ctx.as_ref()).await?;
        },
    }

    Ok(())
}

async fn sync_command(
    service: &str,
    force: bool,
    detailed: bool,
    config_path: &Path,
    db_path: &Path,
) -> Result<()> {
    // Determine which services to sync
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

    let total_services = services.len();
    println!(
        "\n{} {} services to sync",
        "*".bright_blue(),
        total_services
    );

    for (idx, svc) in services.iter().enumerate() {
        println!(
            "\n{} [{}/{}] Service: {}",
            ">".bright_blue(),
            idx + 1,
            total_services,
            svc.bright_yellow()
        );

        // Create MonarchCore instance for this service
        let core = MonarchCore::readwrite(db_path, config_path, svc).await?;

        // Validate first unless forced
        if !force {
            print!("  {} Validating configuration... ", "-".bright_cyan());
            match core.validate(svc).await {
                Ok(result) => {
                    if result.is_valid() {
                        println!("{}", "OK".green());
                    } else {
                        println!("{}", "FAIL".red());
                        for error in &result.errors {
                            eprintln!("  {} {}", "ERROR".red(), error);
                        }
                        for warning in &result.warnings {
                            eprintln!("  {} {}", "WARNING".yellow(), warning);
                        }
                        eprintln!(
                            "  {} Use --force to skip validation (not recommended)",
                            "WARNING".yellow()
                        );
                        std::process::exit(1);
                    }
                },
                Err(e) => {
                    println!("{}", "FAIL".red());
                    eprintln!("  {} Validation failed: {}", "ERROR".red(), e);
                    std::process::exit(1);
                },
            }
        }

        // Create progress bar for sync
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::default_spinner()
                .tick_chars("|/-\\")
                .template("  {spinner:.green} {msg}")
                .unwrap_or_else(|_| ProgressStyle::default_spinner()),
        );
        pb.set_message(format!("Syncing {} configuration...", svc));

        // Perform sync
        match core.sync(svc).await {
            Ok(result) => {
                pb.finish_and_clear();
                // Display result based on whether there were errors
                if result.errors.is_empty() {
                    println!(
                        "  {} {} configuration synced successfully",
                        "SUCCESS".green(),
                        svc
                    );
                } else {
                    println!(
                        "  {} {} configuration synced with {} errors",
                        "WARNING".yellow(),
                        svc,
                        result.errors.len()
                    );
                }

                if detailed {
                    println!("     {} items synced", result.items_synced);
                    if result.items_deleted > 0 {
                        println!("     {} items deleted", result.items_deleted);
                    }
                    if result.warnings > 0 {
                        println!("     {} warnings", result.warnings);
                    }

                    // Display errors if any
                    if !result.errors.is_empty() {
                        println!("     {} errors encountered:", result.errors.len());
                        for error in &result.errors {
                            let err_type = if error.recoverable {
                                "(skipped)".yellow()
                            } else {
                                "(fatal)".red()
                            };
                            println!(
                                "       - {}: {} {}",
                                error.item.red(),
                                error.error,
                                err_type
                            );
                        }
                    }
                }
            },
            Err(e) => {
                pb.finish_and_clear();
                eprintln!("  {} Failed to sync {}: {}", "ERROR".red(), svc, e);
                std::process::exit(1);
            },
        }
    }

    println!(
        "\n{} All configurations synced successfully!",
        "DONE".green()
    );
    Ok(())
}

async fn validate_command(service: &str, detailed: bool, config_path: &Path) -> Result<()> {
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
                if result.is_valid() {
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

    if !all_valid {
        println!("\n{} Some configurations are invalid", "ERROR".red());
        std::process::exit(1);
    }

    println!("\n{} All configurations are valid!", "SUCCESS".green());
    Ok(())
}

async fn status_command(detailed: bool, _config_path: &Path, db_path: &Path) -> Result<()> {
    println!();
    println!("{}", "=".repeat(60).bright_blue());
    println!("{:^60}", "Configuration Database Status".bright_yellow());
    println!("{}", "=".repeat(60).bright_blue());
    println!();

    for service in &["comsrv", "modsrv", "rulesrv"] {
        let db_file = db_path.join(format!("{}.db", service));
        print!("{:12} ", service.bright_cyan());

        if db_file.exists() {
            // Check if database has configuration
            match utils::check_database_status(&db_file).await {
                Ok(status) => {
                    let sync_time = status.last_sync.unwrap_or_else(|| "never".to_string());
                    println!("{} Database exists", "OK".green());

                    if detailed {
                        println!(
                            "              {} Last sync: {}",
                            "-".bright_blue(),
                            sync_time.bright_white()
                        );
                        if let Some(count) = status.item_count {
                            println!("              {} Items: {}", "-".bright_blue(), count);
                        }
                        if let Some(version) = status.schema_version {
                            println!(
                                "              {} Schema version: {}",
                                "-".bright_blue(),
                                version
                            );
                        }
                    }
                },
                Err(_) => {
                    println!("{} Database exists but not initialized", "WARNING".yellow());
                },
            }
        } else {
            println!("{} Database not found", "NOT FOUND".red());
        }
    }

    println!();
    println!("{}", "=".repeat(60).bright_blue());
    Ok(())
}

async fn init_command(service: &str, db_path: &Path) -> Result<()> {
    // Determine which services to initialize and database file
    let (services, db_file) = match service {
        "all" => {
            // Use unified database for all services
            (
                vec!["comsrv", "modsrv", "rulesrv"],
                db_path.join("voltage.db"),
            )
        },
        "global" => {
            // Global doesn't need schema initialization (uses service_config table)
            println!(
                "{} Global config doesn't require schema initialization",
                "INFO".bright_cyan()
            );
            return Ok(());
        },
        s if ["comsrv", "modsrv", "rulesrv"].contains(&s) => {
            // Single service mode (deprecated, kept for backward compatibility)
            println!(
                "{} Single-service mode is deprecated. Use 'monarch init all' for unified database.",
                "WARNING".yellow()
            );
            (vec![s], db_path.join(format!("{}.db", s)))
        },
        _ => {
            eprintln!("{} Unknown service: {}", "ERROR".red(), service.red());
            eprintln!(
                "Valid services: {}",
                "global, comsrv, modsrv, rulesrv, all".green()
            );
            std::process::exit(1);
        },
    };

    println!();

    // Ensure database directory exists
    if let Some(parent) = db_file.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // Remove existing database files to ensure clean initialization
    if db_file.exists() {
        std::fs::remove_file(&db_file)?;
    }
    // Remove WAL and SHM files if they exist
    let wal_file = db_file.with_extension("db-wal");
    if wal_file.exists() {
        let _ = std::fs::remove_file(&wal_file);
    }
    let shm_file = db_file.with_extension("db-shm");
    if shm_file.exists() {
        let _ = std::fs::remove_file(&shm_file);
    }

    // Initialize all service schemas in the unified database
    for svc in services {
        print!(
            "{} Initializing {} schema in {}... ",
            "-".bright_cyan(),
            svc.bright_yellow(),
            db_file
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_else(|| "unknown".to_string())
                .bright_white()
        );

        match schema::init_service_schema(svc, &db_file).await {
            Ok(_) => println!("{}", "OK".green()),
            Err(e) => {
                println!("{}", "FAIL".red());
                eprintln!("   {} Failed to initialize {}: {}", "ERROR".red(), svc, e);
                std::process::exit(1);
            },
        }
    }

    println!("\n{} Database initialization completed!", "DONE".green());
    Ok(())
}

async fn export_command(
    service: &str,
    output: Option<String>,
    detailed: bool,
    config_path: &Path,
    db_path: &Path,
) -> Result<()> {
    // Determine which services to export
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

    println!();

    for svc in services {
        print!(
            "{} Exporting {} configuration from database... ",
            "-".bright_cyan(),
            svc.bright_yellow()
        );

        // Determine output directory
        let output_dir = output
            .clone()
            .map(PathBuf::from)
            .unwrap_or_else(|| config_path.join(svc));

        if detailed {
            println!();
            println!(
                "  {} Output directory: {}",
                "-".bright_blue(),
                output_dir.display()
            );
        }

        // Create MonarchCore instance and perform export
        let core = MonarchCore::readwrite(db_path, config_path, svc).await?;

        let output_path = output_dir
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("Invalid output directory path"))?;
        match core.export(svc, output_path).await {
            Ok(_) => {
                println!("{}", "OK".green());
                println!(
                    "  {} Configuration exported to: {}",
                    "-".bright_blue(),
                    output_dir.display().to_string().bright_white()
                );
            },
            Err(e) => {
                println!("{}", "FAIL".red());
                eprintln!("  {} Export failed: {}", "ERROR".red(), e);
                std::process::exit(1);
            },
        }
    }

    println!("\n{} Export completed successfully!", "DONE".green());
    Ok(())
}

async fn diff_command(
    service: &str,
    detailed: bool,
    config_path: &Path,
    db_path: &Path,
) -> Result<()> {
    // Determine which services to compare
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

    println!();
    for svc in services {
        println!(
            "{} Comparing {} configuration...",
            "-".bright_cyan(),
            svc.bright_yellow()
        );

        if detailed {
            println!("  {} Comparing:", "-".bright_blue());
            println!(
                "    - Database: {}",
                db_path.join(format!("{}.db", svc)).display()
            );
            println!("    - Files: {}/", config_path.join(svc).display());
        }

        // Perform diff comparison
        match perform_diff(svc, detailed, config_path, db_path).await {
            Ok(has_differences) => {
                if has_differences {
                    println!("  {} Differences detected", "WARNING".yellow());
                } else {
                    println!("  {} No differences detected", "OK".green());
                }
            },
            Err(e) => {
                println!("  {} Diff failed: {}", "ERROR".red(), e);
                std::process::exit(1);
            },
        }
    }

    println!("\n{} Diff comparison completed!", "DONE".green());
    Ok(())
}

/// Perform diff comparison between database and files
async fn perform_diff(
    service: &str,
    detailed: bool,
    config_path: &Path,
    db_path: &Path,
) -> Result<bool> {
    let db_file = db_path.join(format!("{}.db", service));
    let config_dir = config_path.join(service);

    // Check if database exists
    if !db_file.exists() {
        if detailed {
            println!(
                "    {} Database not found: {}",
                "WARNING".yellow(),
                db_file.display()
            );
        }
        return Ok(true); // Has differences (database missing)
    }

    // Check if config directory exists
    if !config_dir.exists() {
        if detailed {
            println!(
                "    {} Config directory not found: {}",
                "WARNING".yellow(),
                config_dir.display()
            );
        }
        return Ok(true); // Has differences (config missing)
    }

    // Export to temporary directory
    let temp_dir = tempfile::tempdir()?;
    let temp_path = temp_dir
        .path()
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("Invalid temporary directory path"))?;
    let export_dir = format!("{}/{}", temp_path, service);

    // Export current database state
    let core = MonarchCore::readwrite(db_path, config_path, service).await?;
    core.export(service, &export_dir).await?;

    // Compare exported files with actual config files
    let mut has_differences = false;

    // Compare main YAML file
    let yaml_file = format!("{}.yaml", service);
    let exported_yaml = Path::new(&export_dir).join(&yaml_file);
    let config_yaml = config_dir.join(&yaml_file);

    if let (Ok(exported), Ok(config)) = (
        std::fs::read_to_string(&exported_yaml),
        std::fs::read_to_string(&config_yaml),
    ) {
        if exported != config {
            has_differences = true;
            if detailed {
                println!("    {} {}: Content differs", "≠".yellow(), yaml_file);
            }
        } else if detailed {
            println!("    {} {}: Identical", "=".green(), yaml_file);
        }
    } else {
        has_differences = true;
        if detailed {
            println!("    {} {}: File comparison failed", "!".red(), yaml_file);
        }
    }

    // Compare CSV files for comsrv
    if service == "comsrv" {
        let csv_files = [
            "telemetry.csv",
            "signal.csv",
            "control.csv",
            "adjustment.csv",
        ];
        for csv_file in &csv_files {
            let exported_csv = Path::new(&export_dir).join(csv_file);
            let config_csv = config_dir.join(csv_file);

            if exported_csv.exists() || config_csv.exists() {
                if let (Ok(exported), Ok(config)) = (
                    std::fs::read_to_string(&exported_csv),
                    std::fs::read_to_string(&config_csv),
                ) {
                    if exported != config {
                        has_differences = true;
                        if detailed {
                            println!("    {} {}: Content differs", "≠".yellow(), csv_file);
                        }
                    } else if detailed {
                        println!("    {} {}: Identical", "=".green(), csv_file);
                    }
                } else if exported_csv.exists() != config_csv.exists() {
                    has_differences = true;
                    if detailed {
                        println!(
                            "    {} {}: Exists in one location only",
                            "≠".yellow(),
                            csv_file
                        );
                    }
                }
            }
        }
    }

    // Compare instances.yaml for modsrv
    if service == "modsrv" {
        let instances_file = "instances.yaml";
        let exported_instances = Path::new(&export_dir).join(instances_file);
        let config_instances = config_dir.join(instances_file);

        if let (Ok(exported), Ok(config)) = (
            std::fs::read_to_string(&exported_instances),
            std::fs::read_to_string(&config_instances),
        ) {
            if exported != config {
                has_differences = true;
                if detailed {
                    println!("    {} {}: Content differs", "≠".yellow(), instances_file);
                }
            } else if detailed {
                println!("    {} {}: Identical", "=".green(), instances_file);
            }
        }
    }

    // Clean up temp directory
    temp_dir.close()?;

    Ok(has_differences)
}
