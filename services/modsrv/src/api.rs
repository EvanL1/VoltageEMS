use crate::error::ModelSrvError;
use crate::storage::redis_store::RedisStore;
use crate::rules_engine::RuleExecutor;
use serde_json::{self, json, Value};
use tracing::{info, error};
use std::sync::Arc;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{get, post, put, delete},
    Router,
};
use serde::{Serialize, Deserialize};
use crate::monitoring::{MonitoringService, HealthStatus};
use crate::StorageAgent;
use std::collections::HashMap;
use crate::template::TemplateManager;
use rand;
use tower_http::cors::CorsLayer;
use utoipa::{OpenApi, ToSchema};
use utoipa_swagger_ui::SwaggerUi;

/// API module for the model service
/// Provides HTTP REST API for the model service
/// Uses axum for routing and request handling

#[derive(OpenApi)]
#[openapi(
    paths(
        health_check,
        list_rules,
        get_rule,
        create_rule,
        update_rule,
        delete_rule,
        execute_rule,
        list_templates,
        get_template,
        create_instance,
        list_operations,
        control_operation,
        execute_operation
    ),
    components(
        schemas(
            HealthResponse,
            RuleResponse,
            CreateRuleRequest,
            UpdateRuleRequest,
            ExecuteRuleRequest,
            CreateInstanceRequest,
            ExecuteOperationRequest,
            OperationResponse,
            ErrorResponse
        )
    ),
    tags(
        (name = "health", description = "Health check endpoints"),
        (name = "rules", description = "Rule management endpoints"),
        (name = "templates", description = "Template management endpoints"),
        (name = "instances", description = "Instance management endpoints"),
        (name = "operations", description = "Control operation endpoints")
    )
)]
pub struct ApiDoc;

// Request/Response models
#[derive(Deserialize, Debug, ToSchema)]
struct CreateInstanceRequest {
    template_id: String,
    instance_id: String,
    config: Value
}

#[derive(Deserialize, Debug, ToSchema)]
struct ExecuteOperationRequest {
    instance_id: String,
    parameters: Value
}

#[derive(Deserialize, Debug, ToSchema)]
struct CreateRuleRequest {
    name: String,
    conditions: Value,
    actions: Value,
    enabled: bool,
}

#[derive(Deserialize, Debug, ToSchema)]
struct UpdateRuleRequest {
    name: Option<String>,
    conditions: Option<Value>,
    actions: Option<Value>,
    enabled: Option<bool>,
}

#[derive(Deserialize, Debug, ToSchema)]
struct ExecuteRuleRequest {
    input: Option<Value>,
}

#[derive(Serialize, ToSchema)]
struct HealthResponse {
    status: String,
    version: String,
}

#[derive(Serialize, ToSchema)]
struct RuleResponse {
    id: String,
    name: String,
    enabled: bool,
    conditions: Value,
    actions: Value,
}

#[derive(Serialize, ToSchema)]
struct OperationResponse {
    operations: Vec<String>,
}

#[derive(Serialize, ToSchema)]
struct ErrorResponse {
    error: String,
    message: String,
}

/// Application state
#[derive(Clone)]
pub struct AppState {
    /// Data store
    store: Arc<RedisStore>,
    /// Storage agent
    agent: Arc<StorageAgent>,
    /// Rule executor for running rules
    rule_executor: Arc<RuleExecutor>,
    /// Monitoring service
    monitoring: Arc<MonitoringService>,
}

/// API server for the modsrv service
pub struct ApiServer {
    state: AppState,
    port: u16,
}

impl ApiServer {
    /// Create a new API server
    pub fn new(
        store: Arc<RedisStore>, 
        agent: Arc<StorageAgent>,
        rule_executor: Arc<RuleExecutor>,
        port: u16
    ) -> Self {
        let monitoring = Arc::new(MonitoringService::new(HealthStatus::Healthy));
        let state = AppState {
            store,
            agent,
            rule_executor,
            monitoring,
        };
        Self {
            state,
            port,
        }
    }
    
    /// Start the API server
    pub async fn start(&self) -> Result<(), std::io::Error> {
        let app = Router::new()
            // Health endpoints
            .route("/health", get(health_check))
            
            // Rule endpoints
            .route("/api/rules", get(list_rules).post(create_rule))
            .route("/api/rules/:id", get(get_rule).put(update_rule).delete(delete_rule))
            .route("/api/rules/:id/execute", post(execute_rule))
            
            // Template endpoints
            .route("/api/templates", get(list_templates))
            .route("/api/templates/:id", get(get_template))
            
            // Instance endpoints
            .route("/api/instances", post(create_instance))
            
            // Control operation endpoints
            .route("/api/control/operations", get(list_operations).post(control_operation))
            .route("/api/control/execute/:operation", post(execute_operation))
            
            // Swagger UI
            .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
            
            // CORS
            .layer(CorsLayer::permissive())
            
            // State
            .with_state(self.state.clone());
        
        let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", self.port)).await?;
        info!("Starting API server on port {}", self.port);
        
        axum::serve(listener, app).await?;
        
        Ok(())
    }
}

/// Health check endpoint
#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "Service is healthy", body = HealthResponse)
    ),
    tag = "health"
)]
async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

/// List all rules
#[utoipa::path(
    get,
    path = "/api/rules",
    responses(
        (status = 200, description = "List of rules", body = Vec<RuleResponse>),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "rules"
)]
async fn list_rules(State(state): State<AppState>) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    match state.store.list_rules() {
        Ok(rules) => Ok(Json(json!({
            "status": "success",
            "rules": rules
        }))),
        Err(e) => {
            error!("Failed to list rules: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                error: "InternalError".to_string(),
                message: format!("Failed to list rules: {}", e),
            })))
        }
    }
}

/// Get a specific rule
#[utoipa::path(
    get,
    path = "/api/rules/{id}",
    responses(
        (status = 200, description = "Rule found", body = RuleResponse),
        (status = 404, description = "Rule not found", body = ErrorResponse)
    ),
    params(
        ("id" = String, Path, description = "Rule ID")
    ),
    tag = "rules"
)]
async fn get_rule(
    Path(id): Path<String>,
    State(state): State<AppState>
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    match state.store.get_rule(&id) {
        Ok(Some(rule)) => Ok(Json(json!({
            "status": "success",
            "rule": rule
        }))),
        Ok(None) => Err((StatusCode::NOT_FOUND, Json(ErrorResponse {
            error: "NotFound".to_string(),
            message: format!("Rule with ID '{}' not found", id),
        }))),
        Err(e) => {
            error!("Failed to get rule {}: {}", id, e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                error: "InternalError".to_string(),
                message: format!("Failed to get rule: {}", e),
            })))
        }
    }
}

/// Create a new rule
#[utoipa::path(
    post,
    path = "/api/rules",
    request_body = CreateRuleRequest,
    responses(
        (status = 201, description = "Rule created", body = RuleResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse)
    ),
    tag = "rules"
)]
async fn create_rule(
    State(state): State<AppState>,
    Json(rule_data): Json<serde_json::Value>
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    match state.store.create_rule(rule_data) {
        Ok(rule_id) => Ok(Json(json!({
            "status": "success",
            "message": "Rule created successfully",
            "rule_id": rule_id
        }))),
        Err(e) => {
            error!("Failed to create rule: {}", e);
            Err((StatusCode::BAD_REQUEST, Json(ErrorResponse {
                error: "BadRequest".to_string(),
                message: format!("Failed to create rule: {}", e),
            })))
        }
    }
}

/// Update an existing rule
#[utoipa::path(
    put,
    path = "/api/rules/{id}",
    request_body = UpdateRuleRequest,
    responses(
        (status = 200, description = "Rule updated", body = RuleResponse),
        (status = 404, description = "Rule not found", body = ErrorResponse)
    ),
    params(
        ("id" = String, Path, description = "Rule ID")
    ),
    tag = "rules"
)]
async fn update_rule(
    Path(id): Path<String>,
    State(state): State<AppState>,
    Json(rule_data): Json<serde_json::Value>
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    match state.store.update_rule(&id, rule_data) {
        Ok(_) => Ok(Json(json!({
            "status": "success",
            "message": "Rule updated successfully"
        }))),
        Err(e) => {
            error!("Failed to update rule {}: {}", id, e);
            Err((StatusCode::NOT_FOUND, Json(ErrorResponse {
                error: "NotFound".to_string(),
                message: format!("Failed to update rule: {}", e),
            })))
        }
    }
}

/// Delete a rule
#[utoipa::path(
    delete,
    path = "/api/rules/{id}",
    responses(
        (status = 200, description = "Rule deleted"),
        (status = 404, description = "Rule not found", body = ErrorResponse)
    ),
    params(
        ("id" = String, Path, description = "Rule ID")
    ),
    tag = "rules"
)]
async fn delete_rule(
    Path(id): Path<String>,
    State(state): State<AppState>
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    match state.store.delete_rule(&id) {
        Ok(_) => Ok(Json(json!({
            "status": "success",
            "message": "Rule deleted successfully"
        }))),
        Err(e) => {
            error!("Failed to delete rule {}: {}", id, e);
            Err((StatusCode::NOT_FOUND, Json(ErrorResponse {
                error: "NotFound".to_string(),
                message: format!("Failed to delete rule: {}", e),
            })))
        }
    }
}

/// Execute a rule
#[utoipa::path(
    post,
    path = "/api/rules/{id}/execute",
    request_body = ExecuteRuleRequest,
    responses(
        (status = 200, description = "Rule executed"),
        (status = 404, description = "Rule not found", body = ErrorResponse)
    ),
    params(
        ("id" = String, Path, description = "Rule ID")
    ),
    tag = "rules"
)]
async fn execute_rule(
    Path(id): Path<String>,
    State(state): State<AppState>,
    Json(input): Json<serde_json::Value>
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    match state.rule_executor.execute_rule(&id, Some(input)).await {
        Ok(result) => Ok(Json(result)),
        Err(e) => {
            error!("Failed to execute rule {}: {}", id, e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                error: "ExecutionError".to_string(),
                message: format!("Failed to execute rule: {}", e),
            })))
        }
    }
}

/// List all templates
#[utoipa::path(
    get,
    path = "/api/templates",
    responses(
        (status = 200, description = "List of templates"),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "templates"
)]
async fn list_templates(State(state): State<AppState>) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    match state.store.get_template_manager() {
        Ok(template_manager) => {
            let templates = template_manager.list_templates();
            Ok(Json(json!({
                "status": "success",
                "templates": templates
            })))
        },
        Err(e) => {
            error!("Failed to list templates: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                error: "InternalError".to_string(),
                message: format!("Failed to list templates: {}", e),
            })))
        }
    }
}

/// Get a specific template
#[utoipa::path(
    get,
    path = "/api/templates/{id}",
    responses(
        (status = 200, description = "Template found"),
        (status = 404, description = "Template not found", body = ErrorResponse)
    ),
    params(
        ("id" = String, Path, description = "Template ID")
    ),
    tag = "templates"
)]
async fn get_template(
    Path(id): Path<String>,
    State(state): State<AppState>
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    match state.store.get_template_manager() {
        Ok(template_manager) => {
            match template_manager.get_template(&id) {
                Some(template) => Ok(Json(json!({
                    "status": "success",
                    "template": template
                }))),
                None => Err((StatusCode::NOT_FOUND, Json(ErrorResponse {
                    error: "NotFound".to_string(),
                    message: format!("Template with ID '{}' not found", id),
                })))
            }
        },
        Err(e) => {
            error!("Failed to get template {}: {}", id, e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                error: "InternalError".to_string(),
                message: format!("Failed to get template: {}", e),
            })))
        }
    }
}

/// Create a new instance
#[utoipa::path(
    post,
    path = "/api/instances",
    request_body = CreateInstanceRequest,
    responses(
        (status = 201, description = "Instance created"),
        (status = 400, description = "Invalid request", body = ErrorResponse)
    ),
    tag = "instances"
)]
async fn create_instance(
    State(state): State<AppState>,
    Json(req): Json<CreateInstanceRequest>
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    match state.store.get_template_manager() {
        Ok(mut template_manager) => {
            match template_manager.create_instance(&*state.store, &req.template_id, &req.instance_id, None) {
                Ok(_) => Ok(Json(json!({
                    "status": "success",
                    "message": "Instance created successfully",
                    "instance_id": req.instance_id
                }))),
                Err(e) => {
                    error!("Failed to create instance: {}", e);
                    Err((StatusCode::BAD_REQUEST, Json(ErrorResponse {
                        error: "BadRequest".to_string(),
                        message: format!("Failed to create instance: {}", e),
                    })))
                }
            }
        },
        Err(e) => {
            error!("Failed to get template manager: {}", e);
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse {
                error: "InternalError".to_string(),
                message: format!("Failed to get template manager: {}", e),
            })))
        }
    }
}

/// List available operations
#[utoipa::path(
    get,
    path = "/api/control/operations",
    responses(
        (status = 200, description = "List of operations", body = OperationResponse)
    ),
    tag = "operations"
)]
async fn list_operations(State(_state): State<AppState>) -> Json<Vec<String>> {
    Json(vec![
        "start_motor".to_string(), 
        "stop_motor".to_string(), 
        "change_speed".to_string()
    ])
}

/// Execute control operation
#[utoipa::path(
    post,
    path = "/api/control/operations",
    request_body = ExecuteOperationRequest,
    responses(
        (status = 200, description = "Operation executed"),
        (status = 400, description = "Invalid request", body = ErrorResponse)
    ),
    tag = "operations"
)]
async fn control_operation(
    State(_state): State<AppState>,
    Json(req): Json<ExecuteOperationRequest>
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    // Placeholder implementation
    Ok(Json(json!({
        "status": "success",
        "message": format!("Operation executed for instance {}", req.instance_id)
    })))
}

/// Execute specific operation
#[utoipa::path(
    post,
    path = "/api/control/execute/{operation}",
    request_body = ExecuteOperationRequest,
    responses(
        (status = 200, description = "Operation executed"),
        (status = 400, description = "Invalid request", body = ErrorResponse)
    ),
    params(
        ("operation" = String, Path, description = "Operation name")
    ),
    tag = "operations"
)]
async fn execute_operation(
    Path(operation): Path<String>,
    State(_state): State<AppState>,
    Json(req): Json<ExecuteOperationRequest>
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    // Placeholder implementation
    Ok(Json(json!({
        "status": "success",
        "message": format!("Operation '{}' executed for instance {}", operation, req.instance_id)
    })))
}

/// Add test for the API
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_health_response_serialization() {
        let response = HealthResponse {
            status: "ok".to_string(),
            version: "1.0.0".to_string(),
        };
        
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"status\":\"ok\""));
        assert!(json.contains("\"version\":\"1.0.0\""));
    }
}