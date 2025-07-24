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
        model.validate().map_err(ModelSrvError::InvalidModel)?;

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
        model.validate().map_err(ModelSrvError::InvalidModel)?;

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

        // 更新类型索引
        if let Some(models) = self.type_index.write().await.get_mut(&device_type) {
            models.retain(|id| id != model_id);
        }

        // 更新版本索引
        let base_id = model_id.split('@').next().unwrap_or(model_id).to_string();
        if let Some(versions) = self.version_index.write().await.get_mut(&base_id) {
            versions.retain(|v| v != &model.version);
        }

        tracing::info!("Unregistered device model: {}", model_id);

        Ok(())
    }

    /// 按设备类型查询模型
    pub async fn list_models_by_type(&self, device_type: &DeviceType) -> Vec<DeviceModel> {
        let type_str = format!("{:?}", device_type);

        if let Some(model_ids) = self.type_index.read().await.get(&type_str) {
            let models = self.models.read().await;
            model_ids
                .iter()
                .filter_map(|id| models.get(id).cloned())
                .collect()
        } else {
            Vec::new()
        }
    }

    /// 列出所有模型
    pub async fn list_all_models(&self) -> Vec<DeviceModel> {
        self.models.read().await.values().cloned().collect()
    }

    /// 获取模型版本列表
    pub async fn list_model_versions(&self, base_model_id: &str) -> Vec<String> {
        self.version_index
            .read()
            .await
            .get(base_model_id)
            .cloned()
            .unwrap_or_default()
    }

    /// 从文件加载模型
    pub async fn load_model_from_file(&self, path: &std::path::Path) -> Result<DeviceModel> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| ModelSrvError::IoError(format!("Failed to read model file: {}", e)))?;

        let model: DeviceModel = match path.extension().and_then(|s| s.to_str()) {
            Some("json") => serde_json::from_str(&content).map_err(|e| {
                ModelSrvError::JsonError(format!("Failed to parse model JSON: {}", e))
            })?,
            Some("yaml") | Some("yml") => serde_yaml::from_str(&content).map_err(|e| {
                ModelSrvError::YamlError(format!("Failed to parse model YAML: {}", e))
            })?,
            _ => {
                // Default to JSON if extension is unknown
                serde_json::from_str(&content).map_err(|e| {
                    ModelSrvError::JsonError(format!("Failed to parse model JSON (default): {}", e))
                })?
            }
        };

        model.validate().map_err(ModelSrvError::InvalidModel)?;

        Ok(model)
    }

    /// 从目录加载所有模型
    pub async fn load_models_from_directory(&self, dir_path: &std::path::Path) -> Result<()> {
        if !dir_path.is_dir() {
            return Err(ModelSrvError::IoError(format!(
                "Path is not a directory: {}",
                dir_path.display()
            )));
        }

        let entries = std::fs::read_dir(dir_path)
            .map_err(|e| ModelSrvError::IoError(format!("Failed to read directory: {}", e)))?;

        for entry in entries {
            let entry = entry
                .map_err(|e| ModelSrvError::IoError(format!("Failed to read entry: {}", e)))?;
            let path = entry.path();

            if path.is_file() {
                let extension = path.extension().and_then(|s| s.to_str());
                // Support both JSON (preferred) and YAML files
                if matches!(extension, Some("json") | Some("yaml") | Some("yml")) {
                    match self.load_model_from_file(&path).await {
                        Ok(model) => {
                            if let Err(e) = self.register_model(model).await {
                                tracing::warn!("Failed to register model from {:?}: {}", path, e);
                            }
                        }
                        Err(e) => {
                            tracing::warn!("Failed to load model from {:?}: {}", path, e);
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

impl Default for ModelRegistry {
    fn default() -> Self {
        Self::new()
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
            device_type: DeviceType::PowerMeter,
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
        let retrieved = registry.get_model("power_meter_v1").await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "Power Meter");

        // 按类型查询
        let models = registry.list_models_by_type(&DeviceType::PowerMeter).await;
        assert_eq!(models.len(), 1);

        // 删除模型
        registry.unregister_model("power_meter_v1").await.unwrap();
        assert!(registry.get_model("power_meter_v1").await.is_none());
    }
}
