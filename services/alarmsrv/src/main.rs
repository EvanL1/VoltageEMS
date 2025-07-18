use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

use alarmsrv::{
    api::routes,
    config::AlarmConfig,
    domain::AlarmClassifier,
    redis::{AlarmQueryService, AlarmRedisClient, AlarmStatisticsManager, AlarmStore},
    services::{
        rules::AlarmRulesEngine,
        scanner::{MonitorConfig, RedisDataScanner},
        start_alarm_processor, start_redis_listener,
    },
    AppState,
};

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
        redis_client: redis_client.clone(),
        alarm_store,
        query_service,
        stats_manager,
        classifier,
    };

    // Start background services
    start_redis_listener(state.clone()).await?;
    start_alarm_processor(state.clone()).await?;

    // Start data scanner if monitoring is enabled
    if config.monitoring.enabled {
        info!(
            "Starting data scanner with {} rules",
            config.alarm_rules.len()
        );

        // Create rules engine
        let mut rules_engine = AlarmRulesEngine::new();
        rules_engine.load_rules(config.alarm_rules.clone());
        let rules_engine = Arc::new(RwLock::new(rules_engine));

        // Create scanner configuration
        let monitor_config = MonitorConfig {
            channels: config.monitoring.channels.clone(),
            point_types: config.monitoring.point_types.clone(),
            scan_interval: config.monitoring.scan_interval,
        };

        // Start scanner
        let scanner =
            RedisDataScanner::new(redis_client.clone(), monitor_config, rules_engine).await?;

        scanner.start(state.clone()).await?;
        info!("Data scanner started successfully");
    } else {
        info!("Data scanning is disabled");
    }

    // Create API routes
    let app = routes::create_router(state);

    // Start HTTP server
    let addr = format!("{}:{}", config.service_api.host, config.service_api.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    info!("Alarm service started successfully, listening on: {}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}
