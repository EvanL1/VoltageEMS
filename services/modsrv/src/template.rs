//! 极简模板managingmodular
//!
//! 提供device模板的managingfunction，supportingJSON格式的模板definition

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

use crate::error::{ModelSrvError, Result};

/// 极简模板definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Template {
    /// 模板ID
    pub id: String,
    /// data点definition (key: 点名, value: 单位，nulltable示none单位)
    pub data: HashMap<String, Option<String>>,
    /// operationdefinition (key: operation名, value: parameter单位，nulltable示noneparameter)
    pub action: HashMap<String, Option<String>>,
}

/// 模板managing器
pub struct TemplateManager {
    /// 模板storagepath
    template_dir: PathBuf,
    /// 已loading的模板cache
    pub templates: HashMap<String, Template>,
}

impl TemplateManager {
    /// Create模板managing器
    pub fn new<P: AsRef<Path>>(template_dir: P) -> Self {
        Self {
            template_dir: template_dir.as_ref().to_path_buf(),
            templates: HashMap::new(),
        }
    }

    /// Loadall模板
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

    /// recursiveloadingdirectorymedium的模板（只supportingJSON格式）
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
                // recursiveloading子directory
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

    /// Load单个模板file
    fn load_template_file(&self, path: &Path) -> Result<Template> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| ModelSrvError::template(format!("Failed to read template file: {}", e)))?;

        let template: Template = serde_json::from_str(&content).map_err(|e| {
            ModelSrvError::template(format!("Failed to parse template JSON: {}", e))
        })?;

        // validation模板
        self.validate_template(&template)?;

        Ok(template)
    }

    /// Get模板
    pub fn get_template(&self, template_id: &str) -> Option<&Template> {
        self.templates.get(template_id)
    }

    /// column出all模板
    pub fn list_templates(&self) -> Vec<&Template> {
        self.templates.values().collect()
    }

    /// Validate模板
    pub fn validate_template(&self, template: &Template) -> Result<()> {
        // validation模板ID
        if template.id.is_empty() {
            return Err(ModelSrvError::template("Template ID cannot be empty"));
        }

        // 至少需要一个data点或operation
        if template.data.is_empty() && template.action.is_empty() {
            return Err(ModelSrvError::template(
                "Template must have at least one data point or action",
            ));
        }

        Ok(())
    }

    /// Save模板到file
    pub async fn save_template(&self, template: &Template) -> Result<()> {
        // validation模板
        self.validate_template(template)?;

        let file_path = self.template_dir.join(format!("{}.json", template.id));

        // createdirectory（如果不exists）
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                ModelSrvError::template(format!("Failed to create template directory: {}", e))
            })?;
        }

        // serializing内容
        let content = serde_json::to_string_pretty(template).map_err(|e| {
            ModelSrvError::template(format!("Failed to serialize template to JSON: {}", e))
        })?;

        // writefile
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

        // empty模板应该failed
        let empty_template = Template {
            id: "test".to_string(),
            data: HashMap::new(),
            action: HashMap::new(),
        };
        assert!(manager.validate_template(&empty_template).is_err());

        // 有data点的模板应该通过
        let mut data_template = Template {
            id: "test".to_string(),
            data: HashMap::new(),
            action: HashMap::new(),
        };
        data_template
            .data
            .insert("voltage".to_string(), Some("V".to_string()));
        assert!(manager.validate_template(&data_template).is_ok());

        // 有operation的模板应该通过
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
        // create电table模板
        let mut power_meter = Template {
            id: "power_meter".to_string(),
            data: HashMap::new(),
            action: HashMap::new(),
        };

        // 添加data点
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

        // 添加operation
        power_meter.action.insert("reset".to_string(), None);
        power_meter
            .action
            .insert("set_limit".to_string(), Some("kW".to_string()));

        let manager = TemplateManager::new("templates");
        assert!(manager.validate_template(&power_meter).is_ok());
    }
}
