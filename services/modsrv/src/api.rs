//! ModSrv REST API
//!
//! Provides lightweight HTTP and WebSocket interfaces

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{delete, get, post, put},
    Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tracing::{error, info};

use crate::config::Config;
use crate::error::{ModelSrvError, Result};
use crate::model::{ModelConfig, ModelManager, PointConfig};
use crate::websocket::{ws_handler, WsConnectionManager};

/// API server state
#[derive(Clone)]
pub struct ApiState {
    pub model_manager: Arc<ModelManager>,
    pub ws_manager: Arc<WsConnectionManager>,
    pub config: Config,
}

/// Health check response
#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub service: String,
}

/// Model list response
#[derive(Serialize)]
pub struct ModelListResponse {
    pub models: Vec<ModelSummary>,
    pub total: usize,
}

/// Model summary
#[derive(Serialize)]
pub struct ModelSummary {
    pub id: String,
    pub name: String,
    pub description: String,
    pub monitoring_count: usize,
    pub control_count: usize,
}

/// Model values response
#[derive(Serialize)]
pub struct ModelValuesResponse {
    pub model_id: String,
    pub values: HashMap<String, f64>,
    pub timestamp: i64,
}

/// Control command request
#[derive(Deserialize)]
pub struct ControlRequest {
    pub value: f64,
}

/// Control command response
#[derive(Serialize)]
pub struct ControlResponse {
    pub success: bool,
    pub message: String,
    pub timestamp: i64,
}

/// Model creation request
#[derive(Deserialize)]
pub struct CreateModelRequest {
    pub id: String,
    pub name: String,
    pub description: String,
    pub monitoring: HashMap<String, PointConfig>,
    pub control: HashMap<String, PointConfig>,
}

/// Model update request
#[derive(Deserialize)]
pub struct UpdateModelRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub monitoring: Option<HashMap<String, PointConfig>>,
    pub control: Option<HashMap<String, PointConfig>>,
}

/// Model detail response
#[derive(Serialize)]
pub struct ModelDetailResponse {
    pub id: String,
    pub name: String,
    pub description: String,
    pub monitoring: HashMap<String, PointConfig>,
    pub control: HashMap<String, PointConfig>,
    pub created_at: Option<i64>,
    pub updated_at: Option<i64>,
}

/// Generic success response
#[derive(Serialize)]
pub struct SuccessResponse {
    pub success: bool,
    pub message: String,
    pub timestamp: i64,
}

/// API error response
#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub code: String,
    pub timestamp: i64,
}

impl From<ModelSrvError> for (StatusCode, Json<ErrorResponse>) {
    fn from(err: ModelSrvError) -> Self {
        let (status, code) = match &err {
            ModelSrvError::NotFound(_) => (StatusCode::NOT_FOUND, "NOT_FOUND"),
            ModelSrvError::InvalidData(_) => (StatusCode::BAD_REQUEST, "INVALID_DATA"),
            ModelSrvError::InvalidCommand(_) => (StatusCode::BAD_REQUEST, "INVALID_COMMAND"),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR"),
        };

        let response = ErrorResponse {
            error: err.to_string(),
            code: code.to_string(),
            timestamp: chrono::Utc::now().timestamp(),
        };

        (status, Json(response))
    }
}

/// Health check
pub async fn health_check(State(state): State<ApiState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        version: state.config.version.clone(),
        service: state.config.service_name.clone(),
    })
}

/// Get model list
pub async fn list_models(State(state): State<ApiState>) -> Json<ModelListResponse> {
    let models = state.model_manager.list_models().await;

    let model_summaries: Vec<ModelSummary> = models
        .into_iter()
        .map(|model| ModelSummary {
            id: model.id.clone(),
            name: model.name.clone(),
            description: model.description.clone(),
            monitoring_count: model.monitoring_config.len(),
            control_count: model.control_config.len(),
        })
        .collect();

    Json(ModelListResponse {
        total: model_summaries.len(),
        models: model_summaries,
    })
}

/// Get current model values
pub async fn get_model_values(
    Path(model_id): Path<String>,
    State(state): State<ApiState>,
) -> std::result::Result<Json<ModelValuesResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Check if model exists
    if state.model_manager.get_model(&model_id).await.is_none() {
        return Err(ModelSrvError::NotFound(format!("Model {} not found", model_id)).into());
    }

    // Get real-time values from Redis
    let values = state
        .model_manager
        .get_model_values(&model_id)
        .await
        .map_err(|e| ModelSrvError::redis(format!("Failed to get model values: {}", e)))?;

    Ok(Json(ModelValuesResponse {
        model_id,
        values,
        timestamp: chrono::Utc::now().timestamp(),
    }))
}

/// Get model detail information
pub async fn get_model(
    Path(model_id): Path<String>,
    State(state): State<ApiState>,
) -> std::result::Result<Json<ModelDetailResponse>, (StatusCode, Json<ErrorResponse>)> {
    let model = state
        .model_manager
        .get_model(&model_id)
        .await
        .ok_or_else(|| ModelSrvError::NotFound(format!("Model {} not found", model_id)))?;

    Ok(Json(ModelDetailResponse {
        id: model.id.clone(),
        name: model.name.clone(),
        description: model.description.clone(),
        monitoring: model.monitoring_config.clone(),
        control: model.control_config.clone(),
        created_at: None, // TODO: Get from ModelManager
        updated_at: None, // TODO: Get from ModelManager
    }))
}

/// Create new model
pub async fn create_model(
    State(state): State<ApiState>,
    Json(request): Json<CreateModelRequest>,
) -> std::result::Result<Json<ModelDetailResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Check if model already exists
    if state.model_manager.get_model(&request.id).await.is_some() {
        return Err(
            ModelSrvError::InvalidData(format!("Model {} already exists", request.id)).into(),
        );
    }

    // Create model configuration
    let model_config = ModelConfig {
        id: request.id.clone(),
        name: request.name.clone(),
        description: request.description.clone(),
        monitoring: request.monitoring.clone(),
        control: request.control.clone(),
    };

    // Add to ModelManager
    state
        .model_manager
        .create_model(model_config)
        .await
        .map_err(|e| ModelSrvError::redis(format!("Failed to create model: {}", e)))?;

    Ok(Json(ModelDetailResponse {
        id: request.id,
        name: request.name,
        description: request.description,
        monitoring: request.monitoring,
        control: request.control,
        created_at: Some(chrono::Utc::now().timestamp()),
        updated_at: Some(chrono::Utc::now().timestamp()),
    }))
}

/// Update model
pub async fn update_model(
    Path(model_id): Path<String>,
    State(state): State<ApiState>,
    Json(request): Json<UpdateModelRequest>,
) -> std::result::Result<Json<ModelDetailResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Check if model exists
    let existing_model = state
        .model_manager
        .get_model(&model_id)
        .await
        .ok_or_else(|| ModelSrvError::NotFound(format!("Model {} not found", model_id)))?;

    // Create updated model configuration
    let updated_config = ModelConfig {
        id: model_id.clone(),
        name: request.name.unwrap_or_else(|| existing_model.name.clone()),
        description: request
            .description
            .unwrap_or_else(|| existing_model.description.clone()),
        monitoring: request
            .monitoring
            .unwrap_or_else(|| existing_model.monitoring_config.clone()),
        control: request
            .control
            .unwrap_or_else(|| existing_model.control_config.clone()),
    };

    // Update model
    state
        .model_manager
        .update_model(updated_config.clone())
        .await
        .map_err(|e| ModelSrvError::redis(format!("Failed to update model: {}", e)))?;

    Ok(Json(ModelDetailResponse {
        id: updated_config.id,
        name: updated_config.name,
        description: updated_config.description,
        monitoring: updated_config.monitoring,
        control: updated_config.control,
        created_at: None,
        updated_at: Some(chrono::Utc::now().timestamp()),
    }))
}

/// Delete model
pub async fn delete_model(
    Path(model_id): Path<String>,
    State(state): State<ApiState>,
) -> std::result::Result<Json<SuccessResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Check if model exists
    if state.model_manager.get_model(&model_id).await.is_none() {
        return Err(ModelSrvError::NotFound(format!("Model {} not found", model_id)).into());
    }

    // Delete model
    state
        .model_manager
        .delete_model(&model_id)
        .await
        .map_err(|e| ModelSrvError::redis(format!("Failed to delete model: {}", e)))?;

    Ok(Json(SuccessResponse {
        success: true,
        message: format!("Model {} deleted", model_id),
        timestamp: chrono::Utc::now().timestamp(),
    }))
}

/// Send control command
pub async fn send_control(
    Path((model_id, control_name)): Path<(String, String)>,
    State(state): State<ApiState>,
    Json(request): Json<ControlRequest>,
) -> std::result::Result<Json<ControlResponse>, (StatusCode, Json<ErrorResponse>)> {
    state
        .model_manager
        .send_control(&model_id, &control_name, request.value)
        .await?;

    Ok(Json(ControlResponse {
        success: true,
        message: format!(
            "Control command sent: {}.{} = {:.6}",
            model_id, control_name, request.value
        ),
        timestamp: chrono::Utc::now().timestamp(),
    }))
}

/// Create API routes
pub fn create_routes(api_state: ApiState) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/models", get(list_models).post(create_model))
        .route(
            "/models/{model_id}",
            get(get_model).put(update_model).delete(delete_model),
        )
        .route("/models/{model_id}/values", get(get_model_values))
        .route(
            "/models/{model_id}/control/{control_name}",
            post(send_control),
        )
        .route("/ws/{model_id}", get(ws_handler))
        .with_state(api_state)
}

/// API server
pub struct ApiServer {
    model_manager: Arc<ModelManager>,
    ws_manager: Arc<WsConnectionManager>,
    config: Config,
}

impl ApiServer {
    /// Create new API server
    pub fn new(
        model_manager: Arc<ModelManager>,
        ws_manager: Arc<WsConnectionManager>,
        config: Config,
    ) -> Self {
        Self {
            model_manager,
            ws_manager,
            config,
        }
    }

    /// Start API server (with startup notification)
    pub async fn start_with_notification(
        self,
        tx: mpsc::Sender<std::result::Result<(), String>>,
    ) -> Result<()> {
        let api_state = ApiState {
            model_manager: self.model_manager.clone(),
            ws_manager: self.ws_manager.clone(),
            config: self.config.clone(),
        };

        let app = create_routes(api_state);

        let addr = format!("{}:{}", self.config.api.host, self.config.api.port);
        let listener = match TcpListener::bind(&addr).await {
            Ok(listener) => {
                info!("API server started: http://{}", addr);
                let _ = tx.send(Ok(())).await;
                listener
            }
            Err(e) => {
                let err_msg = format!("Failed to bind address {}: {}", addr, e);
                error!("{}", err_msg);
                let _ = tx.send(Err(err_msg.clone())).await;
                return Err(ModelSrvError::io(err_msg));
            }
        };

        axum::serve(listener, app)
            .await
            .map_err(|e| ModelSrvError::io(format!("API server error: {}", e)))?;

        Ok(())
    }
}
