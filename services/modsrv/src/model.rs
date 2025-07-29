//! ModSrv核心模型系统
//!
//! 轻量级模型管理，专注于元数据管理和API服务

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, error, info};
use voltage_libs::redis::EdgeRedis;

use crate::error::{ModelSrvError, Result};
use crate::mapping::{MappingManager, ModelMappingConfig};

/// 点位配置（静态元数据）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointConfig {
    /// 点位描述
    pub description: String,
    /// 单位（可选）
    pub unit: Option<String>,
}

/// 模型配置（用于加载）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    /// 模型ID
    pub id: String,
    /// 模型名称
    pub name: String,
    /// 模型描述
    pub description: String,
    /// 监视点配置
    pub monitoring: HashMap<String, PointConfig>,
    /// 控制点配置
    pub control: HashMap<String, PointConfig>,
}

/// 模型结构（仅保留元数据）
#[derive(Debug, Clone)]
pub struct Model {
    /// 模型ID
    pub id: String,
    /// 模型名称
    pub name: String,
    /// 模型描述
    pub description: String,
    /// 监视点配置
    pub monitoring_config: HashMap<String, PointConfig>,
    /// 控制点配置
    pub control_config: HashMap<String, PointConfig>,
}

impl Model {
    /// 从配置创建模型
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

/// 模型管理器
pub struct ModelManager {
    /// 模型元数据存储
    models: HashMap<String, Model>,
    /// Redis连接
    redis: Arc<Mutex<EdgeRedis>>,
}

impl ModelManager {
    /// 创建模型管理器
    pub async fn new(redis_url: &str) -> Result<Self> {
        let mut edge_redis = EdgeRedis::new(redis_url)
            .await
            .map_err(|e| ModelSrvError::redis(format!("Redis连接失败: {}", e)))?;

        // 初始化Lua脚本
        edge_redis
            .init_scripts()
            .await
            .map_err(|e| ModelSrvError::redis(format!("Lua脚本初始化失败: {}", e)))?;

        Ok(Self {
            models: HashMap::new(),
            redis: Arc::new(Mutex::new(edge_redis)),
        })
    }

    /// 加载模型配置
    pub async fn load_models(&mut self, configs: Vec<ModelConfig>) -> Result<()> {
        for config in configs {
            let model = Model::from_config(config);
            info!("加载模型: {} ({})", model.name, model.id);
            self.models.insert(model.id.clone(), model);
        }

        info!("已加载 {} 个模型", self.models.len());
        Ok(())
    }

    /// 加载映射到Redis
    pub async fn load_mappings(&self, mappings: &MappingManager) -> Result<()> {
        let mut redis = self.redis.lock().await;

        // 清空旧映射
        let cleared = redis.clear_mappings().await?;
        if cleared > 0 {
            info!("清理了 {} 个旧映射", cleared);
        }

        // 加载新映射
        let mut count = 0;
        for (model_id, config) in mappings.get_all_mappings() {
            // 加载监视点映射 (C2M)
            for (point_name, mapping) in &config.monitoring {
                let key = format!("{}:{}", mapping.channel, mapping.point);
                let value = format!("{}:{}", model_id, point_name);
                redis.init_mapping("c2m", &key, &value).await?;
                count += 1;
            }

            // 加载控制点映射 (M2C)
            for (control_name, mapping) in &config.control {
                let key = format!("{}:{}", model_id, control_name);
                let value = format!("{}:{}", mapping.channel, mapping.point);
                redis.init_mapping("m2c", &key, &value).await?;
                count += 1;
            }
        }

        info!("加载了 {} 个映射到Redis", count);
        Ok(())
    }

    /// 获取模型元数据
    pub fn get_model(&self, id: &str) -> Option<&Model> {
        self.models.get(id)
    }

    /// 列出所有模型
    pub fn list_models(&self) -> Vec<&Model> {
        self.models.values().collect()
    }

    /// 获取Redis连接（用于测试）
    pub fn get_redis(&self) -> Arc<Mutex<EdgeRedis>> {
        self.redis.clone()
    }

    /// 获取模型当前值（从Redis读取）
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

    /// 发送控制命令
    pub async fn send_control(&self, model_id: &str, control_name: &str, value: f64) -> Result<()> {
        // 检查控制点是否存在
        let model = self
            .models
            .get(model_id)
            .ok_or_else(|| ModelSrvError::NotFound(format!("模型 {} 不存在", model_id)))?;

        if !model.control_config.contains_key(control_name) {
            return Err(ModelSrvError::InvalidCommand(format!(
                "控制点 {} 不存在",
                control_name
            )));
        }

        // 通过Lua脚本发送控制命令
        let mut redis = self.redis.lock().await;
        redis
            .send_control(model_id, control_name, value)
            .await
            .map_err(|e| ModelSrvError::redis(format!("发送控制命令失败: {}", e)))?;

        info!("发送控制命令: {}.{} = {:.6}", model_id, control_name, value);
        Ok(())
    }
}
