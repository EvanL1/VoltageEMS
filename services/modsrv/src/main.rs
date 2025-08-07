//! Model Service (ModSrv)
//!
//! 支持 measurement/action 分离架构的模型管理服务

use anyhow::Result;
use axum::{
    extract::{Path, Query, State},
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{net::SocketAddr, sync::Arc};
use tracing::{error, info};
use voltage_libs::config::ConfigLoader;

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
struct Config {
    #[serde(default)]
    service: ServiceConfig,
    redis: RedisConfig,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
struct ServiceConfig {
    #[serde(default = "default_service_name")]
    name: String,
    #[serde(default = "default_port")]
    port: u16,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct RedisConfig {
    #[serde(default = "default_redis_url")]
    url: String,
}

fn default_service_name() -> String {
    "modsrv".to_string()
}

fn default_port() -> u16 {
    6001
}

fn default_redis_url() -> String {
    "redis://127.0.0.1:6379".to_string()
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            url: default_redis_url(),
        }
    }
}

struct AppState {
    redis_client: redis::Client,
}

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志系统
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    info!("Starting Model Service (ModSrv)...");
    info!("Architecture: measurement/action separation");

    // 加载配置
    let config: Config = ConfigLoader::new()
        .with_yaml_file("config/modsrv.yaml")
        .with_env_prefix("MODSRV")
        .build()?;

    // 连接到 Redis
    let redis_client = redis::Client::open(config.redis.url.clone())?;
    info!("Connected to Redis");

    // 创建应用状态
    let state = Arc::new(AppState { redis_client });

    // 创建API路由
    let app = Router::new()
        .route("/health", get(health_check))
        // 模板管理
        .route("/api/templates", get(list_templates).post(create_template))
        .route(
            "/api/templates/:id",
            get(get_template)
                .put(update_template)
                .delete(delete_template),
        )
        // 模型管理
        .route("/api/models", get(list_models).post(create_model))
        .route(
            "/api/models/:id",
            get(get_model)
                .put(update_model)
                .delete(delete_model),
        )
        // 数据操作
        .route("/api/models/:id/data", get(get_model_data))
        .route("/api/models/:id/sync", post(sync_measurement))
        .route("/api/models/:id/action", post(execute_action))
        .route("/api/sync/all", post(sync_all_measurements))
        .with_state(state);

    // 启动HTTP服务
    let addr = SocketAddr::from(([0, 0, 0, 0], config.service.port));
    let listener = tokio::net::TcpListener::bind(addr).await?;

    info!("Model Service started on {}", addr);
    info!("API endpoints:");
    info!("  GET /health - Health check");
    info!("  GET/POST /api/templates - Template management");
    info!("  GET/POST /api/models - Model management");
    info!("  GET /api/models/:id/data - Get model data (measurement/action)");
    info!("  POST /api/models/:id/sync - Sync measurement from channels");
    info!("  POST /api/models/:id/action - Execute action to channels");
    info!("  POST /api/sync/all - Sync all models");

    axum::serve(listener, app).await?;
    Ok(())
}

// === Health Check ===

async fn health_check() -> Json<serde_json::Value> {
    Json(json!({
        "status": "healthy",
        "service": "modsrv",
        "architecture": "measurement/action",
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

// === Template Management ===

async fn list_templates(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let mut conn = match state.redis_client.get_multiplexed_async_connection().await {
        Ok(conn) => conn,
        Err(e) => {
            error!("Redis connection error: {}", e);
            return Json(json!({ "error": "Database connection failed" }));
        },
    };

    // 调用Lua函数列出所有模板
    let result: String = match redis::cmd("FCALL")
        .arg("modsrv_list_templates")
        .arg(0)
        .query_async(&mut conn)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to list templates: {}", e);
            return Json(json!({ "error": "Failed to list templates" }));
        },
    };

    match serde_json::from_str(&result) {
        Ok(templates) => Json(templates),
        Err(e) => {
            error!("Failed to parse templates: {}", e);
            Json(json!({ "error": "Invalid template data" }))
        },
    }
}

async fn create_template(
    State(state): State<Arc<AppState>>,
    Json(template): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    let mut conn = match state.redis_client.get_multiplexed_async_connection().await {
        Ok(conn) => conn,
        Err(e) => {
            error!("Redis connection error: {}", e);
            return Json(json!({ "error": "Database connection failed" }));
        },
    };

    let template_id = template["id"]
        .as_str()
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("template_{}", uuid::Uuid::new_v4()));

    // 调用Lua函数创建模板
    let result: String = match redis::cmd("FCALL")
        .arg("modsrv_upsert_template")
        .arg(1)
        .arg(&template_id)
        .arg(template.to_string())
        .query_async(&mut conn)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to create template: {}", e);
            return Json(json!({ "error": "Failed to create template" }));
        },
    };

    info!("Created template: {}", template_id);
    Json(json!({ "id": template_id, "status": result }))
}

async fn get_template(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<serde_json::Value> {
    let mut conn = match state.redis_client.get_multiplexed_async_connection().await {
        Ok(conn) => conn,
        Err(e) => {
            error!("Redis connection error: {}", e);
            return Json(json!({ "error": "Database connection failed" }));
        },
    };

    // 调用Lua函数获取模板
    let result: String = match redis::cmd("FCALL")
        .arg("modsrv_get_template")
        .arg(1)
        .arg(&id)
        .query_async(&mut conn)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to get template: {}", e);
            return Json(json!({ "error": "Template not found" }));
        },
    };

    match serde_json::from_str(&result) {
        Ok(template) => Json(template),
        Err(_) => Json(json!({ "error": "Template not found" })),
    }
}

async fn update_template(
    State(state): State<Arc<AppState>>,
    Path(_id): Path<String>,
    Json(template): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    create_template(State(state), Json(template)).await
}

async fn delete_template(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<serde_json::Value> {
    let mut conn = match state.redis_client.get_multiplexed_async_connection().await {
        Ok(conn) => conn,
        Err(e) => {
            error!("Redis connection error: {}", e);
            return Json(json!({ "error": "Database connection failed" }));
        },
    };

    // 调用Lua函数删除模板
    let result: String = match redis::cmd("FCALL")
        .arg("modsrv_delete_template")
        .arg(1)
        .arg(&id)
        .query_async(&mut conn)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to delete template: {}", e);
            return Json(json!({ "error": "Failed to delete template" }));
        },
    };

    info!("Deleted template: {}", id);
    Json(json!({ "status": result }))
}

// === Model Management ===

async fn list_models(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let mut conn = match state.redis_client.get_multiplexed_async_connection().await {
        Ok(conn) => conn,
        Err(e) => {
            error!("Redis connection error: {}", e);
            return Json(json!({ "error": "Database connection failed" }));
        },
    };

    // 调用Lua函数列出所有模型
    let result: String = match redis::cmd("FCALL")
        .arg("modsrv_list_models")
        .arg(0)
        .query_async(&mut conn)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to list models: {}", e);
            return Json(json!({ "error": "Failed to list models" }));
        },
    };

    match serde_json::from_str(&result) {
        Ok(models) => Json(models),
        Err(e) => {
            error!("Failed to parse models: {}", e);
            Json(json!({ "error": "Invalid model data" }))
        },
    }
}

async fn create_model(
    State(state): State<Arc<AppState>>,
    Json(model): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    let mut conn = match state.redis_client.get_multiplexed_async_connection().await {
        Ok(conn) => conn,
        Err(e) => {
            error!("Redis connection error: {}", e);
            return Json(json!({ "error": "Database connection failed" }));
        },
    };

    // 验证模型必须有mappings
    if model.get("mappings").is_none() {
        return Json(json!({ "error": "Model must have mappings (measurement and/or action)" }));
    }

    let model_id = model["id"]
        .as_str()
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("model_{}", uuid::Uuid::new_v4()));

    // 调用Lua函数创建模型
    let result: String = match redis::cmd("FCALL")
        .arg("modsrv_upsert_model")
        .arg(1)
        .arg(&model_id)
        .arg(model.to_string())
        .query_async(&mut conn)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to create model: {}", e);
            return Json(json!({ "error": format!("Failed to create model: {}", e) }));
        },
    };

    info!(
        "Created model: {} with measurement/action mappings",
        model_id
    );
    Json(json!({ "id": model_id, "status": result }))
}

async fn get_model(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<serde_json::Value> {
    let mut conn = match state.redis_client.get_multiplexed_async_connection().await {
        Ok(conn) => conn,
        Err(e) => {
            error!("Redis connection error: {}", e);
            return Json(json!({ "error": "Database connection failed" }));
        },
    };

    // 调用Lua函数获取模型
    let result: String = match redis::cmd("FCALL")
        .arg("modsrv_get_model")
        .arg(1)
        .arg(&id)
        .query_async(&mut conn)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to get model: {}", e);
            return Json(json!({ "error": "Model not found" }));
        },
    };

    match serde_json::from_str(&result) {
        Ok(model) => Json(model),
        Err(_) => Json(json!({ "error": "Model not found" })),
    }
}

async fn update_model(
    State(state): State<Arc<AppState>>,
    Path(_id): Path<String>,
    Json(model): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    create_model(State(state), Json(model)).await
}

async fn delete_model(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<serde_json::Value> {
    let mut conn = match state.redis_client.get_multiplexed_async_connection().await {
        Ok(conn) => conn,
        Err(e) => {
            error!("Redis connection error: {}", e);
            return Json(json!({ "error": "Database connection failed" }));
        },
    };

    // 调用Lua函数删除模型
    let result: String = match redis::cmd("FCALL")
        .arg("modsrv_delete_model")
        .arg(1)
        .arg(&id)
        .query_async(&mut conn)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to delete model: {}", e);
            return Json(json!({ "error": "Failed to delete model" }));
        },
    };

    info!("Deleted model: {}", id);
    Json(json!({ "status": result }))
}

// === Data Operations ===

#[derive(Deserialize)]
struct DataTypeQuery {
    #[serde(rename = "type")]
    data_type: Option<String>, // 'measurement', 'action', or null for both
}

async fn get_model_data(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<DataTypeQuery>,
) -> Json<serde_json::Value> {
    let mut conn = match state.redis_client.get_multiplexed_async_connection().await {
        Ok(conn) => conn,
        Err(e) => {
            error!("Redis connection error: {}", e);
            return Json(json!({ "error": "Database connection failed" }));
        },
    };

    // 构建命令参数
    let mut cmd = redis::cmd("FCALL");
    cmd.arg("modsrv_get_model_data").arg(1).arg(&id);

    if let Some(dtype) = query.data_type {
        cmd.arg(dtype);
    }

    // 调用Lua函数获取模型数据
    let result: String = match cmd.query_async(&mut conn).await {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to get model data: {}", e);
            return Json(json!({ "error": "Failed to get model data" }));
        },
    };

    match serde_json::from_str(&result) {
        Ok(data) => Json(data),
        Err(e) => {
            error!("Failed to parse model data: {}", e);
            Json(json!({ "error": "Invalid model data" }))
        },
    }
}

async fn sync_measurement(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Json<serde_json::Value> {
    let mut conn = match state.redis_client.get_multiplexed_async_connection().await {
        Ok(conn) => conn,
        Err(e) => {
            error!("Redis connection error: {}", e);
            return Json(json!({ "error": "Database connection failed" }));
        },
    };

    // 调用Lua函数同步测量数据
    let result: String = match redis::cmd("FCALL")
        .arg("modsrv_sync_measurement")
        .arg(1)
        .arg(&id)
        .query_async(&mut conn)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to sync measurement: {}", e);
            return Json(json!({ "error": format!("Failed to sync measurement: {}", e) }));
        },
    };

    info!("Synced measurement for model: {}", id);

    match serde_json::from_str(&result) {
        Ok(data) => Json(data),
        Err(_) => Json(json!({ "status": "synced", "model_id": id })),
    }
}

#[derive(Deserialize)]
struct ActionRequest {
    action: String,
    value: serde_json::Value,
}

async fn execute_action(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(request): Json<ActionRequest>,
) -> Json<serde_json::Value> {
    let mut conn = match state.redis_client.get_multiplexed_async_connection().await {
        Ok(conn) => conn,
        Err(e) => {
            error!("Redis connection error: {}", e);
            return Json(json!({ "error": "Database connection failed" }));
        },
    };

    // 调用Lua函数执行动作
    let result: String = match redis::cmd("FCALL")
        .arg("modsrv_execute_action")
        .arg(1)
        .arg(&id)
        .arg(&request.action)
        .arg(request.value.to_string())
        .query_async(&mut conn)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to execute action: {}", e);
            return Json(json!({ "error": format!("Failed to execute action: {}", e) }));
        },
    };

    info!("Executed action '{}' for model: {}", request.action, id);

    match serde_json::from_str(&result) {
        Ok(data) => Json(data),
        Err(_) => Json(json!({ "status": "executed", "model_id": id, "action": request.action })),
    }
}

async fn sync_all_measurements(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let mut conn = match state.redis_client.get_multiplexed_async_connection().await {
        Ok(conn) => conn,
        Err(e) => {
            error!("Redis connection error: {}", e);
            return Json(json!({ "error": "Database connection failed" }));
        },
    };

    // 调用Lua函数同步所有模型
    let result: String = match redis::cmd("FCALL")
        .arg("modsrv_sync_all_measurements")
        .arg(0)
        .query_async(&mut conn)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to sync all measurements: {}", e);
            return Json(json!({ "error": "Failed to sync all measurements" }));
        },
    };

    info!("Synced all model measurements");

    match serde_json::from_str(&result) {
        Ok(data) => Json(data),
        Err(e) => {
            error!("Failed to parse sync result: {}", e);
            Json(json!({ "error": "Invalid sync result" }))
        },
    }
}
