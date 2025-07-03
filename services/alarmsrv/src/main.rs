use anyhow::Result;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, error, warn};

mod types;
mod config;
mod config_new;
mod storage;
mod classifier;

use types::*;
use config::*;
use storage::*;
use classifier::*;

/// Application state
#[derive(Clone)]
struct AppState {
    alarms: Arc<RwLock<Vec<Alarm>>>,
    config: Arc<AlarmConfig>,
    redis_storage: Arc<RedisStorage>,
    classifier: Arc<AlarmClassifier>,
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

    // Initialize Redis storage
    let redis_storage = Arc::new(RedisStorage::new(config.clone()).await?);
    info!("Redis storage initialized");

    // Initialize alarm classifier
    let classifier = Arc::new(AlarmClassifier::new(config.clone()));
    info!("Alarm classifier initialized");

    // Create application state
    let state = AppState {
        alarms: Arc::new(RwLock::new(Vec::new())),
        config: config.clone(),
        redis_storage: redis_storage.clone(),
        classifier: classifier.clone(),
    };

    // Start Redis data listener for auto alarm generation
    start_redis_listener(state.clone()).await?;

    // Start alarm processing worker
    start_alarm_processor(state.clone()).await?;

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
        .route("/alarms/:id/resolve", post(resolve_alarm))
        .route("/alarms/classify", post(classify_alarms))
        .route("/alarms/categories", get(get_alarm_categories))
        .route("/status", get(get_status))
        .route("/stats", get(get_statistics))
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
    let redis_status = state.redis_storage.is_connected().await;
    
    Json(StatusResponse {
        service: "alarmsrv".to_string(),
        status: "running".to_string(),
        total_alarms: alarms.len(),
        active_alarms: active_count,
        redis_connected: redis_status,
        classifier_rules: state.classifier.get_rule_count(),
    })
}

/// Get alarm statistics
async fn get_statistics(State(state): State<AppState>) -> Result<Json<AlarmStatistics>, StatusCode> {
    match state.redis_storage.get_alarm_statistics().await {
        Ok(stats) => Ok(Json(stats)),
        Err(e) => {
            error!("Failed to get alarm statistics: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Get alarm list with optional filtering
#[derive(Deserialize)]
struct AlarmQuery {
    category: Option<String>,
    level: Option<AlarmLevel>,
    status: Option<AlarmStatus>,
    limit: Option<usize>,
    offset: Option<usize>,
    start_time: Option<String>,
    end_time: Option<String>,
    keyword: Option<String>,
}

/// Alarm list response with pagination info
#[derive(Serialize)]
struct AlarmListResponse {
    alarms: Vec<Alarm>,
    total: usize,
    offset: usize,
    limit: usize,
}

async fn list_alarms(
    State(state): State<AppState>,
    Query(query): Query<AlarmQuery>,
) -> Result<Json<AlarmListResponse>, StatusCode> {
    let offset = query.offset.unwrap_or(0);
    let limit = query.limit.unwrap_or(10).min(100); // Max 100 items per page
    
    match state.redis_storage.get_alarms_paginated(
        query.category,
        query.level,
        query.status,
        query.start_time,
        query.end_time,
        query.keyword,
        offset,
        limit,
    ).await {
        Ok((alarms, total)) => Ok(Json(AlarmListResponse {
            alarms,
            total,
            offset,
            limit,
        })),
        Err(e) => {
            error!("Failed to get alarms: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Create new alarm
async fn create_alarm(
    State(state): State<AppState>,
    Json(request): Json<CreateAlarmRequest>,
) -> Result<Json<Alarm>, StatusCode> {
    let mut alarm = Alarm::new(request.title, request.description, request.level);
    
    // Classify the alarm
    let classification = state.classifier.classify(&alarm).await;
    alarm.set_classification(classification);
    
    // Store in Redis
    if let Err(e) = state.redis_storage.store_alarm(&alarm).await {
        error!("Failed to store alarm in Redis: {}", e);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }
    
    // Store in memory for quick access
    let mut alarms = state.alarms.write().await;
    alarms.push(alarm.clone());
    
    // Publish for cloud push via netsrv
    if let Err(e) = state.redis_storage.publish_alarm_for_cloud(&alarm).await {
        warn!("Failed to publish alarm for cloud push: {}", e);
    }
    
    info!("Created new alarm: {} (Category: {})", alarm.title, alarm.classification.category);
    Ok(Json(alarm))
}

/// Acknowledge alarm
async fn acknowledge_alarm(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<Json<Alarm>, StatusCode> {
    // Update in Redis first
    match state.redis_storage.acknowledge_alarm(&id, "system".to_string()).await {
        Ok(alarm) => {
            // Update in memory
            let mut alarms = state.alarms.write().await;
            if let Some(mem_alarm) = alarms.iter_mut().find(|a| a.id.to_string() == id) {
                mem_alarm.acknowledge("system".to_string());
            }
            
            // Publish status update for cloud
            if let Err(e) = state.redis_storage.publish_alarm_for_cloud(&alarm).await {
                warn!("Failed to publish alarm update for cloud: {}", e);
            }
            
            info!("Alarm acknowledged: {}", alarm.title);
            Ok(Json(alarm))
        }
        Err(e) => {
            error!("Failed to acknowledge alarm: {}", e);
            Err(StatusCode::NOT_FOUND)
        }
    }
}

/// Resolve alarm
async fn resolve_alarm(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<Json<Alarm>, StatusCode> {
    // Update in Redis first
    match state.redis_storage.resolve_alarm(&id, "system".to_string()).await {
        Ok(alarm) => {
            // Update in memory
            let mut alarms = state.alarms.write().await;
            if let Some(mem_alarm) = alarms.iter_mut().find(|a| a.id.to_string() == id) {
                mem_alarm.resolve("system".to_string());
            }
            
            // Publish status update for cloud
            if let Err(e) = state.redis_storage.publish_alarm_for_cloud(&alarm).await {
                warn!("Failed to publish alarm resolution for cloud: {}", e);
            }
            
            info!("Alarm resolved: {}", alarm.title);
            Ok(Json(alarm))
        }
        Err(e) => {
            error!("Failed to resolve alarm: {}", e);
            Err(StatusCode::NOT_FOUND)
        }
    }
}

/// Classify existing alarms
async fn classify_alarms(State(state): State<AppState>) -> Result<Json<ClassificationResult>, StatusCode> {
    let mut classified_count = 0;
    let mut failed_count = 0;
    
    // Get unclassified alarms from Redis
    match state.redis_storage.get_unclassified_alarms().await {
        Ok(alarms) => {
            for mut alarm in alarms {
                let classification = state.classifier.classify(&alarm).await;
                alarm.set_classification(classification);
                
                match state.redis_storage.update_alarm_classification(&alarm).await {
                    Ok(_) => {
                        classified_count += 1;
                        // Publish updated classification for cloud
                        if let Err(e) = state.redis_storage.publish_alarm_for_cloud(&alarm).await {
                            warn!("Failed to publish classified alarm for cloud: {}", e);
                        }
                    }
                    Err(e) => {
                        error!("Failed to update alarm classification: {}", e);
                        failed_count += 1;
                    }
                }
            }
        }
        Err(e) => {
            error!("Failed to get unclassified alarms: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    }
    
    Ok(Json(ClassificationResult {
        classified_count,
        failed_count,
    }))
}

/// Get alarm categories
async fn get_alarm_categories(State(state): State<AppState>) -> Json<Vec<AlarmCategory>> {
    Json(state.classifier.get_categories())
}

/// Start Redis listener for auto alarm generation
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
                            // Process data message and generate alarms if needed
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

/// Start alarm processing worker
async fn start_alarm_processor(state: AppState) -> Result<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
        
        loop {
            interval.tick().await;
            
            // Process alarm escalation
            if let Err(e) = process_alarm_escalation(&state).await {
                error!("Failed to process alarm escalation: {}", e);
            }
            
            // Clean up old resolved alarms
            if let Err(e) = cleanup_old_alarms(&state).await {
                error!("Failed to cleanup old alarms: {}", e);
            }
        }
    });
    
    Ok(())
}

/// Process data message and generate alarms
async fn process_data_message(state: &AppState, payload: &str) -> Result<()> {
    if let Ok(data) = serde_json::from_str::<serde_json::Value>(payload) {
        // Check various alarm conditions
        if let Some(value) = data.get("value").and_then(|v| v.as_f64()) {
            let mut alarms_to_create = Vec::new();
            
            // Temperature threshold
            if value > 80.0 {
                alarms_to_create.push(Alarm::new(
                    "High Temperature Alert".to_string(),
                    format!("High temperature detected: {:.1}°C", value),
                    AlarmLevel::Warning,
                ));
            }
            
            // Critical temperature
            if value > 90.0 {
                alarms_to_create.push(Alarm::new(
                    "Critical Temperature Alert".to_string(),
                    format!("Critical temperature detected: {:.1}°C", value),
                    AlarmLevel::Critical,
                ));
            }
            
            // Process each alarm
            for mut alarm in alarms_to_create {
                // Classify the alarm
                let classification = state.classifier.classify(&alarm).await;
                alarm.set_classification(classification);
                
                // Store in Redis
                if let Err(e) = state.redis_storage.store_alarm(&alarm).await {
                    error!("Failed to store auto-generated alarm: {}", e);
                    continue;
                }
                
                // Add to memory
                let mut alarms = state.alarms.write().await;
                alarms.push(alarm.clone());
                
                // Publish for cloud push
                if let Err(e) = state.redis_storage.publish_alarm_for_cloud(&alarm).await {
                    warn!("Failed to publish auto-generated alarm for cloud: {}", e);
                }
                
                info!("Auto-triggered alarm: {} (Category: {})", alarm.title, alarm.classification.category);
            }
        }
    }
    
    Ok(())
}

/// Process alarm escalation
async fn process_alarm_escalation(state: &AppState) -> Result<()> {
    let escalation_rules = state.classifier.get_escalation_rules();
    
    for rule in escalation_rules {
        match state.redis_storage.get_alarms_for_escalation(&rule).await {
            Ok(alarms) => {
                for alarm in alarms {
                    // Escalate alarm level
                    let mut escalated_alarm = alarm.clone();
                    escalated_alarm.escalate();
                    
                    // Update in Redis
                    if let Err(e) = state.redis_storage.update_alarm(&escalated_alarm).await {
                        error!("Failed to update escalated alarm: {}", e);
                        continue;
                    }
                    
                    // Publish escalation for cloud
                    if let Err(e) = state.redis_storage.publish_alarm_for_cloud(&escalated_alarm).await {
                        warn!("Failed to publish escalated alarm for cloud: {}", e);
                    }
                    
                    info!("Alarm escalated: {} -> {:?}", escalated_alarm.title, escalated_alarm.level);
                }
            }
            Err(e) => {
                error!("Failed to get alarms for escalation: {}", e);
            }
        }
    }
    
    Ok(())
}

/// Clean up old resolved alarms
async fn cleanup_old_alarms(state: &AppState) -> Result<()> {
    let retention_days = state.config.storage.retention_days;
    
    match state.redis_storage.cleanup_old_alarms(retention_days).await {
        Ok(count) => {
            if count > 0 {
                info!("Cleaned up {} old alarms", count);
            }
        }
        Err(e) => {
            error!("Failed to cleanup old alarms: {}", e);
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
    redis_connected: bool,
    classifier_rules: usize,
}

/// Create alarm request
#[derive(Deserialize)]
struct CreateAlarmRequest {
    title: String,
    description: String,
    level: AlarmLevel,
}

/// Classification result
#[derive(Serialize)]
struct ClassificationResult {
    classified_count: usize,
    failed_count: usize,
} 