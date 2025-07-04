mod config;
mod error;
mod model;
mod redis_handler;
mod control;
mod template;
mod storage;
mod storage_agent;
mod api;
mod rules;
mod rules_engine;
mod monitoring;

use config::Config;
use error::{ModelSrvError, Result};
use model::ModelEngine;
use control::ControlManager;
use storage_agent::StorageAgent;
use crate::storage::DataStore;
use crate::api::ApiServer;
use crate::redis_handler::RedisType;

use clap::{Parser, Subcommand};
use tracing::{error, info, debug, warn};
use std::path::PathBuf;
use std::time::Duration;
use tokio::time;
use crate::storage::hybrid_store::HybridStore;
use std::sync::Arc;
use crate::storage::SyncMode;

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
    
    /// View memory store data
    Memory {
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
        let loader = config::ConfigLoader::new()
            .with_file(config_path.to_string_lossy())
            .with_config_center(std::env::var("CONFIG_CENTER_URL").ok())
            .with_env_prefix("MODSRV_");
        
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

    // Initialize tracing
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
    
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(&config.log_level))
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting Model Service");

    // Create storage agent
    let storage_agent = match StorageAgent::new(config.clone()) {
        Ok(agent) => agent,
        Err(e) => {
            error!("Failed to create storage agent: {}", e);
        return Err(e);
        }
    };
    
    // Run command
    match args.command {
        Some(Commands::Info) => {
            info!("Displaying model information");
            display_model_info(&config, &storage_agent)?;
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
        Some(Commands::Create { template_id, instance_id, name }) => {
            info!("Creating model instance");
            create_instance(&config, &template_id, &instance_id, name.as_deref())?;
        }
        Some(Commands::CreateMultiple { template_id, count, prefix, start_index }) => {
            create_instances(&config, &template_id, count, &prefix, start_index)?;
        }
        Some(Commands::List) => {
            list_templates(&config)?;
        }
        Some(Commands::Debug { pattern }) => {
            debug_redis_data(&config, &pattern)?;
        }
        Some(Commands::Memory { pattern }) => {
            view_memory_data(&pattern)?;
        }
        None => {
            // Default to Info
            info!("Displaying model information (default)");
            display_model_info(&config, &storage_agent)?;
        }
    }

    Ok(())
}

/// Run the model service
async fn run_service(config: &Config) -> Result<()> {
    info!("Starting Model Service");

    // Create storage agent
    let storage_agent = StorageAgent::new(config.clone())?;
    let store = storage_agent.store();

    // Initialize model engine
    let mut model_engine = ModelEngine::new();

    // Initialize control manager
    let mut control_manager = ControlManager::new(&config.redis.key_prefix);

    // Main service loop
    let update_interval = Duration::from_millis(config.model.update_interval_ms);
    let mut interval = time::interval(update_interval);

    // Start API server
    let store_arc = Arc::new(HybridStore::new(config, SyncMode::WriteThrough)?);
    let storage_agent_arc = Arc::new(storage_agent);
    
    // Create rule executor
    let rule_executor = Arc::new(rules_engine::RuleExecutor::new(store_arc.clone()));
    
    // Initialize and register post-processors
    if config.monitoring.enabled {
        // Create and register notification post-processor if threshold is configured
        if let Some(threshold_ms) = config.monitoring.notification_threshold_ms {
            let mut notification_processor = rules_engine::NotificationPostProcessor::new(
                threshold_ms, 
                &config.redis.key_prefix
            );
            
            // Initialize Redis connection if available
            if let Ok(redis_url) = std::env::var("REDIS_URL") {
                if let Err(e) = notification_processor.init(&redis_url) {
                    error!("Failed to initialize notification post-processor: {}", e);
                } else {
                    if let Err(e) = rule_executor.register_post_processor(notification_processor) {
                        error!("Failed to register notification post-processor: {}", e);
                    } else {
                        info!("Notification post-processor registered");
                    }
                }
            }
        }
    }
    
    // Initialize control action handler
    if let Ok(redis_url) = std::env::var("REDIS_URL") {
        match control::ControlActionHandler::new(&redis_url, &config.redis.key_prefix) {
            Ok(mut handler) => {
                // Load control operations
                if let Err(e) = handler.load_operations(&*store_arc, &config.control.operation_key_pattern) {
                    error!("Failed to load control operations: {}", e);
                }
                
                // Register handler with rule executor
                if let Err(e) = rule_executor.register_action_handler(handler) {
                    error!("Failed to register control action handler: {}", e);
                } else {
                    info!("Control action handler registered successfully");
                }
            },
            Err(e) => {
                error!("Failed to create control action handler: {}", e);
            }
        }
    } else {
        warn!("REDIS_URL environment variable not set, control actions will not be available");
    }
    
    // Create and start API server
    let api_server = ApiServer::new(store_arc.clone(), storage_agent_arc, rule_executor.clone(), config.api.port);
    
    tokio::spawn(async move {
        if let Err(e) = api_server.start().await {
            error!("API server error: {}", e);
        }
    });

    info!("Model engine started, API server available at http://0.0.0.0:{}", config.api.port);

    loop {
        interval.tick().await;

        // Load model configurations
        if let Err(e) = model_engine.load_models(&*store, &config.model.config_key_pattern) {
            error!("Failed to load models: {}", e);
            continue;
        }

        // Load control operations if enabled
        if config.control.enabled {
            if let Err(e) = control_manager.load_operations(&*store, &config.control.operation_key_pattern) {
                error!("Failed to load control operations: {}", e);
            }
        }

        // Execute models
        if let Err(e) = model_engine.execute_models(&*store) {
            error!("Failed to execute models: {}", e);
        }

        // Check and execute control operations if enabled
        if config.control.enabled {
            if let Err(e) = control_manager.check_and_execute_operations(&*store) {
                error!("Failed to execute control operations: {}", e);
            }
        }
    }
}

/// List available templates
fn list_templates(config: &Config) -> Result<()> {
    // Initialize template manager
    let template_manager = TemplateManager::new(&config.model.templates_dir, &config.redis.key_prefix);
    
    // Get templates
    let templates = template_manager.list_templates()?;
    
    println!("Available templates:");
    for template in templates {
        println!("  - {} ({}): {}", template.name, template.id, template.description);
    }
    
    Ok(())
}

/// Create a new instance from a template
fn create_instance(config: &Config, template_id: &str, instance_id: &str, instance_name: Option<&str>) -> Result<()> {
    // Create storage agent
    let storage_agent = StorageAgent::new(config.clone())?;
    let store = storage_agent.store();
    
    // Initialize template manager
    let mut template_manager = TemplateManager::new(&config.model.templates_dir, &config.redis.key_prefix);
    
    // Create instance
    template_manager.create_instance(&*store, template_id, instance_id, instance_name)?;
    
    println!("Successfully created instance {} from template {}", instance_id, template_id);
    
    Ok(())
}

/// Create multiple instances from a template
fn create_instances(config: &Config, template_id: &str, count: usize, prefix: &str, start_index: usize) -> Result<()> {
    // Create storage agent
    let storage_agent = StorageAgent::new(config.clone())?;
    let store = storage_agent.store();
    
    // Initialize template manager
    let mut template_manager = TemplateManager::new(&config.model.templates_dir, &config.redis.key_prefix);
    
    // Create instances
    let instance_ids = template_manager.create_instances(&*store, template_id, count, prefix, start_index)?;
    
    println!("Successfully created {} instances from template {}:", count, template_id);
    for id in instance_ids {
        println!("  - {}", id);
    }
    
    Ok(())
}

/// Display model information in terminal
fn display_model_info(config: &Config, storage_agent: &StorageAgent) -> Result<()> {
    let store = storage_agent.store();

    // Display templates
    println!("=== Available Templates ===");
    let template_manager = TemplateManager::new(&config.model.templates_dir, &config.redis.key_prefix);
    
    // try to load templates, but don't fail if it doesn't work
    match template_manager.list_templates() {
        Ok(templates) => {
            if templates.is_empty() {
                println!("No templates available");
            } else {
                for template in templates {
                    println!("  - {} ({}): {}", template.id, template.name, template.description);
                }
            }
        },
        Err(e) => {
            println!("Error loading templates: {}", e);
            println!("No templates available");
        }
    }

    // Display running models
    println!("\n=== Running Models ===");
    let model_pattern = &config.model.config_key_pattern;
    
    match store.get_keys(model_pattern) {
        Ok(keys) => {
            if keys.is_empty() {
                println!("No running models");
            } else {
                for key in keys {
                    // Extract model ID from key
                    let id = key.split(':').last().unwrap_or("unknown");
                    
                    // Try to get model details
                    match store.get_string(&key) {
                        Ok(json_str) => {
                            // Try to parse as ModelWithActions
                            let model_with_actions = if key.ends_with(".yaml") || key.ends_with(".yml") {
                                serde_yaml::from_str::<model::ModelWithActions>(&json_str)
                                    .map_err(ModelSrvError::from)
                            } else {
                                serde_json::from_str::<model::ModelWithActions>(&json_str)
                                    .map_err(ModelSrvError::from)
                            };
                            
                            match model_with_actions {
                                Ok(model_with_actions) => {
                                    println!("  - {} ({}): {}", 
                                        model_with_actions.model.id,
                                        model_with_actions.model.name,
                                        model_with_actions.model.description);
                                    
                                    // Show input mappings
                                    println!("    Input Mappings:");
                                    for mapping in &model_with_actions.model.input_mappings {
                                        println!("      - {} -> {}", mapping.source_field, mapping.target_field);
                                    }
                                    
                                    // Show actions
                                    if !model_with_actions.actions.is_empty() {
                                        println!("    Actions:");
                                        for action in &model_with_actions.actions {
                                            println!("      - {} ({})", action.id, action.name);
                                        }
                                    }
                                    
                                    // Try to get latest output
                                    let output_key = format!("{}model:output:{}", config.redis.key_prefix, id);
                                    if let Ok(output) = store.get_string(&output_key) {
                                        println!("    Latest Output: {}", output);
                                    }
                                },
                                Err(_) => {
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
                                            println!("  - {} ({}): {}", 
                                                model.id,
                                                model.name,
                                                model.description);
                                            
                                            // Show input mappings
                                            println!("    Input Mappings:");
                                            for mapping in &model.input_mappings {
                                                println!("      - {} -> {}", mapping.source_field, mapping.target_field);
                                            }
                                        },
                                        Err(e) => println!("  - {}: Error parsing model: {}", id, e)
                                    }
                                }
                            }
                        },
                        Err(e) => println!("  - {}: Error: {}", id, e)
                    }
                }
            }
        },
        Err(e) => {
            println!("Error getting model keys: {}", e);
            println!("No running models");
        }
    }
    
    // Display control operations
    println!("\n=== Control Operations ===");
    let operation_pattern = &config.control.operation_key_pattern;
    
    match store.get_keys(operation_pattern) {
        Ok(keys) => {
            if keys.is_empty() {
                println!("No control operations");
            } else {
                for key in keys {
                    // Extract operation ID from key
                    let id = key.split(':').last().unwrap_or("unknown");
                    
                    // Try to get operation details
                    match store.get_string(&key) {
                        Ok(json_str) => {
                            match serde_json::from_str::<control::ControlOperation>(&json_str) {
                                Ok(operation) => {
                                    println!("  - {} ({}): {}", 
                                        operation.id,
                                        operation.name,
                                        operation.description.as_deref().unwrap_or(""));
                                    
                                    // Show parameters
                                    if !operation.parameters.is_empty() {
                                        println!("    Parameters:");
                                        for param in &operation.parameters {
                                            println!("      - {}: {}", param.name, param.value);
                                        }
                                    }
                                },
                                Err(e) => println!("  - {}: Error parsing operation: {}", id, e)
                            }
                        },
                        Err(e) => println!("  - {}: Error: {}", id, e)
                    }
                }
            }
        },
        Err(e) => {
            println!("Error getting operation keys: {}", e);
            println!("No control operations");
        }
    }
    
    Ok(())
}

/// Debug Redis data
fn debug_redis_data(config: &Config, pattern: &str) -> Result<()> {
    // Create storage agent
    let storage_agent = StorageAgent::new(config.clone())?;
    
    // Get all keys matching the pattern
    let store = storage_agent.store();
    let keys = store.get_keys(pattern)?;
    
    println!("Found {} keys matching pattern '{}'", keys.len(), pattern);
    
    for key in &keys {
        println!("\nKey: {}", key);
        
        // Get key type
        let store_ref = store.as_ref();
        if let Some(redis) = &store_ref.redis_store() {
            let key_type = redis.get_type(key)?;
            println!("Type: {:?}", key_type);
            
            match key_type {
                RedisType::String => {
                    let value = store_ref.get_string(key)?;
                    println!("Value: {}", value);
                },
                RedisType::Hash => {
                    let hash = store_ref.get_hash(key)?;
                    for (k, v) in hash {
                        println!("{}: {}", k, v);
                    }
                },
                _ => {
                    println!("Unsupported type");
                }
            }
        } else {
            // Handle the case when Redis is not available
            debug!("Redis not available for key type check: {}", key);
            // Try to get it from memory instead
            if let Ok(value) = store_ref.get_string(key) {
                println!("Type: String (from memory)");
                println!("Value: {}", value);
            } else if let Ok(hash) = store_ref.get_hash(key) {
                println!("Type: Hash (from memory)");
                for (k, v) in hash {
                    println!("{}: {}", k, v);
                }
            } else {
                println!("Key not found or unsupported type");
            }
        }
    }
    
    Ok(())
}

/// View memory store data
fn view_memory_data(pattern: &str) -> Result<()> {
    let config = Config::default();
    let store = HybridStore::new(&config, SyncMode::WriteThrough)?;
    
    info!("Retrieving memory data with pattern: {}", pattern);
    
    // Get data from memory store
    let keys = store.get_keys(pattern)?;
    if keys.is_empty() {
        println!("No data found matching pattern: {}", pattern);
        return Ok(());
    }
    
    println!("Memory data ({}): ", keys.len());
    for key in keys {
        match store.get_string(&key) {
            Ok(value) => {
                println!("{}: {}", key, value);
            },
            Err(e) => {
                println!("{}: Error: {}", key, e);
            }
        }
    }
    
    Ok(())
}

/// Start the API server
async fn start_api_server(config: &Config) -> Result<()> {
    // Create storage agent
    let storage_agent = Arc::new(StorageAgent::new(config.clone())?);
    let store = Arc::new(HybridStore::new(config, SyncMode::WriteThrough)?);
    
    // Create rule executor
    let rule_executor = Arc::new(rules_engine::RuleExecutor::new(store.clone()));
    
    // Create API server
    let api_server = ApiServer::new(store, storage_agent, rule_executor, config.api.port);
    
    // Start API server
    api_server.start().await.map_err(|e| ModelSrvError::IoError(e.to_string()))
} 