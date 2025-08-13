use axum::{routing::get, Router};
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

mod config;
mod error;
mod handlers;
mod redis_client;
mod response;

use config::Config;
use redis_client::RedisClient;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub redis_client: Arc<RedisClient>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    // Load configuration from local file
    let config = Arc::new(Config::load().expect("Failed to load configuration"));

    let bind_addr: SocketAddr = format!("{}:{}", config.server.host, 6005).parse()?;
    info!("Starting API Gateway on {}", bind_addr);

    // Initialize Redis client
    let redis_client = Arc::new(
        RedisClient::new(&config.redis.url)
            .await
            .expect("Failed to connect to Redis"),
    );

    // Create app state
    let app_state = AppState {
        config: config.clone(),
        redis_client,
    };

    // Build the application
    let app = Router::new()
        // Health check endpoints
        .route("/health", get(handlers::health_check))
        .route("/health/detailed", get(handlers::detailed_health))
        // API routes can be added here in the future
        .nest(
            "/api",
            Router::new()
                // Channel management endpoints
                .route("/channels", get(handlers::list_channels))
                .route("/channels/{channel_id}/status", get(handlers::get_channel_status))
                .route("/channels/{channel_id}/realtime", get(handlers::get_realtime_data))
                .route("/channels/{channel_id}/history", get(handlers::get_historical_data))
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
