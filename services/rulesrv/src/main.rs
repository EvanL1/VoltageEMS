//! Rule Service (RuleSrv)
//! 规则服务 - 负责管理规则配置和执行

use anyhow::Result;
use axum::{
    extract::{Path, State},
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{net::SocketAddr, sync::Arc};
use tokio::time::{interval, Duration};
use tracing::{error, info};
use voltage_libs::config::ConfigLoader;

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
struct Config {
    #[serde(default)]
    service: ServiceConfig,
    redis: RedisConfig,
    #[serde(default)]
    execution: ExecutionConfig,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
struct ServiceConfig {
    #[serde(default = "default_service_name")]
    name: String,
    #[serde(default = "default_port")]
    port: u16,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
struct ExecutionConfig {
    #[serde(default = "default_interval")]
    interval_seconds: u64,
    #[serde(default = "default_batch_size")]
    batch_size: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct RedisConfig {
    #[serde(default = "default_redis_url")]
    url: String,
}

fn default_service_name() -> String {
    "rulesrv".to_string()
}

fn default_port() -> u16 {
    6003
}

fn default_redis_url() -> String {
    "redis://127.0.0.1:6379".to_string()
}

fn default_interval() -> u64 {
    10 // 默认10秒执行一次规则
}

fn default_batch_size() -> usize {
    100 // 默认批量处理100条规则
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
    config: Config,
}

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    info!("Starting Rule Service...");

    // 加载配置
    let config: Config = ConfigLoader::new()
        .with_yaml_file("config/rulesrv.yaml")
        .with_env_prefix("RULESRV")
        .build()?;

    // 连接Redis
    let redis_client = redis::Client::open(config.redis.url.clone())?;
    info!("Connected to Redis");

    // 创建应用状态
    let state = Arc::new(AppState {
        redis_client,
        config: config.clone(),
    });

    // 启动规则执行任务
    let exec_state = state.clone();
    tokio::spawn(async move {
        let mut interval = interval(Duration::from_secs(
            exec_state.config.execution.interval_seconds,
        ));
        let mut batch_id = 0u64;

        loop {
            interval.tick().await;

            if let Err(e) = execute_rules(&exec_state, batch_id).await {
                error!("Rule execution error: {}", e);
            }

            batch_id += 1;
        }
    });

    // 创建API路由
    let app = Router::new()
        .route("/health", get(health_check))
        // 规则管理
        .route("/api/rules", get(list_rules).post(create_rule))
        .route(
            "/api/rules/:id",
            get(get_rule)
                .put(update_rule)
                .delete(delete_rule),
        )
        .route("/api/rules/:id/enable", post(enable_rule))
        .route("/api/rules/:id/disable", post(disable_rule))
        // 执行历史和统计
        .route("/api/executions", get(list_executions))
        .route("/api/statistics", get(get_statistics))
        .with_state(state);

    // 启动HTTP服务
    let addr = SocketAddr::from(([0, 0, 0, 0], config.service.port));
    let listener = tokio::net::TcpListener::bind(addr).await?;

    info!("Rule Service started on {}", addr);
    info!("API endpoints:");
    info!("  GET /health - Health check");
    info!("  GET/POST /api/rules - Rule management");
    info!("  POST /api/rules/:id/enable - Enable rule");
    info!("  POST /api/rules/:id/disable - Disable rule");
    info!("  GET /api/executions - Execution history");
    info!("  GET /api/statistics - Rule statistics");

    axum::serve(listener, app).await?;
    Ok(())
}

// === Health Check ===

async fn health_check() -> Json<serde_json::Value> {
    Json(json!({
        "status": "healthy",
        "service": "rulesrv",
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

// === Rule Execution ===

async fn execute_rules(state: &AppState, batch_id: u64) -> Result<()> {
    let mut conn = state
        .redis_client
        .get_multiplexed_async_connection()
        .await?;

    // 调用Lua函数执行规则
    let result: String = redis::cmd("FCALL")
        .arg("rulesrv_execute_batch")
        .arg(1)
        .arg(format!("batch_{}", batch_id))
        .arg(state.config.execution.batch_size.to_string())
        .query_async(&mut conn)
        .await?;

    let exec_info: serde_json::Value = serde_json::from_str(&result)?;
    let rules_executed = exec_info["rules_executed"].as_u64().unwrap_or(0);
    let rules_triggered = exec_info["rules_triggered"].as_u64().unwrap_or(0);

    if rules_executed > 0 {
        info!(
            "Executed {} rules, {} triggered (batch {})",
            rules_executed, rules_triggered, batch_id
        );
    }

    Ok(())
}

// === Rule Management ===

async fn list_rules(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let mut conn = match state.redis_client.get_multiplexed_async_connection().await {
        Ok(conn) => conn,
        Err(e) => {
            error!("Redis connection error: {}", e);
            return Json(json!({ "error": "Database connection failed" }));
        },
    };

    // 调用Lua函数列出所有规则
    let result: String = match redis::cmd("FCALL")
        .arg("rulesrv_list_rules")
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

    // 调用Lua函数创建规则
    let result: String = match redis::cmd("FCALL")
        .arg("rulesrv_upsert_rule")
        .arg(1)
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

    // 调用Lua函数获取规则
    let result: String = match redis::cmd("FCALL")
        .arg("rulesrv_get_rule")
        .arg(1)
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

    // 调用Lua函数删除规则
    let result: String = match redis::cmd("FCALL")
        .arg("rulesrv_delete_rule")
        .arg(1)
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

async fn enable_rule(
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

    // 调用Lua函数启用规则
    let result: String = match redis::cmd("FCALL")
        .arg("rulesrv_enable_rule")
        .arg(1)
        .arg(&id)
        .query_async(&mut conn)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to enable rule: {}", e);
            return Json(json!({ "error": "Failed to enable rule" }));
        },
    };

    info!("Enabled rule: {}", id);
    Json(json!({ "id": id, "status": result }))
}

async fn disable_rule(
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

    // 调用Lua函数禁用规则
    let result: String = match redis::cmd("FCALL")
        .arg("rulesrv_disable_rule")
        .arg(1)
        .arg(&id)
        .query_async(&mut conn)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to disable rule: {}", e);
            return Json(json!({ "error": "Failed to disable rule" }));
        },
    };

    info!("Disabled rule: {}", id);
    Json(json!({ "id": id, "status": result }))
}

// === Execution History ===

async fn list_executions(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let mut conn = match state.redis_client.get_multiplexed_async_connection().await {
        Ok(conn) => conn,
        Err(e) => {
            error!("Redis connection error: {}", e);
            return Json(json!({ "error": "Database connection failed" }));
        },
    };

    // 调用Lua函数获取执行历史
    let result: String = match redis::cmd("FCALL")
        .arg("rulesrv_list_executions")
        .arg(1)
        .arg("10") // 最近10次执行
        .query_async(&mut conn)
        .await
    {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to list executions: {}", e);
            return Json(json!({ "error": "Failed to list executions" }));
        },
    };

    match serde_json::from_str(&result) {
        Ok(executions) => Json(executions),
        Err(e) => {
            error!("Failed to parse executions: {}", e);
            Json(json!({ "error": "Invalid execution data" }))
        },
    }
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

    // 调用Lua函数获取统计信息
    let result: String = match redis::cmd("FCALL")
        .arg("rulesrv_get_statistics")
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
