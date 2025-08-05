//! ModSrv REST API
//!
//! Provides lightweight HTTP and WebSocket interfaces

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
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
use crate::model::{ModelManager, ModelMapping};
use crate::template::Template;

/// API server state
#[derive(Clone)]
pub struct ApiState {
    pub model_manager: Arc<ModelManager>,
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
    pub template: Option<String>,
    pub channel: u32,
    pub data_count: usize,
    pub action_count: usize,
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
    pub mapping: ModelMapping,
}

/// Model detail response
#[derive(Serialize)]
pub struct ModelDetailResponse {
    pub id: String,
    pub name: String,
    pub template: Option<String>,
    pub mapping: ModelMapping,
}

/// Generic success response
#[derive(Serialize)]
pub struct SuccessResponse {
    pub success: bool,
    pub message: String,
    pub timestamp: i64,
}

/// Template list response
#[derive(Serialize)]
pub struct TemplateListResponse {
    pub templates: Vec<TemplateSummary>,
    pub total: usize,
}

/// Template summary
#[derive(Serialize)]
pub struct TemplateSummary {
    pub id: String,
    pub data_count: usize,
    pub action_count: usize,
}

/// Create model from template request
#[derive(Deserialize)]
pub struct CreateModelFromTemplateRequest {
    pub template_id: String,
    pub model_id: String,
    pub model_name: String,
    pub mapping: crate::model::ModelMapping,
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
            template: model.template.clone(),
            channel: model.mapping.channel,
            data_count: model.mapping.data.len(),
            action_count: model.mapping.action.len(),
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
        .get_model_data(&model_id)
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
        template: model.template.clone(),
        mapping: model.mapping.clone(),
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

    // Create model
    state
        .model_manager
        .create_model(&request.id, &request.name, request.mapping.clone())
        .await
        .map_err(|e| ModelSrvError::redis(format!("Failed to create model: {}", e)))?;

    // Get the created model
    let model = state
        .model_manager
        .get_model(&request.id)
        .await
        .ok_or_else(|| ModelSrvError::InternalError("Failed to get created model".into()))?;

    Ok(Json(ModelDetailResponse {
        id: model.id.clone(),
        name: model.name.clone(),
        template: model.template.clone(),
        mapping: model.mapping.clone(),
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
        .execute_action(&model_id, &control_name, Some(request.value))
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

/// List templates
pub async fn list_templates(State(state): State<ApiState>) -> Json<TemplateListResponse> {
    let templates = state.model_manager.list_templates().await;

    let summaries: Vec<TemplateSummary> = templates
        .into_iter()
        .map(|template| TemplateSummary {
            id: template.id,
            data_count: template.data.len(),
            action_count: template.action.len(),
        })
        .collect();

    let total = summaries.len();

    Json(TemplateListResponse {
        templates: summaries,
        total,
    })
}

/// Get template
pub async fn get_template(
    Path(template_id): Path<String>,
    State(state): State<ApiState>,
) -> std::result::Result<Json<Template>, (StatusCode, Json<ErrorResponse>)> {
    let template = state
        .model_manager
        .get_template(&template_id)
        .await
        .ok_or_else(|| ModelSrvError::NotFound(format!("Template {} not found", template_id)))?;

    Ok(Json(template))
}

/// Create model from template
pub async fn create_model_from_template(
    State(state): State<ApiState>,
    Json(request): Json<CreateModelFromTemplateRequest>,
) -> std::result::Result<Json<SuccessResponse>, (StatusCode, Json<ErrorResponse>)> {
    state
        .model_manager
        .create_model_from_template(
            &request.template_id,
            &request.model_id,
            &request.model_name,
            request.mapping,
        )
        .await?;

    Ok(Json(SuccessResponse {
        success: true,
        message: format!(
            "Model {} created from template {}",
            request.model_id, request.template_id
        ),
        timestamp: chrono::Utc::now().timestamp(),
    }))
}

/// Create API routes
pub fn create_routes(api_state: ApiState) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/models", get(list_models).post(create_model))
        .route("/models/{model_id}", get(get_model).delete(delete_model))
        .route("/models/{model_id}/values", get(get_model_values))
        .route(
            "/models/{model_id}/control/{control_name}",
            post(send_control),
        )
        .route("/templates", get(list_templates))
        .route("/templates/{template_id}", get(get_template))
        .route("/templates/create-model", post(create_model_from_template))
        .with_state(api_state)
}

/// API server
pub struct ApiServer {
    model_manager: Arc<ModelManager>,
    config: Config,
}

impl ApiServer {
    /// Create new API server
    pub fn new(model_manager: Arc<ModelManager>, config: Config) -> Self {
        Self {
            model_manager,
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
            config: self.config.clone(),
        };

        let app = create_routes(api_state);

        let addr = format!("{}:{}", self.config.api.host, self.config.api.port);
        let listener = match TcpListener::bind(&addr).await {
            Ok(listener) => {
                info!("API server started: http://{}", addr);
                let _ = tx.send(Ok(())).await;
                listener
            },
            Err(e) => {
                let err_msg = format!("Failed to bind address {}: {}", addr, e);
                error!("{}", err_msg);
                let _ = tx.send(Err(err_msg.clone())).await;
                return Err(ModelSrvError::io(err_msg));
            },
        };

        axum::serve(listener, app)
            .await
            .map_err(|e| ModelSrvError::io(format!("API server error: {}", e)))?;

        Ok(())
    }
}
