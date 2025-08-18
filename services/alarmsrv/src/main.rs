//! Alarm Service (AlarmSrv)
//! Alarm service - responsible for managing alarm configuration and triggering (告警服务 - 负责管理告警配置和触发)

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
    "alarmsrv".to_string()
}

fn default_port() -> u16 {
    6002
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

#[derive(Debug, Deserialize)]
struct AlarmQuery {
    status: Option<String>,
    level: Option<String>,
    limit: Option<usize>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize unified logging system
    let log_config = voltage_libs::logging::LogConfig {
        service_name: "alarmsrv".to_string(),
        log_dir: std::path::PathBuf::from("/app/logs"),
        console_level: tracing::Level::INFO,
        file_level: tracing::Level::DEBUG,
        enable_json: false,
        rotation: voltage_libs::logging::Rotation::DAILY,
        max_log_files: 30,
    };

    if let Err(e) = voltage_libs::logging::init_with_config(log_config) {
        eprintln!("Failed to initialize logging: {}", e);
        // Fallback to basic logging
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
            )
            .init();
    }

    info!("Starting Alarm Service...");

    // Load configuration (加载配置)
    let config: Config = ConfigLoader::new()
        .with_yaml_file("config/alarmsrv.yaml")
        .with_env_prefix("ALARMSRV")
        .build()?;

    // Connect to Redis (连接Redis)
    let redis_client = redis::Client::open(config.redis.url.clone())?;
    info!("Connected to Redis");

    // Create application state (创建应用状态)
    let state = Arc::new(AppState { redis_client });

    // Create API routes (创建API路由)
    let app = Router::new()
        .route("/health", get(health_check))
        // Alarm management (告警管理)
        .route("/api/alarms", get(list_alarms).post(trigger_alarm))
        .route("/api/alarms/{id}", get(get_alarm).delete(clear_alarm))
        .route("/api/alarms/{id}/acknowledge", post(acknowledge_alarm))
        // Alarm configuration (告警配置)
        .route("/api/alarm-rules", get(list_rules).post(create_rule))
        .route(
            "/api/alarm-rules/{id}",
            get(get_rule)
                .put(update_rule)
                .delete(delete_rule),
        )
        // Statistics (统计信息)
        .route("/api/statistics", get(get_statistics))
        .with_state(state);

    // Start HTTP service (启动HTTP服务)
    let addr = SocketAddr::from(([0, 0, 0, 0], config.service.port));
    let listener = tokio::net::TcpListener::bind(addr).await?;

    info!("Alarm Service started on {}", addr);
    info!("API endpoints:");
    info!("  GET /health - Health check");
    info!("  GET/POST /api/alarms - Alarm management");
    info!("  GET/POST /api/alarm-rules - Alarm rule configuration");
    info!("  GET /api/statistics - Alarm statistics");

    axum::serve(listener, app).await?;
    Ok(())
}

// === Health Check ===

async fn health_check() -> Json<serde_json::Value> {
    Json(json!({
        "status": "healthy",
        "service": "alarmsrv",
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

// === Alarm Management ===

async fn list_alarms(
    State(state): State<Arc<AppState>>,
    Query(query): Query<AlarmQuery>,
) -> Json<serde_json::Value> {
    let mut conn = match state.redis_client.get_multiplexed_async_connection().await {
        Ok(conn) => conn,
        Err(e) => {
            error!("Redis connection error: {}", e);
            return Json(json!({ "error": "Database connection failed" }));
        },
    };

    // Build query parameters (构建查询参数)
    let params = json!({
        "status": query.status,
        "level": query.level,
        "limit": query.limit.unwrap_or(100)
    });

    // Call Lua function to query alarms (调用Lua函数查询告警)
    let result: String = match redis::cmd("FCALL")
        .arg("alarmsrv_list_alarms")
        .arg(0)
        .arg("query")
        .arg(params.to_string())
        .query_async(&mut conn)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to list alarms: {}", e);
            return Json(json!({ "error": "Failed to list alarms" }));
        },
    };

    match serde_json::from_str(&result) {
        Ok(alarms) => Json(alarms),
        Err(e) => {
            error!("Failed to parse alarms: {}", e);
            Json(json!({ "error": "Invalid alarm data" }))
        },
    }
}

async fn trigger_alarm(
    State(state): State<Arc<AppState>>,
    Json(alarm): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    let mut conn = match state.redis_client.get_multiplexed_async_connection().await {
        Ok(conn) => conn,
        Err(e) => {
            error!("Redis connection error: {}", e);
            return Json(json!({ "error": "Database connection failed" }));
        },
    };

    // Generate alarm ID (生成告警ID)
    let alarm_id = alarm["id"]
        .as_str()
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("alarm_{}", uuid::Uuid::new_v4()));

    // Call Lua function to trigger alarm (调用Lua函数触发告警)
    let result: String = match redis::cmd("FCALL")
        .arg("alarmsrv_trigger_alarm")
        .arg(0)
        .arg(&alarm_id)
        .arg(alarm.to_string())
        .query_async(&mut conn)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to trigger alarm: {}", e);
            return Json(json!({ "error": "Failed to trigger alarm" }));
        },
    };

    info!("Triggered alarm: {}", alarm_id);
    Json(json!({ "id": alarm_id, "status": result }))
}

async fn get_alarm(
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

    // Call Lua function to get alarm (调用Lua函数获取告警)
    let result: String = match redis::cmd("FCALL")
        .arg("alarmsrv_get_alarm")
        .arg(0)
        .arg(&id)
        .query_async(&mut conn)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to get alarm: {}", e);
            return Json(json!({ "error": "Alarm not found" }));
        },
    };

    match serde_json::from_str(&result) {
        Ok(alarm) => Json(alarm),
        Err(_) => Json(json!({ "error": "Alarm not found" })),
    }
}

async fn acknowledge_alarm(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(ack_data): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    let mut conn = match state.redis_client.get_multiplexed_async_connection().await {
        Ok(conn) => conn,
        Err(e) => {
            error!("Redis connection error: {}", e);
            return Json(json!({ "error": "Database connection failed" }));
        },
    };

    // Call Lua function to acknowledge alarm (调用Lua函数确认告警)
    let result: String = match redis::cmd("FCALL")
        .arg("alarmsrv_acknowledge_alarm")
        .arg(0)
        .arg(&id)
        .arg(ack_data.to_string())
        .query_async(&mut conn)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to acknowledge alarm: {}", e);
            return Json(json!({ "error": "Failed to acknowledge alarm" }));
        },
    };

    info!("Acknowledged alarm: {}", id);
    Json(json!({ "id": id, "status": result }))
}

async fn clear_alarm(
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

    // Call Lua function to clear alarm (调用Lua函数清除告警)
    let result: String = match redis::cmd("FCALL")
        .arg("alarmsrv_clear_alarm")
        .arg(0)
        .arg(&id)
        .query_async(&mut conn)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to clear alarm: {}", e);
            return Json(json!({ "error": "Failed to clear alarm" }));
        },
    };

    info!("Cleared alarm: {}", id);
    Json(json!({ "id": id, "status": result }))
}

// === Alarm Rule Management ===

async fn list_rules(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let mut conn = match state.redis_client.get_multiplexed_async_connection().await {
        Ok(conn) => conn,
        Err(e) => {
            error!("Redis connection error: {}", e);
            return Json(json!({ "error": "Database connection failed" }));
        },
    };

    // Call Lua function to list all rules (调用Lua函数列出所有规则)
    let result: String = match redis::cmd("FCALL")
        .arg("alarmsrv_list_rules")
        .arg(0)
        .query_async(&mut conn)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to list rules: {}", e);
            return Json(json!({ "error": "Failed to list rules" }));
        },
    };

    match serde_json::from_str(&result) {
        Ok(rules) => Json(rules),
        Err(e) => {
            error!("Failed to parse rules: {}", e);
            Json(json!({ "error": "Invalid rule data" }))
        },
    }
}

async fn create_rule(
    State(state): State<Arc<AppState>>,
    Json(rule): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    let mut conn = match state.redis_client.get_multiplexed_async_connection().await {
        Ok(conn) => conn,
        Err(e) => {
            error!("Redis connection error: {}", e);
            return Json(json!({ "error": "Database connection failed" }));
        },
    };

    let rule_id = rule["id"]
        .as_str()
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("rule_{}", uuid::Uuid::new_v4()));

    // Call Lua function to create rule (调用Lua函数创建规则)
    let result: String = match redis::cmd("FCALL")
        .arg("alarmsrv_upsert_rule")
        .arg(0)
        .arg(&rule_id)
        .arg(rule.to_string())
        .query_async(&mut conn)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to create rule: {}", e);
            return Json(json!({ "error": "Failed to create rule" }));
        },
    };

    info!("Created rule: {}", rule_id);
    Json(json!({ "id": rule_id, "status": result }))
}

async fn get_rule(
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

    // Call Lua function to get rule (调用Lua函数获取规则)
    let result: String = match redis::cmd("FCALL")
        .arg("alarmsrv_get_rule")
        .arg(0)
        .arg(&id)
        .query_async(&mut conn)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to get rule: {}", e);
            return Json(json!({ "error": "Rule not found" }));
        },
    };

    match serde_json::from_str(&result) {
        Ok(rule) => Json(rule),
        Err(_) => Json(json!({ "error": "Rule not found" })),
    }
}

async fn update_rule(
    State(state): State<Arc<AppState>>,
    Path(_id): Path<String>,
    Json(rule): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    create_rule(State(state), Json(rule)).await
}

async fn delete_rule(
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

    // Call Lua function to delete rule (调用Lua函数删除规则)
    let result: String = match redis::cmd("FCALL")
        .arg("alarmsrv_delete_rule")
        .arg(0)
        .arg(&id)
        .query_async(&mut conn)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to delete rule: {}", e);
            return Json(json!({ "error": "Failed to delete rule" }));
        },
    };

    info!("Deleted rule: {}", id);
    Json(json!({ "id": id, "status": result }))
}

// === Statistics ===

async fn get_statistics(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let mut conn = match state.redis_client.get_multiplexed_async_connection().await {
        Ok(conn) => conn,
        Err(e) => {
            error!("Redis connection error: {}", e);
            return Json(json!({ "error": "Database connection failed" }));
        },
    };

    // Call Lua function to get statistics (调用Lua函数获取统计信息)
    let result: String = match redis::cmd("FCALL")
        .arg("alarmsrv_get_statistics")
        .arg(0)
        .query_async(&mut conn)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to get statistics: {}", e);
            return Json(json!({ "error": "Failed to get statistics" }));
        },
    };

    match serde_json::from_str(&result) {
        Ok(stats) => Json(stats),
        Err(e) => {
            error!("Failed to parse statistics: {}", e);
            Json(json!({ "error": "Invalid statistics data" }))
        },
    }
}
