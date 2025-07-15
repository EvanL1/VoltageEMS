use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use tokio::sync::RwLock;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::config::Config;
use crate::error::{HisSrvError, Result};
use crate::storage::StorageManager;

pub mod handlers;
pub mod handlers_enhanced;
pub mod handlers_history;
pub mod middleware;
pub mod models;
pub mod models_enhanced;
pub mod models_history;

pub use models::*;
pub use models_enhanced::*;
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
        handlers_enhanced::advanced_query,
        handlers_enhanced::batch_query,
        handlers_enhanced::stream_query,
        handlers_enhanced::trend_analysis,
        handlers_enhanced::aggregate_analysis,
        handlers_enhanced::data_quality_report,
        handlers::health_check,
        handlers::get_config,
    ),
    components(
        schemas(
            // History models
            HistoryQueryFilter,
            HistoryDataPoint,
            HistoryValue,
            HistoryQueryResult,
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
            // Enhanced models
            QueryMode,
            AdvancedHistoryQuery,
            QueryFilter,
            FilterOperator,
            AggregationConfig,
            AggregationFunction,
            OrderByConfig,
            SortDirection,
            NullsOrder,
            PaginationConfig,
            OutputConfig,
            EnhancedQueryResponse,
            EnhancedQueryResult,
            EnhancedDataPoint,
            DataQuality,
            QualityCode,
            QueryMetadata,
            QualityInfo,
            BatchHistoryQuery,
            FailureStrategy,
            BatchQueryResponse,
            BatchStatus,
            BatchQueryResult,
            BatchQueryError,
            BatchQueryAccepted,
            StreamHistoryQuery,
            StreamChunk,
            TrendAnalysisRequest,
            TrendAnalysisResponse,
            TrendInfo,
            TrendDirection,
            TrendStatistics,
            AnomalyPoint,
            AnomalyType,
            ForecastResult,
            ForecastPoint,
            ConfidenceInterval,
            AggregateAnalysisRequest,
            AggregateAnalysisResponse,
            AggregateResult,
            GroupByResult,
            DataQualityReport,
            SourceQualityInfo,
            QualityMetrics,
            PointQualityInfo,
            QualityIssue,
            QualityIssueType,
            IssueSeverity,
            MergeStrategy,
            TrendAlgorithm,
            // Query optimizer models
            crate::query_optimizer::QueryPlan,
            crate::query_optimizer::QueryStep,
            crate::query_optimizer::StepType,
            crate::query_optimizer::QuerySource,
            crate::query_optimizer::QueryCost,
            // Common models
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
        .route(
            "/history/sources/:source_id",
            get(handlers_history::get_data_source_detail),
        )
        .route("/history/statistics", get(handlers_history::get_statistics))
        // 增强查询 endpoints
        .route("/history/query/advanced", post(handlers_enhanced::advanced_query))
        .route("/history/query/batch", post(handlers_enhanced::batch_query))
        .route("/history/query/stream", post(handlers_enhanced::stream_query))
        // 数据分析 endpoints
        .route("/history/analysis/trend", post(handlers_enhanced::trend_analysis))
        .route("/history/analysis/aggregate", post(handlers_enhanced::aggregate_analysis))
        .route("/history/quality/report", get(handlers_enhanced::data_quality_report))
        // 数据导出 endpoints
        .route("/history/export", post(handlers_history::create_export_job))
        .route(
            "/history/export/:job_id",
            get(handlers_history::get_export_job_status),
        )
        // 管理和监控 endpoints
        .route("/admin/config", get(handlers::get_config))
        .route("/admin/storage-stats", get(handlers::get_statistics))
        // 健康检查 endpoint
        .route("/health", get(handlers::health_check))
        // Swagger UI
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        // 应用中间件
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(self::middleware::create_body_limit_layer())
                .layer(axum::middleware::from_fn(self::middleware::trace_request))
                .layer(axum::middleware::from_fn_with_state(
                    state.clone(),
                    self::middleware::validate_request,
                ))
        )
        .with_state(state)
}

pub fn create_api_router(storage_manager: Arc<RwLock<StorageManager>>) -> Router {
    let config = Config::default(); // 临时使用默认配置
    let state = AppState {
        storage_manager,
        config: Arc::new(config),
    };
    create_router(state)
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

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| HisSrvError::ConfigError {
            message: format!("Failed to bind to {}: {}", addr, e),
            field: Some("service.host/port".to_string()),
            suggestion: Some("Check if the port is already in use".to_string()),
        })?;

    axum::serve(listener, app)
        .await
        .map_err(|e| HisSrvError::InternalError {
            message: format!("Server error: {}", e),
            context: "Failed to start axum server".to_string(),
        })?;

    Ok(())
}
