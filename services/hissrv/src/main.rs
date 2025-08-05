//! Lightweight HisSrv - Historical Data Management Service
//!
//! This service manages historical data collection configurations and provides APIs.
//! The actual data collection and conversion is handled by Redis Lua Functions.

use anyhow::{Context, Result};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{collections::HashMap, net::SocketAddr, path::PathBuf, sync::Arc};
use tokio::{
    fs,
    sync::{mpsc, RwLock},
    time::{interval, Duration},
};
use tracing::{error, info};
use voltage_libs::influxdb::InfluxClient;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
enum TagRule {
    Static { value: String },
    Extract { field: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FieldMapping {
    source: String,
    target: String,
    #[serde(default)]
    transform: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DataMapping {
    id: String,
    name: String,
    description: Option<String>,
    enabled: bool,
    source: String,
    measurement: String,
    tags: Vec<TagRule>,
    fields: Vec<FieldMapping>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CollectionRule {
    id: String,
    name: String,
    description: Option<String>,
    enabled: bool,
    source_pattern: String,
    interval_seconds: u64,
    #[serde(default)]
    retention_days: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HistoryFile {
    mappings: Vec<DataMapping>,
    #[serde(default)]
    collection_rules: Vec<CollectionRule>,
    #[serde(default)]
    settings: Settings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Settings {
    #[serde(default = "default_polling_interval")]
    polling_interval_ms: u64,
    #[serde(default = "default_batch_size")]
    batch_size: usize,
    #[serde(default = "default_retention_days")]
    default_retention_days: u32,
    #[serde(default = "default_cleanup_interval")]
    cleanup_interval_hours: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            polling_interval_ms: default_polling_interval(),
            batch_size: default_batch_size(),
            default_retention_days: default_retention_days(),
            cleanup_interval_hours: default_cleanup_interval(),
        }
    }
}

fn default_polling_interval() -> u64 {
    60000 // 60 seconds
}
fn default_batch_size() -> usize {
    1000
}
fn default_retention_days() -> u32 {
    30
}
fn default_cleanup_interval() -> u32 {
    24 // hours
}

struct AppState {
    redis_client: redis::Client,
    influx_client: Option<Arc<InfluxClient>>,
    mappings: Arc<RwLock<HashMap<String, DataMapping>>>,
    rules: Arc<RwLock<HashMap<String, CollectionRule>>>,
    settings: Arc<RwLock<Settings>>,
    config_path: PathBuf,
    reload_tx: mpsc::Sender<()>,
}

#[derive(Debug, Deserialize)]
struct HistoryQuery {
    source: String,
    point_id: Option<String>,
    start_time: Option<String>,
    end_time: Option<String>,
    limit: Option<usize>,
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

    info!("Starting Lightweight History Service...");

    // Parse command line arguments
    let args: Vec<String> = std::env::args().collect();
    let config_path = if args.len() > 1 {
        PathBuf::from(&args[1])
    } else {
        PathBuf::from("config/history.yaml")
    };

    // Load initial configuration
    let history_file = load_history_file(&config_path).await?;
    let mappings = Arc::new(RwLock::new(
        history_file
            .mappings
            .into_iter()
            .map(|m| (m.id.clone(), m))
            .collect(),
    ));
    let rules = Arc::new(RwLock::new(
        history_file
            .collection_rules
            .into_iter()
            .map(|r| (r.id.clone(), r))
            .collect(),
    ));
    let settings = Arc::new(RwLock::new(history_file.settings));

    // Connect to Redis
    let redis_url =
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
    let redis_client = redis::Client::open(redis_url)?;

    info!("Connected to Redis");

    // Connect to InfluxDB (optional)
    let influx_client = if let Ok(influx_url) = std::env::var("INFLUXDB_URL") {
        let influx_org = std::env::var("INFLUXDB_ORG").unwrap_or_else(|_| "voltage".to_string());
        let influx_bucket =
            std::env::var("INFLUXDB_BUCKET").unwrap_or_else(|_| "voltage".to_string());
        let influx_token = std::env::var("INFLUXDB_TOKEN").ok();

        match InfluxClient::new(
            &influx_url,
            &influx_org,
            &influx_bucket,
            influx_token.as_deref().unwrap_or(""),
        ) {
            Ok(client) => {
                info!("Connected to InfluxDB");
                Some(Arc::new(client))
            },
            Err(e) => {
                error!("Failed to connect to InfluxDB: {}", e);
                None
            },
        }
    } else {
        info!("InfluxDB connection not configured");
        None
    };

    // Create reload channel
    let (reload_tx, mut reload_rx) = mpsc::channel::<()>(10);

    // Create app state
    let state = Arc::new(AppState {
        redis_client,
        influx_client,
        mappings: mappings.clone(),
        rules: rules.clone(),
        settings: settings.clone(),
        config_path: config_path.clone(),
        reload_tx,
    });

    // Configure initial mappings
    configure_mappings(&state).await?;

    // Start data collection task
    let collection_state = state.clone();
    let collection_handle = tokio::spawn(async move {
        let mut interval = {
            let settings = collection_state.settings.read().await;
            interval(Duration::from_millis(settings.polling_interval_ms))
        };

        let mut batch_counter = 0u64;

        loop {
            interval.tick().await;

            // Create and process batch
            match process_batch(&collection_state, batch_counter).await {
                Ok(line_count) => {
                    if line_count > 0 {
                        info!(
                            "Processed batch {} with {} lines",
                            batch_counter, line_count
                        );
                    }
                },
                Err(e) => error!("Error processing batch: {}", e),
            }

            batch_counter += 1;
        }
    });

    // Start cleanup task
    let cleanup_state = state.clone();
    let cleanup_handle = tokio::spawn(async move {
        let cleanup_interval = {
            let settings = cleanup_state.settings.read().await;
            Duration::from_secs(settings.cleanup_interval_hours as u64 * 3600)
        };

        let mut interval = interval(cleanup_interval);

        loop {
            interval.tick().await;

            match run_cleanup(&cleanup_state).await {
                Ok(count) => {
                    if count > 0 {
                        info!("Cleaned up {} old batches", count);
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

            match load_history_file(&reload_state.config_path).await {
                Ok(history_file) => {
                    // Update mappings
                    let mut mappings_guard = reload_state.mappings.write().await;
                    mappings_guard.clear();
                    for mapping in history_file.mappings {
                        mappings_guard.insert(mapping.id.clone(), mapping);
                    }

                    // Update rules
                    let mut rules_guard = reload_state.rules.write().await;
                    rules_guard.clear();
                    for rule in history_file.collection_rules {
                        rules_guard.insert(rule.id.clone(), rule);
                    }

                    // Update settings
                    *reload_state.settings.write().await = history_file.settings;

                    // Reconfigure mappings
                    if let Err(e) = configure_mappings(&reload_state).await {
                        error!("Failed to reconfigure mappings: {}", e);
                    } else {
                        info!("Configuration reloaded successfully");
                    }
                },
                Err(e) => error!("Failed to reload configuration: {}", e),
            }
        }
    });

    // Create API routes
    let app = create_router(state);

    // Start HTTP server
    let addr = SocketAddr::from(([0, 0, 0, 0], 6004));
    let listener = tokio::net::TcpListener::bind(addr).await?;

    info!(
        "Lightweight History Service started successfully on {}",
        addr
    );
    info!("API endpoints:");
    info!("  GET /health - Health check");
    info!("  GET /api/v1/mappings - List data mappings");
    info!("  GET /api/v1/mappings/:id - Get mapping by ID");
    info!("  GET /api/v1/rules - List collection rules");
    info!("  GET /api/v1/history - Query historical data");
    info!("  GET /api/v1/stats - Get statistics");
    info!("  POST /api/v1/reload - Reload configuration");

    axum::serve(listener, app).await?;

    // Cleanup
    collection_handle.abort();
    cleanup_handle.abort();
    reload_handle.abort();

    Ok(())
}

async fn load_history_file(path: &PathBuf) -> Result<HistoryFile> {
    let content = fs::read_to_string(path)
        .await
        .context("Failed to read history file")?;

    let history_file: HistoryFile =
        serde_yaml::from_str(&content).context("Failed to parse history file")?;

    Ok(history_file)
}

async fn configure_mappings(state: &AppState) -> Result<()> {
    let mut conn = state
        .redis_client
        .get_multiplexed_async_connection()
        .await?;

    let mappings = state.mappings.read().await;

    for (idx, (_, mapping)) in mappings.iter().enumerate() {
        let mapping_id = format!("mapping_{}", idx);

        // Convert tag rules
        let mut tags = HashMap::new();
        for tag_rule in &mapping.tags {
            match tag_rule {
                TagRule::Static { value } => {
                    if let Some((k, v)) = value.split_once('=') {
                        tags.insert(k.trim().to_string(), v.trim().to_string());
                    }
                },
                TagRule::Extract { field } => {
                    tags.insert(format!("__extract_{}", field), "true".to_string());
                },
            }
        }

        // Convert field mappings
        let mut field_mappings = HashMap::new();
        for field in &mapping.fields {
            field_mappings.insert(field.source.clone(), field.target.clone());
        }

        let config = json!({
            "source_pattern": mapping.source,
            "measurement": mapping.measurement,
            "tags": tags,
            "field_mappings": field_mappings,
            "enabled": mapping.enabled
        });

        let _: String = redis::cmd("FCALL")
            .arg("hissrv_configure_mapping")
            .arg(1)
            .arg(&mapping_id)
            .arg(serde_json::to_string(&config)?)
            .query_async(&mut conn)
            .await?;
    }

    info!("Configured {} mappings", mappings.len());
    Ok(())
}

async fn process_batch(state: &AppState, batch_id: u64) -> Result<usize> {
    let batch_id_str = format!("batch_{}", batch_id);

    let mut conn = state
        .redis_client
        .get_multiplexed_async_connection()
        .await?;

    // Create batch and collect data
    let line_count: String = redis::cmd("FCALL")
        .arg("hissrv_create_batch")
        .arg(1)
        .arg(&batch_id_str)
        .query_async(&mut conn)
        .await?;

    let count = line_count.parse::<usize>().unwrap_or(0);

    if count > 0 && state.influx_client.is_some() {
        // Get line protocol data
        let lines: String = redis::cmd("FCALL")
            .arg("hissrv_get_batch_lines")
            .arg(1)
            .arg(&batch_id_str)
            .query_async(&mut conn)
            .await?;

        // Write to InfluxDB
        if let Some(influx) = &state.influx_client {
            influx.write_line_protocol(&lines).await?;
        }

        // Acknowledge batch
        let _: String = redis::cmd("FCALL")
            .arg("hissrv_ack_batch")
            .arg(1)
            .arg(&batch_id_str)
            .arg("written")
            .query_async(&mut conn)
            .await?;
    }

    Ok(count)
}

async fn run_cleanup(state: &AppState) -> Result<usize> {
    let settings = state.settings.read().await;
    let hours_to_keep = settings.cleanup_interval_hours;

    let mut conn = state
        .redis_client
        .get_multiplexed_async_connection()
        .await?;

    let count: String = redis::cmd("FCALL")
        .arg("hissrv_cleanup_old_batches")
        .arg(0)
        .arg(hours_to_keep.to_string())
        .query_async(&mut conn)
        .await?;

    Ok(count.parse().unwrap_or(0))
}

fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/api/v1/mappings", get(list_mappings))
        .route("/api/v1/mappings/{id}", get(get_mapping))
        .route("/api/v1/rules", get(list_rules))
        .route("/api/v1/history", get(query_history))
        .route("/api/v1/stats", get(get_stats))
        .route("/api/v1/reload", post(reload_config))
        .with_state(state)
}

// API Handlers

async fn health_check() -> Json<serde_json::Value> {
    Json(json!({
        "status": "healthy",
        "service": "hissrv-lightweight",
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

async fn list_mappings(State(state): State<Arc<AppState>>) -> Json<Vec<DataMapping>> {
    let mappings = state.mappings.read().await;
    Json(mappings.values().cloned().collect())
}

async fn get_mapping(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<DataMapping>, StatusCode> {
    let mappings = state.mappings.read().await;
    mappings
        .get(&id)
        .cloned()
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

async fn list_rules(State(state): State<Arc<AppState>>) -> Json<Vec<CollectionRule>> {
    let rules = state.rules.read().await;
    Json(rules.values().cloned().collect())
}

async fn query_history(
    State(state): State<Arc<AppState>>,
    Query(query): Query<HistoryQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // For lightweight service, we just return a reference to InfluxDB
    if state.influx_client.is_none() {
        return Ok(Json(json!({
            "error": "InfluxDB not configured",
            "message": "Historical data queries require InfluxDB connection"
        })));
    }

    // In a real implementation, this would query InfluxDB
    // For now, return a placeholder response
    Ok(Json(json!({
        "message": "Query historical data from InfluxDB",
        "query": {
            "source": query.source,
            "point_id": query.point_id,
            "start_time": query.start_time,
            "end_time": query.end_time,
            "limit": query.limit.unwrap_or(100)
        }
    })))
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
        .arg("hissrv_get_mapping_stats")
        .arg(0)
        .query_async(&mut conn)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let stats_json: serde_json::Value =
        serde_json::from_str(&stats).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(stats_json))
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
