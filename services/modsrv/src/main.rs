mod config;
mod error;
mod model;
mod redis_handler;
mod comsrv_handler;
mod control;
mod template;

use crate::config::Config;
use crate::error::Result;
use crate::model::ModelEngine;
use crate::redis_handler::RedisConnection;
use crate::comsrv_handler::ComsrvHandler;
use crate::control::ControlManager;
use crate::template::TemplateManager;
use clap::{Parser, Subcommand};
use log::{error, info};
use std::path::PathBuf;
use std::time::Duration;
use tokio::time;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Path to the configuration file
    #[clap(short, long, value_parser, default_value = "modsrv.toml")]
    config: PathBuf,
    
    /// Subcommand
    #[clap(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run the model service
    Run,
    
    /// List available templates
    ListTemplates,
    
    /// Create a new instance from a template
    CreateInstance {
        /// Template ID
        #[clap(long)]
        template: String,
        
        /// Instance ID
        #[clap(long)]
        instance: String,
        
        /// Instance name (optional)
        #[clap(long)]
        name: Option<String>,
    },
    
    /// Create multiple instances from a template
    CreateInstances {
        /// Template ID
        #[clap(long)]
        template: String,
        
        /// Number of instances to create
        #[clap(long)]
        count: usize,
        
        /// Instance prefix
        #[clap(long, default_value = "instance")]
        prefix: String,
        
        /// Starting index
        #[clap(long, default_value = "1")]
        start_index: usize,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command line arguments
    let args = Args::parse();

    // Load configuration
    let config = match Config::new(args.config.to_str().unwrap_or("modsrv.toml")) {
        Ok(config) => config,
        Err(_) => {
            println!("Failed to load configuration, using default");
            Config::default()
        }
    };

    // Initialize logging
    init_logging(&config);

    // Process commands
    match args.command {
        Some(Commands::Run) => run_service(&config).await,
        Some(Commands::ListTemplates) => list_templates(&config),
        Some(Commands::CreateInstance { template, instance, name }) => {
            create_instance(&config, &template, &instance, name.as_deref())
        },
        Some(Commands::CreateInstances { template, count, prefix, start_index }) => {
            create_instances(&config, &template, count, &prefix, start_index)
        },
        None => run_service(&config).await,
    }
}

/// Run the model service
async fn run_service(config: &Config) -> Result<()> {
    info!("Starting Model Service");

    // Initialize Redis connection
    let mut redis = RedisConnection::new();
    if let Err(e) = redis.connect(config) {
        error!("Failed to connect to Redis: {}", e);
        return Err(e);
    }

    // Initialize model engine
    let mut model_engine = ModelEngine::new();

    // Initialize control manager
    let mut control_manager = ControlManager::new(&config.redis.prefix);

    // Initialize Comsrv handler
    let comsrv_handler = ComsrvHandler::new(&config.redis.prefix);

    // Main service loop
    let update_interval = Duration::from_millis(config.model.update_interval_ms);
    let mut interval = time::interval(update_interval);

    loop {
        interval.tick().await;

        // Load model configurations
        if let Err(e) = model_engine.load_models(&mut redis, &config.model.config_key_pattern) {
            error!("Failed to load models: {}", e);
            continue;
        }

        // Load control operations if enabled
        if config.control.enabled {
            if let Err(e) = control_manager.load_operations(&mut redis, &config.control.operation_key_pattern) {
                error!("Failed to load control operations: {}", e);
            }
        }

        // Execute models
        if let Err(e) = model_engine.execute_models(&mut redis) {
            error!("Failed to execute models: {}", e);
        }

        // Check and execute control operations if enabled
        if config.control.enabled {
            if let Err(e) = control_manager.check_and_execute_operations(&mut redis) {
                error!("Failed to execute control operations: {}", e);
            }
        }
    }
}

/// List available templates
fn list_templates(config: &Config) -> Result<()> {
    // Initialize template manager
    let template_manager = TemplateManager::new(&config.model.templates_dir, &config.redis.prefix);
    
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
    // Initialize Redis connection
    let mut redis = RedisConnection::new();
    if let Err(e) = redis.connect(config) {
        error!("Failed to connect to Redis: {}", e);
        return Err(e);
    }
    
    // Initialize template manager
    let mut template_manager = TemplateManager::new(&config.model.templates_dir, &config.redis.prefix);
    
    // Create instance
    template_manager.create_instance(&mut redis, template_id, instance_id, instance_name)?;
    
    println!("Successfully created instance {} from template {}", instance_id, template_id);
    
    Ok(())
}

/// Create multiple instances from a template
fn create_instances(config: &Config, template_id: &str, count: usize, prefix: &str, start_index: usize) -> Result<()> {
    // Initialize Redis connection
    let mut redis = RedisConnection::new();
    if let Err(e) = redis.connect(config) {
        error!("Failed to connect to Redis: {}", e);
        return Err(e);
    }
    
    // Initialize template manager
    let mut template_manager = TemplateManager::new(&config.model.templates_dir, &config.redis.prefix);
    
    // Create instances
    let instance_ids = template_manager.create_instances(&mut redis, template_id, count, prefix, start_index)?;
    
    println!("Successfully created {} instances from template {}:", count, template_id);
    for id in instance_ids {
        println!("  - {}", id);
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