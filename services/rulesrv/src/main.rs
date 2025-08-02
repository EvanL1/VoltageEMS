mod api;
mod config;
mod engine;
mod error;
mod redis;

use crate::api::ApiServer;
use crate::config::Config;
use crate::engine::RuleEngine;
use crate::error::{Result, RulesrvError};
use crate::redis::RedisStore;

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
    /// Run in service mode (default)
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

    /// Execute a specific rule once
    Execute {
        /// Rule ID to execute
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

    // Initialize tracing directly
    tracing_subscriber::fmt()
        .with_env_filter(&config.log_level)
        .with_target(false)
        .init();

    info!("Starting Rules Service");

    // Run command
    match args.command {
        Some(Commands::Service) | None => {
            run_service(&config).await?;
        },
        Some(Commands::Api) => {
            start_api_server(&config).await?;
        },
        Some(Commands::List) => {
            list_rules(&config).await?;
        },
        Some(Commands::Test { rule_id }) => {
            test_rule(&config, &rule_id).await?;
        },
        Some(Commands::Execute { rule_id }) => {
            execute_rule(&config, &rule_id).await?;
        },
    }

    Ok(())
}

/// Run the rules service
async fn run_service(config: &Config) -> Result<()> {
    info!("Starting Rules Service");

    // Create Redis store
    let store =
        Arc::new(RedisStore::new(&config.redis_url, None).map_err(|e| {
            RulesrvError::ConfigError(format!("Failed to create Redis store: {}", e))
        })?);

    // Create rule engine
    let engine = RuleEngine::new(store.clone());

    info!(
        "Rules Service started, API available at http://0.0.0.0:{}",
        config.service.api_port
    );

    // Start API server
    let api_server = ApiServer::new(
        engine,
        store.clone(),
        config.service.api_port,
        config.api.clone(),
    );

    api_server.start().await?;

    Ok(())
}

/// Start API server only
async fn start_api_server(config: &Config) -> Result<()> {
    let store =
        Arc::new(RedisStore::new(&config.redis_url, None).map_err(|e| {
            RulesrvError::ConfigError(format!("Failed to create Redis store: {}", e))
        })?);
    let engine = RuleEngine::new(store.clone());
    let api_server = ApiServer::new(engine, store, config.service.api_port, config.api.clone());

    info!("Starting API server on port {}", config.service.api_port);
    api_server.start().await?;

    Ok(())
}

/// List all rules
async fn list_rules(config: &Config) -> Result<()> {
    let store =
        Arc::new(RedisStore::new(&config.redis_url, None).map_err(|e| {
            RulesrvError::ConfigError(format!("Failed to create Redis store: {}", e))
        })?);
    let engine = RuleEngine::new(store);
    let rules = engine.list_rules().await?;

    info!("Available rules:");
    for rule in rules {
        info!(
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
    let store =
        Arc::new(RedisStore::new(&config.redis_url, None).map_err(|e| {
            RulesrvError::ConfigError(format!("Failed to create Redis store: {}", e))
        })?);
    let mut engine = RuleEngine::new(store);

    info!("Testing rule: {}", rule_id);

    match engine.execute_rule(rule_id).await {
        Ok(result) => {
            info!("Rule test completed:");
            info!("  Execution ID: {}", result.execution_id);
            info!("  Conditions met: {}", result.conditions_met);
            info!("  Success: {}", result.success);
            info!("  Duration: {}ms", result.duration_ms);

            if !result.actions_executed.is_empty() {
                info!("  Actions executed:");
                for action in &result.actions_executed {
                    info!("    - {}", action);
                }
            }

            if let Some(error) = &result.error {
                error!("  Error: {}", error);
            }
        },
        Err(e) => {
            error!("Rule test failed: {}", e);
        },
    }

    Ok(())
}

/// Execute a specific rule
async fn execute_rule(config: &Config, rule_id: &str) -> Result<()> {
    let store =
        Arc::new(RedisStore::new(&config.redis_url, None).map_err(|e| {
            RulesrvError::ConfigError(format!("Failed to create Redis store: {}", e))
        })?);
    let mut engine = RuleEngine::new(store);

    info!("Executing rule: {}", rule_id);

    match engine.execute_rule(rule_id).await {
        Ok(result) => {
            info!("Rule execution completed:");
            info!("  Execution ID: {}", result.execution_id);
            info!("  Conditions met: {}", result.conditions_met);
            info!("  Success: {}", result.success);
            info!("  Duration: {}ms", result.duration_ms);

            if !result.actions_executed.is_empty() {
                info!("  Actions executed:");
                for action in &result.actions_executed {
                    info!("    - {}", action);
                }
            }

            if let Some(error) = &result.error {
                error!("  Error: {}", error);
            }
        },
        Err(e) => {
            error!("Rule execution failed: {}", e);
        },
    }

    Ok(())
}
