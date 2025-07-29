//! ModSrv REST API
//!
//! 提供轻量级的HTTP和WebSocket接口

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tracing::{error, info};

use crate::config::Config;
use crate::error::{ModelSrvError, Result};
use crate::model::{Model, ModelManager};
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

/// 模型值响应
#[derive(Serialize)]
pub struct ModelValuesResponse {
    pub model_id: String,
    pub values: HashMap<String, f64>,
    pub timestamp: i64,
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
pub async fn list_models(State(state): State<ApiState>) -> Json<ModelListResponse> {
    let models = state.model_manager.list_models();

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

    Json(ModelListResponse {
        total: model_summaries.len(),
        models: model_summaries,
    })
}

/// 获取模型当前值
pub async fn get_model_values(
    Path(model_id): Path<String>,
    State(state): State<ApiState>,
) -> std::result::Result<Json<ModelValuesResponse>, (StatusCode, Json<ErrorResponse>)> {
    // 检查模型是否存在
    if state.model_manager.get_model(&model_id).is_none() {
        return Err(ModelSrvError::NotFound(format!("模型 {} 不存在", model_id)).into());
    }

    // 从Redis获取实时值
    let values = state
        .model_manager
        .get_model_values(&model_id)
        .await
        .map_err(|e| ModelSrvError::redis(format!("获取模型值失败: {}", e)))?;

    Ok(Json(ModelValuesResponse {
        model_id,
        values,
        timestamp: chrono::Utc::now().timestamp(),
    }))
}

/// 发送控制命令
pub async fn send_control(
    Path((model_id, control_name)): Path<(String, String)>,
    State(state): State<ApiState>,
    Json(request): Json<ControlRequest>,
) -> std::result::Result<Json<ControlResponse>, (StatusCode, Json<ErrorResponse>)> {
    state
        .model_manager
        .send_control(&model_id, &control_name, request.value)
        .await?;

    Ok(Json(ControlResponse {
        success: true,
        message: format!(
            "控制命令已发送: {}.{} = {:.6}",
            model_id, control_name, request.value
        ),
        timestamp: chrono::Utc::now().timestamp(),
    }))
}

/// 创建API路由
pub fn create_routes(api_state: ApiState) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/models", get(list_models))
        .route("/models/:model_id/values", get(get_model_values))
        .route(
            "/models/:model_id/control/:control_name",
            post(send_control),
        )
        .route("/ws/:model_id", get(ws_handler))
        .with_state(api_state)
}

/// API服务器
pub struct ApiServer {
    model_manager: Arc<ModelManager>,
    ws_manager: Arc<WsConnectionManager>,
    config: Config,
}

impl ApiServer {
    /// 创建新的API服务器
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

    /// 启动API服务器
    pub async fn start(self) -> Result<()> {
        let api_state = ApiState {
            model_manager: self.model_manager.clone(),
            ws_manager: self.ws_manager.clone(),
            config: self.config.clone(),
        };

        let app = create_routes(api_state);

        let addr = format!("{}:{}", self.config.api.host, self.config.api.port);
        let listener = TcpListener::bind(&addr)
            .await
            .map_err(|e| ModelSrvError::io(format!("绑定地址失败 {}: {}", addr, e)))?;

        info!("API服务器启动: http://{}", addr);

        axum::serve(listener, app)
            .await
            .map_err(|e| ModelSrvError::io(format!("API服务器错误: {}", e)))?;

        Ok(())
    }

    /// 启动API服务器（带启动通知）
    pub async fn start_with_notification(
        self,
        tx: mpsc::Sender<std::result::Result<(), String>>,
    ) -> Result<()> {
        let api_state = ApiState {
            model_manager: self.model_manager.clone(),
            ws_manager: self.ws_manager.clone(),
            config: self.config.clone(),
        };

        let app = create_routes(api_state);

        let addr = format!("{}:{}", self.config.api.host, self.config.api.port);
        let listener = match TcpListener::bind(&addr).await {
            Ok(listener) => {
                info!("API服务器启动: http://{}", addr);
                let _ = tx.send(Ok(())).await;
                listener
            }
            Err(e) => {
                let err_msg = format!("绑定地址失败 {}: {}", addr, e);
                error!("{}", err_msg);
                let _ = tx.send(Err(err_msg.clone())).await;
                return Err(ModelSrvError::io(err_msg));
            }
        };

        axum::serve(listener, app)
            .await
            .map_err(|e| ModelSrvError::io(format!("API服务器错误: {}", e)))?;

        Ok(())
    }
}
