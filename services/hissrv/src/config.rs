//! 标准化配置管理模块 - 严格遵循Redis数据结构规范v3.2

use crate::error::{HisSrvError, Result};
use figment::{providers::Env, providers::Format, providers::Yaml, Figment};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::Path};
use voltage_libs::config::{InfluxConfig, RedisConfig};

/// 服务配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    pub name: String,
    pub version: String,
}

impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            name: "hissrv".to_string(),
            version: "0.3.0".to_string(),
        }
    }
}

/// comsrv数据类型映射配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeMappingConfig {
    /// comsrv数据类型到InfluxDB measurement的映射
    #[serde(default = "default_type_mappings")]
    pub type_mappings: HashMap<String, String>,
}

fn default_type_mappings() -> HashMap<String, String> {
    let mut mappings = HashMap::new();
    mappings.insert("m".to_string(), "measurement".to_string()); // 测量值 (YC)
    mappings.insert("s".to_string(), "signal".to_string()); // 信号值 (YX)
    mappings.insert("c".to_string(), "control".to_string()); // 控制值 (YK)
    mappings.insert("a".to_string(), "adjustment".to_string()); // 调节值 (YT)
    mappings
}

impl Default for TypeMappingConfig {
    fn default() -> Self {
        Self {
            type_mappings: default_type_mappings(),
        }
    }
}

/// 单个数据源配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataSourceConfig {
    /// 是否启用此数据源
    #[serde(default)]
    pub enabled: bool,
    /// 支持的数据类型
    #[serde(default)]
    pub data_types: Vec<String>,
    /// InfluxDB measurement名称
    pub measurement: String,
    /// 可选：只订阅特定通道（仅comsrv适用）
    #[serde(default)]
    pub channels: Vec<u32>,
}

impl Default for DataSourceConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            data_types: Vec::new(),
            measurement: "default".to_string(),
            channels: Vec::new(),
        }
    }
}

/// 多服务数据源配置映射
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataSourcesConfig {
    /// comsrv 通信服务数据配置
    #[serde(default)]
    pub comsrv: DataSourceConfig,
    /// modsrv 模型服务数据配置
    #[serde(default)]
    pub modsrv: DataSourceConfig,
    /// alarmsrv 告警服务数据配置
    #[serde(default)]
    pub alarmsrv: DataSourceConfig,
    /// rulesrv 规则服务数据配置
    #[serde(default)]
    pub rulesrv: DataSourceConfig,
}

impl Default for DataSourcesConfig {
    fn default() -> Self {
        Self {
            comsrv: DataSourceConfig {
                enabled: true,
                data_types: vec![
                    "m".to_string(),
                    "s".to_string(),
                    "c".to_string(),
                    "a".to_string(),
                ],
                measurement: "comsrv_data".to_string(),
                channels: Vec::new(),
            },
            modsrv: DataSourceConfig::default(),
            alarmsrv: DataSourceConfig::default(),
            rulesrv: DataSourceConfig::default(),
        }
    }
}

/// 标准化Redis订阅配置 - 支持多服务兼容
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StandardRedisConfig {
    #[serde(flatten)]
    pub connection: RedisConfig,

    /// 传统订阅模式 - 向后兼容
    #[serde(default)]
    pub patterns: Vec<String>,

    /// 多服务数据源配置（推荐）
    #[serde(default)]
    pub data_sources: DataSourcesConfig,

    /// comsrv数据类型映射（用于向后兼容）
    #[serde(flatten)]
    pub type_config: TypeMappingConfig,
}

impl StandardRedisConfig {
    /// 获取所有生效的订阅模式（data_sources + patterns）
    pub fn get_all_patterns(&self) -> Vec<String> {
        let mut all_patterns = Vec::new();

        // 添加传统模式
        all_patterns.extend(self.patterns.clone());

        // 从数据源配置生成模式
        if self.data_sources.comsrv.enabled {
            if self.data_sources.comsrv.channels.is_empty() {
                // 订阅所有comsrv通道
                all_patterns.push("comsrv:*".to_string());
            } else {
                // 订阅特定通道
                for channel_id in &self.data_sources.comsrv.channels {
                    all_patterns.push(format!("comsrv:{}:*", channel_id));
                }
            }
        }

        if self.data_sources.modsrv.enabled {
            all_patterns.push("modsrv:*".to_string());
        }

        if self.data_sources.alarmsrv.enabled {
            all_patterns.push("alarmsrv:*".to_string());
        }

        if self.data_sources.rulesrv.enabled {
            all_patterns.push("rulesrv:*".to_string());
        }

        // 如果没有任何模式，默认订阅comsrv
        if all_patterns.is_empty() {
            all_patterns.push("comsrv:*".to_string());
        }

        all_patterns
    }

    /// 获取指定服务和数据类型对应的InfluxDB measurement名称
    pub fn get_measurement_for_service_type(&self, service: &str, data_type: &str) -> String {
        match service {
            "comsrv" => {
                if self.data_sources.comsrv.enabled {
                    self.data_sources.comsrv.measurement.clone()
                } else {
                    // 向后兼容：使用type_mappings
                    self.type_config
                        .type_mappings
                        .get(data_type)
                        .cloned()
                        .unwrap_or_else(|| format!("comsrv_{}", data_type))
                }
            }
            "modsrv" => {
                if self.data_sources.modsrv.enabled {
                    self.data_sources.modsrv.measurement.clone()
                } else {
                    format!("modsrv_{}", data_type)
                }
            }
            "alarmsrv" => {
                if self.data_sources.alarmsrv.enabled {
                    self.data_sources.alarmsrv.measurement.clone()
                } else {
                    format!("alarmsrv_{}", data_type)
                }
            }
            "rulesrv" => {
                if self.data_sources.rulesrv.enabled {
                    self.data_sources.rulesrv.measurement.clone()
                } else {
                    format!("rulesrv_{}", data_type)
                }
            }
            _ => format!("{}_{}", service, data_type),
        }
    }

    /// 验证服务和数据类型是否支持
    pub fn is_supported_service_type(&self, service: &str, data_type: &str) -> bool {
        match service {
            "comsrv" => {
                self.data_sources.comsrv.enabled
                    && (self.data_sources.comsrv.data_types.is_empty()
                        || self
                            .data_sources
                            .comsrv
                            .data_types
                            .contains(&data_type.to_string()))
            }
            "modsrv" => {
                self.data_sources.modsrv.enabled
                    && (self.data_sources.modsrv.data_types.is_empty()
                        || self
                            .data_sources
                            .modsrv
                            .data_types
                            .contains(&data_type.to_string()))
            }
            "alarmsrv" => {
                self.data_sources.alarmsrv.enabled
                    && (self.data_sources.alarmsrv.data_types.is_empty()
                        || self
                            .data_sources
                            .alarmsrv
                            .data_types
                            .contains(&data_type.to_string()))
            }
            "rulesrv" => {
                self.data_sources.rulesrv.enabled
                    && (self.data_sources.rulesrv.data_types.is_empty()
                        || self
                            .data_sources
                            .rulesrv
                            .data_types
                            .contains(&data_type.to_string()))
            }
            _ => false,
        }
    }

    /// 获取所有启用的服务列表
    pub fn get_enabled_services(&self) -> Vec<String> {
        let mut services = Vec::new();

        if self.data_sources.comsrv.enabled {
            services.push("comsrv".to_string());
        }
        if self.data_sources.modsrv.enabled {
            services.push("modsrv".to_string());
        }
        if self.data_sources.alarmsrv.enabled {
            services.push("alarmsrv".to_string());
        }
        if self.data_sources.rulesrv.enabled {
            services.push("rulesrv".to_string());
        }

        services
    }

    /// 向后兼容：获取所有支持的数据类型
    pub fn get_supported_types(&self) -> Vec<String> {
        self.data_sources.comsrv.data_types.clone()
    }
}

/// InfluxDB批量写入配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfluxBatchConfig {
    #[serde(flatten)]
    pub connection: InfluxConfig,
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,
    #[serde(default = "default_flush_interval")]
    pub flush_interval_seconds: u64,
}

fn default_batch_size() -> usize {
    1000
}

fn default_flush_interval() -> u64 {
    10
}

impl Default for InfluxBatchConfig {
    fn default() -> Self {
        Self {
            connection: InfluxConfig {
                url: "http://localhost:8086".to_string(),
                org: "default".to_string(),
                bucket: "hissrv_data".to_string(),
                token: "default-token".to_string(),
                timeout_seconds: 30,
                // 向下兼容字段
                database: Some("hissrv_data".to_string()),
                username: None,
                password: None,
            },
            batch_size: default_batch_size(),
            flush_interval_seconds: default_flush_interval(),
        }
    }
}

/// 日志配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    #[serde(default = "default_log_level")]
    pub level: String,
}

fn default_log_level() -> String {
    "info".to_string()
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
        }
    }
}

/// 标准化主配置结构
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub service: ServiceConfig,
    #[serde(default)]
    pub redis: StandardRedisConfig,
    #[serde(default)]
    pub influxdb: InfluxBatchConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
}

impl Config {
    /// 从配置文件和环境变量加载配置
    pub fn load() -> Result<Self> {
        let config_path =
            std::env::var("HISSRV_CONFIG").unwrap_or_else(|_| "config/hissrv.yaml".to_string());

        let mut figment = Figment::new();

        // 如果配置文件存在，加载它
        if Path::new(&config_path).exists() {
            figment = figment.merge(Yaml::file(&config_path));
        } else {
            tracing::warn!("配置文件 {} 不存在，使用默认配置", config_path);
        }

        // 环境变量覆盖，使用 HISSRV_ 前缀
        figment = figment.merge(Env::prefixed("HISSRV_").split("_"));

        let config: Config = figment.extract()?;

        Self::validate(&config)?;
        Ok(config)
    }

    /// 验证配置的有效性
    fn validate(config: &Config) -> Result<()> {
        // 验证 Redis 配置
        if config.redis.connection.host.is_empty() {
            return Err(HisSrvError::Config("Redis host 不能为空".to_string()));
        }

        // 验证 InfluxDB 配置
        if config.influxdb.connection.url.is_empty() {
            return Err(HisSrvError::Config("InfluxDB URL 不能为空".to_string()));
        }
        if config.influxdb.connection.org.is_empty() {
            return Err(HisSrvError::Config("InfluxDB org 不能为空".to_string()));
        }
        if config.influxdb.connection.bucket.is_empty() {
            return Err(HisSrvError::Config("InfluxDB bucket 不能为空".to_string()));
        }
        if config.influxdb.connection.token.is_empty() {
            return Err(HisSrvError::Config("InfluxDB token 不能为空".to_string()));
        }

        // 验证订阅模式 - 检查patterns或data_sources
        let all_patterns = config.redis.get_all_patterns();
        if all_patterns.is_empty() {
            return Err(HisSrvError::Config(
                "至少需要一个Redis订阅模式或启用的数据源".to_string(),
            ));
        }

        // 验证数据类型映射
        if config.redis.type_config.type_mappings.is_empty() {
            return Err(HisSrvError::Config("至少需要一个数据类型映射".to_string()));
        }

        // 验证日志级别
        match config.logging.level.to_lowercase().as_str() {
            "trace" | "debug" | "info" | "warn" | "error" => {}
            _ => return Err(HisSrvError::Config("无效的日志级别".to_string())),
        }

        Ok(())
    }
}
