//! 配置管理 API 模块
//! 提供 RESTful API 用于配置的增删改查

use crate::config::{Config, DataMapping};
use crate::Result;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post, put},
    Json, Router,
};
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use tower_http::cors::CorsLayer;

type SharedState = Arc<ApiState>;

/// API 状态
#[derive(Clone)]
pub struct ApiState {
    pub config: Arc<RwLock<Config>>,
    pub config_path: String,
    pub update_tx: Option<tokio::sync::mpsc::Sender<()>>,
}

/// API 错误响应
#[derive(serde::Serialize)]
struct ErrorResponse {
    error: String,
}

/// 映射查询响应
#[derive(serde::Serialize)]
struct MappingResponse {
    found: bool,
    mapping: Option<DataMapping>,
}

/// 配置更新响应
#[derive(serde::Serialize)]
struct UpdateResponse {
    success: bool,
    message: String,
}

impl ApiState {
    #[allow(dead_code)]
    pub fn new(config: Arc<RwLock<Config>>, config_path: String) -> Self {
        Self {
            config,
            config_path,
            update_tx: None,
        }
    }

    pub fn with_update_channel(
        config: Arc<RwLock<Config>>,
        config_path: String,
        update_tx: tokio::sync::mpsc::Sender<()>,
    ) -> Self {
        Self {
            config,
            config_path,
            update_tx: Some(update_tx),
        }
    }
}

/// 创建配置管理 API 路由
pub fn create_router(state: ApiState) -> Router {
    let shared_state = Arc::new(state);

    Router::new()
        .route("/config", get(get_config))
        .route("/mappings", get(list_mappings))
        .route("/mappings", post(add_mapping))
        .route("/mappings/:source", get(find_mapping))
        .route("/mappings/:source", put(update_mapping))
        .route("/mappings/:source", delete(remove_mapping))
        .route("/reload", post(reload_config))
        .route("/validate", post(validate_config))
        .layer(CorsLayer::permissive())
        .with_state(shared_state)
}

/// 启动 API 服务器
#[allow(dead_code)]
pub async fn start_api_server(
    port: u16,
    config: Arc<RwLock<Config>>,
    config_path: String,
) -> Result<()> {
    let state = ApiState::new(config, config_path);
    let app = create_router(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("Starting config API server on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app.into_make_service()).await?;

    Ok(())
}

// API 处理函数

/// 获取完整配置
async fn get_config(State(state): State<SharedState>) -> impl IntoResponse {
    match state.config.read() {
        Ok(config) => Json(&*config).into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "Failed to read configuration".to_string(),
            }),
        )
            .into_response(),
    }
}

/// 列出所有映射
async fn list_mappings(State(state): State<SharedState>) -> impl IntoResponse {
    match state.config.read() {
        Ok(config) => Json(&config.mappings).into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "Failed to read configuration".to_string(),
            }),
        )
            .into_response(),
    }
}

/// 查找特定映射
async fn find_mapping(
    State(state): State<SharedState>,
    Path(source): Path<String>,
) -> impl IntoResponse {
    match state.config.read() {
        Ok(config) => {
            let mapping = config.find_mapping(&source).cloned();
            Json(MappingResponse {
                found: mapping.is_some(),
                mapping,
            })
            .into_response()
        }
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "Failed to read configuration".to_string(),
            }),
        )
            .into_response(),
    }
}

/// 添加新映射
async fn add_mapping(
    State(state): State<SharedState>,
    Json(mapping): Json<DataMapping>,
) -> impl IntoResponse {
    // 在一个作用域内完成所有需要锁的操作
    let save_result = {
        let mut config = match state.config.write() {
            Ok(config) => config,
            Err(_) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: "Failed to acquire write lock".to_string(),
                    }),
                )
                    .into_response()
            }
        };

        match config.add_mapping(mapping) {
            Ok(()) => config.save_to_file(&state.config_path),
            Err(e) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: e.to_string(),
                    }),
                )
                    .into_response()
            }
        }
    }; // 锁在这里自动释放

    // 检查保存结果
    if let Err(e) = save_result {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to save configuration: {}", e),
            }),
        )
            .into_response();
    }

    // 通知 poller 配置已更新
    if let Some(tx) = &state.update_tx {
        if let Err(e) = tx.send(()).await {
            tracing::error!("Failed to notify poller: {}", e);
        }
    }

    (
        StatusCode::CREATED,
        Json(UpdateResponse {
            success: true,
            message: "Mapping added successfully".to_string(),
        }),
    )
        .into_response()
}

/// 更新映射
async fn update_mapping(
    State(state): State<SharedState>,
    Path(source): Path<String>,
    Json(mapping): Json<DataMapping>,
) -> impl IntoResponse {
    // 在一个作用域内完成所有需要锁的操作
    let save_result = {
        let mut config = match state.config.write() {
            Ok(config) => config,
            Err(_) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: "Failed to acquire write lock".to_string(),
                    }),
                )
                    .into_response()
            }
        };

        match config.update_mapping(&source, mapping) {
            Ok(()) => config.save_to_file(&state.config_path),
            Err(e) => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(ErrorResponse {
                        error: e.to_string(),
                    }),
                )
                    .into_response()
            }
        }
    }; // 锁在这里自动释放

    // 检查保存结果
    if let Err(e) = save_result {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to save configuration: {}", e),
            }),
        )
            .into_response();
    }

    // 通知 poller 配置已更新
    if let Some(tx) = &state.update_tx {
        if let Err(e) = tx.send(()).await {
            tracing::error!("Failed to notify poller: {}", e);
        }
    }

    Json(UpdateResponse {
        success: true,
        message: "Mapping updated successfully".to_string(),
    })
    .into_response()
}

/// 删除映射
async fn remove_mapping(
    State(state): State<SharedState>,
    Path(source): Path<String>,
) -> impl IntoResponse {
    // 在一个作用域内完成所有需要锁的操作
    let save_result = {
        let mut config = match state.config.write() {
            Ok(config) => config,
            Err(_) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: "Failed to acquire write lock".to_string(),
                    }),
                )
                    .into_response()
            }
        };

        match config.remove_mapping(&source) {
            Ok(()) => config.save_to_file(&state.config_path),
            Err(e) => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(ErrorResponse {
                        error: e.to_string(),
                    }),
                )
                    .into_response()
            }
        }
    }; // 锁在这里自动释放

    // 检查保存结果
    if let Err(e) = save_result {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to save configuration: {}", e),
            }),
        )
            .into_response();
    }

    // 通知 poller 配置已更新
    if let Some(tx) = &state.update_tx {
        if let Err(e) = tx.send(()).await {
            tracing::error!("Failed to notify poller: {}", e);
        }
    }

    Json(UpdateResponse {
        success: true,
        message: "Mapping removed successfully".to_string(),
    })
    .into_response()
}

/// 重新加载配置
async fn reload_config(State(state): State<SharedState>) -> impl IntoResponse {
    match Config::reload() {
        Ok(new_config) => {
            // 验证新配置
            if let Err(e) = new_config.validate() {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: format!("Invalid configuration: {}", e),
                    }),
                )
                    .into_response();
            }

            // 更新配置
            let update_result = match state.config.write() {
                Ok(mut config) => {
                    *config = new_config;
                    Ok(())
                }
                Err(_) => Err("Failed to acquire write lock"),
            };

            if let Err(e) = update_result {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: e.to_string(),
                    }),
                )
                    .into_response();
            }

            // 通知 poller 配置已更新
            if let Some(tx) = &state.update_tx {
                if let Err(e) = tx.send(()).await {
                    tracing::error!("Failed to notify poller: {}", e);
                }
            }

            Json(UpdateResponse {
                success: true,
                message: "Configuration reloaded successfully".to_string(),
            })
            .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to reload configuration: {}", e),
            }),
        )
            .into_response(),
    }
}

/// 验证配置
async fn validate_config(State(state): State<SharedState>) -> impl IntoResponse {
    match state.config.read() {
        Ok(config) => match config.validate() {
            Ok(()) => Json(UpdateResponse {
                success: true,
                message: "Configuration is valid".to_string(),
            })
            .into_response(),
            Err(e) => (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: format!("Configuration validation failed: {}", e),
                }),
            )
                .into_response(),
        },
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "Failed to read configuration".to_string(),
            }),
        )
            .into_response(),
    }
}
