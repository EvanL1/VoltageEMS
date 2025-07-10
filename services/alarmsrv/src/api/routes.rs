//! API routes configuration

use axum::{
    routing::{get, post},
    Router,
};

use crate::api::handlers::*;
use crate::AppState;

/// Create API routes
pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/alarms", get(list_alarms).post(create_alarm))
        .route("/alarms/:id/ack", post(acknowledge_alarm))
        .route("/alarms/:id/resolve", post(resolve_alarm))
        .route("/alarms/classify", post(classify_alarms))
        .route("/alarms/categories", get(get_alarm_categories))
        .route("/status", get(get_status))
        .route("/stats", get(get_statistics))
        .with_state(state)
}
