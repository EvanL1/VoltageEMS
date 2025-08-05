//! Rulesrv V2 - 轻量级规则管理服务
//!
//! 核心规则引擎逻辑由Redis Lua Functions实现
//! Rust服务仅提供API接口和管理功能

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{error, info};
use voltage_libs::redis::EdgeRedis;

/// API状态
struct ApiState {
    redis: Arc<EdgeRedis>,
}

/// 通用API响应
#[derive(Serialize)]
struct ApiResponse<T> {
    success: bool,
    data: Option<T>,
    error: Option<String>,
}

/// 健康检查
async fn health_check(State(state): State<Arc<ApiState>>) -> impl IntoResponse {
    // 检查Redis连接
    let redis_ok = state.redis.get("rulesrv:health").await.is_ok();

    Json(json!({
        "status": if redis_ok { "healthy" } else { "unhealthy" },
        "version": "2.0.0",
        "engine": "lua",
        "redis_connected": redis_ok,
    }))
}

/// 创建或更新规则
async fn upsert_rule(
    State(state): State<Arc<ApiState>>,
    Json(rule): Json<Value>,
) -> impl IntoResponse {
    let rule_json = match serde_json::to_string(&rule) {
        Ok(json) => json,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ApiResponse::<()> {
                    success: false,
                    data: None,
                    error: Some(format!("Invalid JSON: {}", e)),
                }),
            )
        },
    };

    // 调用Lua函数
    match state
        .redis
        .call_function("rule_upsert", &[], &[&rule_json])
        .await
    {
        Ok(_) => (
            StatusCode::OK,
            Json(ApiResponse {
                success: true,
                data: Some(rule),
                error: None,
            }),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<()> {
                success: false,
                data: None,
                error: Some(e.to_string()),
            }),
        ),
    }
}

/// 获取规则
async fn get_rule(
    State(state): State<Arc<ApiState>>,
    Path(rule_id): Path<String>,
) -> impl IntoResponse {
    match state
        .redis
        .call_function("rule_get", &[&rule_id], &[])
        .await
    {
        Ok(result) => match serde_json::from_str::<Value>(&result) {
            Ok(data) => (
                StatusCode::OK,
                Json(ApiResponse {
                    success: true,
                    data: Some(data),
                    error: None,
                }),
            ),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<()> {
                    success: false,
                    data: None,
                    error: Some(format!("Parse error: {}", e)),
                }),
            ),
        },
        Err(e) => {
            let status = if e.to_string().contains("not found") {
                StatusCode::NOT_FOUND
            } else {
                StatusCode::INTERNAL_SERVER_ERROR
            };

            (
                status,
                Json(ApiResponse::<()> {
                    success: false,
                    data: None,
                    error: Some(e.to_string()),
                }),
            )
        },
    }
}

/// 删除规则
async fn delete_rule(
    State(state): State<Arc<ApiState>>,
    Path(rule_id): Path<String>,
) -> impl IntoResponse {
    match state
        .redis
        .call_function("rule_delete", &[&rule_id], &[])
        .await
    {
        Ok(_) => (
            StatusCode::OK,
            Json(ApiResponse {
                success: true,
                data: Some(json!({ "deleted": true })),
                error: None,
            }),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<()> {
                success: false,
                data: None,
                error: Some(e.to_string()),
            }),
        ),
    }
}

/// 列出规则
#[derive(Deserialize)]
struct ListQuery {
    enabled: Option<bool>,
}

async fn list_rules(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<ListQuery>,
) -> impl IntoResponse {
    let filter = json!({
        "enabled": query.enabled,
    });

    match state
        .redis
        .call_function("rule_list", &[], &[&filter.to_string()])
        .await
    {
        Ok(result) => match serde_json::from_str::<Vec<Value>>(&result) {
            Ok(rules) => (
                StatusCode::OK,
                Json(ApiResponse {
                    success: true,
                    data: Some(rules),
                    error: None,
                }),
            ),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<()> {
                    success: false,
                    data: None,
                    error: Some(format!("Parse error: {}", e)),
                }),
            ),
        },
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<()> {
                success: false,
                data: None,
                error: Some(e.to_string()),
            }),
        ),
    }
}

/// 执行规则
async fn execute_rule(
    State(state): State<Arc<ApiState>>,
    Path(rule_id): Path<String>,
) -> impl IntoResponse {
    match state
        .redis
        .call_function("rule_execute", &[&rule_id], &[])
        .await
    {
        Ok(result) => match serde_json::from_str::<Value>(&result) {
            Ok(data) => (
                StatusCode::OK,
                Json(ApiResponse {
                    success: true,
                    data: Some(data),
                    error: None,
                }),
            ),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<()> {
                    success: false,
                    data: None,
                    error: Some(format!("Parse error: {}", e)),
                }),
            ),
        },
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<()> {
                success: false,
                data: None,
                error: Some(e.to_string()),
            }),
        ),
    }
}

/// 批量执行所有规则
async fn execute_all_rules(State(state): State<Arc<ApiState>>) -> impl IntoResponse {
    match state
        .redis
        .call_function("rules_execute_all", &[], &[])
        .await
    {
        Ok(result) => match serde_json::from_str::<Value>(&result) {
            Ok(data) => (
                StatusCode::OK,
                Json(ApiResponse {
                    success: true,
                    data: Some(data),
                    error: None,
                }),
            ),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse::<()> {
                    success: false,
                    data: None,
                    error: Some(format!("Parse error: {}", e)),
                }),
            ),
        },
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiResponse::<()> {
                success: false,
                data: None,
                error: Some(e.to_string()),
            }),
        ),
    }
}

#[tokio::main]
async fn main() {
    // 初始化日志
    tracing_subscriber::fmt::init();

    println!(
        r#"
╔═══════════════════════════════════════════════════════════╗
║            VoltageEMS Rules Service V2                    ║
║                  Lua Engine Edition                       ║
╚═══════════════════════════════════════════════════════════╝
    "#
    );

    info!("Initializing Rules Service V2 (Lua Engine)...");

    // 连接Redis
    let redis_url =
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
    let redis = match EdgeRedis::new(&redis_url).await {
        Ok(redis) => Arc::new(redis),
        Err(e) => {
            error!("Failed to connect to Redis: {}", e);
            std::process::exit(1);
        },
    };

    info!("Connected to Redis at {}", redis_url);

    // 加载Lua函数
    info!("Loading Lua rule engine...");
    // 这里假设Lua函数已经通过脚本加载

    // 创建API状态
    let state = Arc::new(ApiState { redis });

    // 创建路由
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/api/v1/rules", get(list_rules).post(upsert_rule))
        .route(
            "/api/v1/rules/:rule_id",
            get(get_rule).put(upsert_rule).delete(delete_rule),
        )
        .route("/api/v1/rules/:rule_id/execute", post(execute_rule))
        .route("/api/v1/rules/execute", post(execute_all_rules))
        .with_state(state);

    // 启动服务器
    let addr = "0.0.0.0:6003";
    info!("Starting API server on {}", addr);

    let listener = TcpListener::bind(addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
