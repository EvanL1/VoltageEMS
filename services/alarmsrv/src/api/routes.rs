//! API routes configuration

use axum::{
    routing::{get, post},
    Router,
};

use crate::api::handlers::*;
use crate::AppState;

/// Create API routes
pub fn create_router(state: AppState) -> Router {
    let api_config = &state.config.api;

    Router::new()
        .route("/health", get(health_check))
        .route(
            &api_config.build_path("alarms"),
            get(list_alarms).post(create_alarm),
        )
        .route(
            &api_config.build_path("alarms/{id}/ack"),
            post(acknowledge_alarm),
        )
        .route(
            &api_config.build_path("alarms/{id}/resolve"),
            post(resolve_alarm),
        )
        .route(&api_config.build_path("status"), get(get_status))
        .route(&api_config.build_path("stats"), get(get_statistics))
        .with_state(state)
}
