use crate::error::{ModelSrvError, Result};
use crate::redis_handler::RedisConnection;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, error, info, warn};

/// Data mapping for model inputs and outputs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataMapping {
    /// Source key for data
    pub source_key: String,
    /// Field name in source data
    pub source_field: String,
    /// Target field name
    pub target_field: String,
    /// Optional transformation expression
    pub transform: Option<String>,
}

/// Model definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelDefinition {
    /// Unique model ID
    pub id: String,
    /// Display name
    pub name: String,
    /// Model description
    pub description: String,
    /// Input mappings
    pub input_mappings: Vec<DataMapping>,
    /// Output key
    pub output_key: String,
    /// Whether the model is enabled
    pub enabled: bool,
    /// Template ID the model is based on
    pub template_id: String,
    /// Model configuration
    pub config: HashMap<String, String>,
}

/// Control action type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ControlActionType {
    /// Remote control action (boolean value)
    RemoteControl,
    /// Remote adjustment action (analog value)
    RemoteAdjust,
}

/// Control action condition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlActionCondition {
    /// Condition field
    pub field: String,
    /// Comparison operator (>, <, >=, <=, ==, !=)
    pub operator: String,
    /// Comparison value
    pub value: String,
}

/// Control action
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlAction {
    /// Action ID
    pub id: String,
    /// Action name
    pub name: String,
    /// Action type
    pub action_type: ControlActionType,
    /// Channel name
    pub channel: String,
    /// Point name
    pub point: String,
    /// Action value
    pub value: String,
    /// Trigger conditions
    pub conditions: Vec<ControlActionCondition>,
    /// Whether enabled
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelWithActions {
    /// Basic model definition
    pub model: ModelDefinition,
    /// Control action list
    pub actions: Vec<ControlAction>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CommandStatus {
    Pending,
    Executing,
    Completed,
    Failed,
    Cancelled,
    Unknown,
}

pub struct ModelEngine {
    models: HashMap<String, ModelDefinition>,
    actions: HashMap<String, Vec<ControlAction>>,
    key_prefix: String,
}

impl ModelEngine {
    pub fn new() -> Self {
        Self {
            models: HashMap::new(),
            actions: HashMap::new(),
            key_prefix: String::new(),
        }
    }

    pub fn load_models(&mut self, redis_conn: &mut RedisConnection, pattern: &str) -> Result<()> {
        // Clear existing models
        self.models.clear();
        self.actions.clear();

        // Get all model configuration keys
        let model_keys = redis_conn.get_keys(pattern)?;
        info!("Found {} model configurations", model_keys.len());

        for key in model_keys {
            match self.load_model_from_store(redis_conn, &key) {
                Ok((model, actions)) => {
                    if model.enabled {
                        info!("Loaded model: {} ({})", model.name, model.id);
                        self.models.insert(model.id.clone(), model);
                        self.actions.insert(key.clone(), actions);
                    } else {
                        debug!("Skipped disabled model: {}", key);
                    }
                }
                Err(e) => {
                    error!("Failed to load model from {}: {}", key, e);
                }
            }
        }

        info!("Loaded {} models", self.models.len());
        Ok(())
    }

    fn load_model_from_store(
        &self,
        redis_conn: &mut RedisConnection,
        key: &str,
    ) -> Result<(ModelDefinition, Vec<ControlAction>)> {
        // Try to load the model from hash table
        let model_hash = match redis_conn.get_hash(key) {
            Ok(hash) => hash,
            Err(e) => {
                debug!("Failed to load model as hash, trying string format: {}", e);
                // If hash table loading fails, try old string format
                let model_json = redis_conn.get_string(key)?;

                // Try to parse as model with control actions
                let model_with_actions = serde_json::from_str::<ModelWithActions>(&model_json);

                if let Ok(model_with_actions) = model_with_actions {
                    return Ok((model_with_actions.model, model_with_actions.actions));
                }

                // If parsing fails, try parsing as basic model
                let model: ModelDefinition = serde_json::from_str(&model_json).map_err(|e| {
                    ModelSrvError::ModelError(format!("Failed to parse model JSON: {}", e))
                })?;

                return Ok((model, Vec::new()));
            }
        };

        debug!("Loading model from hash: {}", key);

        // Build model definition from hash table
        let id = model_hash
            .get("id")
            .ok_or_else(|| ModelSrvError::ModelError("Missing id field in model hash".to_string()))?
            .to_string();

        let name = model_hash
            .get("name")
            .unwrap_or(&"Unnamed Model".to_string())
            .to_string();

        let description = model_hash
            .get("description")
            .unwrap_or(&"No description".to_string())
            .to_string();

        // Parse input mappings
        let input_mappings = if let Some(mappings_json) = model_hash.get("input_mappings") {
            serde_json::from_str::<Vec<DataMapping>>(mappings_json).map_err(|e| {
                ModelSrvError::ModelError(format!("Failed to parse input mappings: {}", e))
            })?
        } else {
            Vec::new()
        };

        let output_key = model_hash
            .get("output_key")
            .unwrap_or(&format!("ems:model:output:{}", id))
            .to_string();

        let enabled = model_hash
            .get("enabled")
            .map(|v| v == "true")
            .unwrap_or(true);

        let template_id = model_hash
            .get("template_id")
            .unwrap_or(&String::new())
            .to_string();

        let config = if let Some(config_json) = model_hash.get("config") {
            serde_json::from_str::<HashMap<String, String>>(config_json)
                .map_err(|e| ModelSrvError::ModelError(format!("Failed to parse config: {}", e)))?
        } else {
            HashMap::new()
        };

        let model = ModelDefinition {
            id,
            name,
            description,
            input_mappings,
            output_key,
            enabled,
            template_id,
            config,
        };

        // Load actions
        let mut actions = Vec::new();

        if let Some(action_count_str) = model_hash.get("action_count") {
            if let Ok(action_count) = action_count_str.parse::<usize>() {
                for i in 0..action_count {
                    let action_key = format!("{}model:action:{}:{}", self.key_prefix, model.id, i);
                    match redis_conn.get_string(&action_key) {
                        Ok(action_json) => {
                            match serde_json::from_str::<ControlAction>(&action_json) {
                                Ok(action) => {
                                    actions.push(action);
                                }
                                Err(e) => {
                                    warn!("Failed to parse action JSON: {}", e);
                                }
                            }
                        }
                        Err(e) => {
                            warn!("Failed to load action {}: {}", i, e);
                        }
                    }
                }
            }
        }

        Ok((model, actions))
    }

    pub fn execute_models(&self, redis_conn: &mut RedisConnection) -> Result<()> {
        for (id, model) in &self.models {
            if !model.enabled {
                debug!("Skipping disabled model: {}", id);
                continue;
            }

            match self.execute_model(redis_conn, model) {
                Ok(_) => {
                    debug!("Successfully executed model: {}", id);

                    // Check if control actions need to be triggered after model execution
                    if let Some(_actions) = self.actions.get(id) {
                        self.check_and_execute_actions(redis_conn, id)?;
                    }
                }
                Err(e) => {
                    error!("Failed to execute model {}: {}", id, e);
                }
            }
        }

        Ok(())
    }

    fn execute_model(
        &self,
        redis_conn: &mut RedisConnection,
        model: &ModelDefinition,
    ) -> Result<()> {
        // Collect input data based on mappings
        let mut model_inputs: HashMap<String, String> = HashMap::new();

        for mapping in &model.input_mappings {
            // Get the source data
            if let Ok(value) = redis_conn.get_string(&mapping.source_key) {
                let transformed_value = self.apply_transform(value, &mapping.transform)?;
                model_inputs.insert(mapping.source_field.clone(), transformed_value);
            } else {
                warn!(
                    "Source field '{}' not found in key '{}'",
                    mapping.source_field, mapping.source_key
                );
            }
        }

        // Process the model (in a real implementation, this would apply the actual model logic)
        let model_outputs = self.process_model_data(model, &model_inputs)?;

        // Store the results in Redis
        let output_json = serde_json::to_string(&model_outputs)
            .map_err(|e| ModelSrvError::JsonError(e.to_string()))?;

        redis_conn.set_string(&model.output_key, &output_json)?;

        // Check and execute control actions
        if let Some(_actions) = self.actions.get(&format!("model:config:{}", model.id)) {
            if let Err(e) = self.check_and_execute_actions(redis_conn, &model.id) {
                error!("Failed to execute actions for model {}: {}", model.id, e);
            }
        }

        Ok(())
    }

    fn apply_transform(&self, value: String, transform: &Option<String>) -> Result<String> {
        let value_str = value.as_str();
        if let Some(transform_expr) = transform {
            // In a real implementation, this would parse and apply the transformation expression
            // For now, we'll just implement a simple scaling transform as an example
            if transform_expr.starts_with("scale:") {
                let scale_factor: f64 = transform_expr
                    .strip_prefix("scale:")
                    .unwrap_or("1.0")
                    .parse()
                    .map_err(|_| {
                        ModelSrvError::DataMappingError(format!(
                            "Invalid scale factor in transform: {}",
                            transform_expr
                        ))
                    })?;

                let value_f64: f64 = value_str.parse().map_err(|_| {
                    ModelSrvError::DataMappingError(format!(
                        "Cannot apply scaling transform to non-numeric value: {}",
                        value_str
                    ))
                })?;

                return Ok((value_f64 * scale_factor).to_string());
            }

            // Add more transform types as needed

            warn!("Unknown transform expression: {}", transform_expr);
        }

        // If no transform or unknown transform, return the original value
        Ok(value_str.to_string())
    }

    fn process_model_data(
        &self,
        model: &ModelDefinition,
        inputs: &HashMap<String, String>,
    ) -> Result<HashMap<String, String>> {
        // In a real implementation, this would apply the actual model logic
        // For this example, we'll just pass through the inputs to outputs
        // with a timestamp added

        let mut outputs = inputs.clone();

        // Add a timestamp
        let timestamp = chrono::Utc::now().to_rfc3339();
        outputs.insert("timestamp".to_string(), timestamp);
        outputs.insert("model_id".to_string(), model.id.clone());
        outputs.insert("model_name".to_string(), model.name.clone());

        Ok(outputs)
    }

    /// Check and execute model actions
    pub fn check_and_execute_actions(
        &self,
        redis_conn: &mut RedisConnection,
        model_id: &str,
    ) -> Result<()> {
        let model_key = format!("{}model:config:{}", self.key_prefix, model_id);
        let model_json = redis_conn.get_string(&model_key)?;

        // Try to parse as model with control actions
        let model_with_actions = serde_json::from_str::<ModelWithActions>(&model_json);

        if let Err(_) = model_with_actions {
            // Skip if not a model with actions
            return Ok(());
        }

        let model_with_actions = model_with_actions.unwrap();
        let model = &model_with_actions.model;
        let actions = &model_with_actions.actions;

        // Skip if model is disabled
        if !model.enabled {
            debug!("Model {} is disabled, skipping actions", model_id);
            return Ok(());
        }

        // Get model output
        let output_key = &model.output_key;
        let output_json = match redis_conn.get_string(output_key) {
            Ok(json) => json,
            Err(_) => {
                debug!("No output found for model {}", model_id);
                return Ok(());
            }
        };

        // Parse model output
        let model_outputs: HashMap<String, String> = match serde_json::from_str(&output_json) {
            Ok(outputs) => outputs,
            Err(e) => {
                warn!("Failed to parse model output: {}", e);
                return Ok(());
            }
        };

        for action in actions {
            if !action.enabled {
                debug!("Skipping disabled action: {}", action.id);
                continue;
            }

            // Check if all conditions are met
            let mut conditions_met = true;

            for condition in &action.conditions {
                // Use String as key for lookup
                if let Some(field_value) = model_outputs.get(&condition.field) {
                    if !self.evaluate_condition(
                        field_value,
                        &condition.operator,
                        &condition.value,
                    )? {
                        conditions_met = false;
                        break;
                    }
                } else {
                    warn!("Field '{}' not found in model outputs", condition.field);
                    conditions_met = false;
                    break;
                }
            }

            if conditions_met {
                info!("Executing action: {} for model {}", action.id, model_id);

                // Execute action
                // In a real implementation, this would perform different operations based on action_type
                // For example, sending control commands to devices, triggering other models, etc.

                // Record action execution
                let _action_log_key =
                    format!("{}model:action:{}:{}", self.key_prefix, model_id, action.id);
                let action_log = format!(
                    r#"{{"model_id":"{}","action_id":"{}","executed_at":{}}}"#,
                    model_id,
                    action.id,
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs()
                );

                // An actual Redis connection should be used here to store the log
                // For this example, we just print the log
                debug!("Action log: {}", action_log);
            }
        }

        Ok(())
    }

    /// Evaluate condition
    fn evaluate_condition(
        &self,
        field_value: &str,
        operator: &str,
        compare_value: &str,
    ) -> Result<bool> {
        // Try to parse both values as numbers for comparison
        let field_num = field_value.parse::<f64>();
        let compare_num = compare_value.parse::<f64>();

        if let (Ok(field_num), Ok(compare_num)) = (field_num, compare_num) {
            // Numeric comparison
            return match operator {
                ">" => Ok(field_num > compare_num),
                "<" => Ok(field_num < compare_num),
                ">=" => Ok(field_num >= compare_num),
                "<=" => Ok(field_num <= compare_num),
                "==" => Ok(field_num == compare_num),
                "!=" => Ok(field_num != compare_num),
                _ => Err(ModelSrvError::DataMappingError(format!(
                    "Unknown operator: {}",
                    operator
                ))),
            };
        }

        // String comparison
        match operator {
            "==" => Ok(field_value == compare_value),
            "!=" => Ok(field_value != compare_value),
            _ => Err(ModelSrvError::DataMappingError(format!(
                "Operator {} not supported for string comparison",
                operator
            ))),
        }
    }

    /// Send remote control command
    pub fn send_remote_control(
        &self,
        _redis_conn: &mut RedisConnection,
        channel: &str,
        point: &str,
        value: bool,
    ) -> Result<String> {
        // Simplified implementation, just log the operation
        let command_id = format!("cmd_{}", uuid::Uuid::new_v4());
        info!(
            "Remote control: channel={}, point={}, value={}",
            channel, point, value
        );
        Ok(command_id)
    }

    /// Send remote adjustment command
    pub fn send_remote_adjust(
        &self,
        _redis_conn: &mut RedisConnection,
        channel: &str,
        point: &str,
        value: f64,
    ) -> Result<String> {
        // Simplified implementation, just log the operation
        let command_id = format!("cmd_{}", uuid::Uuid::new_v4());
        info!(
            "Remote adjust: channel={}, point={}, value={}",
            channel, point, value
        );
        Ok(command_id)
    }

    /// Get command status
    pub fn get_command_status(
        &self,
        _redis_conn: &mut RedisConnection,
        _command_id: &str,
    ) -> Result<CommandStatus> {
        // Simplified implementation, always return completed
        Ok(CommandStatus::Completed)
    }

    /// Cancel command
    pub fn cancel_command(
        &self,
        _redis_conn: &mut RedisConnection,
        command_id: &str,
    ) -> Result<()> {
        // Simplified implementation, just log the operation
        info!("Cancel command: {}", command_id);
        Ok(())
    }
}

impl ModelDefinition {
    /// Validate the model definition
    pub fn validate(&self) -> Result<()> {
        if self.id.is_empty() {
            return Err(ModelSrvError::ValidationError(
                "Model ID cannot be empty".to_string(),
            ));
        }

        if self.template_id.is_empty() {
            return Err(ModelSrvError::ValidationError(
                "Template ID cannot be empty".to_string(),
            ));
        }

        Ok(())
    }

    /// Get an action by ID
    pub fn get_action(&self, _action_id: &str) -> Option<&ControlAction> {
        // This method should get actions from elsewhere
        None // Simplified implementation
    }
}

/// Model service for managing models
pub struct ModelService {
    /// Redis connection for persistence
    redis: Arc<RedisConnection>,
    /// Key prefix for storage
    key_prefix: String,
}

impl ModelService {
    /// Create a new model service
    pub fn new(redis: Arc<RedisConnection>, key_prefix: String) -> Self {
        ModelService { redis, key_prefix }
    }

    /// Get mutable connection
    fn get_mutable_connection(&self) -> Result<RedisConnection> {
        let conn = self.redis.clone();
        conn.duplicate()
    }

    pub async fn get_model(&self, id: &str) -> Result<ModelDefinition> {
        let key = format!("{}model:{}", self.key_prefix, id);
        let mut redis = self.get_mutable_connection()?;
        let json = redis.get_string(&key)?;

        let model: ModelDefinition = serde_json::from_str(&json)?;
        Ok(model)
    }

    pub async fn create_model(&self, model: &ModelDefinition) -> Result<()> {
        let key = format!("{}model:{}", self.key_prefix, model.id);
        let mut redis = self.get_mutable_connection()?;

        if redis.exists(&key)? {
            return Err(ModelSrvError::ModelAlreadyExists(model.id.clone()));
        }

        let json = serde_json::to_string(model)?;
        redis.set_string(&key, &json)?;

        Ok(())
    }

    pub async fn update_model(&self, id: &str, model: &ModelDefinition) -> Result<()> {
        let key = format!("{}model:{}", self.key_prefix, id);
        let mut redis = self.get_mutable_connection()?;

        if !redis.exists(&key)? {
            return Err(ModelSrvError::ModelNotFound(id.to_string()));
        }

        let json = serde_json::to_string(model)?;
        redis.set_string(&key, &json)?;

        Ok(())
    }

    pub async fn delete_model(&self, id: &str) -> Result<()> {
        let key = format!("{}model:{}", self.key_prefix, id);
        let mut redis = self.get_mutable_connection()?;

        if !redis.exists(&key)? {
            return Err(ModelSrvError::ModelNotFound(id.to_string()));
        }

        redis.delete(&key)?;

        Ok(())
    }

    pub async fn list_models(&self) -> Result<Vec<ModelDefinition>> {
        let mut redis = self.get_mutable_connection()?;
        let pattern = format!("{}model:*", self.key_prefix);
        let keys = redis.get_keys(&pattern)?;

        let mut models = Vec::new();
        for key in keys {
            match redis.get_string(&key) {
                Ok(json) => {
                    if let Ok(model) = serde_json::from_str::<ModelDefinition>(&json) {
                        models.push(model);
                    }
                }
                Err(_) => continue,
            }
        }

        Ok(models)
    }
}
