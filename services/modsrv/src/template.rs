//! 极简模板管理模块
//!
//! 提供设备模板的管理功能，支持JSON格式的模板定义

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

use crate::error::{ModelSrvError, Result};

/// 极简模板定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Template {
    /// 模板ID
    pub id: String,
    /// 数据点定义 (key: 点名, value: 单位，null表示无单位)
    pub data: HashMap<String, Option<String>>,
    /// 操作定义 (key: 操作名, value: 参数单位，null表示无参数)
    pub action: HashMap<String, Option<String>>,
}

/// 模板管理器
pub struct TemplateManager {
    /// 模板存储路径
    template_dir: PathBuf,
    /// 已加载的模板缓存
    pub templates: HashMap<String, Template>,
}

impl TemplateManager {
    /// 创建模板管理器
    pub fn new<P: AsRef<Path>>(template_dir: P) -> Self {
        Self {
            template_dir: template_dir.as_ref().to_path_buf(),
            templates: HashMap::new(),
        }
    }

    /// 加载所有模板
    pub async fn load_all_templates(&mut self) -> Result<()> {
        info!("Loading templates from: {:?}", self.template_dir);

        if !self.template_dir.exists() {
            warn!("Template directory does not exist: {:?}", self.template_dir);
            return Ok(());
        }

        let template_dir = self.template_dir.clone();
        self.load_templates_from_dir(&template_dir)?;

        info!("Loaded {} templates", self.templates.len());
        Ok(())
    }

    /// 递归加载目录中的模板（只支持JSON格式）
    fn load_templates_from_dir(&mut self, dir: &Path) -> Result<()> {
        let entries = std::fs::read_dir(dir).map_err(|e| {
            ModelSrvError::template(format!("Failed to read template directory: {}", e))
        })?;

        for entry in entries {
            let entry = entry.map_err(|e| {
                ModelSrvError::template(format!("Failed to read directory entry: {}", e))
            })?;

            let path = entry.path();

            if path.is_dir() {
                // 递归加载子目录
                self.load_templates_from_dir(&path)?;
            } else if path.extension().and_then(|s| s.to_str()) == Some("json") {
                match self.load_template_file(&path) {
                    Ok(template) => {
                        debug!("Loaded template: {} from {:?}", template.id, path);
                        self.templates.insert(template.id.clone(), template);
                    },
                    Err(e) => {
                        warn!("Failed to load template from {:?}: {}", path, e);
                    },
                }
            }
        }

        Ok(())
    }

    /// 加载单个模板文件
    fn load_template_file(&self, path: &Path) -> Result<Template> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| ModelSrvError::template(format!("Failed to read template file: {}", e)))?;

        let template: Template = serde_json::from_str(&content).map_err(|e| {
            ModelSrvError::template(format!("Failed to parse template JSON: {}", e))
        })?;

        // 验证模板
        self.validate_template(&template)?;

        Ok(template)
    }

    /// 获取模板
    pub fn get_template(&self, template_id: &str) -> Option<&Template> {
        self.templates.get(template_id)
    }

    /// 列出所有模板
    pub fn list_templates(&self) -> Vec<&Template> {
        self.templates.values().collect()
    }

    /// 验证模板
    pub fn validate_template(&self, template: &Template) -> Result<()> {
        // 验证模板ID
        if template.id.is_empty() {
            return Err(ModelSrvError::template("Template ID cannot be empty"));
        }

        // 至少需要一个数据点或操作
        if template.data.is_empty() && template.action.is_empty() {
            return Err(ModelSrvError::template(
                "Template must have at least one data point or action",
            ));
        }

        Ok(())
    }

    /// 保存模板到文件
    pub async fn save_template(&self, template: &Template) -> Result<()> {
        // 验证模板
        self.validate_template(template)?;

        let file_path = self.template_dir.join(format!("{}.json", template.id));

        // 创建目录（如果不存在）
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                ModelSrvError::template(format!("Failed to create template directory: {}", e))
            })?;
        }

        // 序列化内容
        let content = serde_json::to_string_pretty(template).map_err(|e| {
            ModelSrvError::template(format!("Failed to serialize template to JSON: {}", e))
        })?;

        // 写入文件
        std::fs::write(&file_path, content).map_err(|e| {
            ModelSrvError::template(format!("Failed to write template file: {}", e))
        })?;

        info!("Saved template {} to {:?}", template.id, file_path);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_template_validation() {
        let manager = TemplateManager::new("templates");

        // 空模板应该失败
        let empty_template = Template {
            id: "test".to_string(),
            data: HashMap::new(),
            action: HashMap::new(),
        };
        assert!(manager.validate_template(&empty_template).is_err());

        // 有数据点的模板应该通过
        let mut data_template = Template {
            id: "test".to_string(),
            data: HashMap::new(),
            action: HashMap::new(),
        };
        data_template
            .data
            .insert("voltage".to_string(), Some("V".to_string()));
        assert!(manager.validate_template(&data_template).is_ok());

        // 有操作的模板应该通过
        let mut action_template = Template {
            id: "test".to_string(),
            data: HashMap::new(),
            action: HashMap::new(),
        };
        action_template.action.insert("start".to_string(), None);
        assert!(manager.validate_template(&action_template).is_ok());
    }

    #[test]
    fn test_template_creation() {
        // 创建电表模板
        let mut power_meter = Template {
            id: "power_meter".to_string(),
            data: HashMap::new(),
            action: HashMap::new(),
        };

        // 添加数据点
        power_meter
            .data
            .insert("voltage".to_string(), Some("V".to_string()));
        power_meter
            .data
            .insert("current".to_string(), Some("A".to_string()));
        power_meter
            .data
            .insert("power".to_string(), Some("kW".to_string()));
        power_meter
            .data
            .insert("energy".to_string(), Some("kWh".to_string()));
        power_meter
            .data
            .insert("frequency".to_string(), Some("Hz".to_string()));
        power_meter.data.insert("power_factor".to_string(), None);

        // 添加操作
        power_meter.action.insert("reset".to_string(), None);
        power_meter
            .action
            .insert("set_limit".to_string(), Some("kW".to_string()));

        let manager = TemplateManager::new("templates");
        assert!(manager.validate_template(&power_meter).is_ok());
    }
}
