//! Simplified AlarmSrv Main Entry Point
//!
//! This main function demonstrates the streamlined approach using direct Redis Functions.

use anyhow::Result;
use std::sync::Arc;
use tracing::info;

use alarmsrv::{
    alarm_service::AlarmService,
    api::{create_router, AppState},
    config::AlarmConfig,
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

    info!("Starting Simplified Voltage EMS Alarm Service...");

    // Load configuration
    let config = AlarmConfig::load().await?;
    info!("Configuration loaded successfully");

    // Initialize alarm service with Redis Functions
    let alarm_service = Arc::new(AlarmService::new(&config.redis.url).await?);
    info!("Alarm service initialized with Redis Functions");

    // Create application state
    let state = AppState {
        alarm_service: alarm_service.clone(),
    };

    // Simple monitoring setup (optional)
    if config.monitoring.enabled {
        info!("Monitoring is enabled but simplified - no complex scanning");
        // TODO: Add simple threshold monitoring if needed
    } else {
        info!("Monitoring is disabled");
    }

    // Create API routes
    let app = create_router(state);

    // Start HTTP server
    let addr = format!("{}:{}", config.api.host, config.api.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    info!(
        "Simplified Alarm service started successfully, listening on: {}",
        addr
    );
    info!("API endpoints:");
    info!("  GET /health - Health check");
    info!("  GET /api/v1/status - Service status");
    info!("  GET /api/v1/alarms - List alarms");
    info!("  POST /api/v1/alarms - Create alarm");
    info!("  GET /api/v1/alarms/{{id}} - Get alarm");
    info!("  POST /api/v1/alarms/{{id}}/ack - Acknowledge alarm");
    info!("  POST /api/v1/alarms/{{id}}/resolve - Resolve alarm");
    info!("  GET /api/v1/stats - Get statistics");

    axum::serve(listener, app).await?;

    Ok(())
}
