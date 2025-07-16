mod api;
mod cache;
mod config;
mod engine;
mod error;
mod model;
mod monitoring;
mod redis_handler;
mod storage;
mod template;

use crate::api::ApiServer;
use crate::redis_handler::RedisConnection;
use config::Config;
use error::{ModelSrvError, Result};
use model::ModelEngine;
use template::TemplateManager;

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::time;
use tracing::{debug, error, info, warn};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to configuration file (supports .toml and .yaml/.yml)
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Create a new instance from a template
    Create {
        /// Template ID
        template_id: String,
        /// Instance ID
        instance_id: String,
        /// Instance name
        #[arg(short, long)]
        name: Option<String>,
    },

    /// Create multiple instances from a template
    CreateMultiple {
        /// Template ID
        template_id: String,
        /// Number of instances to create
        count: usize,
        /// Instance ID prefix
        #[arg(short, long, default_value = "instance")]
        prefix: String,
        /// Starting index
        #[arg(short, long, default_value_t = 1)]
        start_index: usize,
    },

    /// List available templates
    List,

    /// Show model information
    Info,

    /// Run in service mode
    Service,

    /// Start API server
    Api,

    /// Debug Redis data
    Debug {
        /// Key pattern to search for
        #[arg(short, long, default_value = "modsrv:*")]
        pattern: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Load configuration using the new loader
    let config = if let Some(config_path) = args.config {
        // Use specified config file
        let mut loader = config::ConfigLoader::new()
            .with_file(config_path.to_string_lossy())
            .with_env_prefix("MODSRV_");

        if let Ok(config_center_url) = std::env::var("CONFIG_CENTER_URL") {
            loader = loader.with_config_center(config_center_url);
        }

        match loader.load().await {
            Ok(cfg) => cfg,
            Err(e) => {
                eprintln!("Failed to load configuration: {}", e);
                eprintln!("Using default configuration");
                Config::default()
            }
        }
    } else {
        // Use automatic config loading
        match config::load_config().await {
            Ok(cfg) => cfg,
            Err(e) => {
                eprintln!("Failed to load configuration: {}", e);
                eprintln!("Using default configuration");
                Config::default()
            }
        }
    };

    // Initialize logging using voltage-common
    let log_config = voltage_common::logging::LogConfig {
        level: config.log_level.clone(),
        console: true,
        file: None,
        format: voltage_common::logging::LogFormat::Pretty,
        ansi: true,
        span_events: false,
    };

    let _log_guard = voltage_common::logging::init_logging(&log_config)
        .map_err(|e| ModelSrvError::ConfigError(format!("Failed to initialize logging: {}", e)))?;

    info!("Starting Model Service");

    // Create Redis connection
    let redis_conn = RedisConnection::new();

    // Run command
    match args.command {
        Some(Commands::Info) => {
            info!("Displaying model information");
            display_model_info(&config, &redis_conn)?;
        }
        Some(Commands::Service) => {
            info!("Starting service");
            run_service(&config).await?;
        }
        Some(Commands::Api) => {
            info!("Starting API server only");
            if let Err(e) = start_api_server(&config).await {
                error!("API server error: {}", e);
            }
        }
        Some(Commands::Create {
            template_id,
            instance_id,
            name,
        }) => {
            info!("Creating model instance");
            create_instance(&config, &template_id, &instance_id, name.as_deref())?;
        }
        Some(Commands::CreateMultiple {
            template_id,
            count,
            prefix,
            start_index,
        }) => {
            create_instances(&config, &template_id, count, &prefix, start_index)?;
        }
        Some(Commands::List) => {
            list_templates(&config)?;
        }
        Some(Commands::Debug { pattern }) => {
            debug_redis_data(&config, &pattern)?;
        }
        None => {
            // Default to Info
            info!("Displaying model information (default)");
            display_model_info(&config, &redis_conn)?;
        }
    }

    Ok(())
}

/// Run the model service
async fn run_service(config: &Config) -> Result<()> {
    info!("Starting Model Service");

    // Create Redis connection
    let mut redis_conn = RedisConnection::new();

    // Initialize model engine
    let mut model_engine = ModelEngine::new();

    // Main service loop
    let update_interval = Duration::from_millis(config.model.update_interval_ms);
    let mut interval = time::interval(update_interval);

    // Start API server
    let redis_conn_arc = Arc::new(RedisConnection::new());
    let api_server = ApiServer::new_legacy(redis_conn_arc, config.service_api.port, config.clone());

    let api_port = config.service_api.port;
    tokio::spawn(async move {
        if let Err(e) = api_server.start().await {
            error!("API server error: {}", e);
        }
    });

    info!(
        "Model engine started, API server available at http://0.0.0.0:{}",
        api_port
    );

    loop {
        interval.tick().await;

        // Load model configurations
        if let Err(e) = model_engine.load_models(&mut redis_conn, &config.model.config_key_pattern)
        {
            error!("Failed to load models: {}", e);
            continue;
        }

        // Execute models
        if let Err(e) = model_engine.execute_models(&mut redis_conn) {
            error!("Failed to execute models: {}", e);
        }
    }
}

/// List available templates
fn list_templates(config: &Config) -> Result<()> {
    // Initialize template manager
    let template_manager =
        TemplateManager::new(&config.model.templates_dir, &config.redis.key_prefix);

    // Get templates
    let templates = template_manager.list_templates()?;

    println!("Available templates:");
    for template in templates {
        println!(
            "  - {} ({}): {}",
            template.name, template.id, template.description
        );
    }

    Ok(())
}

/// Create a new instance from a template
fn create_instance(
    config: &Config,
    template_id: &str,
    instance_id: &str,
    instance_name: Option<&str>,
) -> Result<()> {
    // Create Redis connection
    let mut redis_conn = RedisConnection::new();

    // Initialize template manager
    let mut template_manager =
        TemplateManager::new(&config.model.templates_dir, &config.redis.key_prefix);

    // Create instance
    template_manager.create_instance(&mut redis_conn, template_id, instance_id, instance_name)?;

    println!(
        "Successfully created instance {} from template {}",
        instance_id, template_id
    );

    Ok(())
}

/// Create multiple instances from a template
fn create_instances(
    config: &Config,
    template_id: &str,
    count: usize,
    prefix: &str,
    start_index: usize,
) -> Result<()> {
    // Create Redis connection
    let mut redis_conn = RedisConnection::new();

    // Initialize template manager
    let mut template_manager =
        TemplateManager::new(&config.model.templates_dir, &config.redis.key_prefix);

    // Create instances
    let instance_ids = template_manager.create_instances(
        &mut redis_conn,
        template_id,
        count,
        prefix,
        start_index,
    )?;

    println!(
        "Successfully created {} instances from template {}:",
        count, template_id
    );
    for id in instance_ids {
        println!("  - {}", id);
    }

    Ok(())
}

/// Display model information in terminal
fn display_model_info(config: &Config, redis_conn: &RedisConnection) -> Result<()> {
    let mut redis_conn = redis_conn.clone();

    // Display templates
    println!("=== Available Templates ===");
    let template_manager =
        TemplateManager::new(&config.model.templates_dir, &config.redis.key_prefix);

    // try to load templates, but don't fail if it doesn't work
    match template_manager.list_templates() {
        Ok(templates) => {
            if templates.is_empty() {
                println!("No templates available");
            } else {
                for template in templates {
                    println!(
                        "  - {} ({}): {}",
                        template.id, template.name, template.description
                    );
                }
            }
        }
        Err(e) => {
            println!("Error loading templates: {}", e);
            println!("No templates available");
        }
    }

    // Display running models
    println!("\n=== Running Models ===");
    let model_pattern = &config.model.config_key_pattern;

    match redis_conn.get_keys(model_pattern) {
        Ok(keys) => {
            if keys.is_empty() {
                println!("No running models");
            } else {
                for key in keys {
                    // Extract model ID from key
                    let id = key.split(':').last().unwrap_or("unknown");

                    // Try to get model details
                    match redis_conn.get_string(&key) {
                        Ok(json_str) => {
                            // Try to parse as ModelDefinition
                            let model = if key.ends_with(".yaml") || key.ends_with(".yml") {
                                serde_yaml::from_str::<model::ModelDefinition>(&json_str)
                                    .map_err(ModelSrvError::from)
                            } else {
                                serde_json::from_str::<model::ModelDefinition>(&json_str)
                                    .map_err(ModelSrvError::from)
                            };

                            match model {
                                Ok(model) => {
                                    println!(
                                        "  - {} ({}): {}",
                                        model.id, model.name, model.description
                                    );

                                    // Show input mappings
                                    println!("    Input Mappings:");
                                    for mapping in &model.input_mappings {
                                        println!(
                                            "      - {} -> {}",
                                            mapping.source_field, mapping.target_field
                                        );
                                    }

                                    // Try to get latest output
                                    let output_key =
                                        format!("{}model:output:{}", config.redis.key_prefix, id);
                                    if let Ok(output) = redis_conn.get_string(&output_key) {
                                        println!("    Latest Output: {}", output);
                                    }
                                }
                                Err(e) => println!("  - {}: Error parsing model: {}", id, e),
                            }
                        }
                        Err(e) => println!("  - {}: Error: {}", id, e),
                    }
                }
            }
        }
        Err(e) => {
            println!("Error getting model keys: {}", e);
            println!("No running models");
        }
    }

    Ok(())
}

/// Debug Redis data
fn debug_redis_data(_config: &Config, pattern: &str) -> Result<()> {
    // Create Redis connection
    let mut redis_conn = RedisConnection::new();

    // Get all keys matching the pattern
    let keys = redis_conn.get_keys(pattern)?;

    println!("Found {} keys matching pattern '{}'", keys.len(), pattern);

    for key in &keys {
        println!("\nKey: {}", key);

        // For now, just try to get as string
        match redis_conn.get_string(key) {
            Ok(value) => println!("Value: {}", value),
            Err(_) => {
                // Try as hash
                if let Ok(hash) = redis_conn.get_hash(key) {
                    println!("Hash values:");
                    for (k, v) in hash {
                        println!("  {}: {}", k, v);
                    }
                } else {
                    println!("  (unable to read value)");
                }
            }
        }
    }

    Ok(())
}

/// Start the API server
async fn start_api_server(config: &Config) -> Result<()> {
    // Create Redis connection
    let redis_conn = Arc::new(RedisConnection::new());

    // Create API server
    let api_server = ApiServer::new_legacy(redis_conn, config.service_api.port, config.clone());

    // Start API server
    api_server
        .start()
        .await
        .map_err(|e| ModelSrvError::IoError(e.to_string()))
}
