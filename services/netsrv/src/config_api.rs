use crate::config::Config;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{delete, get, post, put},
    Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info};

/// Configuration management state
#[derive(Clone)]
pub struct ConfigState {
    /// Current active configuration
    pub current_config: Arc<RwLock<Config>>,
    /// Configuration versions for rollback
    pub config_versions: Arc<RwLock<HashMap<String, Config>>>,
    /// Configuration file path
    pub config_path: std::path::PathBuf,
}

/// API response for configuration operations
#[derive(Serialize, Deserialize)]
pub struct ConfigResponse {
    pub success: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// Configuration update request
#[derive(Deserialize)]
pub struct ConfigUpdateRequest {
    pub config: NetServiceConfig,
    #[serde(default)]
    pub save_as_version: Option<String>,
}

/// Configuration validation request
#[derive(Deserialize)]
pub struct ConfigValidationRequest {
    pub config: NetServiceConfig,
}

impl ConfigState {
    pub fn new(config: NetServiceConfig, config_path: std::path::PathBuf) -> Self {
        Self {
            current_config: Arc::new(RwLock::new(config)),
            config_versions: Arc::new(RwLock::new(HashMap::new())),
            config_path,
        }
    }
}

/// Create the configuration management API router
pub fn create_config_router(state: ConfigState) -> Router {
    Router::new()
        .route("/config", get(get_current_config))
        .route("/config", put(update_config))
        .route("/config/validate", post(validate_config))
        .route("/config/reload", post(reload_config))
        .route("/config/versions", get(list_config_versions))
        .route("/config/versions/:version", get(get_config_version))
        .route("/config/versions/:version", post(save_config_version))
        .route("/config/versions/:version", delete(delete_config_version))
        .route("/config/rollback/:version", post(rollback_to_version))
        .route("/config/export", get(export_config))
        .route("/config/aws/optimize", post(optimize_for_aws))
        .with_state(state)
}

/// Get current configuration
async fn get_current_config(
    State(state): State<ConfigState>,
) -> Result<Json<ConfigResponse>, StatusCode> {
    let config = state.current_config.read().await;

    Ok(Json(ConfigResponse {
        success: true,
        message: "Current configuration retrieved".to_string(),
        data: Some(serde_json::to_value(&*config).unwrap()),
    }))
}

/// Update current configuration
async fn update_config(
    State(state): State<ConfigState>,
    Json(request): Json<ConfigUpdateRequest>,
) -> Result<Json<ConfigResponse>, StatusCode> {
    // Validate configuration
    if let Err(e) = request.config.validate() {
        return Ok(Json(ConfigResponse {
            success: false,
            message: format!("Configuration validation failed: {}", e),
            data: None,
        }));
    }

    // Save as version if requested
    if let Some(version_name) = request.save_as_version {
        let current_config = state.current_config.read().await.clone();
        state
            .config_versions
            .write()
            .await
            .insert(version_name.clone(), current_config);
        info!("Saved current config as version: {}", version_name);
    }

    // Update current configuration
    *state.current_config.write().await = request.config;

    // Save to file
    if let Err(e) = save_config_to_file(&state).await {
        error!("Failed to save configuration to file: {}", e);
        return Ok(Json(ConfigResponse {
            success: false,
            message: format!("Failed to save configuration: {}", e),
            data: None,
        }));
    }

    info!("Configuration updated successfully");

    Ok(Json(ConfigResponse {
        success: true,
        message: "Configuration updated successfully".to_string(),
        data: None,
    }))
}

/// Validate configuration without saving
async fn validate_config(
    Json(request): Json<ConfigValidationRequest>,
) -> Result<Json<ConfigResponse>, StatusCode> {
    match request.config.validate() {
        Ok(_) => Ok(Json(ConfigResponse {
            success: true,
            message: "Configuration is valid".to_string(),
            data: None,
        })),
        Err(e) => Ok(Json(ConfigResponse {
            success: false,
            message: format!("Configuration validation failed: {}", e),
            data: None,
        })),
    }
}

/// Reload configuration from file
async fn reload_config(
    State(state): State<ConfigState>,
) -> Result<Json<ConfigResponse>, StatusCode> {
    match load_config::<NetServiceConfig>(&state.config_path) {
        Ok(new_config) => {
            *state.current_config.write().await = new_config;
            info!("Configuration reloaded from file");

            Ok(Json(ConfigResponse {
                success: true,
                message: "Configuration reloaded successfully".to_string(),
                data: None,
            }))
        }
        Err(e) => {
            error!("Failed to reload configuration: {}", e);
            Ok(Json(ConfigResponse {
                success: false,
                message: format!("Failed to reload configuration: {}", e),
                data: None,
            }))
        }
    }
}

/// List available configuration versions
async fn list_config_versions(
    State(state): State<ConfigState>,
) -> Result<Json<ConfigResponse>, StatusCode> {
    let versions = state.config_versions.read().await;
    let version_names: Vec<String> = versions.keys().cloned().collect();

    Ok(Json(ConfigResponse {
        success: true,
        message: format!("Found {} configuration versions", version_names.len()),
        data: Some(serde_json::to_value(version_names).unwrap()),
    }))
}

/// Get specific configuration version
async fn get_config_version(
    State(state): State<ConfigState>,
    Path(version): Path<String>,
) -> Result<Json<ConfigResponse>, StatusCode> {
    let versions = state.config_versions.read().await;

    if let Some(config) = versions.get(&version) {
        Ok(Json(ConfigResponse {
            success: true,
            message: format!("Configuration version '{}' retrieved", version),
            data: Some(serde_json::to_value(config).unwrap()),
        }))
    } else {
        Ok(Json(ConfigResponse {
            success: false,
            message: format!("Configuration version '{}' not found", version),
            data: None,
        }))
    }
}

/// Save current configuration as a named version
async fn save_config_version(
    State(state): State<ConfigState>,
    Path(version): Path<String>,
) -> Result<Json<ConfigResponse>, StatusCode> {
    let current_config = state.current_config.read().await.clone();
    state
        .config_versions
        .write()
        .await
        .insert(version.clone(), current_config);

    info!("Saved configuration version: {}", version);

    Ok(Json(ConfigResponse {
        success: true,
        message: format!("Configuration saved as version '{}'", version),
        data: None,
    }))
}

/// Delete a configuration version
async fn delete_config_version(
    State(state): State<ConfigState>,
    Path(version): Path<String>,
) -> Result<Json<ConfigResponse>, StatusCode> {
    let mut versions = state.config_versions.write().await;

    if versions.remove(&version).is_some() {
        info!("Deleted configuration version: {}", version);
        Ok(Json(ConfigResponse {
            success: true,
            message: format!("Configuration version '{}' deleted", version),
            data: None,
        }))
    } else {
        Ok(Json(ConfigResponse {
            success: false,
            message: format!("Configuration version '{}' not found", version),
            data: None,
        }))
    }
}

/// Rollback to a specific configuration version
async fn rollback_to_version(
    State(state): State<ConfigState>,
    Path(version): Path<String>,
) -> Result<Json<ConfigResponse>, StatusCode> {
    let versions = state.config_versions.read().await;
    let config_to_rollback = versions.get(&version).cloned();
    drop(versions); // Release read lock

    if let Some(config) = config_to_rollback {
        // Save current config as backup before rollback
        let current_config = state.current_config.read().await.clone();

        let backup_name = format!(
            "backup_before_rollback_{}",
            chrono::Utc::now().format("%Y%m%d_%H%M%S")
        );
        state
            .config_versions
            .write()
            .await
            .insert(backup_name.clone(), current_config);

        // Perform rollback
        *state.current_config.write().await = config.clone();

        // Save to file
        if let Err(e) = save_config_to_file(&state).await {
            error!("Failed to save configuration during rollback: {}", e);
            return Ok(Json(ConfigResponse {
                success: false,
                message: format!("Rollback failed to save: {}", e),
                data: None,
            }));
        }

        info!("Rolled back to configuration version: {}", version);
        info!("Current config backed up as: {}", backup_name);

        Ok(Json(ConfigResponse {
            success: true,
            message: format!(
                "Rolled back to version '{}', backup saved as '{}'",
                version, backup_name
            ),
            data: None,
        }))
    } else {
        Ok(Json(ConfigResponse {
            success: false,
            message: format!("Configuration version '{}' not found", version),
            data: None,
        }))
    }
}

/// Export configuration in different formats
async fn export_config(
    State(state): State<ConfigState>,
) -> Result<Json<ConfigResponse>, StatusCode> {
    let config = state.current_config.read().await;

    let mut exports = HashMap::new();

    // Export as YAML
    if let Ok(yaml) = serde_yaml::to_string(&*config) {
        exports.insert("yaml", yaml);
    }

    // Export as JSON
    if let Ok(json) = serde_json::to_string_pretty(&*config) {
        exports.insert("json", json);
    }

    Ok(Json(ConfigResponse {
        success: true,
        message: "Configuration exported".to_string(),
        data: Some(serde_json::to_value(exports).unwrap()),
    }))
}

/// Optimize configuration for AWS IoT
async fn optimize_for_aws(
    State(state): State<ConfigState>,
) -> Result<Json<ConfigResponse>, StatusCode> {
    let mut config = state.current_config.write().await;

    // Enable AWS features for cloud MQTT configurations
    for network in &mut config.networks {
        if let crate::config_new::NetworkConfig::CloudMqtt(cloud_config) = network {
            if matches!(cloud_config.provider, crate::config_new::CloudProvider::Aws) {
                cloud_config.aws_features.jobs_enabled = true;
                cloud_config.aws_features.device_shadow_enabled = true;
                cloud_config.aws_features.auto_respond_jobs = true;

                // Set AWS-optimized ALPN protocols
                cloud_config.tls.alpn_protocols = vec!["x-amzn-mqtt-ca".to_string()];

                info!(
                    "Optimized AWS IoT configuration for network: {}",
                    cloud_config.name
                );
            }
        }
    }

    // Save optimized configuration
    if let Err(e) = save_config_to_file(&state).await {
        error!("Failed to save AWS-optimized configuration: {}", e);
        return Ok(Json(ConfigResponse {
            success: false,
            message: format!("Failed to save optimized configuration: {}", e),
            data: None,
        }));
    }

    Ok(Json(ConfigResponse {
        success: true,
        message: "Configuration optimized for AWS IoT".to_string(),
        data: None,
    }))
}

/// Save current configuration to file
async fn save_config_to_file(
    state: &ConfigState,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = state.current_config.read().await;
    let yaml_content = serde_yaml::to_string(&*config)?;
    tokio::fs::write(&state.config_path, yaml_content).await?;
    Ok(())
}
