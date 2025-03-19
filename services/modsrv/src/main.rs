mod config;
mod error;
mod model;
mod redis_handler;
mod control;
mod template;
mod storage;
mod storage_agent;
mod api;

// Only include GUI module when the 'gui' feature is enabled
#[cfg(feature = "gui")]
mod gui;

use crate::config::Config;
use crate::error::{Result, ModelSrvError};
use crate::model::ModelEngine;
use crate::redis_handler::RedisConnection;
use crate::control::ControlManager;
use crate::template::TemplateManager;
use crate::storage_agent::StorageAgent;
use crate::storage::DataStore;
use crate::api::start_api_server;

// Only include GUI imports when the 'gui' feature is enabled
#[cfg(feature = "gui")]
use crate::gui::start_gui;

use clap::{Parser, Subcommand};
use log::{error, info};
use std::path::PathBuf;
use std::time::Duration;
use tokio::time;

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

fn main() -> Result<()> {
    let args = Args::parse();

    // Load configuration
    let config_path = args.config.unwrap_or_else(|| {
        // Try to find configuration file by priority
        // 1. Check in /etc/voltageems/config/modsrv directory
        if std::path::Path::new("/etc/voltageems/config/modsrv/modsrv.yaml").exists() {
            PathBuf::from("/etc/voltageems/config/modsrv/modsrv.yaml")
        } else if std::path::Path::new("/etc/voltageems/config/modsrv/modsrv.yml").exists() {
            PathBuf::from("/etc/voltageems/config/modsrv/modsrv.yml")
        } else if std::path::Path::new("/etc/voltageems/config/modsrv/modsrv.toml").exists() {
            PathBuf::from("/etc/voltageems/config/modsrv/modsrv.toml")
        } 
        // 2. Fall back to checking in current directory
        else if std::path::Path::new("modsrv.yaml").exists() {
            PathBuf::from("modsrv.yaml")
        } else if std::path::Path::new("modsrv.yml").exists() {
            PathBuf::from("modsrv.yml")
        } else if std::path::Path::new("modsrv.toml").exists() {
            PathBuf::from("modsrv.toml")
        } else {
            // Default to /etc/voltageems/config/modsrv/modsrv.yaml
            PathBuf::from("/etc/voltageems/config/modsrv/modsrv.yaml")
        }
    });
    
    let config = match Config::from_file(&config_path) {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Failed to load configuration from {:?}: {}", config_path, e);
            eprintln!("Using default configuration");
            Config::default()
        }
    };

    // Initialize logger
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(&config.log_level))
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
            #[cfg(feature = "gui")]
            {
                // Start GUI version if the feature is enabled
                start_gui(config.clone())?;
            }
            
            #[cfg(not(feature = "gui"))]
            {
                // Start the regular service if GUI is not enabled
                let rt = tokio::runtime::Runtime::new()?;
                rt.block_on(run_service(&config))?;
            }
        }
        Some(Commands::Api) => {
            info!("Starting API server only");
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async {
                if let Err(e) = start_api_server(config).await {
                    error!("API server error: {}", e);
                }
            });
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

    // 启动API服务器
    let config_clone = config.clone();
    tokio::spawn(async move {
        if let Err(e) = start_api_server(config_clone).await {
            error!("API server error: {}", e);
        }
    });

    info!("Model engine started, API server available at http://0.0.0.0:8000");

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
    
    // 尝试加载模板，但不要因为模板加载错误而中断程序
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
    let mut storage_agent = StorageAgent::new(config.clone())?;
    
    // Get all keys matching the pattern
    let store = storage_agent.store();
    let keys = store.get_keys(pattern)?;
    
    println!("Found {} keys matching pattern '{}'", keys.len(), pattern);
    
    for key in &keys {
        println!("\nKey: {}", key);
        
        // Get key type
        let key_type = store.get_type(key)?;
        println!("Type: {:?}", key_type);
        
        match key_type {
            crate::redis_handler::RedisType::String => {
                let value = store.get_string(key)?;
                println!("Value: {}", value);
            },
            crate::redis_handler::RedisType::Hash => {
                let hash = store.get_hash(key)?;
                println!("Hash fields:");
                for (field, value) in hash {
                    println!("  {} = {}", field, value);
                }
            },
            _ => {
                println!("Unsupported type");
            }
        }
    }
    
    Ok(())
}

fn init_logging(config: &Config) {
    // Initialize logging based on configuration
    let env = env_logger::Env::default()
        .filter_or("RUST_LOG", &config.logging.level);
    
    env_logger::Builder::from_env(env)
        .format_timestamp_millis()
        .init();
    
    info!("Logging initialized at level: {}", config.logging.level);
} 