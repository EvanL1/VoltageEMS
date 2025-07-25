//! ModSrv配置管理
//!
//! 提供统一的配置加载和管理功能

use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::{debug, info, warn};

use crate::error::{ModelSrvError, Result};
use crate::model::ModelConfig;

/// Redis配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    pub url: String,
    pub key_prefix: String,
    pub connection_timeout_ms: u64,
    pub retry_attempts: usize,
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            url: "redis://localhost:6379".to_string(),
            key_prefix: "modsrv:".to_string(),
            connection_timeout_ms: 5000,
            retry_attempts: 3,
        }
    }
}

/// API服务配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    pub host: String,
    pub port: u16,
    pub timeout_seconds: u64,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 8092,
            timeout_seconds: 30,
        }
    }
}

/// 日志配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogConfig {
    pub level: String,
    pub file_path: Option<String>,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            file_path: None,
        }
    }
}

/// 主配置结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub service_name: String,
    pub version: String,
    pub redis: RedisConfig,
    pub api: ApiConfig,
    pub log: LogConfig,
    pub models: Vec<ModelConfig>,
    pub update_interval_ms: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            service_name: "modsrv".to_string(),
            version: "1.0.0".to_string(),
            redis: RedisConfig::default(),
            api: ApiConfig::default(),
            log: LogConfig::default(),
            models: Vec::new(),
            update_interval_ms: 1000,
        }
    }
}

impl Config {
    /// 从文件加载配置
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let content = std::fs::read_to_string(path).map_err(|e| {
            ModelSrvError::ConfigError(format!("无法读取配置文件 {}: {}", path.display(), e))
        })?;

        let config: Config = if path.extension().and_then(|s| s.to_str()) == Some("yaml")
            || path.extension().and_then(|s| s.to_str()) == Some("yml")
        {
            serde_yaml::from_str(&content)
                .map_err(|e| ModelSrvError::ConfigError(format!("YAML配置解析失败: {}", e)))?
        } else {
            serde_json::from_str(&content)
                .map_err(|e| ModelSrvError::ConfigError(format!("JSON配置解析失败: {}", e)))?
        };

        info!("配置加载成功: {}", path.display());
        debug!("配置内容: {:?}", config);
        Ok(config)
    }

    /// 从环境变量加载配置
    pub fn from_env() -> Result<Self> {
        let mut config = Config::default();

        // Redis配置
        if let Ok(redis_url) = std::env::var("REDIS_URL") {
            config.redis.url = redis_url;
        }
        if let Ok(redis_prefix) = std::env::var("REDIS_KEY_PREFIX") {
            config.redis.key_prefix = redis_prefix;
        }

        // API配置
        if let Ok(api_host) = std::env::var("API_HOST") {
            config.api.host = api_host;
        }
        if let Ok(api_port) = std::env::var("API_PORT") {
            config.api.port = api_port
                .parse()
                .map_err(|e| ModelSrvError::ConfigError(format!("无效的API端口: {}", e)))?;
        }

        // 日志配置
        if let Ok(log_level) = std::env::var("LOG_LEVEL") {
            config.log.level = log_level;
        }
        if let Ok(log_file) = std::env::var("LOG_FILE") {
            config.log.file_path = Some(log_file);
        }

        // 更新间隔
        if let Ok(interval) = std::env::var("UPDATE_INTERVAL_MS") {
            config.update_interval_ms = interval
                .parse()
                .map_err(|e| ModelSrvError::ConfigError(format!("无效的更新间隔: {}", e)))?;
        }

        info!("从环境变量加载配置完成");
        Ok(config)
    }

    /// 自动加载配置（文件优先，环境变量补充）
    pub fn load() -> Result<Self> {
        // 按优先级尝试加载配置文件
        let config_files = [
            "config/modsrv.yaml",
            "config/modsrv.yml",
            "config/default.yaml",
            "config/default.yml",
            "modsrv.yaml",
            "modsrv.yml",
            "config.yaml",
            "config.yml",
        ];

        let mut config = None;
        for file_path in &config_files {
            if Path::new(file_path).exists() {
                match Self::from_file(file_path) {
                    Ok(cfg) => {
                        config = Some(cfg);
                        break;
                    }
                    Err(e) => {
                        warn!("配置文件 {} 加载失败: {}", file_path, e);
                    }
                }
            }
        }

        let mut config = config.unwrap_or_else(|| {
            info!("未找到配置文件，使用默认配置");
            Config::default()
        });

        // 环境变量覆盖配置
        if let Ok(redis_url) = std::env::var("REDIS_URL") {
            config.redis.url = redis_url;
        }
        if let Ok(api_port) = std::env::var("API_PORT") {
            if let Ok(port) = api_port.parse::<u16>() {
                config.api.port = port;
            }
        }
        if let Ok(log_level) = std::env::var("LOG_LEVEL") {
            config.log.level = log_level;
        }

        Ok(config)
    }

    /// 验证配置
    pub fn validate(&self) -> Result<()> {
        // 验证Redis URL
        if self.redis.url.is_empty() {
            return Err(ModelSrvError::ConfigError("Redis URL不能为空".to_string()));
        }

        // 验证API端口
        if self.api.port == 0 {
            return Err(ModelSrvError::ConfigError("API端口不能为0".to_string()));
        }

        // 验证模型配置
        for model in &self.models {
            if model.id.is_empty() {
                return Err(ModelSrvError::ConfigError("模型ID不能为空".to_string()));
            }
            if model.monitoring.is_empty() && model.control.is_empty() {
                return Err(ModelSrvError::ConfigError(format!(
                    "模型 {} 必须包含监视或控制点",
                    model.id
                )));
            }
        }

        info!("配置验证通过");
        Ok(())
    }

    /// 保存配置到文件
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let path = path.as_ref();
        let content = if path.extension().and_then(|s| s.to_str()) == Some("yaml")
            || path.extension().and_then(|s| s.to_str()) == Some("yml")
        {
            serde_yaml::to_string(self)
                .map_err(|e| ModelSrvError::ConfigError(format!("YAML序列化失败: {}", e)))?
        } else {
            serde_json::to_string_pretty(self)
                .map_err(|e| ModelSrvError::ConfigError(format!("JSON序列化失败: {}", e)))?
        };

        std::fs::write(path, content)
            .map_err(|e| ModelSrvError::ConfigError(format!("写入配置文件失败: {}", e)))?;

        info!("配置已保存到: {}", path.display());
        Ok(())
    }

    /// 添加模型配置
    pub fn add_model(&mut self, model: ModelConfig) {
        self.models.push(model);
        info!("添加模型配置: {}", self.models.last().unwrap().id);
    }

    /// 移除模型配置
    pub fn remove_model(&mut self, model_id: &str) -> bool {
        let original_len = self.models.len();
        self.models.retain(|m| m.id != model_id);
        let removed = self.models.len() < original_len;
        if removed {
            info!("移除模型配置: {}", model_id);
        }
        removed
    }

    /// 获取启用的模型配置
    pub fn enabled_models(&self) -> Vec<&ModelConfig> {
        self.models.iter().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.service_name, "modsrv");
        assert_eq!(config.redis.url, "redis://localhost:6379");
        assert_eq!(config.api.port, 8092);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validation() {
        let mut config = Config::default();
        assert!(config.validate().is_ok());

        // 测试无效配置
        config.redis.url = "".to_string();
        assert!(config.validate().is_err());

        config.redis.url = "redis://localhost:6379".to_string();
        config.api.port = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_file_operations() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test_config.yaml");

        let config = Config::default();
        assert!(config.save_to_file(&file_path).is_ok());
        assert!(file_path.exists());

        let loaded_config = Config::from_file(&file_path).unwrap();
        assert_eq!(config.service_name, loaded_config.service_name);
        assert_eq!(config.api.port, loaded_config.api.port);
    }
}
