//! 设备模型实例管理
//!
//! 提供设备实例的创建、管理和数据处理

use super::*;
use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// 设备实例
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInstance {
    /// 实例ID
    pub instance_id: String,
    /// 模型ID
    pub model_id: String,
    /// 实例名称
    pub name: String,
    /// 实例描述
    pub description: Option<String>,
    /// 属性值
    pub properties: HashMap<String, serde_json::Value>,
    /// 配置参数
    pub config: HashMap<String, serde_json::Value>,
    /// 实例状态
    pub status: DeviceStatus,
    /// 创建时间
    pub created_at: i64,
    /// 更新时间
    pub updated_at: i64,
    /// 元数据
    pub metadata: HashMap<String, String>,
}

/// 设备实例管理器
pub struct InstanceManager {
    /// 设备模型注册表
    model_registry: Arc<ModelRegistry>,
    /// 实例存储
    instances: Arc<RwLock<HashMap<String, DeviceInstance>>>,
    /// 实例数据缓存
    data_cache: Arc<RwLock<HashMap<String, DeviceData>>>,
}

impl InstanceManager {
    /// 创建实例管理器
    pub fn new(model_registry: Arc<ModelRegistry>) -> Self {
        Self {
            model_registry,
            instances: Arc::new(RwLock::new(HashMap::new())),
            data_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 创建设备实例
    pub async fn create_instance(
        &self,
        model_id: &str,
        instance_id: &str,
        name: &str,
        properties: Option<HashMap<String, serde_json::Value>>,
        config: Option<HashMap<String, serde_json::Value>>,
    ) -> Result<DeviceInstance> {
        // 获取模型定义
        let model = self
            .model_registry
            .get_model(model_id)
            .await
            .ok_or_else(|| crate::error::ModelSrvError::ModelNotFound(model_id.to_string()))?;

        // 初始化属性值
        let mut instance_properties = HashMap::new();
        for prop_def in &model.properties {
            let value = if let Some(props) = &properties {
                props.get(&prop_def.identifier).cloned()
            } else {
                None
            };

            let final_value = value
                .or_else(|| prop_def.default_value.clone())
                .unwrap_or_else(|| prop_def.data_type.default_value());

            // 验证数据类型
            if !prop_def.data_type.validate_value(&final_value) {
                return Err(crate::error::ModelSrvError::InvalidValue(format!(
                    "Invalid value for property '{}'",
                    prop_def.identifier
                )));
            }

            instance_properties.insert(prop_def.identifier.clone(), final_value);
        }

        let now = chrono::Utc::now().timestamp_millis();
        let instance = DeviceInstance {
            instance_id: instance_id.to_string(),
            model_id: model_id.to_string(),
            name: name.to_string(),
            description: None,
            properties: instance_properties,
            config: config.unwrap_or_default(),
            status: DeviceStatus::Online,
            created_at: now,
            updated_at: now,
            metadata: HashMap::new(),
        };

        // 保存实例
        self.instances
            .write()
            .await
            .insert(instance_id.to_string(), instance.clone());

        // 初始化数据缓存
        let device_data = DeviceData {
            instance_id: instance_id.to_string(),
            timestamp: now,
            properties: HashMap::new(),
            telemetry: HashMap::new(),
            status: DeviceStatus::Online,
        };
        self.data_cache
            .write()
            .await
            .insert(instance_id.to_string(), device_data);

        tracing::info!(
            "Created device instance: {} (model: {})",
            instance_id,
            model_id
        );

        Ok(instance)
    }

    /// 获取设备实例
    pub async fn get_instance(&self, instance_id: &str) -> Option<DeviceInstance> {
        self.instances.read().await.get(instance_id).cloned()
    }

    /// 更新设备实例属性
    pub async fn update_instance_properties(
        &self,
        instance_id: &str,
        properties: HashMap<String, serde_json::Value>,
    ) -> Result<()> {
        let mut instances = self.instances.write().await;
        let instance = instances.get_mut(instance_id).ok_or_else(|| {
            crate::error::ModelSrvError::InstanceNotFound(instance_id.to_string())
        })?;

        // 获取模型定义进行验证
        let model = self
            .model_registry
            .get_model(&instance.model_id)
            .await
            .ok_or_else(|| crate::error::ModelSrvError::ModelNotFound(instance.model_id.clone()))?;

        // 验证并更新属性
        for (key, value) in properties {
            if let Some(prop_def) = model.properties.iter().find(|p| p.identifier == key) {
                if !prop_def.data_type.validate_value(&value) {
                    return Err(crate::error::ModelSrvError::InvalidValue(format!(
                        "Invalid value for property '{}'",
                        key
                    )));
                }
                instance.properties.insert(key, value);
            }
        }

        instance.updated_at = chrono::Utc::now().timestamp_millis();

        Ok(())
    }

    /// 更新设备遥测数据
    pub async fn update_telemetry(
        &self,
        instance_id: &str,
        telemetry_id: &str,
        value: f64,
        raw_value: Option<f64>,
    ) -> Result<()> {
        let mut data_cache = self.data_cache.write().await;
        let device_data = data_cache.get_mut(instance_id).ok_or_else(|| {
            crate::error::ModelSrvError::InstanceNotFound(instance_id.to_string())
        })?;

        let telemetry_value = TelemetryValue {
            value: serde_json::Value::Number(serde_json::Number::from_f64(value).unwrap()),
            timestamp: chrono::Utc::now().timestamp_millis(),
            quality: DataQuality::Good,
            raw_value,
        };

        device_data
            .telemetry
            .insert(telemetry_id.to_string(), telemetry_value);
        device_data.timestamp = chrono::Utc::now().timestamp_millis();

        Ok(())
    }

    /// 执行设备命令
    pub async fn execute_command(
        &self,
        instance_id: &str,
        command_id: &str,
        params: HashMap<String, serde_json::Value>,
    ) -> Result<CommandResponse> {
        let instance = self.get_instance(instance_id).await.ok_or_else(|| {
            crate::error::ModelSrvError::InstanceNotFound(instance_id.to_string())
        })?;

        let model = self
            .model_registry
            .get_model(&instance.model_id)
            .await
            .ok_or_else(|| crate::error::ModelSrvError::ModelNotFound(instance.model_id.clone()))?;

        let command_def = model
            .commands
            .iter()
            .find(|c| c.identifier == command_id)
            .ok_or_else(|| crate::error::ModelSrvError::CommandNotFound(command_id.to_string()))?;

        // 验证参数
        for param_def in &command_def.input_params {
            if param_def.required && !params.contains_key(&param_def.name) {
                return Err(crate::error::ModelSrvError::InvalidValue(format!(
                    "Missing required parameter: {}",
                    param_def.name
                )));
            }

            if let Some(value) = params.get(&param_def.name) {
                if !param_def.data_type.validate_value(value) {
                    return Err(crate::error::ModelSrvError::InvalidValue(format!(
                        "Invalid value for parameter '{}'",
                        param_def.name
                    )));
                }
            }
        }

        // TODO: 实际执行命令（通过Redis发送到comsrv）
        let request_id = uuid::Uuid::new_v4().to_string();
        let response = CommandResponse {
            request_id: request_id.clone(),
            success: true,
            result: None,
            error: None,
            timestamp: chrono::Utc::now().timestamp_millis(),
        };

        Ok(response)
    }

    /// 获取设备数据
    pub async fn get_device_data(&self, instance_id: &str) -> Option<DeviceData> {
        self.data_cache.read().await.get(instance_id).cloned()
    }

    /// 列出所有实例
    pub async fn list_instances(&self) -> Vec<DeviceInstance> {
        self.instances.read().await.values().cloned().collect()
    }

    /// 删除设备实例
    pub async fn delete_instance(&self, instance_id: &str) -> Result<()> {
        self.instances
            .write()
            .await
            .remove(instance_id)
            .ok_or_else(|| {
                crate::error::ModelSrvError::InstanceNotFound(instance_id.to_string())
            })?;

        self.data_cache.write().await.remove(instance_id);

        tracing::info!("Deleted device instance: {}", instance_id);

        Ok(())
    }

    /// 获取设备模型
    pub async fn get_model(&self, model_id: &str) -> Result<DeviceModel> {
        self.model_registry
            .get_model(model_id)
            .await
            .ok_or_else(|| crate::error::ModelSrvError::ModelNotFound(model_id.to_string()))
    }
}

/// 实例批量操作
impl InstanceManager {
    /// 批量创建实例
    pub async fn create_instances_batch(
        &self,
        model_id: &str,
        instances: Vec<(String, String, Option<HashMap<String, serde_json::Value>>)>,
    ) -> Result<Vec<DeviceInstance>> {
        let mut created = Vec::new();

        for (instance_id, name, properties) in instances {
            match self
                .create_instance(model_id, &instance_id, &name, properties, None)
                .await
            {
                Ok(instance) => created.push(instance),
                Err(e) => {
                    tracing::error!("Failed to create instance {}: {}", instance_id, e);
                }
            }
        }

        Ok(created)
    }

    /// 批量更新遥测数据
    pub async fn update_telemetry_batch(
        &self,
        updates: Vec<(String, String, f64, Option<f64>)>,
    ) -> Result<()> {
        for (instance_id, telemetry_id, value, raw_value) in updates {
            if let Err(e) = self
                .update_telemetry(&instance_id, &telemetry_id, value, raw_value)
                .await
            {
                tracing::error!("Failed to update telemetry for {}: {}", instance_id, e);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_instance_creation() {
        let registry = Arc::new(ModelRegistry::new());
        let manager = InstanceManager::new(registry.clone());

        // 先注册一个模型
        let model = DeviceModel {
            id: "test_model".to_string(),
            name: "Test Model".to_string(),
            version: "1.0.0".to_string(),
            description: "Test model".to_string(),
            device_type: DeviceType::Sensor,
            properties: vec![PropertyDefinition {
                identifier: "location".to_string(),
                name: "Location".to_string(),
                data_type: DataType::String,
                required: true,
                default_value: Some(serde_json::json!("Unknown")),
                constraints: None,
                unit: None,
                description: None,
            }],
            telemetry: vec![],
            commands: vec![],
            events: vec![],
            calculations: vec![],
            metadata: HashMap::new(),
        };

        registry.register_model(model).await.unwrap();

        // 创建实例
        let instance = manager
            .create_instance("test_model", "instance_001", "Test Instance", None, None)
            .await
            .unwrap();

        assert_eq!(instance.instance_id, "instance_001");
        assert_eq!(instance.model_id, "test_model");
        assert!(instance.properties.contains_key("location"));
    }
}
