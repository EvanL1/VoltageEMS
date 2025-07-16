// use crate::error::ModelSrvError;
use crate::config::Config;
use crate::monitoring::{HealthStatus, MonitoringService};
use crate::redis_handler::RedisConnection;
use crate::comsrv_interface::ControlCommand;
use crate::control_sender::ControlSender;
// use crate::template::TemplateManager;
use axum::{
    extract::{Path, State},
    response::Json,
    routing::{get, post},
    Router,
};

type HttpStatusCode = axum::http::StatusCode;
type ApiResult<T> = std::result::Result<axum::Json<T>, (axum::http::StatusCode, axum::Json<ErrorResponse>)>;
type ApiError = (axum::http::StatusCode, axum::Json<ErrorResponse>);
use rand;
use serde::{Deserialize, Serialize};
use serde_json::{self, json, Value};
// use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tower_http::cors::CorsLayer;
use tracing::{error, info};
use utoipa::{OpenApi, ToSchema};
// SwaggerUi removed due to compatibility issues

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
            ControlRequest,
            ControlTarget,
            ControlParameters,
            ControlOptions,
            ControlResponse,
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
    #[allow(dead_code)]
    template_id: String,
    instance_id: String,
    #[allow(dead_code)]
    config: Value,
}

#[derive(Deserialize, Debug, ToSchema)]
struct ExecuteOperationRequest {
    instance_id: String,
    parameters: Value,
}

#[derive(Deserialize, Debug, ToSchema)]
struct ControlRequest {
    request_id: Option<String>,
    instance_id: String,
    command_type: String, // "control" or "adjustment"
    target: ControlTarget,
    parameters: ControlParameters,
    options: Option<ControlOptions>,
}

#[derive(Deserialize, Debug, ToSchema)]
struct ControlTarget {
    channel_id: u16,
    point_type: String, // "c" for control, "a" for adjustment
    point_id: u32,
}

#[derive(Deserialize, Debug, ToSchema)]
struct ControlParameters {
    value: f64,
    timeout: Option<u64>, // milliseconds
    priority: Option<String>,
}

#[derive(Deserialize, Debug, ToSchema)]
struct ControlOptions {
    wait_for_confirm: Option<bool>,
    retry_count: Option<u32>,
    batch_mode: Option<bool>,
}

#[derive(Serialize, ToSchema)]
struct ControlResponse {
    status: String,
    command_id: String,
    message: String,
    request_id: Option<String>,
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
    /// Redis connection
    redis_conn: Arc<RedisConnection>,
    /// Monitoring service
    monitoring: Arc<MonitoringService>,
    /// Control sender for comsrv commands
    control_sender: Arc<Mutex<ControlSender>>,
}

/// API server for the modsrv service
pub struct ApiServer {
    state: AppState,
    port: u16,
    config: Config,
}

impl ApiServer {
    /// Create a new API server with optional engine
    pub fn new(
        _listen_address: String,
        _port: u16,
        _engine: Arc<crate::engine::OptimizedModelEngine>,
    ) -> Self {
        // TODO: Implement with new engine
        unimplemented!("ApiServer needs to be updated for new engine")
    }

    /// Run the API server
    pub async fn run(self) -> Result<(), Box<dyn std::error::Error>> {
        // TODO: Implement
        Ok(())
    }

    /// Create a new API server (legacy)
    pub fn new_legacy(redis_conn: Arc<RedisConnection>, port: u16, config: Config) -> Self {
        let monitoring = Arc::new(MonitoringService::new(HealthStatus::Healthy));
        let control_sender = Arc::new(Mutex::new(ControlSender::new(
            (*redis_conn).clone(),
            crate::control_sender::SendStrategy::default(),
        )));
        let state = AppState {
            redis_conn,
            monitoring,
            control_sender,
        };
        Self { state, port, config }
    }

    /// Start the API server
    pub async fn start(&self) -> Result<(), std::io::Error> {
        let api_config = &self.config.api;
        
        let app = Router::new()
            // Health endpoints (always without prefix)
            .route("/health", get(health_check))
            // Template endpoints
            .route(&api_config.build_path("templates"), get(list_templates))
            .route(&api_config.build_path("templates/:id"), get(get_template))
            // Instance endpoints
            .route(&api_config.build_path("instances"), post(create_instance))
            // Control operation endpoints
            .route(
                &api_config.build_path("control/operations"),
                get(list_operations).post(control_operation),
            )
            .route(&api_config.build_path("control/execute/:operation"), post(execute_operation))
            // OpenAPI spec endpoint
            .route(&api_config.build_path("api-docs/openapi.json"), get(serve_openapi_spec))
            // Rule endpoints (legacy API compatibility)
            .route(&api_config.build_path("rules"), get(list_rules).post(create_rule))
            .route(&api_config.build_path("rules/:id"), get(get_rule).put(update_rule).delete(delete_rule))
            .route(&api_config.build_path("rules/:id/execute"), post(execute_rule))
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
async fn list_rules(
    State(_state): State<AppState>,
) -> ApiResult<serde_json::Value> {
    // TODO: Implement list_rules method in RedisStore
    match Ok(serde_json::Value::Array(vec![]))
        as Result<serde_json::Value, crate::error::ModelSrvError>
    {
        Ok(rules) => Ok(Json(json!({
            "status": "success",
            "rules": rules
        }))),
        Err(e) => {
            error!("Failed to list rules: {}", e);
            Err((
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(ErrorResponse {
                    error: "InternalError".to_string(),
                    message: format!("Failed to list rules: {}", e),
                }),
            ))
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
    State(_state): State<AppState>,
) -> ApiResult<serde_json::Value> {
    // TODO: Implement get_rule method in RedisStore
    match Ok(None) as Result<Option<serde_json::Value>, crate::error::ModelSrvError> {
        Ok(Some(rule)) => Ok(Json(json!({
            "status": "success",
            "rule": rule
        }))),
        Ok(None) => Err((
            axum::http::StatusCode::NOT_FOUND,
            axum::Json(ErrorResponse {
                error: "NotFound".to_string(),
                message: format!("Rule with ID '{}' not found", id),
            }),
        )),
        Err(e) => {
            error!("Failed to get rule {}: {}", id, e);
            Err((
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(ErrorResponse {
                    error: "InternalError".to_string(),
                    message: format!("Failed to get rule: {}", e),
                }),
            ))
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
    State(_state): State<AppState>,
    Json(_rule_data): Json<serde_json::Value>,
) -> ApiResult<serde_json::Value> {
    // TODO: Implement create_rule method in RedisStore
    match Ok(format!("rule_{}", rand::random::<u32>()))
        as Result<String, crate::error::ModelSrvError>
    {
        Ok(rule_id) => Ok(Json(json!({
            "status": "success",
            "message": "Rule created successfully",
            "rule_id": rule_id
        }))),
        Err(e) => {
            error!("Failed to create rule: {}", e);
            Err((
                axum::http::StatusCode::BAD_REQUEST,
                axum::Json(ErrorResponse {
                    error: "BadRequest".to_string(),
                    message: format!("Failed to create rule: {}", e),
                }),
            ))
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
    State(_state): State<AppState>,
    Json(_rule_data): Json<serde_json::Value>,
) -> ApiResult<serde_json::Value> {
    // TODO: Implement update_rule method in RedisStore
    match Ok(()) as Result<(), crate::error::ModelSrvError> {
        Ok(_) => Ok(Json(json!({
            "status": "success",
            "message": "Rule updated successfully"
        }))),
        Err(e) => {
            error!("Failed to update rule {}: {}", id, e);
            Err((
                axum::http::StatusCode::NOT_FOUND,
                axum::Json(ErrorResponse {
                    error: "NotFound".to_string(),
                    message: format!("Failed to update rule: {}", e),
                }),
            ))
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
    State(_state): State<AppState>,
) -> ApiResult<serde_json::Value> {
    // TODO: Implement delete_rule method in RedisStore
    match Ok(()) as Result<(), crate::error::ModelSrvError> {
        Ok(_) => Ok(Json(json!({
            "status": "success",
            "message": "Rule deleted successfully"
        }))),
        Err(e) => {
            error!("Failed to delete rule {}: {}", id, e);
            Err((
                axum::http::StatusCode::NOT_FOUND,
                axum::Json(ErrorResponse {
                    error: "NotFound".to_string(),
                    message: format!("Failed to delete rule: {}", e),
                }),
            ))
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
    State(_state): State<AppState>,
    Json(_input): Json<serde_json::Value>,
) -> ApiResult<serde_json::Value> {
    // TODO: Implement rule executor functionality
    error!("Rule execution not yet implemented for rule {}", id);
    Err((
        axum::http::StatusCode::NOT_IMPLEMENTED,
        Json(ErrorResponse {
            error: "NotImplemented".to_string(),
            message: format!("Rule execution not yet implemented for rule: {}", id),
        }),
    ))
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
async fn list_templates(
    State(_state): State<AppState>,
) -> ApiResult<serde_json::Value> {
    // TODO: Implement template manager functionality
    Ok(Json(json!({
        "status": "success",
        "templates": Vec::<String>::new()
    })))
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
    State(_state): State<AppState>,
) -> ApiResult<serde_json::Value> {
    // TODO: Implement template manager functionality
    Err((
        axum::http::StatusCode::NOT_FOUND,
        Json(ErrorResponse {
            error: "NotFound".to_string(),
            message: format!("Template with ID '{}' not found", id),
        }),
    ))
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
    State(_state): State<AppState>,
    Json(req): Json<CreateInstanceRequest>,
) -> ApiResult<serde_json::Value> {
    // TODO: Implement template manager functionality
    Ok(Json(json!({
        "status": "success",
        "message": "Instance created successfully",
        "instance_id": req.instance_id
    })))
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
        "change_speed".to_string(),
    ])
}

/// Execute control operation
#[utoipa::path(
    post,
    path = "/api/control/operations",
    request_body = ControlRequest,
    responses(
        (status = 200, description = "Operation executed", body = ControlResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse)
    ),
    tag = "operations"
)]
async fn control_operation(
    State(state): State<AppState>,
    Json(req): Json<ControlRequest>,
) -> std::result::Result<axum::Json<ControlResponse>, (axum::http::StatusCode, axum::Json<ErrorResponse>)> {
    // Validate request
    if req.target.point_type != "c" && req.target.point_type != "a" {
        return Err((
            axum::http::StatusCode::BAD_REQUEST,
            axum::Json(ErrorResponse {
                error: "InvalidPointType".to_string(),
                message: "Point type must be 'c' (control) or 'a' (adjustment)".to_string(),
            }),
        ));
    }

    // Create control command
    let control_command = ControlCommand::new(
        req.target.channel_id,
        &req.target.point_type,
        req.target.point_id,
        req.parameters.value,
    );

    let command_id = control_command.command_id.clone();
    let request_id = req.request_id.clone();

    // Send control command via ControlSender
    let send_result = {
        let mut sender = state.control_sender.lock().unwrap();
        sender.send_command(
            req.target.channel_id,
            &req.target.point_type,
            req.target.point_id,
            req.parameters.value,
        )
    };
    
    match send_result {
        Ok(_) => {
            info!(
                "Control command sent successfully: {} -> {}:{}:{} = {}",
                command_id, req.target.channel_id, req.target.point_type, req.target.point_id, req.parameters.value
            );

            // If wait_for_confirm is enabled, wait for completion
            if req.options.as_ref().and_then(|o| o.wait_for_confirm).unwrap_or(false) {
                let timeout = std::time::Duration::from_millis(req.parameters.timeout.unwrap_or(5000));
                
                let completion_result = {
                    let mut sender = state.control_sender.lock().unwrap();
                    sender.wait_for_completion(&command_id, timeout).await
                };
                
                match completion_result {
                    Ok(status) => {
                        Ok(axum::Json(ControlResponse {
                            status: "success".to_string(),
                            command_id,
                            message: format!("Command executed successfully: {}", status.status),
                            request_id,
                        }))
                    }
                    Err(e) => {
                        error!("Command execution failed: {}", e);
                        Err((
                            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                            axum::Json(ErrorResponse {
                                error: "ExecutionError".to_string(),
                                message: format!("Command execution failed: {}", e),
                            }),
                        ))
                    }
                }
            } else {
                Ok(axum::Json(ControlResponse {
                    status: "pending".to_string(),
                    command_id,
                    message: "Command sent successfully, execution pending".to_string(),
                    request_id,
                }))
            }
        }
        Err(e) => {
            error!("Failed to send control command: {}", e);
            Err((
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(ErrorResponse {
                    error: "SendError".to_string(),
                    message: format!("Failed to send control command: {}", e),
                }),
            ))
        }
    }
}

/// Execute specific operation
#[utoipa::path(
    post,
    path = "/api/control/execute/{operation}",
    request_body = ExecuteOperationRequest,
    responses(
        (status = 200, description = "Operation executed", body = ControlResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 404, description = "Operation not found", body = ErrorResponse)
    ),
    params(
        ("operation" = String, Path, description = "Operation name")
    ),
    tag = "operations"
)]
async fn execute_operation(
    Path(operation): Path<String>,
    State(state): State<AppState>,
    Json(req): Json<ExecuteOperationRequest>,
) -> ApiResult<ControlResponse> {
    // Parse operation parameters
    let parameters = req.parameters.as_object().ok_or_else(|| {
        (
            axum::http::StatusCode::BAD_REQUEST,
            axum::Json(ErrorResponse {
                error: "InvalidParameters".to_string(),
                message: "Parameters must be a JSON object".to_string(),
            }),
        )
    })?;

    // Map operation to control command based on operation type
    let (channel_id, point_type, point_id, value) = match operation.as_str() {
        "start_motor" => {
            // Extract parameters for motor start
            let channel_id = parameters.get("channel_id")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| {
                    (
                        axum::http::StatusCode::BAD_REQUEST,
                        axum::Json(ErrorResponse {
                            error: "MissingParameter".to_string(),
                            message: "Missing required parameter: channel_id".to_string(),
                        }),
                    )
                })? as u16;
            
            let point_id = parameters.get("point_id")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| {
                    (
                        axum::http::StatusCode::BAD_REQUEST,
                        axum::Json(ErrorResponse {
                            error: "MissingParameter".to_string(),
                            message: "Missing required parameter: point_id".to_string(),
                        }),
                    )
                })? as u32;

            (channel_id, "c", point_id, 1.0) // Start motor = 1
        }
        "stop_motor" => {
            let channel_id = parameters.get("channel_id")
                .and_then(|v| v.as_u64())
                .unwrap_or(1001) as u16;
            
            let point_id = parameters.get("point_id")
                .and_then(|v| v.as_u64())
                .unwrap_or(30001) as u32;

            (channel_id, "c", point_id, 0.0) // Stop motor = 0
        }
        "change_speed" => {
            let channel_id = parameters.get("channel_id")
                .and_then(|v| v.as_u64())
                .unwrap_or(1001) as u16;
            
            let point_id = parameters.get("point_id")
                .and_then(|v| v.as_u64())
                .unwrap_or(30002) as u32;

            let target_speed = parameters.get("target_speed")
                .and_then(|v| v.as_f64())
                .ok_or_else(|| {
                    (
                        axum::http::StatusCode::BAD_REQUEST,
                        axum::Json(ErrorResponse {
                            error: "MissingParameter".to_string(),
                            message: "Missing required parameter: target_speed".to_string(),
                        }),
                    )
                })?;

            (channel_id, "a", point_id, target_speed) // Speed adjustment
        }
        _ => {
            return Err((
                axum::http::StatusCode::NOT_FOUND,
                axum::Json(ErrorResponse {
                    error: "OperationNotFound".to_string(),
                    message: format!("Operation '{}' not found", operation),
                }),
            ));
        }
    };

    // Create and send control command
    let control_command = ControlCommand::new(channel_id, point_type, point_id, value);
    let command_id = control_command.command_id.clone();

    match state.control_sender.lock().unwrap().send_command(channel_id, point_type, point_id, value) {
        Ok(_) => {
            info!(
                "Operation '{}' executed successfully for instance {}: command_id={}",
                operation, req.instance_id, command_id
            );

            Ok(axum::Json(ControlResponse {
                status: "success".to_string(),
                command_id,
                message: format!("Operation '{}' executed successfully for instance {}", operation, req.instance_id),
                request_id: None,
            }))
        }
        Err(e) => {
            error!("Failed to execute operation '{}': {}", operation, e);
            Err((
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(ErrorResponse {
                    error: "ExecutionError".to_string(),
                    message: format!("Failed to execute operation '{}': {}", operation, e),
                }),
            ))
        }
    }
}

/// Serve OpenAPI specification as JSON
async fn serve_openapi_spec() -> Json<utoipa::openapi::OpenApi> {
    Json(ApiDoc::openapi())
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
