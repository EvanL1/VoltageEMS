use axum::{
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

mod auth;
mod config;
mod direct_reader;
mod error;
mod handlers;
mod redis_client;
mod redis_wrapper;
mod response;
mod websocket;

use config::Config;
use direct_reader::DirectReader;
use redis_client::RedisClient;
use redis_wrapper::RedisWrapper;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub redis_client: Arc<RedisClient>,
    pub direct_reader: Arc<DirectReader>,
    pub http_client: Arc<reqwest::Client>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Load configuration from local file
    let config = Arc::new(Config::load().expect("Failed to load configuration"));

    let bind_addr: SocketAddr = format!("{}:{}", config.server.host, config.server.port).parse()?;
    info!("Starting API Gateway on {}", bind_addr);

    // Initialize Redis client
    let redis_client = Arc::new(
        RedisClient::new(&config.redis.url)
            .await
            .expect("Failed to connect to Redis"),
    );

    // Initialize Redis wrapper for direct reader
    let redis_wrapper = Arc::new(RedisWrapper::new(config.redis.url.clone()));
    let direct_reader = Arc::new(DirectReader::new(redis_wrapper));

    // Create HTTP client for backend services
    let http_client = Arc::new(reqwest::Client::new());

    // Create app state
    let app_state = AppState {
        config: config.clone(),
        redis_client: redis_client.clone(),
        direct_reader: direct_reader.clone(),
        http_client: http_client.clone(),
    };

    // Build the application
    let app = Router::new()
        // Health check (no auth)
        .route("/health", get(handlers::health_check))
        .route("/health/detailed", get(handlers::detailed_health))
        // API routes
        .nest(
            "/api",
            Router::new()
                // Auth routes (no auth required)
                .route("/auth/login", post(handlers::login))
                .route("/auth/refresh", post(handlers::refresh_token))
                // Protected routes
                .nest(
                    "",
                    Router::new()
                        .route("/auth/logout", post(handlers::logout))
                        .route("/auth/me", get(handlers::current_user))
                        // Direct read endpoints
                        .route("/v2/realtime/:type/:id", get(handlers::direct_read))
                        .route("/v2/realtime/batch", post(handlers::batch_read))
                        // Service proxy endpoints
                        .route("/comsrv/*path", axum::routing::any(handlers::comsrv_proxy))
                        .route("/modsrv/*path", axum::routing::any(handlers::modsrv_proxy))
                        .route("/hissrv/*path", axum::routing::any(handlers::hissrv_proxy))
                        .route("/netsrv/*path", axum::routing::any(handlers::netsrv_proxy))
                        .route(
                            "/alarmsrv/*path",
                            axum::routing::any(handlers::alarmsrv_proxy),
                        )
                        .route(
                            "/rulesrv/*path",
                            axum::routing::any(handlers::rulesrv_proxy),
                        )
                        .layer(auth::middleware::auth_layer(config.auth.jwt_secret.clone())),
                ),
        )
        // WebSocket endpoint (需要认证)
        .nest(
            "/ws",
            Router::new()
                .route("/realtime", get(websocket::ws_handler))
                .layer(auth::middleware::auth_layer(config.auth.jwt_secret.clone())),
        )
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .with_state(app_state);

    // Run the server
    let listener = tokio::net::TcpListener::bind(bind_addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
