//! Lightweight ModSrv - Model Configuration Management Service
//!
//! This service manages model templates and instances, providing APIs for model management.
//! The actual data mapping and operations are handled by Redis Lua Functions.

use anyhow::{Context, Result};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{collections::HashMap, net::SocketAddr, path::PathBuf, sync::Arc};
use tokio::{
    fs,
    sync::{mpsc, RwLock},
};
use tracing::{error, info};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Template {
    id: String,
    name: String,
    description: Option<String>,
    data_points: HashMap<String, DataPoint>,
    #[serde(default)]
    actions: HashMap<String, ActionPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DataPoint {
    base_id: u32,
    unit: String,
    description: String,
    #[serde(default)]
    value_map: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ActionPoint {
    base_id: u32,
    description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Model {
    id: String,
    name: String,
    template: String,
    description: Option<String>,
    mapping: Mapping,
    #[serde(default)]
    metadata: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Mapping {
    channel: u32,
    #[serde(default)]
    data: HashMap<String, u32>,
    #[serde(default)]
    action: HashMap<String, u32>,
    #[serde(default)]
    data_offset: Option<u32>,
    #[serde(default)]
    action_offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ModelsFile {
    #[serde(default)]
    templates: Vec<Template>,
    models: Vec<Model>,
    #[serde(default)]
    settings: Settings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Settings {
    #[serde(default = "default_auto_reverse_mapping")]
    auto_reverse_mapping: bool,
    #[serde(default = "default_enable_notifications")]
    enable_value_notifications: bool,
    #[serde(default = "default_cache_ttl")]
    cache_ttl: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            auto_reverse_mapping: default_auto_reverse_mapping(),
            enable_value_notifications: default_enable_notifications(),
            cache_ttl: default_cache_ttl(),
        }
    }
}

fn default_auto_reverse_mapping() -> bool {
    true
}
fn default_enable_notifications() -> bool {
    true
}
fn default_cache_ttl() -> u64 {
    3600
}

struct AppState {
    redis_client: redis::Client,
    templates: Arc<RwLock<HashMap<String, Template>>>,
    models: Arc<RwLock<HashMap<String, Model>>>,
    settings: Arc<RwLock<Settings>>,
    config_path: PathBuf,
    reload_tx: mpsc::Sender<()>,
}

#[derive(Debug, Deserialize)]
struct ListQuery {
    template: Option<String>,
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

    info!("Starting Lightweight Model Service...");

    // Parse command line arguments
    let args: Vec<String> = std::env::args().collect();
    let config_path = if args.len() > 1 {
        PathBuf::from(&args[1])
    } else {
        PathBuf::from("config/models.yaml")
    };

    // Load initial configuration
    let models_file = load_models_file(&config_path).await?;
    let templates = Arc::new(RwLock::new(
        models_file
            .templates
            .into_iter()
            .map(|t| (t.id.clone(), t))
            .collect(),
    ));
    let models = Arc::new(RwLock::new(
        models_file
            .models
            .into_iter()
            .map(|m| (m.id.clone(), m))
            .collect(),
    ));
    let settings = Arc::new(RwLock::new(models_file.settings));

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
        models: models.clone(),
        settings: settings.clone(),
        config_path: config_path.clone(),
        reload_tx,
    });

    // Sync models to Redis on startup
    sync_models_to_redis(&state).await?;

    // Start configuration reload handler
    let reload_state = state.clone();
    let reload_handle = tokio::spawn(async move {
        while reload_rx.recv().await.is_some() {
            info!("Reloading configuration...");

            match load_models_file(&reload_state.config_path).await {
                Ok(models_file) => {
                    // Update templates
                    let mut templates_guard = reload_state.templates.write().await;
                    templates_guard.clear();
                    for template in models_file.templates {
                        templates_guard.insert(template.id.clone(), template);
                    }

                    // Update models
                    let mut models_guard = reload_state.models.write().await;
                    models_guard.clear();
                    for model in models_file.models {
                        models_guard.insert(model.id.clone(), model);
                    }

                    // Update settings
                    *reload_state.settings.write().await = models_file.settings;

                    // Sync to Redis
                    if let Err(e) = sync_models_to_redis(&reload_state).await {
                        error!("Failed to sync models to Redis: {}", e);
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
    let addr = SocketAddr::from(([0, 0, 0, 0], 6001));
    let listener = tokio::net::TcpListener::bind(addr).await?;

    info!("Lightweight Model Service started successfully on {}", addr);
    info!("API endpoints:");
    info!("  GET /health - Health check");
    info!("  GET /api/v1/templates - List all templates");
    info!("  GET /api/v1/templates/:id - Get template by ID");
    info!("  GET /api/v1/models - List all models");
    info!("  GET /api/v1/models/:id - Get model by ID");
    info!("  POST /api/v1/models - Create new model");
    info!("  PATCH /api/v1/models/:id - Update model");
    info!("  DELETE /api/v1/models/:id - Delete model");
    info!("  GET /api/v1/models/:id/values/:point - Get model value");
    info!("  POST /api/v1/models/:id/values/:point - Set model value");
    info!("  POST /api/v1/models/:id/values - Get multiple values");
    info!("  POST /api/v1/reload - Reload configuration");
    info!("  GET /api/v1/stats - Get statistics");

    axum::serve(listener, app).await?;

    // Cleanup
    reload_handle.abort();

    Ok(())
}

async fn load_models_file(path: &PathBuf) -> Result<ModelsFile> {
    let content = fs::read_to_string(path)
        .await
        .context("Failed to read models file")?;

    let models_file: ModelsFile =
        serde_yaml::from_str(&content).context("Failed to parse models file")?;

    Ok(models_file)
}

async fn sync_models_to_redis(state: &AppState) -> Result<()> {
    let mut conn = state
        .redis_client
        .get_multiplexed_async_connection()
        .await?;

    // First, store templates
    let templates = state.templates.read().await;
    for (id, template) in templates.iter() {
        let template_json = serde_json::to_string(template)?;
        let template_key = format!("modsrv:template:{}", id);
        let _: () = conn.set(&template_key, &template_json).await?;
    }
    info!("Synced {} templates to Redis", templates.len());

    // Then, process and store models
    let models = state.models.read().await;
    for (_id, model) in models.iter() {
        // Expand model with template data if using offsets
        let expanded_model = expand_model_with_template(model, &templates)?;
        let model_json = serde_json::to_string(&expanded_model)?;

        // Call Redis function to upsert model
        let _: String = redis::cmd("FCALL")
            .arg("model_upsert")
            .arg(1)  // number of keys
            .arg(&model.id)  // key
            .arg(&model_json)  // args
            .query_async(&mut conn)
            .await?;
    }

    info!("Synced {} models to Redis", models.len());
    Ok(())
}

fn expand_model_with_template(
    model: &Model,
    templates: &HashMap<String, Template>,
) -> Result<serde_json::Value> {
    let template = templates
        .get(&model.template)
        .context("Template not found")?;

    let mut expanded_mapping = model.mapping.clone();

    // Apply data offset if specified
    if let Some(data_offset) = model.mapping.data_offset {
        for (point_name, data_point) in &template.data_points {
            expanded_mapping
                .data
                .insert(point_name.clone(), data_point.base_id + data_offset);
        }
    }

    // Apply action offset if specified
    if let Some(action_offset) = model.mapping.action_offset {
        for (action_name, action_point) in &template.actions {
            expanded_mapping
                .action
                .insert(action_name.clone(), action_point.base_id + action_offset);
        }
    }

    Ok(json!({
        "id": model.id,
        "name": model.name,
        "template": model.template,
        "description": model.description,
        "mapping": expanded_mapping,
        "metadata": model.metadata,
    }))
}

fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/api/v1/templates", get(list_templates))
        .route("/api/v1/templates/{id}", get(get_template))
        .route("/api/v1/models", get(list_models).post(create_model))
        .route(
            "/api/v1/models/{id}",
            get(get_model).patch(update_model).delete(delete_model),
        )
        .route(
            "/api/v1/models/{id}/values/{point_name}",
            get(get_model_value).post(set_model_value),
        )
        .route("/api/v1/models/{id}/values", post(get_model_values))
        .route("/api/v1/reload", post(reload_config))
        .route("/api/v1/stats", get(get_stats))
        .with_state(state)
}

// API Handlers

async fn health_check() -> Json<serde_json::Value> {
    Json(json!({
        "status": "healthy",
        "service": "modsrv-lightweight",
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

async fn list_templates(State(state): State<Arc<AppState>>) -> Json<Vec<Template>> {
    let templates = state.templates.read().await;
    Json(templates.values().cloned().collect())
}

async fn get_template(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Template>, StatusCode> {
    let templates = state.templates.read().await;
    templates
        .get(&id)
        .cloned()
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

async fn list_models(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Vec<Model>>, StatusCode> {
    let models = state.models.read().await;

    let filtered_models: Vec<Model> = if let Some(template_filter) = query.template {
        models
            .values()
            .filter(|m| m.template == template_filter)
            .cloned()
            .collect()
    } else {
        models.values().cloned().collect()
    };

    Ok(Json(filtered_models))
}

async fn get_model(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Model>, StatusCode> {
    let models = state.models.read().await;
    models
        .get(&id)
        .cloned()
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

async fn create_model(
    State(state): State<Arc<AppState>>,
    Json(mut model): Json<Model>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // Generate ID if not provided
    if model.id.is_empty() {
        model.id = Uuid::new_v4().to_string();
    }

    // Validate template exists
    let templates = state.templates.read().await;
    if !templates.contains_key(&model.template) {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Expand model with template
    let expanded_model = expand_model_with_template(&model, &templates)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Add to local storage
    state
        .models
        .write()
        .await
        .insert(model.id.clone(), model.clone());

    // Sync to Redis
    let mut conn = state
        .redis_client
        .get_multiplexed_async_connection()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let model_json =
        serde_json::to_string(&expanded_model).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let _: String = redis::cmd("FCALL")
        .arg("model_upsert")
        .arg(1)
        .arg(&model.id)
        .arg(&model_json)
        .query_async(&mut conn)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(json!({
        "id": model.id,
        "message": "Model created successfully"
    })))
}

async fn update_model(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(updates): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let mut models = state.models.write().await;

    if let Some(model) = models.get_mut(&id) {
        // Apply updates
        if let Some(name) = updates.get("name").and_then(|v| v.as_str()) {
            model.name = name.to_string();
        }
        if let Some(description) = updates.get("description").and_then(|v| v.as_str()) {
            model.description = Some(description.to_string());
        }
        // ... apply other updates

        // Expand and sync to Redis
        let templates = state.templates.read().await;
        let expanded_model = expand_model_with_template(model, &templates)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let mut conn = state
            .redis_client
            .get_multiplexed_async_connection()
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let model_json = serde_json::to_string(&expanded_model)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let _: String = redis::cmd("FCALL")
            .arg("model_upsert")
            .arg(1)
            .arg(&model.id)
            .arg(&model_json)
            .query_async(&mut conn)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        Ok(Json(json!({
            "id": id,
            "message": "Model updated successfully"
        })))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

async fn delete_model(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    // Remove from local storage
    let removed = state.models.write().await.remove(&id).is_some();

    if removed {
        // Remove from Redis
        let mut conn = state
            .redis_client
            .get_multiplexed_async_connection()
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        let _: String = redis::cmd("FCALL")
            .arg("model_delete")
            .arg(1)
            .arg(&id)
            .query_async(&mut conn)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        Ok(Json(json!({
            "id": id,
            "message": "Model deleted successfully"
        })))
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

async fn get_model_value(
    State(state): State<Arc<AppState>>,
    Path((model_id, point_name)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let mut conn = state
        .redis_client
        .get_multiplexed_async_connection()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let result: String = redis::cmd("FCALL")
        .arg("model_get_value")
        .arg(2)
        .arg(&model_id)
        .arg(&point_name)
        .query_async(&mut conn)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let result_json: serde_json::Value =
        serde_json::from_str(&result).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(result_json))
}

async fn set_model_value(
    State(state): State<Arc<AppState>>,
    Path((model_id, point_name)): Path<(String, String)>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let value = body
        .get("value")
        .ok_or(StatusCode::BAD_REQUEST)?
        .to_string();

    let mut conn = state
        .redis_client
        .get_multiplexed_async_connection()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let _: String = redis::cmd("FCALL")
        .arg("model_set_value")
        .arg(2)
        .arg(&model_id)
        .arg(&point_name)
        .arg(&value)
        .query_async(&mut conn)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(json!({
        "model_id": model_id,
        "point_name": point_name,
        "value": value,
        "message": "Value set successfully"
    })))
}

async fn get_model_values(
    State(state): State<Arc<AppState>>,
    Path(model_id): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let point_names = body
        .get("points")
        .and_then(|v| v.as_array())
        .ok_or(StatusCode::BAD_REQUEST)?;

    let points_json =
        serde_json::to_string(point_names).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut conn = state
        .redis_client
        .get_multiplexed_async_connection()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let result: String = redis::cmd("FCALL")
        .arg("model_get_values_batch")
        .arg(1)
        .arg(&model_id)
        .arg(&points_json)
        .query_async(&mut conn)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let result_json: serde_json::Value =
        serde_json::from_str(&result).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(result_json))
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
        .arg("model_stats")
        .arg(0)
        .query_async(&mut conn)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let stats_json: serde_json::Value =
        serde_json::from_str(&stats).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(stats_json))
}
