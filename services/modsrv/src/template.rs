use crate::error::{ModelSrvError, Result};
use crate::model::{ModelDefinition, ControlAction, ModelWithActions, DataMapping};
use crate::redis_handler::RedisConnection;
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use crate::storage::DataStore;

/// Template information in the template index
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub file_path: String,
}

/// Template index
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateIndex {
    pub templates: Vec<TemplateInfo>,
}

/// Template manager
pub struct TemplateManager {
    /// Templates directory path
    templates_dir: PathBuf,
    /// Redis prefix
    key_prefix: String,
}

impl TemplateManager {
    /// Create a new template manager
    pub fn new(templates_dir: &str, key_prefix: &str) -> Self {
        Self {
            templates_dir: PathBuf::from(templates_dir),
            key_prefix: key_prefix.to_string(),
        }
    }
    
    /// Load template index
    pub fn load_template_index(&self) -> Result<TemplateIndex> {
        // First try to load YAML format index file
        let yaml_index_path = Path::new(&self.templates_dir).join("index.yaml");
        let yml_index_path = Path::new(&self.templates_dir).join("index.yml");
        let json_index_path = Path::new(&self.templates_dir).join("index.json");
        
        let (index_path, is_yaml) = if yaml_index_path.exists() {
            (yaml_index_path, true)
        } else if yml_index_path.exists() {
            (yml_index_path, true)
        } else if json_index_path.exists() {
            (json_index_path, false)
        } else {
            return Err(ModelSrvError::TemplateError(
                format!("Template index file not found in directory: {}", self.templates_dir.display())
            ));
        };
        
        let index_content = fs::read_to_string(&index_path)
            .map_err(|e| ModelSrvError::TemplateError(
                format!("Failed to read template index: {}", e)
            ))?;
            
        let index: TemplateIndex = if is_yaml {
            serde_yaml::from_str(&index_content)
                .map_err(|e| ModelSrvError::TemplateError(
                    format!("Failed to parse YAML template index: {}", e)
                ))?
        } else {
            serde_json::from_str(&index_content)
                .map_err(|e| ModelSrvError::TemplateError(
                    format!("Failed to parse JSON template index: {}", e)
                ))?
        };
            
        Ok(index)
    }
    
    /// Load template
    pub fn load_template(&self, template_id: &str) -> Result<(ModelDefinition, Vec<ControlAction>)> {
        let template = self.get_template_by_id(template_id)?;
        let template_path = Path::new(&template.file_path);
        
        let content = fs::read_to_string(template_path)
            .map_err(|e| ModelSrvError::IoError(e))?;
            
        // Determine format based on file extension
        let model_with_actions = match template_path.extension().and_then(|ext| ext.to_str()) {
            Some("yaml") | Some("yml") => {
                serde_yaml::from_str::<ModelWithActions>(&content)
                    .map_err(|e| ModelSrvError::YamlError(e))?
            },
            Some("json") => {
                serde_json::from_str::<ModelWithActions>(&content)
                    .map_err(|e| ModelSrvError::JsonError(e))?
            },
            _ => {
                // Default to YAML format
                serde_yaml::from_str::<ModelWithActions>(&content)
                    .map_err(|e| ModelSrvError::YamlError(e))?
            }
        };
            
        Ok((model_with_actions.model, model_with_actions.actions))
    }
    
    /// Get template by ID
    pub fn get_template_by_id(&self, template_id: &str) -> Result<TemplateInfo> {
        let templates_dir = Path::new(&self.templates_dir);
        let entries = fs::read_dir(templates_dir).map_err(|e| ModelSrvError::IoError(e))?;
        
        for entry_result in entries {
            let entry = entry_result.map_err(|e| ModelSrvError::IoError(e))?;
            let path = entry.path();
            
            if path.is_file() && path.extension().map_or(false, |ext| {
                ext == "json" || ext == "yaml" || ext == "yml"
            }) {
                let template = self.load_template_info(&path)?;
                if template.id == template_id {
                    return Ok(template);
                }
            }
        }
        
        Err(ModelSrvError::TemplateError(format!("Template not found: {}", template_id)))
    }

    /// List all available templates
    pub fn list_templates(&self) -> Result<Vec<TemplateInfo>> {
        let templates_dir = Path::new(&self.templates_dir);
        println!("Looking for templates in directory: {}", templates_dir.display());
        let entries = fs::read_dir(templates_dir).map_err(|e| ModelSrvError::IoError(e))?;
        
        let mut templates = Vec::new();
        
        for entry_result in entries {
            let entry = entry_result.map_err(|e| ModelSrvError::IoError(e))?;
            let path = entry.path();
            
            println!("Found file: {}", path.display());
            
            if path.is_file() && path.extension().map_or(false, |ext| {
                ext == "json" || ext == "yaml" || ext == "yml"
            }) {
                println!("Loading template info from: {}", path.display());
                let template = self.load_template_info(&path)?;
                templates.push(template);
            }
        }
        
        Ok(templates)
    }
    
    /// Load template info
    fn load_template_info(&self, path: &Path) -> Result<TemplateInfo> {
        let file_content = fs::read_to_string(path)
            .map_err(|e| ModelSrvError::IoError(e))?;
            
        println!("File content: {}", file_content);

        // Determine format based on file extension
        let template = match path.extension().and_then(|ext| ext.to_str()) {
            Some("yaml") | Some("yml") => {
                debug!("Parsing as YAML");
                serde_yaml::from_str(&file_content)
                    .map_err(|e| {
                        error!("YAML parsing error: {}", e);
                        ModelSrvError::YamlError(e)
                    })?
            },
            Some("json") => {
                debug!("Parsing as JSON");
                serde_json::from_str(&file_content)
                    .map_err(|e| {
                        error!("JSON parsing error: {}", e);
                        ModelSrvError::JsonError(e)
                    })?
            },
            _ => {
                // Default to YAML format
                debug!("Parsing as default YAML");
                serde_yaml::from_str(&file_content)
                    .map_err(|e| {
                        error!("YAML parsing error: {}", e);
                        ModelSrvError::YamlError(e)
                    })?
            }
        };

        Ok(template)
    }
    
    /// Create a new instance from a template
    pub fn create_instance<T: DataStore>(
        &mut self,
        store: &T,
        template_id: &str,
        instance_id: &str,
        instance_name: Option<&str>,
    ) -> Result<()> {
        // Find template
        let templates = self.list_templates()?;
        let template = templates
            .iter()
            .find(|t| t.id == template_id)
            .ok_or_else(|| ModelSrvError::TemplateError(format!("Template not found: {}", template_id)))?;
            
        // Read template file
        let template_path = Path::new(&template.file_path);
        let template_content = fs::read_to_string(template_path)
            .map_err(|e| ModelSrvError::IoError(e))?;
            
        // Create instance configuration
        let instance_key = format!("{}model:config:{}", self.key_prefix, instance_id);
        
        // Check if instance already exists
        if store.exists(&instance_key)? {
            return Err(ModelSrvError::TemplateError(format!(
                "Instance already exists: {}",
                instance_id
            )));
        }
        
        // Replace variables in template
        let mut instance_content = template_content.replace("{{id}}", instance_id);
        
        if let Some(name) = instance_name {
            instance_content = instance_content.replace("{{name}}", name);
        } else {
            instance_content = instance_content.replace("{{name}}", &format!("Instance of {}", template.name));
        }
        
        // Parse the JSON content
        let model_with_actions: serde_json::Value = match template_path.extension().and_then(|ext| ext.to_str()) {
            Some("yaml") | Some("yml") => {
                debug!("Parsing YAML template: {}", template_path.display());
                serde_yaml::from_str(&instance_content)
                    .map_err(|e| ModelSrvError::YamlError(e))?
            },
            _ => {
                debug!("Parsing JSON template: {}", template_path.display());
                serde_json::from_str(&instance_content)
                    .map_err(|e| ModelSrvError::JsonError(e))?
            }
        };
        
        // 创建哈希表存储实例配置
        let mut instance_hash = HashMap::new();
        instance_hash.insert("id".to_string(), instance_id.to_string());
        instance_hash.insert("template_id".to_string(), template_id.to_string());
        
        // 添加名称和描述
        if let Some(name) = instance_name {
            instance_hash.insert("name".to_string(), name.to_string());
        } else {
            instance_hash.insert("name".to_string(), format!("Instance of {}", template.name));
        }
        
        instance_hash.insert("description".to_string(), template.description.clone());
        
        // 存储模型定义
        if let Some(model) = model_with_actions.get("model") {
            // 存储模型字段
            let model_key = format!("{}model:definition:{}", self.key_prefix, instance_id);
            
            // 将模型存储为JSON字符串
            let model_json = serde_json::to_string(model)
                .map_err(|e| ModelSrvError::JsonError(e))?;
            store.set_string(&model_key, &model_json)?;
            
            // 在配置键中存储对模型定义的引用
            instance_hash.insert("model_key".to_string(), model_key);
            
            // 从模型中提取关键字段添加到哈希表
            if let Some(input_mappings) = model.get("input_mappings") {
                let input_mappings_json = serde_json::to_string(input_mappings)
                    .map_err(|e| ModelSrvError::JsonError(e))?;
                instance_hash.insert("input_mappings".to_string(), input_mappings_json);
            }
            
            if let Some(output_key) = model.get("output_key") {
                if let Some(output_key_str) = output_key.as_str() {
                    instance_hash.insert("output_key".to_string(), output_key_str.to_string());
                }
            }
            
            if let Some(calculation) = model.get("calculation") {
                let calculation_json = serde_json::to_string(calculation)
                    .map_err(|e| ModelSrvError::JsonError(e))?;
                instance_hash.insert("calculation".to_string(), calculation_json);
            }
        }
        
        // 存储哈希表
        store.set_hash(&instance_key, &instance_hash)?;
        
        // 单独存储动作
        if let Some(actions) = model_with_actions.get("actions") {
            if let Some(actions_array) = actions.as_array() {
                for (i, action) in actions_array.iter().enumerate() {
                    let action_key = format!("{}model:action:{}:{}", self.key_prefix, instance_id, i);
                    let action_json = serde_json::to_string(action)
                        .map_err(|e| ModelSrvError::JsonError(e))?;
                    store.set_string(&action_key, &action_json)?;
                }
                
                // 在配置中存储动作计数
                store.set_hash_field(&instance_key, "action_count", &actions_array.len().to_string())?;
            }
        }
        
        // 同时保存到文件用于调试
        let instances_dir = Path::new("instances");
        if !instances_dir.exists() {
            fs::create_dir(instances_dir).map_err(|e| ModelSrvError::IoError(e))?;
        }
        
        let instance_file_path = instances_dir.join(format!("{}.json", instance_id));
        fs::write(&instance_file_path, &instance_content)
            .map_err(|e| ModelSrvError::IoError(e))?;
        
        println!("Instance also saved to file: {:?}", instance_file_path);
        
        Ok(())
    }
    
    /// Batch create template instances
    pub fn create_instances<T: DataStore>(
        &mut self,
        store: &T,
        template_id: &str,
        count: usize,
        prefix: &str,
        start_index: usize,
    ) -> Result<Vec<String>> {
        let mut instance_ids = Vec::new();
        
        for i in start_index..(start_index + count) {
            let instance_id = format!("{}_{}", prefix, i);
            let instance_name = format!("{} #{}", template_id, i);
            
            self.create_instance(store, template_id, &instance_id, Some(&instance_name))?;
            
            instance_ids.push(instance_id);
        }
        
        info!("Created {} instances from template {}", count, template_id);
        
        Ok(instance_ids)
    }
    
    /// Instantiate template
    fn instantiate_template(
        &self,
        template: &ModelWithActions,
        instance_id: &str,
        instance_name: Option<&str>,
    ) -> Result<ModelWithActions> {
        // Create a copy of the model definition
        let mut model = template.model.clone();
        
        // Modify ID
        model.id = instance_id.to_string();
        
        // Modify name
        if let Some(name) = instance_name {
            model.name = name.to_string();
        } else {
            model.name = format!("{} ({})", model.name, instance_id);
        }
        
        // Modify data source keys
        let source_key = format!("{}data:{}", self.key_prefix, instance_id);
        for mapping in &mut model.input_mappings {
            mapping.source_key = source_key.clone();
        }
        
        // Modify output key
        model.output_key = format!("{}model:output:{}", self.key_prefix, instance_id);
        
        // Create a copy of control actions
        let mut actions = Vec::new();
        
        for action in &template.actions {
            let mut new_action = action.clone();
            
            // Modify control channel
            if new_action.channel.contains("Control") {
                new_action.channel = format!("{}_Control", instance_id);
            }
            
            actions.push(new_action);
        }
        
        Ok(ModelWithActions {
            model,
            actions,
        })
    }
    
    /// Save instance to store
    fn save_instance_to_store<T: DataStore>(
        &self,
        store: &T,
        instance: &ModelWithActions,
        instance_id: &str,
    ) -> Result<()> {
        let key = format!("{}model:config:{}", self.key_prefix, instance_id);
        
        // Convert to YAML
        let yaml = serde_yaml::to_string(instance)
            .map_err(|e| ModelSrvError::YamlError(e))?;
            
        // Save to store
        store.set_string(&key, &yaml)?;
        
        Ok(())
    }
} 