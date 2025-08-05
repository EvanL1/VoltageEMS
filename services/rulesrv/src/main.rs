//! Lightweight RuleSrv - Configuration Management Service
//!
//! This service manages rule configurations and provides APIs for rule management.
//! The actual rule execution is handled by Redis Lua Functions.

use anyhow::{Context, Result};
use axum::{
    extract::{Path, State},
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
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RuleConfig {
    id: String,
    name: String,
    description: Option<String>,
    enabled: bool,
    priority: u32,
    cooldown: Option<u64>,
    condition_logic: String,
    condition_groups: Vec<ConditionGroup>,
    actions: Vec<Action>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ConditionGroup {
    logic: String,
    conditions: Vec<Condition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Condition {
    source: String,
    op: String,
    value: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Action {
    action_type: String,
    #[serde(flatten)]
    params: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RulesFile {
    rules: Vec<RuleConfig>,
    #[serde(default)]
    rule_groups: Vec<RuleGroup>,
    #[serde(default)]
    settings: Settings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RuleGroup {
    name: String,
    description: Option<String>,
    rule_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Settings {
    #[serde(default = "default_execution_interval")]
    execution_interval: u64,
    #[serde(default = "default_enable_logging")]
    enable_logging: bool,
    #[serde(default = "default_max_concurrent")]
    max_concurrent_executions: usize,
    #[serde(default = "default_cooldown")]
    default_cooldown: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            execution_interval: default_execution_interval(),
            enable_logging: default_enable_logging(),
            max_concurrent_executions: default_max_concurrent(),
            default_cooldown: default_cooldown(),
        }
    }
}

fn default_execution_interval() -> u64 {
    1000
}
fn default_enable_logging() -> bool {
    true
}
fn default_max_concurrent() -> usize {
    10
}
fn default_cooldown() -> u64 {
    60
}

struct AppState {
    redis_client: redis::Client,
    rules: Arc<RwLock<HashMap<String, RuleConfig>>>,
    settings: Arc<RwLock<Settings>>,
    config_path: PathBuf,
    reload_tx: mpsc::Sender<()>,
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

    info!("Starting Lightweight Rules Service...");

    // Parse command line arguments
    let args: Vec<String> = std::env::args().collect();
    let config_path = if args.len() > 1 {
        PathBuf::from(&args[1])
    } else {
        PathBuf::from("config/rules.yaml")
    };

    // Load initial configuration
    let rules_file = load_rules_file(&config_path).await?;
    let rules = Arc::new(RwLock::new(
        rules_file
            .rules
            .into_iter()
            .map(|r| (r.id.clone(), r))
            .collect(),
    ));
    let settings = Arc::new(RwLock::new(rules_file.settings));

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
        rules: rules.clone(),
        settings: settings.clone(),
        config_path: config_path.clone(),
        reload_tx,
    });

    // Sync rules to Redis on startup
    sync_rules_to_redis(&state).await?;

    // Start periodic rule execution
    let execution_state = state.clone();
    let execution_handle = tokio::spawn(async move {
        let mut interval = interval(Duration::from_millis(
            execution_state.settings.read().await.execution_interval,
        ));

        loop {
            interval.tick().await;

            // Execute all active rules
            match execute_all_rules(&execution_state).await {
                Ok(results) => {
                    if execution_state.settings.read().await.enable_logging {
                        info!("Executed {} rules", results.len());
                    }
                },
                Err(e) => error!("Error executing rules: {}", e),
            }
        }
    });

    // Start configuration reload handler
    let reload_state = state.clone();
    let reload_handle = tokio::spawn(async move {
        while reload_rx.recv().await.is_some() {
            info!("Reloading configuration...");

            match load_rules_file(&reload_state.config_path).await {
                Ok(rules_file) => {
                    // Update rules
                    let mut rules_guard = reload_state.rules.write().await;
                    rules_guard.clear();
                    for rule in rules_file.rules {
                        rules_guard.insert(rule.id.clone(), rule);
                    }

                    // Update settings
                    *reload_state.settings.write().await = rules_file.settings;

                    // Sync to Redis
                    if let Err(e) = sync_rules_to_redis(&reload_state).await {
                        error!("Failed to sync rules to Redis: {}", e);
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
    let addr = SocketAddr::from(([0, 0, 0, 0], 6003));
    let listener = tokio::net::TcpListener::bind(addr).await?;

    info!("Lightweight Rules Service started successfully on {}", addr);
    info!("API endpoints:");
    info!("  GET /health - Health check");
    info!("  GET /api/v1/rules - List all rules");
    info!("  GET /api/v1/rules/:id - Get rule by ID");
    info!("  POST /api/v1/rules - Create new rule");
    info!("  PATCH /api/v1/rules/:id - Update rule");
    info!("  DELETE /api/v1/rules/:id - Delete rule");
    info!("  POST /api/v1/rules/:id/execute - Execute rule");
    info!("  POST /api/v1/rules/execute - Execute all rules");
    info!("  POST /api/v1/reload - Reload configuration");
    info!("  GET /api/v1/stats - Get statistics");

    axum::serve(listener, app).await?;

    // Cleanup
    execution_handle.abort();
    reload_handle.abort();

    Ok(())
}

async fn load_rules_file(path: &PathBuf) -> Result<RulesFile> {
    let content = fs::read_to_string(path)
        .await
        .context("Failed to read rules file")?;

    let rules_file: RulesFile =
        serde_yaml::from_str(&content).context("Failed to parse rules file")?;

    Ok(rules_file)
}

async fn sync_rules_to_redis(state: &AppState) -> Result<()> {
    let mut conn = state
        .redis_client
        .get_multiplexed_async_connection()
        .await?;

    let rules = state.rules.read().await;

    for (_id, rule) in rules.iter() {
        let rule_json = serde_json::to_string(rule)?;

        // Call Redis function to upsert rule
        let _: String = redis::cmd("FCALL")
            .arg("rule_upsert")
            .arg(1)  // number of keys
            .arg(&rule.id)  // key
            .arg(&rule_json)  // args
            .query_async(&mut conn)
            .await?;
    }

    info!("Synced {} rules to Redis", rules.len());
    Ok(())
}

async fn execute_all_rules(state: &AppState) -> Result<Vec<serde_json::Value>> {
    let mut conn = state
        .redis_client
        .get_multiplexed_async_connection()
        .await?;

    // Call Redis function to execute all rules
    let result: String = redis::cmd("FCALL")
        .arg("rule_execute_batch")
        .arg(0)  // no keys
        .query_async(&mut conn)
        .await?;

    let results: serde_json::Value = serde_json::from_str(&result)?;

    Ok(results["results"].as_array().cloned().unwrap_or_default())
}

fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/api/v1/rules", get(list_rules).post(create_rule))
        .route(
            "/api/v1/rules/{id}",
            get(get_rule).patch(update_rule).delete(delete_rule),
        )
        .route("/api/v1/rules/{id}/execute", post(execute_rule))
        .route("/api/v1/rules/execute", post(execute_all_rules_handler))
        .route("/api/v1/reload", post(reload_config))
        .route("/api/v1/stats", get(get_stats))
        .with_state(state)
}

// API Handlers

async fn health_check() -> Json<serde_json::Value> {
    Json(json!({
        "status": "healthy",
        "service": "rulesrv-lightweight",
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

async fn list_rules(State(state): State<Arc<AppState>>) -> Json<Vec<RuleConfig>> {
    let rules = state.rules.read().await;
    Json(rules.values().cloned().collect())
}

async fn get_rule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<RuleConfig>, StatusCode> {
    let rules = state.rules.read().await;
    rules
        .get(&id)
        .cloned()
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

async fn create_rule(
    State(state): State<Arc<AppState>>,
    Json(mut rule): Json<RuleConfig>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // Generate ID if not provided
    if rule.id.is_empty() {
        rule.id = Uuid::new_v4().to_string();
    }

    // Add to local storage
    state
        .rules
        .write()
        .await
        .insert(rule.id.clone(), rule.clone());

    // Sync to Redis
    let mut conn = state
        .redis_client
        .get_multiplexed_async_connection()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let rule_json = serde_json::to_string(&rule).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let _: String = redis::cmd("FCALL")
        .arg("rule_upsert")
        .arg(1)
        .arg(&rule.id)
        .arg(&rule_json)
        .query_async(&mut conn)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(json!({
        "id": rule.id,
        "message": "Rule created successfully"
    })))
}

async fn update_rule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(updates): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let mut rules = state.rules.write().await;

    if let Some(rule) = rules.get_mut(&id) {
        // Apply updates
        if let Some(name) = updates.get("name").and_then(|v| v.as_str()) {
            rule.name = name.to_string();
        }
        if let Some(enabled) = updates.get("enabled").and_then(|v| v.as_bool()) {
            rule.enabled = enabled;
        }
        // ... apply other updates

        // Sync to Redis
        let mut conn = state
            .redis_client
            .get_multiplexed_async_connection()
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let rule_json =
            serde_json::to_string(&rule).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let _: String = redis::cmd("FCALL")
            .arg("rule_upsert")
            .arg(1)
            .arg(&rule.id)
            .arg(&rule_json)
            .query_async(&mut conn)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        Ok(Json(json!({
            "id": id,
            "message": "Rule updated successfully"
        })))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

async fn delete_rule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // Remove from local storage
    let removed = state.rules.write().await.remove(&id).is_some();

    if removed {
        // Remove from Redis
        let mut conn = state
            .redis_client
            .get_multiplexed_async_connection()
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let _: String = redis::cmd("FCALL")
            .arg("rule_delete")
            .arg(1)
            .arg(&id)
            .query_async(&mut conn)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        Ok(Json(json!({
            "id": id,
            "message": "Rule deleted successfully"
        })))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

async fn execute_rule(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let mut conn = state
        .redis_client
        .get_multiplexed_async_connection()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let result: String = redis::cmd("FCALL")
        .arg("rule_execute")
        .arg(1)
        .arg(&id)
        .arg("false")  // not forced
        .query_async(&mut conn)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let result_json: serde_json::Value =
        serde_json::from_str(&result).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(result_json))
}

async fn execute_all_rules_handler(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let results = execute_all_rules(&state)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(json!({
        "executed": results.len(),
        "results": results
    })))
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

async fn get_stats(
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let mut conn = state
        .redis_client
        .get_multiplexed_async_connection()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let stats: String = redis::cmd("FCALL")
        .arg("rule_stats")
        .arg(0)
        .query_async(&mut conn)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let stats_json: serde_json::Value =
        serde_json::from_str(&stats).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(stats_json))
}
