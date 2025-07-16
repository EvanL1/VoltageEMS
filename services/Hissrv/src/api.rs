use crate::data_processor::ProcessingStats;
use crate::error::{HisSrvError, Result};
use crate::influx_client::InfluxDBClient;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower::ServiceBuilder;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::info;

/// API 状态
#[derive(Clone)]
pub struct ApiState {
    pub influx_client: Arc<InfluxDBClient>,
    pub processing_stats: Arc<Mutex<ProcessingStats>>,
    pub config: Arc<crate::config::Config>,
}

/// 健康检查响应
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub service: String,
    pub version: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub components: HashMap<String, ComponentHealth>,
}

/// 组件健康状态
#[derive(Debug, Serialize)]
pub struct ComponentHealth {
    pub status: String,
    pub message: Option<String>,
    pub last_check: chrono::DateTime<chrono::Utc>,
}

/// 统计信息响应
#[derive(Debug, Serialize)]
pub struct StatsResponse {
    pub processing: ProcessingStats,
    pub influxdb: InfluxDBStats,
    pub uptime_seconds: u64,
}

/// InfluxDB 统计信息
#[derive(Debug, Serialize)]
pub struct InfluxDBStats {
    pub connected: bool,
    pub database: String,
    pub url: String,
}

/// 查询请求
#[derive(Debug, Deserialize)]
pub struct QueryRequest {
    pub sql: String,
}

/// 查询参数
#[derive(Debug, Deserialize)]
pub struct QueryParams {
    pub measurement: Option<String>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub limit: Option<u32>,
}

/// 通用API响应
#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl<T> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
            timestamp: chrono::Utc::now(),
        }
    }

    pub fn error(message: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message),
            timestamp: chrono::Utc::now(),
        }
    }
}

/// 创建 API 路由
pub fn create_router(state: ApiState) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/stats", get(get_stats))
        .route("/query", post(query_data))
        .route("/query/simple", get(simple_query))
        .route("/flush", post(flush_data))
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(CorsLayer::permissive())
        )
        .with_state(state)
}

/// 健康检查端点
pub async fn health_check(State(state): State<ApiState>) -> std::result::Result<Json<ApiResponse<HealthResponse>>, (StatusCode, Json<ApiResponse<()>>)> {
    let mut components = HashMap::new();

    // 检查 InfluxDB 连接
    let influx_health = match state.influx_client.ping().await {
        Ok(_) => ComponentHealth {
            status: "healthy".to_string(),
            message: None,
            last_check: chrono::Utc::now(),
        },
        Err(e) => ComponentHealth {
            status: "unhealthy".to_string(),
            message: Some(e.to_string()),
            last_check: chrono::Utc::now(),
        },
    };
    components.insert("influxdb".to_string(), influx_health);

    // 检查处理器状态
    let stats = state.processing_stats.lock().await;
    let processor_health = if stats.last_processed_time.is_some() {
        ComponentHealth {
            status: "healthy".to_string(),
            message: Some(format!("已处理 {} 条消息", stats.messages_processed)),
            last_check: chrono::Utc::now(),
        }
    } else {
        ComponentHealth {
            status: "warning".to_string(),
            message: Some("尚未处理任何消息".to_string()),
            last_check: chrono::Utc::now(),
        }
    };
    components.insert("processor".to_string(), processor_health);

    let overall_status = if components.values().all(|c| c.status == "healthy") {
        "healthy"
    } else if components.values().any(|c| c.status == "unhealthy") {
        "unhealthy"
    } else {
        "warning"
    };

    let response = HealthResponse {
        status: overall_status.to_string(),
        service: state.config.service.name.clone(),
        version: state.config.service.version.clone(),
        timestamp: chrono::Utc::now(),
        components,
    };

    Ok(Json(ApiResponse::success(response)))
}

/// 获取统计信息
pub async fn get_stats(State(state): State<ApiState>) -> std::result::Result<Json<ApiResponse<StatsResponse>>, (StatusCode, Json<ApiResponse<()>>)> {
    let processing_stats = state.processing_stats.lock().await.clone();
    
    let influxdb_stats = InfluxDBStats {
        connected: state.influx_client.ping().await.is_ok(),
        database: state.config.influxdb.database.clone(),
        url: state.config.influxdb.url.clone(),
    };

    // 计算运行时间（简化版本）
    let uptime_seconds = 0; // TODO: 实际实现需要记录启动时间

    let response = StatsResponse {
        processing: processing_stats,
        influxdb: influxdb_stats,
        uptime_seconds,
    };

    Ok(Json(ApiResponse::success(response)))
}

/// 执行 SQL 查询
pub async fn query_data(
    State(state): State<ApiState>,
    Json(request): Json<QueryRequest>,
) -> std::result::Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiResponse<()>>)> {
    info!("执行查询: {}", request.sql);

    match state.influx_client.query(&request.sql).await {
        Ok(result) => Ok(Json(ApiResponse::success(result))),
        Err(e) => {
            let error_response = ApiResponse::error(format!("查询失败: {}", e));
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error_response)))
        }
    }
}

/// 简单查询端点（通过URL参数）
pub async fn simple_query(
    State(state): State<ApiState>,
    Query(params): Query<QueryParams>,
) -> std::result::Result<Json<ApiResponse<serde_json::Value>>, (StatusCode, Json<ApiResponse<()>>)> {
    let measurement = params.measurement.unwrap_or_else(|| "telemetry".to_string());
    let limit = params.limit.unwrap_or(100);

    // 构建基本的 SQL 查询
    let mut sql = format!("SELECT * FROM {}", measurement);
    
    let mut conditions = Vec::new();
    
    if let Some(start_time) = params.start_time {
        conditions.push(format!("time >= '{}'", start_time));
    }
    
    if let Some(end_time) = params.end_time {
        conditions.push(format!("time <= '{}'", end_time));
    }
    
    if !conditions.is_empty() {
        sql.push_str(" WHERE ");
        sql.push_str(&conditions.join(" AND "));
    }
    
    sql.push_str(&format!(" ORDER BY time DESC LIMIT {}", limit));

    info!("执行简单查询: {}", sql);

    match state.influx_client.query(&sql).await {
        Ok(result) => Ok(Json(ApiResponse::success(result))),
        Err(e) => {
            let error_response = ApiResponse::error(format!("查询失败: {}", e));
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error_response)))
        }
    }
}

/// 强制刷新缓冲区
pub async fn flush_data(State(state): State<ApiState>) -> std::result::Result<Json<ApiResponse<String>>, (StatusCode, Json<ApiResponse<()>>)> {
    info!("收到手动刷新请求");

    match state.influx_client.flush().await {
        Ok(_) => {
            let message = "缓冲区已刷新".to_string();
            Ok(Json(ApiResponse::success(message)))
        }
        Err(e) => {
            let error_response = ApiResponse::error(format!("刷新失败: {}", e));
            Err((StatusCode::INTERNAL_SERVER_ERROR, Json(error_response)))
        }
    }
}

/// 启动 API 服务器
pub async fn start_api_server(
    config: Arc<crate::config::Config>,
    influx_client: Arc<InfluxDBClient>,
    processing_stats: Arc<Mutex<ProcessingStats>>,
) -> Result<()> {
    let api_state = ApiState {
        influx_client,
        processing_stats,
        config: Arc::clone(&config),
    };

    let app = create_router(api_state);
    let listener = tokio::net::TcpListener::bind(&config.listen_addr()).await?;
    
    info!("API 服务器启动在 {}", config.listen_addr());
    
    // 打印可用的端点
    info!("可用端点:");
    info!("  GET  /health      - 健康检查");
    info!("  GET  /stats       - 统计信息");
    info!("  POST /query       - SQL查询");
    info!("  GET  /query/simple - 简单查询");
    info!("  POST /flush       - 强制刷新");

    axum::serve(listener, app).await.map_err(|e| {
        HisSrvError::Internal(anyhow::anyhow!("API 服务器错误: {}", e))
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum_test::TestServer;
    use crate::config::InfluxDBConfig;

    fn create_test_state() -> ApiState {
        let config = Arc::new(crate::config::Config::default());
        let influx_config = InfluxDBConfig::default();
        let influx_client = Arc::new(crate::influx_client::InfluxDBClient::new(influx_config));
        let processing_stats = Arc::new(Mutex::new(ProcessingStats::default()));

        ApiState {
            influx_client,
            processing_stats,
            config,
        }
    }

    #[tokio::test]
    async fn test_health_check() {
        let state = create_test_state();
        let app = create_router(state);
        let server = TestServer::new(app).unwrap();

        let response = server.get("/health").await;
        assert_eq!(response.status_code(), 200);

        let body: ApiResponse<HealthResponse> = response.json();
        assert!(body.success);
        assert!(body.data.is_some());
    }

    #[tokio::test]
    async fn test_stats_endpoint() {
        let state = create_test_state();
        let app = create_router(state);
        let server = TestServer::new(app).unwrap();

        let response = server.get("/stats").await;
        assert_eq!(response.status_code(), 200);

        let body: ApiResponse<StatsResponse> = response.json();
        assert!(body.success);
        assert!(body.data.is_some());
    }

    #[tokio::test]
    async fn test_flush_endpoint() {
        let state = create_test_state();
        let app = create_router(state);
        let server = TestServer::new(app).unwrap();

        let response = server.post("/flush").await;
        // 这个测试可能会失败，因为没有真实的 InfluxDB 连接
        // 但至少可以验证路由是否工作
        assert!(response.status_code() == 200 || response.status_code() == 500);
    }
}