pub mod handlers;

use axum::{
    routing::{delete, get, post, put},
    Router,
};
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tracing::info;

use crate::config::ApiConfig;
use crate::engine::RuleEngine;
use crate::redis::RedisStore;

pub use handlers::ApiState;

/// API server for the rule engine
pub struct ApiServer {
    engine: Arc<tokio::sync::RwLock<RuleEngine>>,
    store: Arc<RedisStore>,
    port: u16,
    api_config: ApiConfig,
}

impl ApiServer {
    pub fn new(
        engine: RuleEngine,
        store: Arc<RedisStore>,
        port: u16,
        api_config: ApiConfig,
    ) -> Self {
        Self {
            engine: Arc::new(tokio::sync::RwLock::new(engine)),
            store,
            port,
            api_config,
        }
    }

    pub async fn start(self) -> crate::error::Result<()> {
        let state = Arc::new(ApiState {
            engine: self.engine,
            store: self.store,
        });

        let app = Router::new()
            // Health check
            .route("/health", get(handlers::health_check))
            // Rule management
            .route(
                &self.api_config.build_path("rules"),
                get(handlers::list_rules),
            )
            .route(
                &self.api_config.build_path("rules"),
                post(handlers::create_rule),
            )
            .route(
                &self.api_config.build_path("rules/{rule_id}"),
                get(handlers::get_rule),
            )
            .route(
                &self.api_config.build_path("rules/{rule_id}"),
                put(handlers::update_rule),
            )
            .route(
                &self.api_config.build_path("rules/{rule_id}"),
                delete(handlers::delete_rule),
            )
            // Rule execution
            .route(
                &self.api_config.build_path("rules/{rule_id}/execute"),
                post(handlers::execute_rule),
            )
            .route(
                &self.api_config.build_path("rules/test"),
                post(handlers::test_rule),
            )
            // Rule history and stats
            .route(
                &self.api_config.build_path("rules/{rule_id}/history"),
                get(handlers::get_rule_history),
            )
            .route(
                &self.api_config.build_path("rules/{rule_id}/stats"),
                get(handlers::get_rule_stats),
            )
            // Examples for documentation
            .route(
                &self.api_config.build_path("examples"),
                get(handlers::get_example_rules),
            )
            // Add CORS support
            .layer(CorsLayer::permissive())
            // Add state
            .with_state(state);

        let addr = format!("0.0.0.0:{}", self.port);

        let listener = tokio::net::TcpListener::bind(&addr).await.map_err(|e| {
            crate::error::RulesrvError::ApiError(format!("Failed to bind to {}: {}", addr, e))
        })?;

        axum::serve(listener, app)
            .await
            .map_err(|e| crate::error::RulesrvError::ApiError(format!("Server error: {}", e)))?;

        Ok(())
    }
}
