use crate::config::Config;
use crate::error::Result;
use crate::storage::DataStore;
use crate::storage::hybrid_store::HybridStore;
use serde_json::{self, json, Value};
use log::info;
use std::sync::Arc;
use warp::{self, Filter};
use std::convert::Infallible;

/// API module for the model service
/// Provides HTTP REST API for the model service
/// Uses warp for routing and request handling

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
    
    // Combine routes
    let routes = health_route
        .or(get_model_route)
        .or(list_models_route)
        .with(warp::cors().allow_any_origin());
    
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