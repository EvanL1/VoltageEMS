//! HTTP API for simulator state observability during E2E testing.

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde_json::{json, Map, Value};

use crate::state_machine::StateMachineStore;

type SharedStore = Arc<StateMachineStore>;

pub async fn run_http_server(addr: &str, sm_store: Arc<StateMachineStore>) -> anyhow::Result<()> {
    let app = Router::new()
        .route("/health", get(health))
        .route("/state", get(all_states))
        .route("/state/:unit_id", get(single_state))
        .with_state(sm_store);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("simulator HTTP API listening on {addr}");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn health() -> impl IntoResponse {
    Json(json!({"status": "ok"}))
}

async fn all_states(State(store): State<SharedStore>) -> impl IntoResponse {
    let mut map = Map::new();
    for (unit_id, sm) in store.iter() {
        map.insert(
            unit_id.to_string(),
            Value::String(sm.current_state().as_str().to_string()),
        );
    }
    Json(Value::Object(map))
}

async fn single_state(
    State(store): State<SharedStore>,
    Path(unit_id): Path<u8>,
) -> impl IntoResponse {
    match store.get(&unit_id) {
        Some(sm) => {
            let body = json!({"unit_id": unit_id, "state": sm.current_state().as_str()});
            (StatusCode::OK, Json(body)).into_response()
        },
        None => StatusCode::NOT_FOUND.into_response(),
    }
}
