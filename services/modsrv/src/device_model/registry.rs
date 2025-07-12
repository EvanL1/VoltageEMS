//! 设备模型注册表
//!
//! 提供设备模型的注册、查询和管理功能

use super::*;
use crate::error::{ModelSrvError, Result};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 模型注册表
pub struct ModelRegistry {
    /// 模型存储 - key: model_id
    models: Arc<RwLock<HashMap<String, DeviceModel>>>,
    /// 模型索引 - key: device_type
    type_index: Arc<RwLock<HashMap<String, Vec<String>>>>,
    /// 版本索引 - key: model_id, value: versions
    version_index: Arc<RwLock<HashMap<String, Vec<String>>>>,
}

impl ModelRegistry {
    /// 创建新的注册表
    pub fn new() -> Self {
        Self {
            models: Arc::new(RwLock::new(HashMap::new())),
            type_index: Arc::new(RwLock::new(HashMap::new())),
            version_index: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 注册设备模型
    pub async fn register_model(&self, model: DeviceModel) -> Result<()> {
        // 验证模型
        model
            .validate()
            .map_err(|e| ModelSrvError::InvalidModel(e))?;

        let model_id = model.id.clone();
        let device_type = format!("{:?}", model.device_type);

        // 检查是否已存在
        if self.models.read().await.contains_key(&model_id) {
            return Err(ModelSrvError::ModelAlreadyExists(model_id));
        }

        // 保存模型
        self.models
            .write()
            .await
            .insert(model_id.clone(), model.clone());

        // 更新类型索引
        self.type_index
            .write()
            .await
            .entry(device_type)
            .or_insert_with(Vec::new)
            .push(model_id.clone());

        // 更新版本索引
        let base_id = model_id.split('@').next().unwrap_or(&model_id).to_string();
        self.version_index
            .write()
            .await
            .entry(base_id)
            .or_insert_with(Vec::new)
            .push(model.version.clone());

        tracing::info!("Registered device model: {} v{}", model_id, model.version);

        Ok(())
    }

    /// 获取设备模型
    pub async fn get_model(&self, model_id: &str) -> Option<DeviceModel> {
        self.models.read().await.get(model_id).cloned()
    }

    /// 更新设备模型
    pub async fn update_model(&self, model: DeviceModel) -> Result<()> {
        // 验证模型
        model
            .validate()
            .map_err(|e| ModelSrvError::InvalidModel(e))?;

        let model_id = model.id.clone();

        // 检查是否存在
        if !self.models.read().await.contains_key(&model_id) {
            return Err(ModelSrvError::ModelNotFound(model_id));
        }

        // 更新模型
        self.models.write().await.insert(model_id.clone(), model);

        tracing::info!("Updated device model: {}", model_id);

        Ok(())
    }

    /// 删除设备模型
    pub async fn unregister_model(&self, model_id: &str) -> Result<()> {
        let model = self
            .models
            .write()
            .await
            .remove(model_id)
            .ok_or_else(|| ModelSrvError::ModelNotFound(model_id.to_string()))?;

        let device_type = format!("{:?}", model.device_type);

        // 从类型索引中移除
        if let Some(models) = self.type_index.write().await.get_mut(&device_type) {
            models.retain(|id| id != model_id);
        }

        // 从版本索引中移除
        let base_id = model_id.split('@').next().unwrap_or(model_id);
        if let Some(versions) = self.version_index.write().await.get_mut(base_id) {
            versions.retain(|v| v != &model.version);
        }

        tracing::info!("Unregistered device model: {}", model_id);

        Ok(())
    }

    /// 按设备类型查询模型
    pub async fn find_by_type(&self, device_type: &DeviceType) -> Vec<DeviceModel> {
        let type_key = format!("{:?}", device_type);
        let models = self.models.read().await;

        if let Some(model_ids) = self.type_index.read().await.get(&type_key) {
            model_ids
                .iter()
                .filter_map(|id| models.get(id).cloned())
                .collect()
        } else {
            Vec::new()
        }
    }

    /// 列出所有模型
    pub async fn list_models(&self) -> Vec<DeviceModel> {
        self.models.read().await.values().cloned().collect()
    }

    /// 获取模型的所有版本
    pub async fn get_model_versions(&self, base_model_id: &str) -> Vec<String> {
        self.version_index
            .read()
            .await
            .get(base_model_id)
            .cloned()
            .unwrap_or_default()
    }

    /// 从JSON加载模型
    pub async fn load_from_json(&self, json_str: &str) -> Result<()> {
        let model: DeviceModel =
            serde_json::from_str(json_str).map_err(|e| ModelSrvError::JsonError(e.to_string()))?;

        self.register_model(model).await
    }

    /// 导出模型为JSON
    pub async fn export_to_json(&self, model_id: &str) -> Result<String> {
        let model = self
            .get_model(model_id)
            .await
            .ok_or_else(|| ModelSrvError::ModelNotFound(model_id.to_string()))?;

        serde_json::to_string_pretty(&model).map_err(|e| ModelSrvError::JsonError(e.to_string()))
    }

    /// 批量注册模型
    pub async fn register_models_batch(&self, models: Vec<DeviceModel>) -> Result<()> {
        for model in models {
            if let Err(e) = self.register_model(model).await {
                tracing::error!("Failed to register model: {}", e);
            }
        }
        Ok(())
    }
}

/// 模型存储接口（用于持久化）
#[async_trait::async_trait]
pub trait ModelStore: Send + Sync {
    /// 保存模型
    async fn save_model(&self, model: &DeviceModel) -> Result<()>;

    /// 加载模型
    async fn load_model(&self, model_id: &str) -> Result<Option<DeviceModel>>;

    /// 列出所有模型ID
    async fn list_model_ids(&self) -> Result<Vec<String>>;

    /// 删除模型
    async fn delete_model(&self, model_id: &str) -> Result<()>;
}

/// Redis模型存储实现
pub struct RedisModelStore {
    storage: Arc<RwLock<crate::storage::ModelStorage>>,
    key_prefix: String,
}

impl RedisModelStore {
    pub async fn new(redis_url: &str, key_prefix: &str) -> Result<Self> {
        let storage = crate::storage::ModelStorage::new(redis_url).await?;
        Ok(Self {
            storage: Arc::new(RwLock::new(storage)),
            key_prefix: key_prefix.to_string(),
        })
    }

    fn make_key(&self, model_id: &str) -> String {
        format!("{}model:definition:{}", self.key_prefix, model_id)
    }
}

#[async_trait::async_trait]
impl ModelStore for RedisModelStore {
    async fn save_model(&self, model: &DeviceModel) -> Result<()> {
        let key = self.make_key(&model.id);
        let json =
            serde_json::to_string(model).map_err(|e| ModelSrvError::JsonError(e.to_string()))?;

        let mut storage = self.storage.write().await;
        storage
            .set_model_output_json(&key, &serde_json::json!(json))
            .await
    }

    async fn load_model(&self, model_id: &str) -> Result<Option<DeviceModel>> {
        let key = self.make_key(model_id);
        let mut storage = self.storage.write().await;

        match storage.get_model_output(&key).await? {
            Some(output) => {
                if let Some(json_str) = output.outputs.get("value").and_then(|v| v.as_str()) {
                    let model = serde_json::from_str(json_str)
                        .map_err(|e| ModelSrvError::JsonError(e.to_string()))?;
                    Ok(Some(model))
                } else {
                    Ok(None)
                }
            }
            None => Ok(None),
        }
    }

    async fn list_model_ids(&self) -> Result<Vec<String>> {
        let pattern = format!("{}model:definition:*", self.key_prefix);
        let mut storage = self.storage.write().await;
        let configs = storage.get_model_configs(&pattern).await?;

        let prefix_len = format!("{}model:definition:", self.key_prefix).len();
        let ids: Vec<String> = configs
            .keys()
            .map(|k| k[prefix_len..].to_string())
            .collect();

        Ok(ids)
    }

    async fn delete_model(&self, model_id: &str) -> Result<()> {
        // TODO: 实现Redis删除
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_model_registry() {
        let registry = ModelRegistry::new();

        let model = DeviceModel {
            id: "power_meter_v1".to_string(),
            name: "Power Meter".to_string(),
            version: "1.0.0".to_string(),
            description: "Smart power meter".to_string(),
            device_type: DeviceType::Energy,
            properties: vec![],
            telemetry: vec![],
            commands: vec![],
            events: vec![],
            calculations: vec![],
            metadata: HashMap::new(),
        };

        // 注册模型
        registry.register_model(model.clone()).await.unwrap();

        // 获取模型
        let retrieved = registry.get_model("power_meter_v1").await.unwrap();
        assert_eq!(retrieved.name, "Power Meter");

        // 按类型查询
        let energy_models = registry.find_by_type(&DeviceType::Energy).await;
        assert_eq!(energy_models.len(), 1);

        // 删除模型
        registry.unregister_model("power_meter_v1").await.unwrap();
        assert!(registry.get_model("power_meter_v1").await.is_none());
    }
}
