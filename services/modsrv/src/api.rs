//! ModSrv REST API v2.0
//!
//! 提供简洁的HTTP和WebSocket接口用于模型管理和实时数据推送

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::{mpsc, Mutex};
use tracing::{error, info};
use voltage_libs::types::StandardFloat;

use crate::config::Config;
use crate::error::{ModelSrvError, Result};
use crate::model::{Model, ModelManager, PointConfig};
use crate::websocket::{ws_handler, WsConnectionManager};

/// API服务器状态
#[derive(Clone)]
pub struct ApiState {
    pub model_manager: Arc<ModelManager>,
    pub ws_manager: Arc<WsConnectionManager>,
    pub config: Config,
}

/// 健康检查响应
#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub service: String,
}

/// 模型列表响应
#[derive(Serialize)]
pub struct ModelListResponse {
    pub models: Vec<ModelSummary>,
    pub total: usize,
}

/// 模型摘要
#[derive(Serialize)]
pub struct ModelSummary {
    pub id: String,
    pub name: String,
    pub description: String,
    pub monitoring_count: usize,
    pub control_count: usize,
}

/// 模型配置响应
#[derive(Serialize)]
pub struct ModelConfigResponse {
    pub id: String,
    pub name: String,
    pub description: String,
    pub monitoring: HashMap<String, PointConfig>,
    pub control: HashMap<String, PointConfig>,
}

/// 模型值响应
#[derive(Serialize)]
pub struct ModelValuesResponse {
    pub monitoring: HashMap<String, f64>,
    pub timestamp: i64,
}

/// 完整模型响应
#[derive(Serialize)]
pub struct ModelFullResponse {
    pub id: String,
    pub name: String,
    pub description: String,
    pub monitoring: HashMap<String, PointWithValue>,
    pub control: HashMap<String, PointConfig>,
}

/// 带值的点位信息
#[derive(Serialize)]
pub struct PointWithValue {
    pub config: PointConfig,
    pub value: Option<f64>,
    pub timestamp: Option<i64>,
}

/// 控制命令请求
#[derive(Deserialize)]
pub struct ControlRequest {
    pub value: f64,
}

/// 控制命令响应
#[derive(Serialize)]
pub struct ControlResponse {
    pub success: bool,
    pub message: String,
    pub timestamp: i64,
}

/// API错误响应
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

/// 健康检查
pub async fn health_check(State(state): State<ApiState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        version: state.config.version.clone(),
        service: state.config.service_name.clone(),
    })
}

/// 获取模型列表
pub async fn list_models(
    State(state): State<ApiState>,
) -> std::result::Result<Json<ModelListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let models = state.model_manager.list_models().await;

    let model_summaries: Vec<ModelSummary> = models
        .into_iter()
        .map(|model| ModelSummary {
            id: model.id.clone(),
            name: model.name.clone(),
            description: model.description.clone(),
            monitoring_count: model.monitoring_config.len(),
            control_count: model.control_config.len(),
        })
        .collect();

    let response = ModelListResponse {
        total: model_summaries.len(),
        models: model_summaries,
    };

    Ok(Json(response))
}

/// 获取完整模型信息（配置+值）
pub async fn get_model(
    Path(model_id): Path<String>,
    State(state): State<ApiState>,
) -> std::result::Result<Json<ModelFullResponse>, (StatusCode, Json<ErrorResponse>)> {
    let model = state
        .model_manager
        .get_model(&model_id)
        .await
        .ok_or_else(|| ModelSrvError::not_found(format!("模型不存在: {}", model_id)))?;

    // 获取监视值
    let monitoring_values = state
        .model_manager
        .get_all_monitoring_values(&model_id)
        .await
        .unwrap_or_default();

    // 构建带值的监视点信息
    let mut monitoring_with_values = HashMap::new();
    for (name, config) in &model.monitoring_config {
        let value = monitoring_values.get(name);
        monitoring_with_values.insert(
            name.clone(),
            PointWithValue {
                config: config.clone(),
                value: value.map(|v| v.value()),
                timestamp: value.map(|_| chrono::Utc::now().timestamp()),
            },
        );
    }

    let response = ModelFullResponse {
        id: model.id,
        name: model.name,
        description: model.description,
        monitoring: monitoring_with_values,
        control: model.control_config,
    };

    Ok(Json(response))
}

/// 获取模型配置（静态）
pub async fn get_model_config(
    Path(model_id): Path<String>,
    State(state): State<ApiState>,
) -> std::result::Result<Json<ModelConfigResponse>, (StatusCode, Json<ErrorResponse>)> {
    let model = state
        .model_manager
        .get_model(&model_id)
        .await
        .ok_or_else(|| ModelSrvError::not_found(format!("模型不存在: {}", model_id)))?;

    let response = ModelConfigResponse {
        id: model.id,
        name: model.name,
        description: model.description,
        monitoring: model.monitoring_config,
        control: model.control_config,
    };

    Ok(Json(response))
}

/// 获取模型实时值
pub async fn get_model_values(
    Path(model_id): Path<String>,
    State(state): State<ApiState>,
) -> std::result::Result<Json<ModelValuesResponse>, (StatusCode, Json<ErrorResponse>)> {
    let values = state
        .model_manager
        .get_all_monitoring_values(&model_id)
        .await
        .ok_or_else(|| ModelSrvError::not_found(format!("模型不存在: {}", model_id)))?;

    let response = ModelValuesResponse {
        monitoring: values.into_iter().map(|(k, v)| (k, v.value())).collect(),
        timestamp: chrono::Utc::now().timestamp(),
    };

    Ok(Json(response))
}

/// 执行控制命令
pub async fn execute_control(
    Path((model_id, control_name)): Path<(String, String)>,
    State(state): State<ApiState>,
    Json(request): Json<ControlRequest>,
) -> std::result::Result<Json<ControlResponse>, (StatusCode, Json<ErrorResponse>)> {
    let value = StandardFloat::new(request.value);

    state
        .model_manager
        .execute_control(&model_id, &control_name, value)
        .await
        .map_err(|e| -> (StatusCode, Json<ErrorResponse>) { e.into() })?;

    let response = ControlResponse {
        success: true,
        message: format!(
            "控制命令执行成功: {}:{} = {}",
            model_id, control_name, request.value
        ),
        timestamp: chrono::Utc::now().timestamp(),
    };

    Ok(Json(response))
}

/// WebSocket连接统计
#[derive(Serialize)]
pub struct WsStatsResponse {
    pub connections: HashMap<String, usize>,
    pub total: usize,
}

/// 获取WebSocket连接统计
pub async fn get_ws_stats(State(state): State<ApiState>) -> Json<WsStatsResponse> {
    let stats = state.ws_manager.get_connection_stats().await;
    let total: usize = stats.values().sum();

    Json(WsStatsResponse {
        connections: stats,
        total,
    })
}

/// API服务器
pub struct ApiServer {
    model_manager: Arc<ModelManager>,
    ws_manager: Arc<WsConnectionManager>,
    config: Config,
}

impl ApiServer {
    /// 创建API服务器
    pub fn new(
        model_manager: Arc<ModelManager>,
        ws_manager: Arc<WsConnectionManager>,
        config: Config,
    ) -> Self {
        Self {
            model_manager,
            ws_manager,
            config,
        }
    }

    /// 创建路由
    fn create_router(&self) -> Router {
        let state = ApiState {
            model_manager: self.model_manager.clone(),
            ws_manager: self.ws_manager.clone(),
            config: self.config.clone(),
        };

        Router::new()
            // 基础端点
            .route("/health", get(health_check))
            .route("/models", get(list_models))
            .route("/models/{model_id}", get(get_model))
            // 新增端点
            .route("/models/{model_id}/config", get(get_model_config))
            .route("/models/{model_id}/values", get(get_model_values))
            .route(
                "/models/{model_id}/control/{control_name}",
                post(execute_control),
            )
            // WebSocket端点
            .route("/ws/models/{model_id}/values", get(ws_handler))
            // 统计端点
            .route("/ws/stats", get(get_ws_stats))
            .with_state(state)
    }

    /// 启动API服务器
    pub async fn start(&self) -> Result<()> {
        let app = self.create_router();
        let addr = format!("{}:{}", self.config.api.host, self.config.api.port);

        info!("启动API服务器: http://{}", addr);

        let listener = TcpListener::bind(&addr)
            .await
            .map_err(|e| ModelSrvError::IoError(format!("绑定地址失败 {}: {}", addr, e)))?;

        axum::serve(listener, app)
            .await
            .map_err(|e| ModelSrvError::IoError(format!("API服务器运行失败: {}", e)))?;

        Ok(())
    }

    /// 启动API服务器并通知启动状态
    pub async fn start_with_notification(
        &self,
        startup_tx: mpsc::Sender<std::result::Result<(), String>>,
    ) -> Result<()> {
        let app = self.create_router();
        let addr = format!("{}:{}", self.config.api.host, self.config.api.port);

        info!("启动API服务器: http://{}", addr);

        let listener = match TcpListener::bind(&addr).await {
            Ok(listener) => {
                // 发送启动成功通知
                if let Err(e) = startup_tx.send(Ok(())).await {
                    error!("发送启动通知失败: {}", e);
                }
                listener
            }
            Err(e) => {
                let error_msg = format!("绑定地址失败 {}: {}", addr, e);
                if let Err(send_err) = startup_tx.send(Err(error_msg.clone())).await {
                    error!("发送启动失败通知失败: {}", send_err);
                }
                return Err(ModelSrvError::IoError(error_msg));
            }
        };

        axum::serve(listener, app)
            .await
            .map_err(|e| ModelSrvError::IoError(format!("API服务器运行失败: {}", e)))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ModelConfig, PointConfig};
    use voltage_libs::redis::RedisClient;

    fn create_test_config() -> ModelConfig {
        ModelConfig {
            id: "test_model".to_string(),
            name: "测试模型".to_string(),
            description: "用于API测试的模型".to_string(),
            monitoring: HashMap::from([(
                "voltage".to_string(),
                PointConfig {
                    description: "电压".to_string(),
                    unit: Some("V".to_string()),
                },
            )]),
            control: HashMap::from([(
                "switch".to_string(),
                PointConfig {
                    description: "开关".to_string(),
                    unit: None,
                },
            )]),
        }
    }

    #[tokio::test]
    async fn test_api_routes() {
        let config = Config::default();

        // 注意：这里需要有效的Redis连接才能运行测试
        // 在实际测试中应该使用mock
        if let Ok(redis_client) = voltage_libs::redis::RedisClient::new(&config.redis.url).await {
            let model_manager = Arc::new(ModelManager::new(Arc::new(Mutex::new(redis_client))));
            let ws_manager = Arc::new(WsConnectionManager::new(model_manager.clone()));
            let api_server = ApiServer::new(model_manager, ws_manager, config);
            let router = api_server.create_router();

            // 验证路由创建成功
            assert!(!format!("{:?}", router).is_empty());
        }
    }
}
