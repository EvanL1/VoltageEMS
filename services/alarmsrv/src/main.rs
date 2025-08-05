//! Lightweight AlarmSrv - Alarm Configuration Management Service
//!
//! This service manages alarm configurations and provides APIs for alarm management.
//! The actual alarm processing is handled by Redis Lua Functions.

use anyhow::{Context, Result};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{collections::HashMap, net::SocketAddr, path::PathBuf, sync::Arc};
use tokio::{
    fs,
    sync::{mpsc, RwLock},
};
use tracing::{error, info};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum AlarmLevel {
    Critical,
    Major,
    Minor,
    Warning,
    Info,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
enum AlarmStatus {
    New,
    Acknowledged,
    Resolved,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AlarmConfig {
    id: String,
    title: String,
    description: String,
    level: AlarmLevel,
    source: Option<String>,
    tags: Vec<String>,
    #[serde(default)]
    auto_resolve: bool,
    #[serde(default)]
    auto_resolve_timeout: u64,
    #[serde(default)]
    notification_channels: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AlarmTemplate {
    id: String,
    name: String,
    description: Option<String>,
    level: AlarmLevel,
    title_template: String,
    description_template: String,
    #[serde(default)]
    tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AlarmsFile {
    #[serde(default)]
    templates: Vec<AlarmTemplate>,
    #[serde(default)]
    alarm_rules: Vec<AlarmRule>,
    #[serde(default)]
    settings: Settings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AlarmRule {
    id: String,
    name: String,
    description: Option<String>,
    enabled: bool,
    condition: String,
    template_id: Option<String>,
    level: AlarmLevel,
    source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Settings {
    #[serde(default = "default_retention_days")]
    retention_days: u32,
    #[serde(default = "default_cleanup_interval")]
    cleanup_interval: u64,
    #[serde(default = "default_max_active_alarms")]
    max_active_alarms: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            retention_days: default_retention_days(),
            cleanup_interval: default_cleanup_interval(),
            max_active_alarms: default_max_active_alarms(),
        }
    }
}

fn default_retention_days() -> u32 {
    30
}
fn default_cleanup_interval() -> u64 {
    86400000 // 24 hours in milliseconds
}
fn default_max_active_alarms() -> usize {
    10000
}

struct AppState {
    redis_client: redis::Client,
    templates: Arc<RwLock<HashMap<String, AlarmTemplate>>>,
    rules: Arc<RwLock<HashMap<String, AlarmRule>>>,
    settings: Arc<RwLock<Settings>>,
    config_path: PathBuf,
    reload_tx: mpsc::Sender<()>,
}

#[derive(Debug, Deserialize)]
struct CreateAlarmRequest {
    title: String,
    description: String,
    level: AlarmLevel,
    source: Option<String>,
    tags: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct AlarmActionRequest {
    user: String,
}

#[derive(Debug, Deserialize)]
struct AlarmQueryParams {
    status: Option<String>,
    level: Option<String>,
    source: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
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

    info!("Starting Lightweight Alarm Service...");

    // Parse command line arguments
    let args: Vec<String> = std::env::args().collect();
    let config_path = if args.len() > 1 {
        PathBuf::from(&args[1])
    } else {
        PathBuf::from("config/alarms.yaml")
    };

    // Load initial configuration
    let alarms_file = load_alarms_file(&config_path).await?;
    let templates = Arc::new(RwLock::new(
        alarms_file
            .templates
            .into_iter()
            .map(|t| (t.id.clone(), t))
            .collect(),
    ));
    let rules = Arc::new(RwLock::new(
        alarms_file
            .alarm_rules
            .into_iter()
            .map(|r| (r.id.clone(), r))
            .collect(),
    ));
    let settings = Arc::new(RwLock::new(alarms_file.settings));

    // Connect to Redis
    let redis_url =
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
    let redis_client = redis::Client::open(redis_url)?;

    info!("Connected to Redis");

    // Create reload channel
    let (reload_tx, mut reload_rx) = mpsc::channel::<()>(10);

    // Create app state
    let state = Arc::new(AppState {
        redis_client,
        templates: templates.clone(),
        rules: rules.clone(),
        settings: settings.clone(),
        config_path: config_path.clone(),
        reload_tx,
    });

    // Start cleanup task
    let cleanup_state = state.clone();
    let cleanup_handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(
            cleanup_state.settings.read().await.cleanup_interval,
        ));

        loop {
            interval.tick().await;

            // Run cleanup
            match run_cleanup(&cleanup_state).await {
                Ok(count) => {
                    if count > 0 {
                        info!("Cleaned up {} old alarms", count);
                    }
                },
                Err(e) => error!("Error during cleanup: {}", e),
            }
        }
    });

    // Start configuration reload handler
    let reload_state = state.clone();
    let reload_handle = tokio::spawn(async move {
        while reload_rx.recv().await.is_some() {
            info!("Reloading configuration...");

            match load_alarms_file(&reload_state.config_path).await {
                Ok(alarms_file) => {
                    // Update templates
                    let mut templates_guard = reload_state.templates.write().await;
                    templates_guard.clear();
                    for template in alarms_file.templates {
                        templates_guard.insert(template.id.clone(), template);
                    }

                    // Update rules
                    let mut rules_guard = reload_state.rules.write().await;
                    rules_guard.clear();
                    for rule in alarms_file.alarm_rules {
                        rules_guard.insert(rule.id.clone(), rule);
                    }

                    // Update settings
                    *reload_state.settings.write().await = alarms_file.settings;

                    info!("Configuration reloaded successfully");
                },
                Err(e) => error!("Failed to reload configuration: {}", e),
            }
        }
    });

    // Create API routes
    let app = create_router(state);

    // Start HTTP server
    let addr = SocketAddr::from(([0, 0, 0, 0], 6002));
    let listener = tokio::net::TcpListener::bind(addr).await?;

    info!("Lightweight Alarm Service started successfully on {}", addr);
    info!("API endpoints:");
    info!("  GET /health - Health check");
    info!("  POST /api/v1/alarms - Create new alarm");
    info!("  GET /api/v1/alarms - List alarms");
    info!("  GET /api/v1/alarms/:id - Get alarm by ID");
    info!("  POST /api/v1/alarms/:id/acknowledge - Acknowledge alarm");
    info!("  POST /api/v1/alarms/:id/resolve - Resolve alarm");
    info!("  POST /api/v1/alarms/acknowledge - Batch acknowledge");
    info!("  GET /api/v1/stats - Get statistics");
    info!("  GET /api/v1/active-count - Get active alarm count");
    info!("  POST /api/v1/reload - Reload configuration");

    axum::serve(listener, app).await?;

    // Cleanup
    cleanup_handle.abort();
    reload_handle.abort();

    Ok(())
}

async fn load_alarms_file(path: &PathBuf) -> Result<AlarmsFile> {
    let content = fs::read_to_string(path)
        .await
        .context("Failed to read alarms file")?;

    let alarms_file: AlarmsFile =
        serde_yaml::from_str(&content).context("Failed to parse alarms file")?;

    Ok(alarms_file)
}

async fn run_cleanup(state: &AppState) -> Result<usize> {
    let settings = state.settings.read().await;
    let retention_days = settings.retention_days;

    let cutoff_time = Utc::now() - chrono::Duration::days(retention_days as i64);
    let cutoff_timestamp = cutoff_time.to_rfc3339();

    let mut conn = state
        .redis_client
        .get_multiplexed_async_connection()
        .await?;

    let count: String = redis::cmd("FCALL")
        .arg("cleanup_old_alarms")
        .arg(0)
        .arg(retention_days.to_string())
        .arg(&cutoff_timestamp)
        .query_async(&mut conn)
        .await?;

    Ok(count.parse().unwrap_or(0))
}

fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/api/v1/alarms", post(create_alarm).get(list_alarms))
        .route("/api/v1/alarms/{id}", get(get_alarm))
        .route("/api/v1/alarms/{id}/acknowledge", post(acknowledge_alarm))
        .route("/api/v1/alarms/{id}/resolve", post(resolve_alarm))
        .route("/api/v1/alarms/acknowledge", post(batch_acknowledge))
        .route("/api/v1/stats", get(get_stats))
        .route("/api/v1/active-count", get(get_active_count))
        .route("/api/v1/reload", post(reload_config))
        .with_state(state)
}

// API Handlers

async fn health_check() -> Json<serde_json::Value> {
    Json(json!({
        "status": "healthy",
        "service": "alarmsrv-lightweight",
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

async fn create_alarm(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateAlarmRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let alarm_id = Uuid::new_v4();
    let now = Utc::now();

    let priority = match req.level {
        AlarmLevel::Critical => 90,
        AlarmLevel::Major => 70,
        AlarmLevel::Minor => 50,
        AlarmLevel::Warning => 30,
        AlarmLevel::Info => 10,
    };

    let alarm = json!({
        "id": alarm_id,
        "title": req.title,
        "description": req.description,
        "level": req.level,
        "status": AlarmStatus::New,
        "source": req.source,
        "tags": req.tags.unwrap_or_default(),
        "priority": priority,
        "created_at": now.to_rfc3339(),
        "updated_at": now.to_rfc3339(),
    });

    let mut conn = state
        .redis_client
        .get_multiplexed_async_connection()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let _: String = redis::cmd("FCALL")
        .arg("store_alarm")
        .arg(1)
        .arg(alarm_id.to_string())
        .arg(serde_json::to_string(&alarm).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?)
        .query_async(&mut conn)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(json!({
        "id": alarm_id,
        "message": "Alarm created successfully"
    })))
}

async fn list_alarms(
    State(state): State<Arc<AppState>>,
    Query(params): Query<AlarmQueryParams>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let query = json!({
        "status": params.status,
        "level": params.level,
        "source": params.source,
        "limit": params.limit.unwrap_or(100),
        "offset": params.offset.unwrap_or(0),
    });

    let mut conn = state
        .redis_client
        .get_multiplexed_async_connection()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let result: String = redis::cmd("FCALL")
        .arg("query_alarms")
        .arg(0)
        .arg(serde_json::to_string(&query).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?)
        .query_async(&mut conn)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let result_json: serde_json::Value =
        serde_json::from_str(&result).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(result_json))
}

async fn get_alarm(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let mut conn = state
        .redis_client
        .get_multiplexed_async_connection()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let result: Option<String> = redis::cmd("FCALL")
        .arg("get_alarm")
        .arg(1)
        .arg(&id)
        .query_async(&mut conn)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    match result {
        Some(alarm_json) => {
            let alarm: serde_json::Value =
                serde_json::from_str(&alarm_json).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            Ok(Json(alarm))
        },
        None => Err(StatusCode::NOT_FOUND),
    }
}

async fn acknowledge_alarm(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<AlarmActionRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let mut conn = state
        .redis_client
        .get_multiplexed_async_connection()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let result: String = redis::cmd("FCALL")
        .arg("acknowledge_alarm")
        .arg(1)
        .arg(&id)
        .arg(&req.user)
        .arg(Utc::now().to_rfc3339())
        .query_async(&mut conn)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let alarm: serde_json::Value =
        serde_json::from_str(&result).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(alarm))
}

async fn resolve_alarm(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<AlarmActionRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let mut conn = state
        .redis_client
        .get_multiplexed_async_connection()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let result: String = redis::cmd("FCALL")
        .arg("resolve_alarm")
        .arg(1)
        .arg(&id)
        .arg(&req.user)
        .arg(Utc::now().to_rfc3339())
        .query_async(&mut conn)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let alarm: serde_json::Value =
        serde_json::from_str(&result).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(alarm))
}

async fn batch_acknowledge(
    State(state): State<Arc<AppState>>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let alarm_ids = body
        .get("alarm_ids")
        .and_then(|v| v.as_array())
        .ok_or(StatusCode::BAD_REQUEST)?;

    let user = body
        .get("user")
        .and_then(|v| v.as_str())
        .ok_or(StatusCode::BAD_REQUEST)?;

    let mut conn = state
        .redis_client
        .get_multiplexed_async_connection()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let result: String = redis::cmd("FCALL")
        .arg("acknowledge_alarms_batch")
        .arg(0)
        .arg(serde_json::to_string(alarm_ids).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?)
        .arg(user)
        .arg(Utc::now().to_rfc3339())
        .query_async(&mut conn)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let results: serde_json::Value =
        serde_json::from_str(&result).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(results))
}

async fn get_stats(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let mut conn = state
        .redis_client
        .get_multiplexed_async_connection()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let stats: String = redis::cmd("FCALL")
        .arg("get_alarm_stats")
        .arg(0)
        .query_async(&mut conn)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let stats_json: serde_json::Value =
        serde_json::from_str(&stats).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(stats_json))
}

async fn get_active_count(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let mut conn = state
        .redis_client
        .get_multiplexed_async_connection()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let count: String = redis::cmd("FCALL")
        .arg("get_active_alarm_count")
        .arg(0)
        .query_async(&mut conn)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let count_json: serde_json::Value =
        serde_json::from_str(&count).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(count_json))
}

async fn reload_config(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    state
        .reload_tx
        .send(())
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(json!({
        "message": "Configuration reload initiated"
    })))
}
