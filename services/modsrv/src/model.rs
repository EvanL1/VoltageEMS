//! ModSrv core model system
//!
//! Lightweight model management, focusing on metadata management and API services

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tracing::info;
use voltage_libs::redis::EdgeRedis;

use crate::error::{ModelSrvError, Result};
use crate::mapping::MappingManager;

/// Point configuration (static metadata)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointConfig {
    /// Point description
    pub description: String,
    /// Unit (optional)
    pub unit: Option<String>,
}

/// Model configuration (for loading)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    /// Model ID
    pub id: String,
    /// Model name
    pub name: String,
    /// Model description
    pub description: String,
    /// Monitoring point configuration
    pub monitoring: HashMap<String, PointConfig>,
    /// Control point configuration
    pub control: HashMap<String, PointConfig>,
}

/// Model structure (metadata only)
#[derive(Debug, Clone)]
pub struct Model {
    /// Model ID
    pub id: String,
    /// Model name
    pub name: String,
    /// Model description
    pub description: String,
    /// Monitoring point configuration
    pub monitoring_config: HashMap<String, PointConfig>,
    /// Control point configuration
    pub control_config: HashMap<String, PointConfig>,
}

impl Model {
    /// Create model from configuration
    pub fn from_config(config: ModelConfig) -> Self {
        Self {
            id: config.id,
            name: config.name,
            description: config.description,
            monitoring_config: config.monitoring,
            control_config: config.control,
        }
    }
}

/// Model manager
pub struct ModelManager {
    /// Model metadata storage
    models: Arc<RwLock<HashMap<String, Arc<Model>>>>,
    /// Redis connection
    redis: Arc<Mutex<EdgeRedis>>,
}

impl ModelManager {
    /// Create model manager
    pub async fn new(redis_url: &str) -> Result<Self> {
        let edge_redis = EdgeRedis::new(redis_url)
            .await
            .map_err(|e| ModelSrvError::redis(format!("Redis connection failed: {}", e)))?;

        Ok(Self {
            models: Arc::new(RwLock::new(HashMap::new())),
            redis: Arc::new(Mutex::new(edge_redis)),
        })
    }

    /// Load model configurations
    pub async fn load_models(&self, configs: Vec<ModelConfig>) -> Result<()> {
        let mut models = self.models.write().await;
        for config in configs {
            let model = Arc::new(Model::from_config(config));
            info!("Loading model: {} ({})", model.name, model.id);
            models.insert(model.id.clone(), model);
        }

        info!("Loaded {} models", models.len());
        Ok(())
    }

    /// Load mappings to Redis
    pub async fn load_mappings(&self, mappings: &MappingManager) -> Result<()> {
        let mut redis = self.redis.lock().await;

        // Clear old mappings
        let cleared = redis.clear_mappings().await?;
        if cleared > 0 {
            info!("Cleared {} old mappings", cleared);
        }

        // Load new mappings
        let mut count = 0;
        for (model_id, config) in mappings.get_all_mappings() {
            // Load monitoring point mappings (C2M)
            for (point_name, mapping) in &config.monitoring {
                let key = format!("{}:{}", mapping.channel, mapping.point);
                let value = format!("{}:{}", model_id, point_name);
                redis.init_mapping("c2m", &key, &value).await?;
                count += 1;
            }

            // Load control point mappings (M2C)
            for (control_name, mapping) in &config.control {
                let key = format!("{}:{}", model_id, control_name);
                let value = format!("{}:{}", mapping.channel, mapping.point);
                redis.init_mapping("m2c", &key, &value).await?;
                count += 1;
            }
        }

        info!("Loaded {} mappings to Redis", count);
        Ok(())
    }

    /// Get model metadata
    pub async fn get_model(&self, id: &str) -> Option<Arc<Model>> {
        let models = self.models.read().await;
        models.get(id).cloned()
    }

    /// List all models
    pub async fn list_models(&self) -> Vec<Arc<Model>> {
        let models = self.models.read().await;
        models.values().cloned().collect()
    }

    /// Get current model values (read from Redis)
    pub async fn get_model_values(&self, model_id: &str) -> Result<HashMap<String, f64>> {
        let mut redis = self.redis.lock().await;
        let values = redis.get_model_values(model_id).await?;

        let mut result = HashMap::new();
        for (key, value) in values {
            if let Ok(parsed) = value.parse::<f64>() {
                result.insert(key, parsed);
            }
        }

        Ok(result)
    }

    /// Create new model
    pub async fn create_model(&self, config: ModelConfig) -> Result<()> {
        let mut models = self.models.write().await;
        let model = Arc::new(Model::from_config(config));
        info!("Created model: {} ({})", model.name, model.id);
        models.insert(model.id.clone(), model);
        Ok(())
    }

    /// Update model
    pub async fn update_model(&self, config: ModelConfig) -> Result<()> {
        let mut models = self.models.write().await;
        let model = Arc::new(Model::from_config(config));
        info!("Updated model: {} ({})", model.name, model.id);
        models.insert(model.id.clone(), model);
        Ok(())
    }

    /// Delete model
    pub async fn delete_model(&self, model_id: &str) -> Result<()> {
        let mut models = self.models.write().await;
        if let Some(model) = models.remove(model_id) {
            info!("Deleted model: {} ({})", model.name, model.id);
        }
        Ok(())
    }

    /// Send control command
    pub async fn send_control(&self, model_id: &str, control_name: &str, value: f64) -> Result<()> {
        // Check if control point exists
        let models = self.models.read().await;
        let model = models
            .get(model_id)
            .ok_or_else(|| ModelSrvError::NotFound(format!("Model {} not found", model_id)))?;

        if !model.control_config.contains_key(control_name) {
            return Err(ModelSrvError::InvalidCommand(format!(
                "Control point {} not found",
                control_name
            )));
        }
        drop(models); // Release read lock

        // Send control command via Lua script
        let mut redis = self.redis.lock().await;
        redis
            .send_control(model_id, control_name, value)
            .await
            .map_err(|e| ModelSrvError::redis(format!("Failed to send control command: {}", e)))?;

        info!(
            "Control command sent: {}.{} = {:.6}",
            model_id, control_name, value
        );
        Ok(())
    }
}
