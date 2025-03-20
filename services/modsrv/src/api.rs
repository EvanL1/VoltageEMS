use crate::config::Config;
use crate::error::Result;
use crate::storage::DataStore;
use crate::storage::hybrid_store::HybridStore;
use crate::model::{ModelDefinition, ModelWithActions, ControlAction};
use crate::template::TemplateInfo;
use serde_json::{self, json, Value};
use log::{info, error};
use std::sync::Arc;
use warp::{self, Filter};
use std::convert::Infallible;
use serde::{Serialize, Deserialize};
use warp::http::StatusCode;

/// API module for the model service
/// Provides HTTP REST API for the model service
/// Uses warp for routing and request handling

// Create Instance Request
#[derive(Debug, Deserialize)]
struct CreateInstanceRequest {
    template_id: String,
    instance_id: String,
    config: Value
}

// Execute Operation Request
#[derive(Debug, Deserialize)]
struct ExecuteOperationRequest {
    instance_id: String,
    parameters: Value
}

/// Start the API server
pub async fn start_api_server(config: Config, store: Arc<HybridStore>) -> Result<()> {
    let store_filter = warp::any().map(move || store.clone());
    
    // Health check endpoint
    let health_route = warp::path!("api" / "health")
        .and(warp::get())
        .map(|| {
            warp::reply::json(&json!({
                "status": "ok",
                "version": env!("CARGO_PKG_VERSION")
            }))
        });
    
    // Get model endpoint
    let get_model_route = warp::path!("api" / "models" / String)
        .and(warp::get())
        .and(store_filter.clone())
        .map(|id: String, store: Arc<HybridStore>| {
            let model_key = format!("model:{}", id);
            info!("Getting model with key: {}", model_key);
            
            match store.get_string(&model_key) {
                Ok(model_json) => {
                    warp::reply::json(&json!({
                        "id": id,
                        "model": serde_json::from_str::<Value>(&model_json).unwrap_or(json!({}))
                    }))
                },
                Err(e) => {
                    info!("Error getting model: {:?}", e);
                    warp::reply::json(&json!({
                        "error": format!("Model not found: {}", e)
                    }))
                }
            }
        });
    
    // List models endpoint
    let list_models_route = warp::path!("api" / "models")
        .and(warp::get())
        .and(store_filter.clone())
        .map(|store: Arc<HybridStore>| {
            let model_pattern = "model:*";
            info!("Listing models with pattern: {}", model_pattern);
            
            match store.get_keys(model_pattern) {
                Ok(keys) => {
                    let models = keys.iter()
                        .map(|key| {
                            let id = key.replace("model:", "");
                            match store.get_string(key) {
                                Ok(model_json) => json!({
                                    "id": id,
                                    "model": serde_json::from_str::<Value>(&model_json).unwrap_or(json!({}))
                                }),
                                Err(_) => json!({
                                    "id": id,
                                    "error": "Failed to get model data"
                                })
                            }
                        })
                        .collect::<Vec<_>>();
                    
                    warp::reply::json(&json!({
                        "models": models
                    }))
                },
                Err(e) => {
                    info!("Error listing models: {:?}", e);
                    warp::reply::json(&json!({
                        "error": format!("Failed to list models: {}", e)
                    }))
                }
            }
        });
    
    // List templates endpoint
    let templates_route = warp::path!("api" / "templates")
        .and(warp::get())
        .and(store_filter.clone())
        .map(|store: Arc<HybridStore>| {
            info!("Listing templates");
            // In real implementation, we'd fetch templates from a templates directory
            // For now, return a mock template
            warp::reply::json(&json!({
                "templates": [
                    {
                        "id": "stepper_motor_template",
                        "name": "Stepper Motor Template",
                        "description": "Template for stepper motor control",
                        "file_path": "templates/stepper_motor.json"
                    }
                ]
            }))
        });
    
    // Create instance endpoint
    let create_instance_route = warp::path!("api" / "instances")
        .and(warp::post())
        .and(warp::body::json())
        .and(store_filter.clone())
        .map(|req: CreateInstanceRequest, store: Arc<HybridStore>| {
            info!("Creating instance with ID: {} from template: {}", 
                  req.instance_id, req.template_id);
            
            // In real implementation, we'd instantiate from template
            // For now, just create a basic model instance
            let instance_key = format!("model:{}", req.instance_id);
            
            let instance = json!({
                "id": req.instance_id,
                "name": req.config.get("name").and_then(|v| v.as_str()).unwrap_or("Unnamed Instance"),
                "description": req.config.get("description").and_then(|v| v.as_str()).unwrap_or(""),
                "template_id": req.template_id,
                "parameters": req.config.get("parameters").unwrap_or(&json!({})),
                "enabled": true
            });
            
            match store.set_string(&instance_key, &instance.to_string()) {
                Ok(_) => {
                    warp::reply::with_status(
                        warp::reply::json(&json!({
                            "id": req.instance_id,
                            "status": "created",
                            "instance": instance
                        })),
                        StatusCode::CREATED
                    )
                },
                Err(e) => {
                    error!("Error creating instance: {:?}", e);
                    warp::reply::with_status(
                        warp::reply::json(&json!({
                            "error": format!("Failed to create instance: {}", e)
                        })),
                        StatusCode::INTERNAL_SERVER_ERROR
                    )
                }
            }
        });
    
    // List control operations endpoint
    let list_operations_route = warp::path!("api" / "control" / "operations")
        .and(warp::get())
        .map(|| {
            info!("Listing control operations");
            
            // In real implementation, we'd fetch available operations
            // For now, return mock operations
            warp::reply::json(&json!([
                "start_motor",
                "stop_motor",
                "set_speed"
            ]))
        });
    
    // Execute control operation endpoint
    let execute_operation_route = warp::path!("api" / "control" / "execute" / String)
        .and(warp::post())
        .and(warp::body::json())
        .and(store_filter.clone())
        .map(|operation: String, req: ExecuteOperationRequest, store: Arc<HybridStore>| {
            info!("Executing operation: {} on instance: {} with parameters: {:?}", 
                  operation, req.instance_id, req.parameters);
            
            // In real implementation, we'd execute the actual operation
            // For now, just record the operation attempt
            let operation_key = format!("operation:{}:{}", req.instance_id, operation);
            let operation_data = json!({
                "operation": operation,
                "instance_id": req.instance_id,
                "parameters": req.parameters,
                "timestamp": chrono::Utc::now().to_rfc3339(),
                "status": "executed"
            });
            
            match store.set_string(&operation_key, &operation_data.to_string()) {
                Ok(_) => {
                    warp::reply::json(&json!({
                        "operation": operation,
                        "instance_id": req.instance_id,
                        "status": "success",
                        "message": format!("Operation {} executed successfully", operation)
                    }))
                },
                Err(e) => {
                    error!("Error executing operation: {:?}", e);
                    warp::reply::json(&json!({
                        "error": format!("Failed to execute operation: {}", e)
                    }))
                }
            }
        });
    
    // Combine routes
    let routes = health_route
        .or(get_model_route)
        .or(list_models_route)
        .or(templates_route)
        .or(create_instance_route)
        .or(list_operations_route)
        .or(execute_operation_route)
        .with(warp::cors().allow_any_origin())
        .recover(handle_rejection);
    
    // Parse host and port from config
    let port = config.api.port;
    let host = [0, 0, 0, 0]; // Use default binding to all interfaces
    
    info!("Starting API server at http://{}:{}", 
          host.iter().map(|n| n.to_string()).collect::<Vec<_>>().join("."), 
          port);
    
    warp::serve(routes)
        .run((host, port))
        .await;
    
    Ok(())
}

// Custom rejection handler function
async fn handle_rejection(err: warp::Rejection) -> std::result::Result<impl warp::Reply, Infallible> {
    let code = if err.is_not_found() {
        warp::http::StatusCode::NOT_FOUND
    } else {
        warp::http::StatusCode::INTERNAL_SERVER_ERROR
    };

    Ok(warp::reply::with_status(
        warp::reply::json(&json!({
            "error": "An error occurred",
            "code": code.as_u16()
        })),
        code,
    ))
}