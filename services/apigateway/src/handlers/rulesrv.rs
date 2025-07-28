use axum::{extract::State, routing::any, Router};

use crate::handlers::handle_proxy;
use crate::AppState;

/// Create routes for rulesrv proxy
pub fn proxy_routes() -> Router<AppState> {
    Router::new().fallback(any(proxy_handler))
}

async fn proxy_handler(
    state: State<AppState>,
    req: axum::extract::Request,
) -> axum::response::Response {
    handle_proxy("rulesrv", req, state).await
}
