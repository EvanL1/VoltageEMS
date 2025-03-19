use crate::config::Config;
use crate::error::{ModelSrvError, Result};
use crate::template::TemplateManager;
use crate::storage_agent::StorageAgent;
use crate::model::ModelEngine;
use crate::control::ControlManager;

use std::sync::Arc;
use tokio::sync::Mutex;
use warp::{self, Filter, Reply, Rejection, reject};
use serde::{Deserialize, Serialize};
use log::{info, error};

// Shared state
#[derive(Clone)]
pub struct ApiState {
    pub config: Config,
    pub storage_agent: Arc<StorageAgent>,
    pub model_engine: Arc<Mutex<ModelEngine>>,
    pub control_manager: Arc<Mutex<ControlManager>>,
}

// Template list response
#[derive(Debug, Serialize, Deserialize)]
pub struct TemplateListResponse {
    templates: Vec<TemplateInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TemplateInfo {
    id: String,
    name: String,
    description: String,
    file_path: String,
}

// Create instance request
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateInstanceRequest {
    template_id: String,
    instance_id: String,
    name: Option<String>,
}

// Create instance response
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateInstanceResponse {
    success: bool,
    message: String,
    instance_id: String,
}

// Error response
#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    status: String,
    message: String,
}

// Health handler
async fn health_handler() -> impl Reply {
    warp::reply::json(&serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "redis_connected": true,
    }))
}

// List templates handler - no Result wrapper
async fn list_templates_handler(state: ApiState) -> impl Reply {
    let template_manager = TemplateManager::new(&state.config.model.templates_dir, &state.config.redis.key_prefix);
    
    match template_manager.list_templates() {
        Ok(templates) => {
            let template_infos: Vec<TemplateInfo> = templates
                .into_iter()
                .map(|template| TemplateInfo {
                    id: template.id,
                    name: template.name,
                    description: template.description,
                    file_path: template.file_path,
                })
                .collect();
            
            warp::reply::json(&TemplateListResponse {
                templates: template_infos,
            })
        },
        Err(e) => {
            error!("Error listing templates: {}", e);
            warp::reply::json(&ErrorResponse {
                status: "error".to_string(),
                message: e.to_string(),
            })
        }
    }
}

// Create instance handler - no Result wrapper
async fn create_instance_handler(create_req: CreateInstanceRequest, state: ApiState) -> impl Reply {
    info!("Creating instance from template: {}", create_req.template_id);
    
    let mut template_manager = TemplateManager::new(&state.config.model.templates_dir, &state.config.redis.key_prefix);
    let store = state.storage_agent.store();
    
    match template_manager.create_instance(
        &*store, 
        &create_req.template_id, 
        &create_req.instance_id, 
        create_req.name.as_deref()
    ) {
        Ok(_) => {
            warp::reply::json(&CreateInstanceResponse {
                success: true,
                message: format!("Instance {} created successfully", create_req.instance_id),
                instance_id: create_req.instance_id,
            })
        },
        Err(e) => {
            error!("Error creating instance: {}", e);
            warp::reply::json(&ErrorResponse {
                status: "error".to_string(),
                message: e.to_string(),
            })
        }
    }
}

// List control operations handler - no Result wrapper
async fn list_control_operations_handler(state: ApiState) -> impl Reply {
    let mut control_manager = state.control_manager.lock().await;
    let store = state.storage_agent.store();
    
    match control_manager.load_operations(&*store, &state.config.control.operation_key_pattern) {
        Ok(_) => {
            let operations = control_manager.get_operations();
            warp::reply::json(&operations)
        },
        Err(e) => {
            error!("Error listing control operations: {}", e);
            warp::reply::json(&ErrorResponse {
                status: "error".to_string(),
                message: e.to_string(),
            })
        }
    }
}

// Start API server
pub async fn start_api_server(config: Config) -> Result<()> {
    // Check if API is enabled
    if !config.api.enabled {
        info!("API server is disabled in configuration");
        return Ok(());
    }
    
    // Create resources
    let storage_agent = Arc::new(StorageAgent::new(config.clone())?);
    let model_engine = Arc::new(Mutex::new(ModelEngine::new()));
    let control_manager = Arc::new(Mutex::new(ControlManager::new(&config.redis.key_prefix)));
    
    // Create shared state
    let state = ApiState {
        config: config.clone(),
        storage_agent,
        model_engine,
        control_manager,
    };
    
    // Create health route
    let health_route = warp::path!("api" / "v1" / "health")
        .and(warp::get())
        .and_then(|| async {
            Ok::<_, warp::Rejection>(health_handler().await)
        });
    
    // Clone state for each route
    let state_clone = state.clone();
    let templates_route = warp::path!("api" / "v1" / "templates")
        .and(warp::get())
        .and(warp::any().map(move || state_clone.clone()))
        .and_then(|state: ApiState| async move {
            Ok::<_, warp::Rejection>(list_templates_handler(state).await)
        });
    
    // Clone state for each route
    let state_clone = state.clone();
    let create_instance_route = warp::path!("api" / "v1" / "instances")
        .and(warp::post())
        .and(warp::body::json())
        .and(warp::any().map(move || state_clone.clone()))
        .and_then(|create_req: CreateInstanceRequest, state: ApiState| async move {
            Ok::<_, warp::Rejection>(create_instance_handler(create_req, state).await)
        });
    
    // Clone state for each route
    let state_clone = state.clone();
    let control_operations_route = warp::path!("api" / "v1" / "control" / "operations")
        .and(warp::get())
        .and(warp::any().map(move || state_clone.clone()))
        .and_then(|state: ApiState| async move {
            Ok::<_, warp::Rejection>(list_control_operations_handler(state).await)
        });
    
    // Combine routes
    let routes = health_route
        .or(templates_route)
        .or(create_instance_route)
        .or(control_operations_route)
        .with(warp::cors().allow_any_origin());
    
    // Parse host address
    let host_parts: Vec<u8> = config.api.host
        .split('.')
        .filter_map(|part| part.parse().ok())
        .collect();
    
    let host = if host_parts.len() == 4 {
        [host_parts[0], host_parts[1], host_parts[2], host_parts[3]]
    } else {
        [0, 0, 0, 0]
    };
    
    // Set address and port
    let addr = std::net::SocketAddr::from((host, config.api.port));
    
    // Start server
    info!("Starting API server at http://{}:{}", config.api.host, config.api.port);
    warp::serve(routes).run(addr).await;
    
    Ok(())
} 