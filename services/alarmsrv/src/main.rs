use anyhow::Result;
use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, error};

mod types;
mod config;

use types::*;
use config::*;

/// Application state
#[derive(Clone)]
struct AppState {
    alarms: Arc<RwLock<Vec<Alarm>>>,
    config: Arc<AlarmConfig>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    info!("Starting Voltage EMS Alarm Service...");

    // Load configuration
    let config = Arc::new(AlarmConfig::load().await?);
    info!("Configuration loaded successfully");

    // Create application state
    let state = AppState {
        alarms: Arc::new(RwLock::new(Vec::new())),
        config: config.clone(),
    };

    // Start Redis listener
    start_redis_listener(state.clone()).await?;

    // Create API routes
    let app = create_router(state);

    // Start HTTP server
    let addr = format!("{}:{}", config.api.host, config.api.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    
    info!("Alarm service started successfully, listening on: {}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}

/// Create API routes
fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/alarms", get(list_alarms).post(create_alarm))
        .route("/alarms/:id/ack", post(acknowledge_alarm))
        .route("/status", get(get_status))
        .with_state(state)
}

/// Health check endpoint
async fn health_check() -> &'static str {
    "OK"
}

/// Get system status
async fn get_status(State(state): State<AppState>) -> Json<StatusResponse> {
    let alarms = state.alarms.read().await;
    let active_count = alarms.iter().filter(|a| a.is_active()).count();
    
    Json(StatusResponse {
        service: "alarmsrv".to_string(),
        status: "running".to_string(),
        total_alarms: alarms.len(),
        active_alarms: active_count,
    })
}

/// Get alarm list
async fn list_alarms(State(state): State<AppState>) -> Json<Vec<Alarm>> {
    let alarms = state.alarms.read().await;
    Json(alarms.clone())
}

/// Create new alarm
async fn create_alarm(
    State(state): State<AppState>,
    Json(request): Json<CreateAlarmRequest>,
) -> Result<Json<Alarm>, StatusCode> {
    let alarm = Alarm::new(request.title, request.description, request.level);
    
    let mut alarms = state.alarms.write().await;
    alarms.push(alarm.clone());
    
    info!("Created new alarm: {}", alarm.title);
    Ok(Json(alarm))
}

/// Acknowledge alarm
async fn acknowledge_alarm(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<Json<Alarm>, StatusCode> {
    let mut alarms = state.alarms.write().await;
    
    if let Some(alarm) = alarms.iter_mut().find(|a| a.id.to_string() == id) {
        alarm.acknowledge("system".to_string());
        info!("Alarm acknowledged: {}", alarm.title);
        Ok(Json(alarm.clone()))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

/// Start Redis listener
async fn start_redis_listener(state: AppState) -> Result<()> {
    let redis_url = format!("redis://{}:{}", state.config.redis.host, state.config.redis.port);
    let client = redis::Client::open(redis_url)?;
    
    tokio::spawn(async move {
        loop {
            match client.get_async_connection().await {
                Ok(conn) => {
                    info!("Redis connection successful, starting to listen for data...");
                    
                    // Listen to data channels
                    let mut pubsub = conn.into_pubsub();
                    if let Err(e) = pubsub.subscribe("ems:data:*").await {
                        error!("Redis subscription failed: {}", e);
                        continue;
                    }
                    
                    let mut stream = pubsub.into_on_message();
                    while let Some(msg) = stream.next().await {
                        if let Ok(payload) = msg.get_payload::<String>() {
                            // Simple data processing - check if alarm should be triggered
                            if let Err(e) = process_data_message(&state, &payload).await {
                                error!("Failed to process data message: {}", e);
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("Redis connection failed: {}", e);
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                }
            }
        }
    });
    
    Ok(())
}

/// Process data message
async fn process_data_message(state: &AppState, payload: &str) -> Result<()> {
    // Simple alarm triggering logic
    if let Ok(data) = serde_json::from_str::<serde_json::Value>(payload) {
        if let Some(value) = data.get("value").and_then(|v| v.as_f64()) {
            // Example: trigger alarm when temperature exceeds 80 degrees
            if value > 80.0 {
                let alarm = Alarm::new(
                    "High Temperature Alert".to_string(),
                    format!("High temperature detected: {:.1}°C", value),
                    AlarmLevel::Warning,
                );
                
                let mut alarms = state.alarms.write().await;
                alarms.push(alarm.clone());
                
                info!("Auto-triggered alarm: {}", alarm.title);
            }
        }
    }
    
    Ok(())
}

/// Status response
#[derive(Serialize)]
struct StatusResponse {
    service: String,
    status: String,
    total_alarms: usize,
    active_alarms: usize,
}

/// Create alarm request
#[derive(Deserialize)]
struct CreateAlarmRequest {
    title: String,
    description: String,
    level: AlarmLevel,
} 