use axum::{extract::State, routing::any, Router};

use crate::handlers::handle_proxy;
use crate::AppState;

/// Create routes for modsrv proxy
pub fn proxy_routes() -> Router<AppState> {
    Router::new().fallback(any(proxy_handler))
}

#[axum::debug_handler]
async fn proxy_handler(
    state: State<AppState>,
    req: axum::extract::Request,
) -> axum::response::Response {
    handle_proxy("modsrv", req, state).await
}
