//! 极简模型实例管理模块
//!
//! 提供模型实例的创建、管理和映射功能

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tracing::info;
use voltage_libs::redis::EdgeRedis;

use crate::error::{ModelSrvError, Result};
use crate::template::{Template, TemplateManager};

/// 模型映射配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMapping {
    /// 通道ID
    pub channel: u32,
    /// 数据点映射 (点名 -> point_id)
    pub data: HashMap<String, u32>,
    /// 操作映射 (操作名 -> point_id)
    pub action: HashMap<String, u32>,
}

/// 模型实例定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInstance {
    /// 模型ID
    pub id: String,
    /// 模型名称
    pub name: String,
    /// 来源模板ID（可选）
    pub template: Option<String>,
    /// 映射配置
    pub mapping: ModelMapping,
}

/// 模型管理器
pub struct ModelManager {
    /// 模型实例存储
    models: Arc<RwLock<HashMap<String, Arc<ModelInstance>>>>,
    /// Redis连接
    redis: Arc<Mutex<EdgeRedis>>,
    /// 模板管理器
    template_manager: Arc<Mutex<TemplateManager>>,
}

impl ModelManager {
    /// 创建模型管理器
    pub async fn new(redis_url: &str, template_dir: &str) -> Result<Self> {
        let edge_redis = EdgeRedis::new(redis_url)
            .await
            .map_err(|e| ModelSrvError::redis(format!("Redis connection failed: {}", e)))?;

        let mut template_manager = TemplateManager::new(template_dir);
        template_manager.load_all_templates().await?;

        Ok(Self {
            models: Arc::new(RwLock::new(HashMap::new())),
            redis: Arc::new(Mutex::new(edge_redis)),
            template_manager: Arc::new(Mutex::new(template_manager)),
        })
    }

    /// 从模板创建模型实例
    pub async fn create_model_from_template(
        &self,
        template_id: &str,
        model_id: &str,
        model_name: &str,
        mapping: ModelMapping,
    ) -> Result<()> {
        // 验证模板存在
        let template_manager = self.template_manager.lock().await;
        let template = template_manager.get_template(template_id).ok_or_else(|| {
            ModelSrvError::NotFound(format!("Template {} not found", template_id))
        })?;

        // 验证映射完整性
        self.validate_mapping(&template, &mapping)?;
        drop(template_manager);

        // 创建模型实例
        let model = Arc::new(ModelInstance {
            id: model_id.to_string(),
            name: model_name.to_string(),
            template: Some(template_id.to_string()),
            mapping,
        });

        // 存储模型
        let mut models = self.models.write().await;
        models.insert(model.id.clone(), model.clone());

        info!("Created model {} from template {}", model_id, template_id);

        // 加载映射到Redis
        self.load_model_mapping(&model).await?;

        Ok(())
    }

    /// 创建独立模型（不基于模板）
    pub async fn create_model(
        &self,
        model_id: &str,
        model_name: &str,
        mapping: ModelMapping,
    ) -> Result<()> {
        let model = Arc::new(ModelInstance {
            id: model_id.to_string(),
            name: model_name.to_string(),
            template: None,
            mapping,
        });

        let mut models = self.models.write().await;
        models.insert(model.id.clone(), model.clone());

        info!("Created standalone model {}", model_id);

        // 加载映射到Redis
        self.load_model_mapping(&model).await?;

        Ok(())
    }

    /// 验证映射配置
    fn validate_mapping(&self, template: &Template, mapping: &ModelMapping) -> Result<()> {
        // 检查数据点映射
        for data_key in template.data.keys() {
            if !mapping.data.contains_key(data_key) {
                return Err(ModelSrvError::InvalidMapping(format!(
                    "Missing mapping for data point: {}",
                    data_key
                )));
            }
        }

        // 检查操作映射
        for action_key in template.action.keys() {
            if !mapping.action.contains_key(action_key) {
                return Err(ModelSrvError::InvalidMapping(format!(
                    "Missing mapping for action: {}",
                    action_key
                )));
            }
        }

        Ok(())
    }

    /// 加载模型映射到Redis
    async fn load_model_mapping(&self, model: &ModelInstance) -> Result<()> {
        let mut redis = self.redis.lock().await;

        // 加载数据点映射 (C2M: channel:point -> model:data_name)
        for (data_name, point_id) in &model.mapping.data {
            let key = format!("{}:{}", model.mapping.channel, point_id);
            let value = format!("{}:{}", model.id, data_name);
            redis.init_mapping("c2m", &key, &value).await?;
        }

        // 加载操作映射 (M2C: model:action_name -> channel:point)
        for (action_name, point_id) in &model.mapping.action {
            let key = format!("{}:{}", model.id, action_name);
            let value = format!("{}:{}", model.mapping.channel, point_id);
            redis.init_mapping("m2c", &key, &value).await?;
        }

        Ok(())
    }

    /// 获取模型实例
    pub async fn get_model(&self, id: &str) -> Option<Arc<ModelInstance>> {
        let models = self.models.read().await;
        models.get(id).cloned()
    }

    /// 列出所有模型
    pub async fn list_models(&self) -> Vec<Arc<ModelInstance>> {
        let models = self.models.read().await;
        models.values().cloned().collect()
    }

    /// 获取模型数据值
    pub async fn get_model_data(&self, model_id: &str) -> Result<HashMap<String, f64>> {
        let models = self.models.read().await;
        let model = models
            .get(model_id)
            .ok_or_else(|| ModelSrvError::NotFound(format!("Model {} not found", model_id)))?;

        let mut redis = self.redis.lock().await;
        let mut result = HashMap::new();

        // 使用新的映射系统获取数据
        let channel_key = format!("comsrv:{}:T", model.mapping.channel);

        // 遍历数据映射，获取每个数据点的值
        for (data_name, point_id) in &model.mapping.data {
            // 从Redis获取值
            if let Ok(Some(value_str)) = redis
                .hget::<_, _, String>(&channel_key, &point_id.to_string())
                .await
            {
                if let Ok(parsed) = value_str.parse::<f64>() {
                    result.insert(data_name.clone(), parsed);
                }
            }
        }

        Ok(result)
    }

    /// 执行模型操作
    pub async fn execute_action(
        &self,
        model_id: &str,
        action_name: &str,
        value: Option<f64>,
    ) -> Result<()> {
        let models = self.models.read().await;
        let model = models
            .get(model_id)
            .ok_or_else(|| ModelSrvError::NotFound(format!("Model {} not found", model_id)))?;

        let point_id = model.mapping.action.get(action_name).ok_or_else(|| {
            ModelSrvError::InvalidCommand(format!(
                "Action {} not found in model {}",
                action_name, model_id
            ))
        })?;

        // 复制需要的值
        let channel = model.mapping.channel;
        let point_id = *point_id;

        drop(models);

        // 使用新的映射系统发送控制命令
        let mut redis = self.redis.lock().await;
        let control_key = format!("comsrv:{}:C", channel);
        let value_str = format!("{:.6}", value.unwrap_or(1.0));

        redis
            .hset(&control_key, &point_id.to_string(), &value_str)
            .await
            .map_err(|e| ModelSrvError::redis(format!("Failed to execute action: {}", e)))?;

        info!(
            "Executed action {}.{} with value {:?}",
            model_id, action_name, value
        );
        Ok(())
    }

    /// 删除模型
    pub async fn delete_model(&self, model_id: &str) -> Result<()> {
        let mut models = self.models.write().await;
        if let Some(model) = models.remove(model_id) {
            info!("Deleted model: {} ({})", model.name, model.id);

            // TODO: 清理Redis中的映射
        }
        Ok(())
    }

    /// 获取模板管理器
    pub fn template_manager(&self) -> Arc<Mutex<TemplateManager>> {
        self.template_manager.clone()
    }

    /// 列出所有模板
    pub async fn list_templates(&self) -> Vec<Template> {
        let template_manager = self.template_manager.lock().await;
        template_manager
            .list_templates()
            .into_iter()
            .cloned()
            .collect()
    }

    /// 获取模板
    pub async fn get_template(&self, template_id: &str) -> Option<Template> {
        let template_manager = self.template_manager.lock().await;
        template_manager.get_template(template_id).cloned()
    }
}

impl Clone for ModelManager {
    fn clone(&self) -> Self {
        Self {
            models: self.models.clone(),
            redis: self.redis.clone(),
            template_manager: self.template_manager.clone(),
        }
    }
}
