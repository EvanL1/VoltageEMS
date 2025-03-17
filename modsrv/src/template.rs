use crate::error::{ModelSrvError, Result};
use crate::model::{ModelDefinition, ControlAction, ModelWithActions, DataMapping};
use crate::redis_handler::RedisConnection;
use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::fs;

/// 模板索引中的模板信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub file: String,
}

/// 模板索引
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateIndex {
    pub templates: Vec<TemplateInfo>,
}

/// 模板管理器
pub struct TemplateManager {
    /// 模板目录路径
    templates_dir: String,
    /// 模板缓存
    templates: HashMap<String, ModelWithActions>,
    /// Redis前缀
    redis_prefix: String,
}

impl TemplateManager {
    /// 创建新的模板管理器
    pub fn new(templates_dir: &str, redis_prefix: &str) -> Self {
        TemplateManager {
            templates_dir: templates_dir.to_string(),
            templates: HashMap::new(),
            redis_prefix: redis_prefix.to_string(),
        }
    }
    
    /// 加载模板索引
    pub fn load_template_index(&self) -> Result<TemplateIndex> {
        let index_path = Path::new(&self.templates_dir).join("index.json");
        
        if !index_path.exists() {
            return Err(ModelSrvError::TemplateError(
                format!("Template index file not found: {}", index_path.display())
            ));
        }
        
        let index_content = fs::read_to_string(index_path)
            .map_err(|e| ModelSrvError::TemplateError(
                format!("Failed to read template index: {}", e)
            ))?;
            
        let index: TemplateIndex = serde_json::from_str(&index_content)
            .map_err(|e| ModelSrvError::TemplateError(
                format!("Failed to parse template index: {}", e)
            ))?;
            
        Ok(index)
    }
    
    /// 加载模板
    pub fn load_template(&mut self, template_id: &str) -> Result<&ModelWithActions> {
        // 如果模板已经加载，直接返回
        if let Some(template) = self.templates.get(template_id) {
            return Ok(template);
        }
        
        // 加载模板索引
        let index = self.load_template_index()?;
        
        // 查找模板信息
        let template_info = index.templates.iter()
            .find(|t| t.id == template_id)
            .ok_or_else(|| ModelSrvError::TemplateError(
                format!("Template not found: {}", template_id)
            ))?;
            
        // 加载模板文件
        let template_path = Path::new(&self.templates_dir).join(&template_info.file);
        
        if !template_path.exists() {
            return Err(ModelSrvError::TemplateError(
                format!("Template file not found: {}", template_path.display())
            ));
        }
        
        let template_content = fs::read_to_string(template_path)
            .map_err(|e| ModelSrvError::TemplateError(
                format!("Failed to read template file: {}", e)
            ))?;
            
        // 解析模板
        let template: ModelWithActions = serde_json::from_str(&template_content)
            .map_err(|e| ModelSrvError::TemplateError(
                format!("Failed to parse template: {}", e)
            ))?;
            
        // 缓存模板
        self.templates.insert(template_id.to_string(), template);
        
        Ok(self.templates.get(template_id).unwrap())
    }
    
    /// 创建模板实例
    pub fn create_instance(
        &mut self,
        redis: &mut RedisConnection,
        template_id: &str,
        instance_id: &str,
        instance_name: Option<&str>,
    ) -> Result<()> {
        // 加载模板
        let template = self.load_template(template_id)?;
        
        // 创建实例
        let instance = self.instantiate_template(template, instance_id, instance_name)?;
        
        // 保存到Redis
        self.save_instance_to_redis(redis, &instance, instance_id)?;
        
        info!("Created instance {} from template {}", instance_id, template_id);
        
        Ok(())
    }
    
    /// 批量创建模板实例
    pub fn create_instances(
        &mut self,
        redis: &mut RedisConnection,
        template_id: &str,
        count: usize,
        prefix: &str,
        start_index: usize,
    ) -> Result<Vec<String>> {
        let mut instance_ids = Vec::new();
        
        for i in start_index..(start_index + count) {
            let instance_id = format!("{}_{}", prefix, i);
            let instance_name = format!("{} #{}", template_id, i);
            
            self.create_instance(redis, template_id, &instance_id, Some(&instance_name))?;
            
            instance_ids.push(instance_id);
        }
        
        info!("Created {} instances from template {}", count, template_id);
        
        Ok(instance_ids)
    }
    
    /// 实例化模板
    fn instantiate_template(
        &self,
        template: &ModelWithActions,
        instance_id: &str,
        instance_name: Option<&str>,
    ) -> Result<ModelWithActions> {
        // 创建模型定义副本
        let mut model = template.model.clone();
        
        // 修改ID
        model.id = instance_id.to_string();
        
        // 修改名称
        if let Some(name) = instance_name {
            model.name = name.to_string();
        } else {
            model.name = format!("{} ({})", model.name, instance_id);
        }
        
        // 修改数据源键
        let source_key = format!("{}data:{}", self.redis_prefix, instance_id);
        for mapping in &mut model.input_mappings {
            mapping.source_key = source_key.clone();
        }
        
        // 修改输出键
        model.output_key = format!("{}model:output:{}", self.redis_prefix, instance_id);
        
        // 创建控制动作副本
        let mut actions = Vec::new();
        
        for action in &template.actions {
            let mut new_action = action.clone();
            
            // 修改控制通道
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
    
    /// 保存实例到Redis
    fn save_instance_to_redis(
        &self,
        redis: &mut RedisConnection,
        instance: &ModelWithActions,
        instance_id: &str,
    ) -> Result<()> {
        // 序列化实例
        let instance_json = serde_json::to_string(instance)
            .map_err(|e| ModelSrvError::TemplateError(
                format!("Failed to serialize instance: {}", e)
            ))?;
            
        // 保存到Redis
        let key = format!("{}model:config:{}", self.redis_prefix, instance_id);
        redis.set_string(&key, &instance_json)?;
        
        Ok(())
    }
    
    /// 获取可用模板列表
    pub fn list_templates(&self) -> Result<Vec<TemplateInfo>> {
        let index = self.load_template_index()?;
        Ok(index.templates)
    }
} 