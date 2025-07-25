//! ModSrv核心模型系统 v2.0
//!
//! 简化的二分模型设计，只包含监视（Monitoring）和控制（Control）两个概念

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, error, info, warn};
use voltage_libs::redis::RedisClient;
use voltage_libs::types::StandardFloat;

use crate::error::{ModelSrvError, Result};
use crate::mapping::{MappingManager, PointMapping};
use crate::websocket::WsConnectionManager;

/// 点位配置（静态元数据）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointConfig {
    /// 点位描述
    pub description: String,
    /// 单位（可选）
    pub unit: Option<String>,
}

/// 点位值（动态数据）
#[derive(Debug, Clone)]
pub struct PointValue {
    /// 数值（6位小数精度）
    pub value: StandardFloat,
    /// 更新时间戳
    pub timestamp: i64,
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

/// 核心模型
#[derive(Debug, Clone)]
pub struct Model {
    /// 模型ID
    pub id: String,
    /// 模型名称
    pub name: String,
    /// 模型描述
    pub description: String,
    /// 监视点配置（静态）
    pub monitoring_config: HashMap<String, PointConfig>,
    /// 控制点配置（静态）
    pub control_config: HashMap<String, PointConfig>,
    /// 监视点数值（动态）
    pub monitoring_values: Arc<RwLock<HashMap<String, PointValue>>>,
    /// 控制点状态（动态）
    pub control_states: Arc<RwLock<HashMap<String, PointValue>>>,
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
            monitoring_values: Arc::new(RwLock::new(HashMap::new())),
            control_states: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 获取监视点配置
    pub fn get_monitoring_config(&self, name: &str) -> Option<&PointConfig> {
        self.monitoring_config.get(name)
    }

    /// 获取控制点配置
    pub fn get_control_config(&self, name: &str) -> Option<&PointConfig> {
        self.control_config.get(name)
    }
}

/// 模型管理器
pub struct ModelManager {
    /// 模型存储
    models: Arc<RwLock<HashMap<String, Model>>>,
    /// 映射管理器
    mappings: Arc<RwLock<MappingManager>>,
    /// Redis客户端
    redis_client: Arc<Mutex<RedisClient>>,
    /// WebSocket管理器（可选）
    ws_manager: Option<Arc<WsConnectionManager>>,
}

impl ModelManager {
    /// 创建模型管理器
    pub fn new(redis_client: Arc<Mutex<RedisClient>>) -> Self {
        Self {
            models: Arc::new(RwLock::new(HashMap::new())),
            mappings: Arc::new(RwLock::new(MappingManager::new())),
            redis_client,
            ws_manager: None,
        }
    }

    /// 设置WebSocket管理器
    pub fn set_ws_manager(&mut self, ws_manager: Arc<WsConnectionManager>) {
        self.ws_manager = Some(ws_manager);
    }

    /// 加载模型配置
    pub async fn load_models(&self, configs: Vec<ModelConfig>) -> Result<()> {
        let mut models = self.models.write().await;

        for config in configs {
            let model = Model::from_config(config);
            info!("加载模型: {} ({})", model.name, model.id);
            models.insert(model.id.clone(), model);
        }

        info!("已加载 {} 个模型", models.len());
        Ok(())
    }

    /// 加载映射配置
    pub async fn load_mappings_directory(&self, dir: &str) -> Result<()> {
        let mut mappings = self.mappings.write().await;
        mappings.load_directory(dir).await?;
        Ok(())
    }

    /// 获取模型
    pub async fn get_model(&self, id: &str) -> Option<Model> {
        let models = self.models.read().await;
        models.get(id).cloned()
    }

    /// 列出所有模型
    pub async fn list_models(&self) -> Vec<Model> {
        let models = self.models.read().await;
        models.values().cloned().collect()
    }

    /// 更新监视点值（内部使用）
    pub async fn update_monitoring_value(
        &self,
        model_id: &str,
        point_name: &str,
        value: StandardFloat,
    ) -> Result<()> {
        let models = self.models.read().await;

        if let Some(model) = models.get(model_id) {
            let mut values = model.monitoring_values.write().await;
            values.insert(
                point_name.to_string(),
                PointValue {
                    value,
                    timestamp: chrono::Utc::now().timestamp(),
                },
            );

            debug!("更新监视值 {}:{} = {}", model_id, point_name, value);

            // 触发WebSocket推送
            if let Some(ws_manager) = &self.ws_manager {
                let updates = HashMap::from([(point_name.to_string(), value)]);
                ws_manager.broadcast_update(model_id, updates).await;
            }

            Ok(())
        } else {
            Err(ModelSrvError::not_found(format!(
                "模型不存在: {}",
                model_id
            )))
        }
    }

    /// 获取单个监视点值
    pub async fn get_monitoring_value(
        &self,
        model_id: &str,
        point_name: &str,
    ) -> Option<StandardFloat> {
        let models = self.models.read().await;

        if let Some(model) = models.get(model_id) {
            let values = model.monitoring_values.read().await;
            values.get(point_name).map(|pv| pv.value)
        } else {
            None
        }
    }

    /// 获取所有监视点值
    pub async fn get_all_monitoring_values(
        &self,
        model_id: &str,
    ) -> Option<HashMap<String, StandardFloat>> {
        let models = self.models.read().await;

        if let Some(model) = models.get(model_id) {
            let values = model.monitoring_values.read().await;
            Some(values.iter().map(|(k, v)| (k.clone(), v.value)).collect())
        } else {
            None
        }
    }

    /// 执行控制命令
    pub async fn execute_control(
        &self,
        model_id: &str,
        control_name: &str,
        value: StandardFloat,
    ) -> Result<()> {
        // 验证控制点存在
        let models = self.models.read().await;
        let model = models
            .get(model_id)
            .ok_or_else(|| ModelSrvError::not_found(format!("模型不存在: {}", model_id)))?;

        if !model.control_config.contains_key(control_name) {
            return Err(ModelSrvError::not_found(format!(
                "控制点不存在: {}:{}",
                model_id, control_name
            )));
        }

        // 查找映射
        let mappings = self.mappings.read().await;
        let mapping = mappings
            .get_control_mapping(model_id, control_name)
            .ok_or_else(|| {
                ModelSrvError::not_found(format!("控制点映射不存在: {}:{}", model_id, control_name))
            })?;

        // 构建Redis控制通道
        let channel = match mapping.point_type.as_str() {
            "c" => format!("cmd:{}:control", mapping.channel),
            "a" => format!("cmd:{}:adjustment", mapping.channel),
            _ => {
                return Err(ModelSrvError::invalid_data(format!(
                    "未知的控制类型: {}",
                    mapping.point_type
                )))
            }
        };

        let command = format!("{}:{:.6}", mapping.point, value.value());

        // 发布到Redis
        let mut redis_client = self.redis_client.lock().await;
        redis_client
            .publish(&channel, &command)
            .await
            .map_err(|e| ModelSrvError::redis(format!("发送控制命令失败: {}", e)))?;

        // 更新控制状态
        let mut states = model.control_states.write().await;
        states.insert(
            control_name.to_string(),
            PointValue {
                value,
                timestamp: chrono::Utc::now().timestamp(),
            },
        );

        info!("执行控制命令 {}:{} = {}", model_id, control_name, value);
        Ok(())
    }

    /// 订阅Redis数据更新
    pub async fn subscribe_data_updates(&self) -> Result<()> {
        let models = self.models.read().await;
        let mappings = self.mappings.read().await;

        // 收集所有需要订阅的通道
        let mut channels = std::collections::HashSet::new();

        for model in models.values() {
            if let Some(model_mappings) = mappings.get_all_monitoring_mappings(&model.id) {
                for mapping in model_mappings.values() {
                    let channel = format!("comsrv:{}:{}", mapping.channel, mapping.point_type);
                    channels.insert(channel);
                }
            }
        }

        info!("订阅 {} 个数据通道", channels.len());

        // TODO: 实现Redis订阅逻辑
        // 这里需要创建订阅任务，处理接收到的数据更新

        Ok(())
    }

    /// 处理Redis数据更新（内部使用）
    async fn handle_redis_update(&self, channel: &str, message: &str) -> Result<()> {
        // 解析通道格式: comsrv:{channel}:{type}
        let parts: Vec<&str> = channel.split(':').collect();
        if parts.len() != 3 || parts[0] != "comsrv" {
            return Ok(());
        }

        let channel_id: u16 = parts[1]
            .parse()
            .map_err(|_| ModelSrvError::format("无效的通道ID"))?;

        // 解析消息格式: {point_id}:{value}
        let msg_parts: Vec<&str> = message.split(':').collect();
        if msg_parts.len() != 2 {
            return Ok(());
        }

        let point_id: u32 = msg_parts[0]
            .parse()
            .map_err(|_| ModelSrvError::format("无效的点位ID"))?;
        let value: f64 = msg_parts[1]
            .parse()
            .map_err(|_| ModelSrvError::format("无效的数值"))?;

        // 查找对应的模型和点位
        let models = self.models.read().await;
        let mappings = self.mappings.read().await;

        for model in models.values() {
            if let Some(point_name) =
                mappings.find_point_name(&model.id, channel_id, point_id, false)
            {
                // 更新监视值
                self.update_monitoring_value(&model.id, &point_name, StandardFloat::new(value))
                    .await?;
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_creation() {
        let config = ModelConfig {
            id: "test_model".to_string(),
            name: "测试模型".to_string(),
            description: "用于测试的模型".to_string(),
            monitoring: HashMap::from([(
                "voltage".to_string(),
                PointConfig {
                    description: "电压".to_string(),
                    unit: Some("V".to_string()),
                },
            )]),
            control: HashMap::from([(
                "switch".to_string(),
                PointConfig {
                    description: "开关".to_string(),
                    unit: None,
                },
            )]),
        };

        let model = Model::from_config(config);
        assert_eq!(model.id, "test_model");
        assert_eq!(model.monitoring_config.len(), 1);
        assert_eq!(model.control_config.len(), 1);
        assert!(model.get_monitoring_config("voltage").is_some());
        assert!(model.get_control_config("switch").is_some());
    }
}
