//! hissrv 配置模块 - 极简配置系统
//! 支持灵活的数据源配置和映射规则

use crate::Result;
use figment::{
    providers::{Env, Format, Yaml},
    Figment,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::time::Duration;

/// 服务配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    pub name: String,
    #[serde(with = "humantime_serde")]
    pub polling_interval: Duration,
    #[serde(default)]
    pub enable_api: bool,
    #[serde(default = "default_api_port")]
    pub api_port: u16,
}

fn default_api_port() -> u16 {
    8082
}

/// Redis 数据源配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisDataKey {
    pub pattern: String,
    #[serde(rename = "type")]
    pub data_type: String, // "list" or "hash"
}

/// Redis 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    pub url: String,
    pub data_keys: Vec<RedisDataKey>,
}

/// InfluxDB 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfluxConfig {
    pub url: String,
    pub org: String,
    pub bucket: String,
    pub token: String,
    pub batch_size: usize,
    #[serde(with = "humantime_serde")]
    pub write_timeout: Duration,
}

/// 标签提取规则
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TagRule {
    #[serde(rename = "extract")]
    Extract { field: String },
    #[serde(rename = "static")]
    Static { value: String },
}

/// 字段映射规则
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldMapping {
    pub name: String,
    pub field_type: String, // "float", "int", "bool", "string"
}

/// 数据映射配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataMapping {
    pub source: String,
    pub measurement: String,
    pub tags: Vec<TagRule>,
    pub fields: Vec<FieldMapping>,
}

/// 完整配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_version")]
    pub version: String,
    pub service: ServiceConfig,
    pub redis: RedisConfig,
    pub influxdb: InfluxConfig,
    pub mappings: Vec<DataMapping>,
}

fn default_version() -> String {
    "0.0.1".to_string()
}

impl Config {
    /// 加载配置文件
    pub fn load() -> Result<Self> {
        let config = Figment::new()
            .merge(Yaml::file("config/hissrv.yaml"))
            .merge(Env::prefixed("HISSRV_").split("_"))
            .extract()
            .map_err(|e| hissrv::anyhow!("Failed to load configuration: {}", e))?;

        Ok(config)
    }

    /// 根据源模式查找映射规则
    pub fn find_mapping(&self, source: &str) -> Option<&DataMapping> {
        self.mappings
            .iter()
            .find(|m| source.starts_with(&m.source.replace("*", "")))
    }

    /// 根据源模式查找映射规则（可变引用）
    #[allow(dead_code)]
    pub fn find_mapping_mut(&mut self, source: &str) -> Option<&mut DataMapping> {
        self.mappings
            .iter_mut()
            .find(|m| source.starts_with(&m.source.replace("*", "")))
    }

    /// 添加新的映射规则
    pub fn add_mapping(&mut self, mapping: DataMapping) -> Result<()> {
        // 检查是否已存在相同的源模式
        if self.find_mapping(&mapping.source).is_some() {
            return Err(hissrv::anyhow!(
                "Mapping for source '{}' already exists",
                mapping.source
            ));
        }
        self.mappings.push(mapping);
        Ok(())
    }

    /// 更新现有映射规则
    pub fn update_mapping(&mut self, source: &str, new_mapping: DataMapping) -> Result<()> {
        if let Some(pos) = self.mappings.iter().position(|m| m.source == source) {
            self.mappings[pos] = new_mapping;
            Ok(())
        } else {
            Err(hissrv::anyhow!("Mapping for source '{}' not found", source))
        }
    }

    /// 删除映射规则
    pub fn remove_mapping(&mut self, source: &str) -> Result<()> {
        if let Some(pos) = self.mappings.iter().position(|m| m.source == source) {
            self.mappings.remove(pos);
            Ok(())
        } else {
            Err(hissrv::anyhow!("Mapping for source '{}' not found", source))
        }
    }

    /// 保存配置到文件
    pub fn save_to_file(&self, path: impl AsRef<Path>) -> Result<()> {
        // 备份原文件
        let path = path.as_ref();
        if path.exists() {
            let backup_path = path.with_extension("yaml.bak");
            fs::copy(path, backup_path)?;
        }

        // 写入新配置
        let yaml = serde_yaml::to_string(self)?;
        fs::write(path, yaml)?;
        Ok(())
    }

    /// 重新加载配置
    pub fn reload() -> Result<Self> {
        Self::load()
    }

    /// 验证配置完整性
    pub fn validate(&self) -> Result<()> {
        // 验证服务配置
        if self.service.name.is_empty() {
            return Err(hissrv::anyhow!("Service name cannot be empty"));
        }

        // 验证 Redis 配置
        if self.redis.url.is_empty() {
            return Err(hissrv::anyhow!("Redis URL cannot be empty"));
        }

        if self.redis.data_keys.is_empty() {
            return Err(hissrv::anyhow!("At least one data key must be configured"));
        }

        // 验证 InfluxDB 配置
        if self.influxdb.url.is_empty() {
            return Err(hissrv::anyhow!("InfluxDB URL cannot be empty"));
        }

        if self.influxdb.org.is_empty() {
            return Err(hissrv::anyhow!("InfluxDB organization cannot be empty"));
        }

        if self.influxdb.bucket.is_empty() {
            return Err(hissrv::anyhow!("InfluxDB bucket cannot be empty"));
        }

        if self.influxdb.token.is_empty() {
            return Err(hissrv::anyhow!("InfluxDB token cannot be empty"));
        }

        // 验证映射规则
        for mapping in &self.mappings {
            if mapping.source.is_empty() {
                return Err(hissrv::anyhow!("Mapping source cannot be empty"));
            }

            if mapping.measurement.is_empty() {
                return Err(hissrv::anyhow!("Mapping measurement cannot be empty"));
            }

            if mapping.fields.is_empty() {
                return Err(hissrv::anyhow!(
                    "At least one field must be defined for mapping '{}'",
                    mapping.source
                ));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config() -> Config {
        Config {
            version: "0.0.1".to_string(),
            service: ServiceConfig {
                name: "hissrv".to_string(),
                polling_interval: Duration::from_secs(10),
                enable_api: false,
                api_port: 8082,
            },
            redis: RedisConfig {
                url: "redis://localhost".to_string(),
                data_keys: vec![RedisDataKey {
                    pattern: "archive:pending".to_string(),
                    data_type: "list".to_string(),
                }],
            },
            influxdb: InfluxConfig {
                url: "http://localhost:8086".to_string(),
                org: "test".to_string(),
                bucket: "test".to_string(),
                token: "test".to_string(),
                batch_size: 1000,
                write_timeout: Duration::from_secs(30),
            },
            mappings: vec![DataMapping {
                source: "archive:1m:*".to_string(),
                measurement: "metrics_1m".to_string(),
                tags: vec![],
                fields: vec![FieldMapping {
                    name: "value".to_string(),
                    field_type: "float".to_string(),
                }],
            }],
        }
    }

    #[test]
    fn test_find_mapping() {
        let config = create_test_config();
        assert!(config.find_mapping("archive:1m:test").is_some());
        assert!(config.find_mapping("other:data").is_none());
    }

    #[test]
    fn test_add_mapping() {
        let mut config = create_test_config();
        let new_mapping = DataMapping {
            source: "archive:5m:*".to_string(),
            measurement: "metrics_5m".to_string(),
            tags: vec![],
            fields: vec![],
        };

        assert!(config.add_mapping(new_mapping.clone()).is_ok());
        assert!(config.find_mapping("archive:5m:test").is_some());

        // 测试重复添加
        assert!(config.add_mapping(new_mapping).is_err());
    }

    #[test]
    fn test_update_mapping() {
        let mut config = create_test_config();
        let updated_mapping = DataMapping {
            source: "archive:1m:*".to_string(),
            measurement: "metrics_1m_updated".to_string(),
            tags: vec![],
            fields: vec![],
        };

        assert!(config
            .update_mapping("archive:1m:*", updated_mapping)
            .is_ok());
        assert_eq!(config.mappings[0].measurement, "metrics_1m_updated");

        // 测试更新不存在的映射
        assert!(config
            .update_mapping(
                "non_existent",
                DataMapping {
                    source: "test".to_string(),
                    measurement: "test".to_string(),
                    tags: vec![],
                    fields: vec![],
                }
            )
            .is_err());
    }

    #[test]
    fn test_remove_mapping() {
        let mut config = create_test_config();
        assert!(config.remove_mapping("archive:1m:*").is_ok());
        assert!(config.mappings.is_empty());

        // 测试删除不存在的映射
        assert!(config.remove_mapping("non_existent").is_err());
    }

    #[test]
    fn test_validate_config() {
        let mut config = create_test_config();
        assert!(config.validate().is_ok());

        // 测试空服务名
        config.service.name.clear();
        assert!(config.validate().is_err());
    }
}
