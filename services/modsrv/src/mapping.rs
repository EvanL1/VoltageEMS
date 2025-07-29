//! 点位映射管理模块
//!
//! 处理ModSrv模型点位名称与底层comsrv channel/point ID的映射关系

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::error::{ModelSrvError, Result};

/// 单个点位的映射信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointMapping {
    /// 通道ID
    pub channel: u16,
    /// 点位ID
    pub point: u32,
    /// 点位类型: "m"(测量), "s"(信号), "c"(控制), "a"(调节)
    #[serde(rename = "type")]
    pub point_type: String,
}

/// 模型的映射配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMappingConfig {
    /// 监视点映射
    pub monitoring: HashMap<String, PointMapping>,
    /// 控制点映射
    pub control: HashMap<String, PointMapping>,
}

/// 映射管理器
pub struct MappingManager {
    /// model_id -> ModelMappingConfig
    mappings: HashMap<String, ModelMappingConfig>,
}

impl MappingManager {
    /// 创建新的映射管理器
    pub fn new() -> Self {
        Self {
            mappings: HashMap::new(),
        }
    }

    /// 从文件加载映射配置
    pub async fn load_from_file<P: AsRef<Path>>(&mut self, model_id: &str, path: P) -> Result<()> {
        let path = path.as_ref();
        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| ModelSrvError::io(format!("读取映射文件失败: {}", e)))?;

        let config: ModelMappingConfig = serde_json::from_str(&content)
            .map_err(|e| ModelSrvError::format(format!("解析映射配置失败: {}", e)))?;

        self.mappings.insert(model_id.to_string(), config);
        Ok(())
    }

    /// 加载映射配置
    pub fn load_mappings(&mut self, model_id: &str, config: ModelMappingConfig) {
        self.mappings.insert(model_id.to_string(), config);
    }

    /// 获取监视点映射
    pub fn get_monitoring_mapping(
        &self,
        model_id: &str,
        point_name: &str,
    ) -> Option<&PointMapping> {
        self.mappings.get(model_id)?.monitoring.get(point_name)
    }

    /// 获取控制点映射
    pub fn get_control_mapping(&self, model_id: &str, control_name: &str) -> Option<&PointMapping> {
        self.mappings.get(model_id)?.control.get(control_name)
    }

    /// 根据channel和point查找点位名称（反向查找）
    pub fn find_point_name(
        &self,
        model_id: &str,
        channel: u16,
        point: u32,
        is_control: bool,
    ) -> Option<String> {
        let model_mapping = self.mappings.get(model_id)?;

        let points = if is_control {
            &model_mapping.control
        } else {
            &model_mapping.monitoring
        };

        for (name, mapping) in points {
            if mapping.channel == channel && mapping.point == point {
                return Some(name.clone());
            }
        }

        None
    }

    /// 获取模型的所有监视点映射
    pub fn get_all_monitoring_mappings(
        &self,
        model_id: &str,
    ) -> Option<&HashMap<String, PointMapping>> {
        self.mappings.get(model_id).map(|m| &m.monitoring)
    }

    /// 获取模型的所有控制点映射
    pub fn get_all_control_mappings(
        &self,
        model_id: &str,
    ) -> Option<&HashMap<String, PointMapping>> {
        self.mappings.get(model_id).map(|m| &m.control)
    }

    /// 批量加载目录下的所有映射文件
    pub async fn load_directory<P: AsRef<Path>>(&mut self, dir: P) -> Result<()> {
        let dir = dir.as_ref();
        let mut entries = tokio::fs::read_dir(dir)
            .await
            .map_err(|e| ModelSrvError::io(format!("读取映射目录失败: {}", e)))?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| ModelSrvError::io(format!("读取目录项失败: {}", e)))?
        {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Some(model_id) = path.file_stem().and_then(|s| s.to_str()) {
                    match self.load_from_file(model_id, &path).await {
                        Ok(_) => tracing::info!("加载映射配置: {}", model_id),
                        Err(e) => tracing::warn!("加载映射配置失败 {}: {}", model_id, e),
                    }
                }
            }
        }

        Ok(())
    }

    /// 获取所有映射（用于加载到Redis）
    pub fn get_all_mappings(&self) -> &HashMap<String, ModelMappingConfig> {
        &self.mappings
    }
}

impl Default for MappingManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mapping_lookup() {
        let mut manager = MappingManager::new();

        let config = ModelMappingConfig {
            monitoring: HashMap::from([(
                "voltage_a".to_string(),
                PointMapping {
                    channel: 1001,
                    point: 10001,
                    point_type: "m".to_string(),
                },
            )]),
            control: HashMap::from([(
                "main_switch".to_string(),
                PointMapping {
                    channel: 1001,
                    point: 30001,
                    point_type: "c".to_string(),
                },
            )]),
        };

        manager.load_mappings("test_model", config);

        // 测试监视点映射
        let mapping = manager
            .get_monitoring_mapping("test_model", "voltage_a")
            .unwrap();
        assert_eq!(mapping.channel, 1001);
        assert_eq!(mapping.point, 10001);

        // 测试控制点映射
        let mapping = manager
            .get_control_mapping("test_model", "main_switch")
            .unwrap();
        assert_eq!(mapping.channel, 1001);
        assert_eq!(mapping.point, 30001);

        // 测试反向查找
        let name = manager
            .find_point_name("test_model", 1001, 10001, false)
            .unwrap();
        assert_eq!(name, "voltage_a");
    }
}
