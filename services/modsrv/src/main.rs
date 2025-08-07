//! Model Service (ModSrv)

use anyhow::Result;
use axum::{
    extract::{Path, State},
    response::Json,
    routing::get,
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
    // init the logging system
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    info!("Starting Model Service...");

    // load the configure
    let config: Config = ConfigLoader::new()
        .with_yaml_file("config/modsrv.yaml")
        .with_env_prefix("MODSRV")
        .build()?;

    // Connect to Redis
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
        // 模型实例管理
        .route("/api/models", get(list_models).post(create_model))
        .route(
            "/api/models/:id",
            get(get_model)
                .put(update_model)
                .delete(delete_model),
        )
        // 模型数据查询
        .route("/api/models/:id/data", get(get_model_data))
        .with_state(state);

    // 启动HTTP服务
    let addr = SocketAddr::from(([0, 0, 0, 0], config.service.port));
    let listener = tokio::net::TcpListener::bind(addr).await?;

    info!("Model Service started on {}", addr);
    info!("API endpoints:");
    info!("  GET /health - Health check");
    info!("  GET/POST /api/templates - Template management");
    info!("  GET/POST /api/models - Model instance management");

    axum::serve(listener, app).await?;
    Ok(())
}

// === Health Check ===

async fn health_check() -> Json<serde_json::Value> {
    Json(json!({
        "status": "healthy",
        "service": "modsrv",
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
    Json(json!({ "id": id, "status": result }))
}

// === Model Instance Management ===

async fn list_models(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let mut conn = match state.redis_client.get_multiplexed_async_connection().await {
        Ok(conn) => conn,
        Err(e) => {
            error!("Redis connection error: {}", e);
            return Json(json!({ "error": "Database connection failed" }));
        },
    };

    // 调用Lua函数列出所有模型实例
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

    let model_id = model["id"]
        .as_str()
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("model_{}", uuid::Uuid::new_v4()));

    // 调用Lua函数创建模型实例
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
            return Json(json!({ "error": "Failed to create model" }));
        },
    };

    info!("Created model: {}", model_id);
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

    // 调用Lua函数获取模型实例
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

    // 调用Lua函数删除模型实例
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
    Json(json!({ "id": id, "status": result }))
}

async fn get_model_data(
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

    // 调用Lua函数获取模型数据
    let result: String = match redis::cmd("FCALL")
        .arg("modsrv_get_model_data")
        .arg(1)
        .arg(&id)
        .query_async(&mut conn)
        .await
    {
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
