//! API Routes Registration Module
//!
//! This module handles route registration and global definitions for the Communication Service REST API.
//! All handler implementations are in separate handler modules.

use axum::{
    routing::{get, post},
    Router,
};
use chrono::{DateTime, Utc};
use std::sync::{Arc, OnceLock};
use tokio::sync::RwLock;
use utoipa::OpenApi;

use crate::core::channels::ChannelManager;
use voltage_rtdb::Rtdb;

// Import handler modules
use crate::api::{
    handlers::health::*,
    handlers::{
        channel_handlers::*, channel_management_handlers::*, control_handlers::*,
        mapping_handlers::*, point_handlers::*,
    },
};
use common::admin_api::{get_log_level, set_log_level};

/// Global service start time storage
static SERVICE_START_TIME: OnceLock<DateTime<Utc>> = OnceLock::new();

/// Set the service start time (should be called once at startup)
pub fn set_service_start_time(start_time: DateTime<Utc>) {
    let _ = SERVICE_START_TIME.set(start_time);
}

/// Get the service start time
pub fn get_service_start_time() -> DateTime<Utc> {
    *SERVICE_START_TIME.get().unwrap_or(&Utc::now())
}

/// Application state containing the channel manager
pub struct AppState<R: voltage_rtdb::Rtdb> {
    pub channel_manager: Arc<RwLock<ChannelManager<R>>>,
    pub rtdb: Arc<R>,
    pub sqlite_pool: sqlx::SqlitePool,
}

// Manual Clone implementation to avoid requiring R: Clone
// (Arc<R> is Clone regardless of R's Clone bound)
impl<R: voltage_rtdb::Rtdb> Clone for AppState<R> {
    fn clone(&self) -> Self {
        Self {
            channel_manager: self.channel_manager.clone(),
            rtdb: self.rtdb.clone(),
            sqlite_pool: self.sqlite_pool.clone(),
        }
    }
}

impl<R: voltage_rtdb::Rtdb> AppState<R> {
    /// Create AppState with RTDB backend and SQLite pool
    pub fn new(
        channel_manager: Arc<RwLock<ChannelManager<R>>>,
        rtdb: Arc<R>,
        sqlite_pool: sqlx::SqlitePool,
    ) -> Self {
        Self {
            channel_manager,
            rtdb,
            sqlite_pool,
        }
    }
}

impl AppState<voltage_rtdb::RedisRtdb> {
    /// Create AppState with Redis client (production use)
    pub fn with_redis_client(
        channel_manager: Arc<RwLock<ChannelManager<voltage_rtdb::RedisRtdb>>>,
        redis_client: Arc<common::redis::RedisClient>,
        sqlite_pool: sqlx::SqlitePool,
    ) -> Self {
        let rtdb = Arc::new(voltage_rtdb::RedisRtdb::from_client(redis_client));
        Self {
            channel_manager,
            rtdb,
            sqlite_pool,
        }
    }
}

/// Type alias for production AppState (uses RedisRtdb)
pub type ProductionAppState = AppState<voltage_rtdb::RedisRtdb>;

#[derive(OpenApi)]
#[openapi(
    paths(
        // Health and service status
        crate::api::handlers::health::get_service_status,
        crate::api::handlers::health::health_check,

        // Channel queries and status
        crate::api::handlers::channel_handlers::get_all_channels,
        crate::api::handlers::channel_handlers::list_channels,
        crate::api::handlers::channel_handlers::search_channels,
        crate::api::handlers::channel_handlers::get_channel_detail_handler,
        crate::api::handlers::channel_handlers::get_channel_status,
        crate::api::handlers::channel_handlers::list_all_points,

        // Control operations
        crate::api::handlers::control_handlers::control_channel,
        crate::api::handlers::control_handlers::write_channel_point,  // Unified write endpoint (supports single & batch)

        // Point information
        crate::api::handlers::point_handlers::get_point_info_handler,
        crate::api::handlers::point_handlers::get_channel_points_handler,
        crate::api::handlers::point_handlers::get_unmapped_points_handler,
        crate::api::handlers::point_handlers::get_point_mapping_with_type_handler,

        // Point CRUD operations
        crate::api::handlers::point_handlers::create_telemetry_point_handler,
        crate::api::handlers::point_handlers::create_signal_point_handler,
        crate::api::handlers::point_handlers::create_control_point_handler,
        crate::api::handlers::point_handlers::create_adjustment_point_handler,
        crate::api::handlers::point_handlers::update_point_handler,
        crate::api::handlers::point_handlers::delete_point_handler,
        crate::api::handlers::point_handlers::get_point_config_handler,
        crate::api::handlers::point_handlers::batch_point_operations_handler,

        // Channel management (CRUD)
        crate::api::handlers::channel_management_handlers::create_channel_handler,
        crate::api::handlers::channel_management_handlers::update_channel_handler,
        crate::api::handlers::channel_management_handlers::set_channel_enabled_handler,
        crate::api::handlers::channel_management_handlers::delete_channel_handler,
        crate::api::handlers::channel_management_handlers::reload_configuration_handler,
        crate::api::handlers::channel_management_handlers::reload_routing_handler,

        // Mapping management
        crate::api::handlers::mapping_handlers::get_channel_mappings_handler,
        crate::api::handlers::mapping_handlers::update_channel_mappings_handler,

        // Admin endpoints
        common::admin_api::set_log_level,
        common::admin_api::get_log_level
    ),
    components(
        schemas(
            crate::dto::ServiceStatus,
            crate::dto::ChannelStatusResponse,
            crate::dto::ChannelStatus,
            crate::dto::ChannelDetail,
            crate::dto::ChannelRuntimeStatus,
            crate::dto::PointCounts,
            crate::dto::ChannelListQuery,
            crate::dto::PaginatedResponse<crate::dto::ChannelStatusResponse>,
            crate::dto::ChannelOperation,
            crate::dto::ControlRequest,
            crate::dto::AdjustmentRequest,
            crate::dto::ControlValueRequest,
            crate::dto::AdjustmentValueRequest,
            crate::dto::BatchControlRequest,
            crate::dto::BatchAdjustmentRequest,
            crate::dto::BatchCommandResult,
            crate::dto::BatchCommandError,
            crate::dto::ChannelCreateRequest,
            crate::dto::ChannelConfigUpdateRequest,
            crate::dto::ChannelEnabledRequest,
            crate::dto::ChannelCrudResult,
            crate::dto::ReloadConfigResult,
            crate::dto::RoutingReloadResult,
            crate::dto::PointDefinition,
            crate::dto::GroupedPoints,
            crate::dto::GroupedMappings,
            crate::dto::PointMappingDetail,
            crate::dto::PointMappingItem,
            crate::dto::MappingBatchUpdateRequest,
            crate::dto::MappingBatchUpdateResult,
            crate::dto::ParameterChangeType,
            // Point CRUD DTOs
            crate::api::handlers::point_handlers::PointCrudResult,
            crate::api::handlers::point_handlers::PointUpdateRequest,
            // Batch Point CRUD DTOs
            crate::api::handlers::point_handlers::PointBatchRequest,
            crate::api::handlers::point_handlers::PointBatchResult,
            crate::api::handlers::point_handlers::PointBatchCreateItem,
            crate::api::handlers::point_handlers::PointBatchUpdateItem,
            crate::api::handlers::point_handlers::PointBatchDeleteItem,
            crate::api::handlers::point_handlers::OperationStats,
            crate::api::handlers::point_handlers::OperationStat,
            crate::api::handlers::point_handlers::PointBatchError,
            // Admin schemas
            common::admin_api::SetLogLevelRequest,
            common::admin_api::LogLevelResponse
        )
    ),
    tags(
        (name = "comsrv", description = "Communication Service API"),
        (name = "admin", description = "Administration and service management")
    )
)]
pub struct ComsrvApiDoc;

/// Create the API router with all routes (production version with Redis)
pub fn create_api_routes(
    channel_manager: Arc<RwLock<ChannelManager<voltage_rtdb::RedisRtdb>>>,
    redis_client: Arc<common::redis::RedisClient>,
    sqlite_pool: sqlx::SqlitePool,
) -> Router {
    let rtdb = Arc::new(voltage_rtdb::RedisRtdb::from_client(redis_client));
    create_api_routes_generic(channel_manager, rtdb, sqlite_pool)
}

/// Generic version of create_api_routes that accepts any Rtdb implementation.
/// Used by tests with MemoryRtdb.
pub fn create_api_routes_generic<R: Rtdb>(
    channel_manager: Arc<RwLock<ChannelManager<R>>>,
    rtdb: Arc<R>,
    sqlite_pool: sqlx::SqlitePool,
) -> Router {
    let state = AppState::new(channel_manager, rtdb, sqlite_pool);

    Router::new()
        // Health check (top-level for monitoring systems)
        .route("/health", get(health_check))
        // Service management
        .route("/api/status", get(get_service_status))
        // Channel management (CRUD)
        .route("/api/channels", get(get_all_channels).post(create_channel_handler))
        .route("/api/channels/list", get(list_channels))
        .route("/api/channels/search", get(search_channels))
        .route("/api/points", get(list_all_points))
        .route("/api/channels/{id}", get(get_channel_detail_handler).put(update_channel_handler).delete(delete_channel_handler))
        .route("/api/channels/{id}/status", get(get_channel_status))
        .route("/api/channels/{id}/control", post(control_channel))
        .route("/api/channels/{id}/enabled", axum::routing::put(set_channel_enabled_handler))
        .route("/api/channels/{id}/points", get(get_channel_points_handler))
        .route("/api/channels/{id}/unmapped-points", get(get_unmapped_points_handler))
        .route("/api/channels/{id}/mappings", get(get_channel_mappings_handler).put(update_channel_mappings_handler))
        .route("/api/channels/{channel_id}/{type}/points/{point_id}/mapping", get(get_point_mapping_with_type_handler))
        .route("/api/channels/reload", post(reload_configuration_handler))
        .route("/api/routing/reload", post(reload_routing_handler))
        // Point CRUD routes - type-specific for all operations
        .route("/api/channels/{channel_id}/T/points/{point_id}",
            get(get_telemetry_point_config_handler)
                .post(create_telemetry_point_handler)
                .put(update_telemetry_point_handler)
                .delete(delete_telemetry_point_handler))
        .route("/api/channels/{channel_id}/S/points/{point_id}",
            get(get_signal_point_config_handler)
                .post(create_signal_point_handler)
                .put(update_signal_point_handler)
                .delete(delete_signal_point_handler))
        .route("/api/channels/{channel_id}/C/points/{point_id}",
            get(get_control_point_config_handler)
                .post(create_control_point_handler)
                .put(update_control_point_handler)
                .delete(delete_control_point_handler))
        .route("/api/channels/{channel_id}/A/points/{point_id}",
            get(get_adjustment_point_config_handler)
                .post(create_adjustment_point_handler)
                .put(update_adjustment_point_handler)
                .delete(delete_adjustment_point_handler))
        // Batch point operations endpoint (create/update/delete in single request)
        .route("/api/channels/{channel_id}/points/batch", post(batch_point_operations_handler))
        // Unified write endpoint for all point types (T/S/C/A)
        .route("/api/channels/{channel_id}/write", post(write_channel_point))
        .route(
            "/api/channels/{channel_id}/{telemetry_type}/{point_id}",
            get(get_point_info_handler),
        )
        // Admin endpoints (log level management)
        .route(
            "/api/admin/logs/level",
            get(get_log_level).post(set_log_level),
        )
        // CRITICAL: Apply middleware BEFORE .with_state() for it to work
        .layer(axum::middleware::from_fn(common::logging::http_request_logger))
        .with_state(state)
}

// NOTE: These tests are temporarily disabled during AFIT migration.
// The handlers use ProductionAppState (hardcoded to RedisRtdb), but tests use MemoryRtdb.
// TODO: Either genericize handlers or convert these to integration tests with Redis.
#[cfg(test)]
#[path = "routes_tests.rs"]
mod tests;
