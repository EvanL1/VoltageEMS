//! Configuration management API module
//! Provides RESTful API for CRUD operations on configuration

use crate::config::{Config, DataMapping};
use crate::Result;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post, put},
    Json, Router,
};
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use tower_http::cors::CorsLayer;

type SharedState = Arc<ApiState>;

/// API state
#[derive(Clone)]
pub struct ApiState {
    pub config: Arc<RwLock<Config>>,
    pub config_path: String,
    pub update_tx: Option<tokio::sync::mpsc::Sender<()>>,
}

/// API error response
#[derive(serde::Serialize)]
struct ErrorResponse {
    error: String,
}

/// Mapping query response
#[derive(serde::Serialize)]
struct MappingResponse {
    found: bool,
    mapping: Option<DataMapping>,
}

/// Configuration update response
#[derive(serde::Serialize)]
struct UpdateResponse {
    success: bool,
    message: String,
}

impl ApiState {
    #[allow(dead_code)]
    pub fn new(config: Arc<RwLock<Config>>, config_path: String) -> Self {
        Self {
            config,
            config_path,
            update_tx: None,
        }
    }

    pub fn with_update_channel(
        config: Arc<RwLock<Config>>,
        config_path: String,
        update_tx: tokio::sync::mpsc::Sender<()>,
    ) -> Self {
        Self {
            config,
            config_path,
            update_tx: Some(update_tx),
        }
    }
}

/// Create configuration management API routes
pub fn create_router(state: ApiState) -> Router {
    let shared_state = Arc::new(state);

    Router::new()
        .route("/config", get(get_config))
        .route("/mappings", get(list_mappings))
        .route("/mappings", post(add_mapping))
        .route("/mappings/{source}", get(find_mapping))
        .route("/mappings/{source}", put(update_mapping))
        .route("/mappings/{source}", delete(remove_mapping))
        .route("/reload", post(reload_config))
        .route("/validate", post(validate_config))
        .layer(CorsLayer::permissive())
        .with_state(shared_state)
}

/// Start API server
#[allow(dead_code)]
pub async fn start_api_server(
    port: u16,
    config: Arc<RwLock<Config>>,
    config_path: String,
) -> Result<()> {
    let state = ApiState::new(config, config_path);
    let app = create_router(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("Starting config API server on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app.into_make_service()).await?;

    Ok(())
}

// API handler functions

/// Get complete configuration
async fn get_config(State(state): State<SharedState>) -> impl IntoResponse {
    match state.config.read() {
        Ok(config) => Json(&*config).into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "Failed to read configuration".to_string(),
            }),
        )
            .into_response(),
    }
}

/// List all mappings
async fn list_mappings(State(state): State<SharedState>) -> impl IntoResponse {
    match state.config.read() {
        Ok(config) => Json(&config.mappings).into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "Failed to read configuration".to_string(),
            }),
        )
            .into_response(),
    }
}

/// Find specific mapping
async fn find_mapping(
    State(state): State<SharedState>,
    Path(source): Path<String>,
) -> impl IntoResponse {
    match state.config.read() {
        Ok(config) => {
            let mapping = config.find_mapping(&source).cloned();
            Json(MappingResponse {
                found: mapping.is_some(),
                mapping,
            })
            .into_response()
        }
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "Failed to read configuration".to_string(),
            }),
        )
            .into_response(),
    }
}

/// Add new mapping
async fn add_mapping(
    State(state): State<SharedState>,
    Json(mapping): Json<DataMapping>,
) -> impl IntoResponse {
    // Complete all operations requiring lock in one scope
    let save_result = {
        let mut config = match state.config.write() {
            Ok(config) => config,
            Err(_) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: "Failed to acquire write lock".to_string(),
                    }),
                )
                    .into_response()
            }
        };

        match config.add_mapping(mapping) {
            Ok(()) => config.save_to_file(&state.config_path),
            Err(e) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: e.to_string(),
                    }),
                )
                    .into_response()
            }
        }
    }; // Lock is automatically released here

    // Check save result
    if let Err(e) = save_result {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to save configuration: {}", e),
            }),
        )
            .into_response();
    }

    // Notify poller that configuration has been updated
    if let Some(tx) = &state.update_tx {
        if let Err(e) = tx.send(()).await {
            tracing::error!("Failed to notify poller: {}", e);
        }
    }

    (
        StatusCode::CREATED,
        Json(UpdateResponse {
            success: true,
            message: "Mapping added successfully".to_string(),
        }),
    )
        .into_response()
}

/// Update mapping
async fn update_mapping(
    State(state): State<SharedState>,
    Path(source): Path<String>,
    Json(mapping): Json<DataMapping>,
) -> impl IntoResponse {
    // Complete all operations requiring lock in one scope
    let save_result = {
        let mut config = match state.config.write() {
            Ok(config) => config,
            Err(_) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: "Failed to acquire write lock".to_string(),
                    }),
                )
                    .into_response()
            }
        };

        match config.update_mapping(&source, mapping) {
            Ok(()) => config.save_to_file(&state.config_path),
            Err(e) => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(ErrorResponse {
                        error: e.to_string(),
                    }),
                )
                    .into_response()
            }
        }
    }; // Lock is automatically released here

    // Check save result
    if let Err(e) = save_result {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to save configuration: {}", e),
            }),
        )
            .into_response();
    }

    // Notify poller that configuration has been updated
    if let Some(tx) = &state.update_tx {
        if let Err(e) = tx.send(()).await {
            tracing::error!("Failed to notify poller: {}", e);
        }
    }

    Json(UpdateResponse {
        success: true,
        message: "Mapping updated successfully".to_string(),
    })
    .into_response()
}

/// Remove mapping
async fn remove_mapping(
    State(state): State<SharedState>,
    Path(source): Path<String>,
) -> impl IntoResponse {
    // Complete all operations requiring lock in one scope
    let save_result = {
        let mut config = match state.config.write() {
            Ok(config) => config,
            Err(_) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: "Failed to acquire write lock".to_string(),
                    }),
                )
                    .into_response()
            }
        };

        match config.remove_mapping(&source) {
            Ok(()) => config.save_to_file(&state.config_path),
            Err(e) => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(ErrorResponse {
                        error: e.to_string(),
                    }),
                )
                    .into_response()
            }
        }
    }; // Lock is automatically released here

    // Check save result
    if let Err(e) = save_result {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to save configuration: {}", e),
            }),
        )
            .into_response();
    }

    // Notify poller that configuration has been updated
    if let Some(tx) = &state.update_tx {
        if let Err(e) = tx.send(()).await {
            tracing::error!("Failed to notify poller: {}", e);
        }
    }

    Json(UpdateResponse {
        success: true,
        message: "Mapping removed successfully".to_string(),
    })
    .into_response()
}

/// Reload configuration
async fn reload_config(State(state): State<SharedState>) -> impl IntoResponse {
    match Config::reload() {
        Ok(new_config) => {
            // Validate new configuration
            if let Err(e) = new_config.validate() {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: format!("Invalid configuration: {}", e),
                    }),
                )
                    .into_response();
            }

            // Update configuration
            let update_result = match state.config.write() {
                Ok(mut config) => {
                    *config = new_config;
                    Ok(())
                }
                Err(_) => Err("Failed to acquire write lock"),
            };

            if let Err(e) = update_result {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: e.to_string(),
                    }),
                )
                    .into_response();
            }

            // Notify poller that configuration has been updated
            if let Some(tx) = &state.update_tx {
                if let Err(e) = tx.send(()).await {
                    tracing::error!("Failed to notify poller: {}", e);
                }
            }

            Json(UpdateResponse {
                success: true,
                message: "Configuration reloaded successfully".to_string(),
            })
            .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to reload configuration: {}", e),
            }),
        )
            .into_response(),
    }
}

/// Validate configuration
async fn validate_config(State(state): State<SharedState>) -> impl IntoResponse {
    match state.config.read() {
        Ok(config) => match config.validate() {
            Ok(()) => Json(UpdateResponse {
                success: true,
                message: "Configuration is valid".to_string(),
            })
            .into_response(),
            Err(e) => (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: format!("Configuration validation failed: {}", e),
                }),
            )
                .into_response(),
        },
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "Failed to read configuration".to_string(),
            }),
        )
            .into_response(),
    }
}
