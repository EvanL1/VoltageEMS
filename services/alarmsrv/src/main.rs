use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

mod api;
mod config;
mod domain;
mod redis;
mod services;

use api::routes;
use config::AlarmConfig;
use domain::{Alarm, AlarmClassifier};
use redis::{AlarmQueryService, AlarmRedisClient, AlarmStatisticsManager, AlarmStore};
use services::{start_alarm_processor, start_redis_listener};

/// Application state
#[derive(Clone)]
pub struct AppState {
    pub alarms: Arc<RwLock<Vec<Alarm>>>,
    pub config: Arc<AlarmConfig>,
    pub redis_client: Arc<AlarmRedisClient>,
    pub alarm_store: Arc<AlarmStore>,
    pub query_service: Arc<AlarmQueryService>,
    pub stats_manager: Arc<AlarmStatisticsManager>,
    pub classifier: Arc<AlarmClassifier>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_thread_ids(true)
        .with_level(true)
        .init();

    info!("Starting Voltage EMS Alarm Service...");

    // Load configuration
    let config = Arc::new(AlarmConfig::load().await?);
    info!("Configuration loaded successfully");

    // Initialize Redis client
    let redis_client = Arc::new(AlarmRedisClient::new(config.clone()).await?);
    info!("Redis client initialized");

    // Initialize Redis-based services
    let alarm_store = Arc::new(AlarmStore::new(redis_client.clone()).await?);
    let query_service = Arc::new(AlarmQueryService::new(redis_client.clone()));
    let stats_manager = Arc::new(AlarmStatisticsManager::new(redis_client.clone()));

    // Initialize alarm classifier
    let classifier = Arc::new(AlarmClassifier::new(config.clone()));
    info!("Alarm classifier initialized");

    // Create application state
    let state = AppState {
        alarms: Arc::new(RwLock::new(Vec::new())),
        config: config.clone(),
        redis_client,
        alarm_store,
        query_service,
        stats_manager,
        classifier,
    };

    // Start background services
    start_redis_listener(state.clone()).await?;
    start_alarm_processor(state.clone()).await?;

    // Create API routes
    let app = routes::create_router(state);

    // Start HTTP server
    let addr = format!("{}:{}", config.api.host, config.api.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    info!("Alarm service started successfully, listening on: {}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}
