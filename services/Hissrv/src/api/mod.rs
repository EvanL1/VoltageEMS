use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use tokio::sync::RwLock;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::config::Config;
use crate::storage::StorageManager;
use crate::error::{HisSrvError, Result};

pub mod handlers;
pub mod models;
pub mod handlers_history;
pub mod models_history;

pub use models::*;
pub use models_history::*;

#[derive(OpenApi)]
#[openapi(
    paths(
        handlers_history::query_history,
        handlers_history::get_data_sources,
        handlers_history::get_data_source_detail,
        handlers_history::get_statistics,
        handlers_history::create_export_job,
        handlers_history::get_export_job_status,
        handlers::health_check,
        handlers::get_config,
    ),
    components(
        schemas(
            HistoryQueryFilter,
            HistoryDataPoint,
            HistoryValue,
            HistoryQueryResult,
            // HistoryApiResponse is generic, can't be listed here directly
            DataSourceInfo,
            DataPointInfo,
            TimeSeriesStatistics,
            ExportRequest,
            ExportJob,
            QuerySummary,
            TimeRange,
            AggregatedDataPoint,
            PaginationInfo,
            SourceStatistics,
            OverallStatistics,
            StorageUsage,
            HealthStatus,
            StorageStatistics,
            ErrorResponse,
        )
    ),
    tags(
        (name = "history", description = "历史数据查询接口"),
        (name = "analytics", description = "数据分析和统计接口"),
        (name = "export", description = "数据导出接口"),
        (name = "admin", description = "管理和监控接口"),
        (name = "health", description = "健康检查接口")
    ),
    info(
        title = "HisSrv Historical Data API",
        version = "0.2.0",
        description = "历史数据服务专用 REST API - 专注于历史数据查询、分析和导出",
        contact(
            name = "API Support",
            email = "support@voltageenergy.com"
        )
    ),
)]
pub struct ApiDoc;

#[derive(Clone)]
pub struct AppState {
    pub storage_manager: Arc<RwLock<StorageManager>>,
    pub config: Arc<Config>,
}

pub fn create_router(state: AppState) -> Router {
    Router::new()
        // 历史数据查询 endpoints
        .route("/history/query", get(handlers_history::query_history))
        .route("/history/sources", get(handlers_history::get_data_sources))
        .route("/history/sources/:source_id", get(handlers_history::get_data_source_detail))
        .route("/history/statistics", get(handlers_history::get_statistics))
        
        // 数据导出 endpoints
        .route("/history/export", post(handlers_history::create_export_job))
        .route("/history/export/:job_id", get(handlers_history::get_export_job_status))
        
        // 管理和监控 endpoints
        .route("/admin/config", get(handlers::get_config))
        .route("/admin/storage-stats", get(handlers::get_statistics))
        
        // 健康检查 endpoint
        .route("/health", get(handlers::health_check))
        
        // Swagger UI
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        
        .with_state(state)
}

pub async fn start_api_server(
    config: Config,
    storage_manager: Arc<RwLock<StorageManager>>,
) -> Result<()> {
    let state = AppState {
        storage_manager,
        config: Arc::new(config.clone()),
    };

    let app = Router::new()
        .nest(&config.api.prefix, create_router(state))
        .layer(tower::ServiceBuilder::new().into_inner());

    let addr = format!("{}:{}", config.service.host, config.service.port);
    tracing::info!("Starting API server on {}", addr);

    axum::Server::bind(&addr.parse().unwrap())
        .serve(app.into_make_service())
        .await
        .map_err(|e| HisSrvError::ConfigError(format!("Server error: {}", e)))?;

    Ok(())
}