mod actions;
mod api;
mod config;
mod engine;
mod error;
mod redis;
mod rules;

use crate::api::ApiServer;
use crate::config::Config;
use crate::engine::RuleExecutor;
use crate::error::{Result, RulesrvError};
use crate::redis::Subscriber;

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{error, info};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to configuration file
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Run in service mode
    Service,

    /// Start API server only
    Api,

    /// List all rules
    List,

    /// Test a specific rule
    Test {
        /// Rule ID to test
        rule_id: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Load configuration
    let config = if let Some(config_path) = args.config {
        Config::from_file(config_path)?
    } else {
        Config::from_env()?
    };

    // Initialize logging
    let log_config = voltage_common::logging::LogConfig {
        level: config.log_level.clone(),
        console: true,
        file: None,
        format: voltage_common::logging::LogFormat::Pretty,
        ansi: true,
        span_events: false,
    };

    let _log_guard = voltage_common::logging::init_logging(&log_config)
        .map_err(|e| RulesrvError::ConfigError(format!("Failed to initialize logging: {}", e)))?;

    info!("Starting Rules Service");

    // Run command
    match args.command {
        Some(Commands::Service) | None => {
            run_service(&config).await?;
        }
        Some(Commands::Api) => {
            start_api_server(&config).await?;
        }
        Some(Commands::List) => {
            list_rules(&config).await?;
        }
        Some(Commands::Test { rule_id }) => {
            test_rule(&config, &rule_id).await?;
        }
    }

    Ok(())
}

/// Run the rules service
async fn run_service(config: &Config) -> Result<()> {
    info!("Starting Rules Service in service mode");

    // Create rule executor
    let executor = Arc::new(RuleExecutor::new(config.clone())?);

    // Start Redis subscriber
    let subscriber = Subscriber::new(config.clone());
    let executor_clone = executor.clone();

    tokio::spawn(async move {
        if let Err(e) = subscriber.start(executor_clone).await {
            error!("Redis subscriber error: {}", e);
        }
    });

    // Start API server
    let api_server = ApiServer::new(executor.clone(), config.api.port);

    info!(
        "Rules service started, API server available at http://0.0.0.0:{}",
        config.api.port
    );

    api_server.start().await?;

    Ok(())
}

/// Start API server only
async fn start_api_server(config: &Config) -> Result<()> {
    let executor = Arc::new(RuleExecutor::new(config.clone())?);
    let api_server = ApiServer::new(executor, config.api.port);

    info!("Starting API server on port {}", config.api.port);
    api_server.start().await?;

    Ok(())
}

/// List all rules
async fn list_rules(config: &Config) -> Result<()> {
    let executor = RuleExecutor::new(config.clone())?;
    let rules = executor.list_rules()?;

    println!("Available rules:");
    for rule in rules {
        println!(
            "  - {} ({}): {}",
            rule.id,
            rule.name,
            rule.description.unwrap_or_default()
        );
    }

    Ok(())
}

/// Test a specific rule
async fn test_rule(config: &Config, rule_id: &str) -> Result<()> {
    let executor = RuleExecutor::new(config.clone())?;

    println!("Testing rule: {}", rule_id);

    // TODO: Implement rule testing logic
    println!("Rule test not implemented yet");

    Ok(())
}
