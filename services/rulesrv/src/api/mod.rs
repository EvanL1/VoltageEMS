pub mod handlers;
pub mod simple_handlers;

use axum::{
    routing::{delete, get, post, put},
    Router,
};
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use tracing::info;

use crate::config::ApiConfig;
use crate::engine::{RuleExecutor, SimpleRuleEngine};
use crate::redis::RedisStore;

pub use handlers::ApiState;
pub use simple_handlers::SimpleApiState;

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

/// Simple API server for the simplified rule engine
pub struct SimpleApiServer {
    engine: Arc<tokio::sync::RwLock<SimpleRuleEngine>>,
    store: Arc<RedisStore>,
    port: u16,
    api_config: ApiConfig,
}

impl SimpleApiServer {
    pub fn new(
        engine: SimpleRuleEngine,
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
        let state = Arc::new(SimpleApiState {
            engine: self.engine,
            store: self.store,
        });

        let app = Router::new()
            // Health check
            .route("/health", get(simple_handlers::simple_health_check))
            // Simple rule management
            .route(
                &self.api_config.build_path("simple/rules"),
                get(simple_handlers::list_simple_rules),
            )
            .route(
                &self.api_config.build_path("simple/rules"),
                post(simple_handlers::create_simple_rule),
            )
            .route(
                &self.api_config.build_path("simple/rules/{rule_id}"),
                get(simple_handlers::get_simple_rule),
            )
            .route(
                &self.api_config.build_path("simple/rules/{rule_id}"),
                put(simple_handlers::update_simple_rule),
            )
            .route(
                &self.api_config.build_path("simple/rules/{rule_id}"),
                delete(simple_handlers::delete_simple_rule),
            )
            // Simple rule execution
            .route(
                &self.api_config.build_path("simple/rules/{rule_id}/execute"),
                post(simple_handlers::execute_simple_rule),
            )
            .route(
                &self.api_config.build_path("simple/rules/test"),
                post(simple_handlers::test_simple_rule),
            )
            // Simple rule history and stats
            .route(
                &self.api_config.build_path("simple/rules/{rule_id}/history"),
                get(simple_handlers::get_simple_rule_history),
            )
            .route(
                &self.api_config.build_path("simple/rules/{rule_id}/stats"),
                get(simple_handlers::get_simple_rule_stats),
            )
            // Examples for documentation
            .route(
                &self.api_config.build_path("simple/examples"),
                get(simple_handlers::get_example_rules),
            )
            // Add CORS support
            .layer(CorsLayer::permissive())
            // Add state
            .with_state(state);

        let addr = format!("0.0.0.0:{}", self.port);
        info!("Starting Simple API server on {}", addr);

        let listener = tokio::net::TcpListener::bind(&addr).await.map_err(|e| {
            crate::error::RulesrvError::ApiError(format!("Failed to bind to {}: {}", addr, e))
        })?;

        axum::serve(listener, app)
            .await
            .map_err(|e| crate::error::RulesrvError::ApiError(format!("Server error: {}", e)))?;

        Ok(())
    }
}
