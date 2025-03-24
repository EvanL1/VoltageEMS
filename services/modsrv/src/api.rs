use crate::error::ModelSrvError;
use crate::storage::hybrid_store::HybridStore;
use crate::rules_engine::RuleExecutor;
use serde_json::{self, json, Value};
use log::info;
use std::sync::Arc;
use warp::{self, Filter};
use std::convert::Infallible;
use serde::{Serialize, Deserialize};
use warp::http::StatusCode;
use crate::monitoring::{MonitoringService, HealthStatus};
use crate::StorageAgent;
use std::collections::HashMap;

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

/// API server for the modsrv service
pub struct ApiServer {
    /// Data store
    store: Arc<HybridStore>,
    /// Storage agent
    agent: Arc<StorageAgent>,
    /// Rule executor for running rules
    rule_executor: Arc<RuleExecutor>,
    /// Monitoring service
    monitoring: Arc<MonitoringService>,
    port: u16,
}

impl ApiServer {
    /// Create a new API server
    pub fn new(
        store: Arc<HybridStore>, 
        agent: Arc<StorageAgent>,
        rule_executor: Arc<RuleExecutor>,
        port: u16
    ) -> Self {
        let monitoring = Arc::new(MonitoringService::new(HealthStatus::default()));
        Self {
            store,
            agent,
            rule_executor,
            monitoring,
            port,
        }
    }
    
    /// Start the API server
    pub async fn start(&self) -> std::result::Result<(), warp::Error> {
        // Health check route
        let health_route = warp::path("health")
            .map(move || {
                warp::reply::json(&json!({
                    "status": "ok",
                    "version": env!("CARGO_PKG_VERSION")
                }))
            });
            
        // Get monitoring service reference
        let monitoring = self.monitoring.clone();
        
        // Cors configuration
        let cors = warp::cors()
            .allow_any_origin()
            .allow_headers(vec!["content-type"])
            .allow_methods(vec!["GET", "POST", "PUT", "DELETE"]);
            
        // Create API routes
        let store = self.store.clone();
        let rules_list_route = list_rules_route(store.clone());
        let get_rule_route = get_rule_route(store.clone());
        let create_rule_route = create_rule_route(store.clone());
        let update_rule_route = update_rule_route(store.clone());
        let delete_rule_route = delete_rule_route(store.clone());
        let rule_executor = self.rule_executor.clone();
        let execute_rule_route = execute_rule_route(rule_executor);
        
        // Template routes
        let templates_route = list_templates_route(store.clone());
        let get_template_route = get_template_route(store.clone());
        
        // Combine all routes
        let api_routes = health_route
            .or(rules_list_route)
            .or(get_rule_route)
            .or(create_rule_route)
            .or(update_rule_route)
            .or(delete_rule_route)
            .or(execute_rule_route)
            .or(templates_route)
            .or(get_template_route)
            .with(cors);
        
        // Start the server
        info!("Starting API server on port {}", self.port);
        
        warp::serve(api_routes)
            .run(([0, 0, 0, 0], self.port))
            .await;
            
        Ok(())
    }
}

/// Handle rejections to return appropriate responses
async fn handle_rejection(err: warp::Rejection) -> std::result::Result<impl warp::Reply, Infallible> {
    let status;
    let message;
    
    if let Some(e) = err.find::<ModelSrvError>() {
        match e {
            ModelSrvError::ModelNotFound(_) => {
                status = warp::http::StatusCode::NOT_FOUND;
                message = e.to_string();
            }
            ModelSrvError::RuleNotFound(_) => {
                status = warp::http::StatusCode::NOT_FOUND;
                message = e.to_string();
            }
            ModelSrvError::TemplateNotFound(_) => {
                status = warp::http::StatusCode::NOT_FOUND;
                message = e.to_string();
            }
            ModelSrvError::RuleDisabled(_) => {
                status = warp::http::StatusCode::BAD_REQUEST;
                message = e.to_string();
            }
            _ => {
                status = warp::http::StatusCode::INTERNAL_SERVER_ERROR;
                message = e.to_string();
            }
        }
    } else if let Some(e) = err.find::<warp::reject::InvalidQuery>() {
        status = warp::http::StatusCode::BAD_REQUEST;
        message = format!("Invalid query parameters: {}", e);
    } else if let Some(e) = err.find::<warp::reject::InvalidHeader>() {
        status = warp::http::StatusCode::BAD_REQUEST;
        message = format!("Invalid header: {}", e);
    } else if let Some(_) = err.find::<warp::reject::UnsupportedMediaType>() {
        status = warp::http::StatusCode::UNSUPPORTED_MEDIA_TYPE;
        message = "Unsupported media type".to_string();
    } else if let Some(_) = err.find::<warp::reject::MissingHeader>() {
        status = warp::http::StatusCode::BAD_REQUEST;
        message = "Missing required header".to_string();
    } else if let Some(_) = err.find::<warp::reject::PayloadTooLarge>() {
        status = warp::http::StatusCode::PAYLOAD_TOO_LARGE;
        message = "Payload too large".to_string();
    } else if let Some(_) = err.find::<warp::reject::LengthRequired>() {
        status = warp::http::StatusCode::LENGTH_REQUIRED;
        message = "Length required".to_string();
    } else if let Some(_) = err.find::<warp::body::BodyDeserializeError>() {
        status = warp::http::StatusCode::BAD_REQUEST;
        message = "Invalid JSON payload".to_string();
    } else {
        status = warp::http::StatusCode::INTERNAL_SERVER_ERROR;
        message = "Internal server error".to_string();
    }
    
    let json = warp::reply::json(&json!({
        "status": "error",
        "message": message
    }));
    
    Ok(warp::reply::with_status(json, status))
}

#[derive(Serialize)]
struct ErrorResponse {
    code: u16,
    message: String,
}

/// Route for getting metrics for all rules
fn metrics_route(
    monitoring: Arc<MonitoringService>
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("api" / "metrics")
        .and(warp::get())
        .and_then(move || {
            let monitoring = monitoring.clone();
            async move {
                match monitoring.get_all_metrics() {
                    Ok(metrics) => Ok(warp::reply::json(&metrics)),
                    Err(e) => Err(warp::reject::custom(e)),
                }
            }
        })
}

/// Route for getting metrics for a specific rule
fn rule_metrics_route(
    monitoring: Arc<MonitoringService>
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "rules" / String / "metrics")
        .and(warp::get())
        .and_then(move |rule_id: String| {
            let monitoring = monitoring.clone();
            async move {
                match monitoring.get_rule_metrics(&rule_id) {
                    Ok(Some(metrics)) => Ok(warp::reply::json(&metrics)),
                    Ok(None) => Err(warp::reject::not_found()),
                    Err(e) => Err(warp::reject::custom(e)),
                }
            }
        })
}

/// Route for detailed health check
fn detailed_health_route(
    monitoring: Arc<MonitoringService>
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    warp::path!("api" / "health" / "detailed")
        .and(warp::get())
        .and_then(move || {
            let monitoring = monitoring.clone();
            async move {
                match monitoring.run_health_check().await {
                    Ok(health) => {
                        let status = match health.status {
                            HealthStatus::Healthy => StatusCode::OK,
                            HealthStatus::Degraded => StatusCode::OK,
                            HealthStatus::Unhealthy => StatusCode::SERVICE_UNAVAILABLE,
                        };
                        
                        Ok(warp::reply::with_status(
                            warp::reply::json(&health),
                            status
                        ))
                    },
                    Err(e) => Err(warp::reject::custom(e)),
                }
            }
        })
}

/// Add store to filter chain
fn with_store(store: Arc<HybridStore>) -> impl Filter<Extract = (Arc<HybridStore>,), Error = Infallible> + Clone {
    warp::any().map(move || store.clone())
}

fn with_rule_executor(executor: Arc<RuleExecutor>) -> impl Filter<Extract = (Arc<RuleExecutor>,), Error = Infallible> + Clone {
    warp::any().map(move || executor.clone())
}

/// Create API route for listing rules
pub fn list_rules_route(
    store: Arc<HybridStore>
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "rules")
        .and(warp::get())
        .map(move || store.clone())
        .and_then(handle_list_rules)
}

/// Create API route for getting a rule by ID
pub fn get_rule_route(
    store: Arc<HybridStore>
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "rules" / String)
        .and(warp::get())
        .map(move |rule_id: String| (store.clone(), rule_id))
        .and_then(handle_get_rule)
}

/// Create API route for creating a rule
pub fn create_rule_route(
    store: Arc<HybridStore>
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "rules")
        .and(warp::post())
        .and(warp::body::json())
        .map(move |rule_json| (store.clone(), rule_json))
        .and_then(handle_create_rule)
}

/// Create API route for updating a rule
pub fn update_rule_route(
    store: Arc<HybridStore>
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "rules" / String)
        .and(warp::put())
        .and(warp::body::json())
        .map(move |rule_id: String, rule_json| (store.clone(), rule_id, rule_json))
        .and_then(handle_update_rule)
}

/// Create API route for deleting a rule
pub fn delete_rule_route(
    store: Arc<HybridStore>
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "rules" / String)
        .and(warp::delete())
        .map(move |rule_id: String| (store.clone(), rule_id))
        .and_then(handle_delete_rule)
}

/// Create API route for executing a rule
pub fn execute_rule_route(
    executor: Arc<RuleExecutor>
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "rules" / String / "execute")
        .and(warp::post())
        .and(warp::body::json::<serde_json::Value>().or(warp::query::<HashMap<String, String>>().map(|q| json!(q))))
        .map(move |rule_id: String, input: serde_json::Value| {
            (executor.clone(), rule_id, input)
        })
        .and_then(handle_execute_rule)
}

/// Handle request to list all rules
async fn handle_list_rules(
    store: Arc<HybridStore>
) -> std::result::Result<impl warp::Reply, warp::Rejection> {
    let rules = match store.list_rules() {
        Ok(rules) => rules,
        Err(e) => {
            return Ok(warp::reply::json(&json!({
                "status": "error",
                "message": format!("Failed to list rules: {}", e)
            })));
        }
    };
    
    Ok(warp::reply::json(&json!({
        "status": "success",
        "rules": rules
    })))
}

/// Handle request to get a rule by ID
async fn handle_get_rule(
    params: (Arc<HybridStore>, String)
) -> std::result::Result<impl warp::Reply, warp::Rejection> {
    let (store, rule_id) = params;
    
    match store.get_rule(&rule_id) {
        Ok(rule) => {
            Ok(warp::reply::json(&json!({
                "status": "success",
                "rule": rule
            })))
        },
        Err(e) => {
            Ok(warp::reply::json(&json!({
                "status": "error",
                "message": format!("Failed to get rule: {}", e)
            })))
        }
    }
}

/// Handle request to create a rule
async fn handle_create_rule(
    params: (Arc<HybridStore>, serde_json::Value)
) -> std::result::Result<impl warp::Reply, warp::Rejection> {
    let (store, rule_json) = params;
    
    match store.create_rule(&rule_json) {
        Ok(_) => {
            Ok(warp::reply::json(&json!({
                "status": "success",
                "message": "Rule created successfully"
            })))
        },
        Err(e) => {
            Ok(warp::reply::json(&json!({
                "status": "error",
                "message": format!("Failed to create rule: {}", e)
            })))
        }
    }
}

/// Handle request to update a rule
async fn handle_update_rule(
    params: (Arc<HybridStore>, String, serde_json::Value)
) -> std::result::Result<impl warp::Reply, warp::Rejection> {
    let (store, rule_id, rule_copy) = params;
    
    match store.update_rule(&rule_id, &rule_copy) {
        Ok(_) => {
            Ok(warp::reply::json(&json!({
                "status": "success",
                "message": "Rule updated successfully"
            })))
        },
        Err(e) => {
            Ok(warp::reply::json(&json!({
                "status": "error",
                "message": format!("Failed to update rule: {}", e)
            })))
        }
    }
}

/// Handle request to delete a rule
async fn handle_delete_rule(
    params: (Arc<HybridStore>, String)
) -> std::result::Result<impl warp::Reply, warp::Rejection> {
    let (store, rule_id) = params;
    
    match store.delete_rule(&rule_id) {
        Ok(_) => {
            Ok(warp::reply::json(&json!({
                "status": "success",
                "message": "Rule deleted successfully"
            })))
        },
        Err(e) => {
            Ok(warp::reply::json(&json!({
                "status": "error",
                "message": format!("Failed to delete rule: {}", e)
            })))
        }
    }
}

/// Handle request to execute a rule
async fn handle_execute_rule(
    params: (Arc<RuleExecutor>, String, serde_json::Value)
) -> std::result::Result<impl warp::Reply, warp::Rejection> {
    let (executor, rule_id, input) = params;
    
    // Execute rule with input data
    match executor.execute_rule(&rule_id, Some(input)).await {
        Ok(result) => {
            Ok(warp::reply::json(&result))
        },
        Err(e) => {
            Ok(warp::reply::json(&json!({
                "status": "error",
                "message": format!("Failed to execute rule: {}", e)
            })))
        }
    }
}

/// Create API route for listing templates
pub fn list_templates_route(
    store: Arc<HybridStore>
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "templates")
        .and(warp::get())
        .map(move || store.clone())
        .and_then(handle_list_templates)
}

/// Create API route for getting a template by ID
pub fn get_template_route(
    store: Arc<HybridStore>
) -> impl Filter<Extract = (impl warp::Reply,), Error = warp::Rejection> + Clone {
    warp::path!("api" / "templates" / String)
        .and(warp::get())
        .map(move |template_id: String| (store.clone(), template_id))
        .and_then(handle_get_template)
}

/// Handle request to list all templates
async fn handle_list_templates(
    store: Arc<HybridStore>
) -> std::result::Result<impl warp::Reply, warp::Rejection> {
    let template_manager = match store.get_template_manager() {
        Ok(tm) => tm,
        Err(e) => {
            return Ok(warp::reply::json(&json!({
                "status": "error",
                "message": format!("Failed to get template manager: {}", e)
            })));
        }
    };
    
    match template_manager.list_templates() {
        Ok(templates) => {
            Ok(warp::reply::json(&json!({
                "status": "success",
                "templates": templates
            })))
        },
        Err(e) => {
            Ok(warp::reply::json(&json!({
                "status": "error",
                "message": format!("Failed to list templates: {}", e)
            })))
        }
    }
}

/// Handle request to get a template by ID
async fn handle_get_template(
    params: (Arc<HybridStore>, String)
) -> std::result::Result<impl warp::Reply, warp::Rejection> {
    let (store, template_id) = params;
    
    let template_manager = match store.get_template_manager() {
        Ok(tm) => tm,
        Err(e) => {
            return Ok(warp::reply::json(&json!({
                "status": "error",
                "message": format!("Failed to get template manager: {}", e)
            })));
        }
    };
    
    match template_manager.get_template_by_id(&template_id) {
        Ok(template) => {
            Ok(warp::reply::json(&json!({
                "status": "success",
                "template": template
            })))
        },
        Err(e) => {
            Ok(warp::reply::json(&json!({
                "status": "error",
                "message": format!("Failed to get template: {}", e)
            })))
        }
    }
}