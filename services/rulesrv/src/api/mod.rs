pub mod handlers;

use axum::{
    routing::{delete, get, post, put},
    Router,
};
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tracing::info;

use crate::engine::RuleExecutor;
use crate::redis::RedisStore;
use voltage_common::config::ApiConfig;

pub use handlers::ApiState;

pub struct ApiServer {
    executor: Arc<RuleExecutor>,
    store: Arc<RedisStore>,
    port: u16,
    api_config: ApiConfig,
}

impl ApiServer {
    pub fn new(
        executor: Arc<RuleExecutor>,
        store: Arc<RedisStore>,
        port: u16,
        api_config: ApiConfig,
    ) -> Self {
        Self {
            executor,
            store,
            port,
            api_config,
        }
    }

    pub async fn start(self) -> crate::error::Result<()> {
        let state = Arc::new(ApiState {
            executor: self.executor,
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
            // Rule history
            .route(
                &self.api_config.build_path("rules/{rule_id}/history"),
                get(handlers::get_rule_history),
            )
            // Rule groups
            .route(
                &self.api_config.build_path("groups"),
                get(handlers::list_rule_groups),
            )
            .route(
                &self.api_config.build_path("groups"),
                post(handlers::create_rule_group),
            )
            .route(
                &self.api_config.build_path("groups/{group_id}"),
                get(handlers::get_rule_group),
            )
            .route(
                &self.api_config.build_path("groups/{group_id}"),
                delete(handlers::delete_rule_group),
            )
            .route(
                &self.api_config.build_path("groups/{group_id}/rules"),
                get(handlers::get_group_rules),
            )
            // Add CORS support
            .layer(CorsLayer::permissive())
            // Add state
            .with_state(state);

        let addr = format!("0.0.0.0:{}", self.port);
        info!("Starting API server on {}", addr);

        let listener = tokio::net::TcpListener::bind(&addr).await.map_err(|e| {
            crate::error::RulesrvError::ApiError(format!("Failed to bind to {}: {}", addr, e))
        })?;

        axum::serve(listener, app)
            .await
            .map_err(|e| crate::error::RulesrvError::ApiError(format!("Server error: {}", e)))?;

        Ok(())
    }
}
