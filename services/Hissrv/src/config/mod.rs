pub mod points;

use crate::error::{HisSrvError, Result};
use figment::{providers::Env, providers::Yaml, providers::Format, Figment};
use serde::{Deserialize, Serialize};
use std::path::Path;
use self::points::PointStorageConfig;

/// 服务配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    pub name: String,
    pub version: String,
    pub port: u16,
    pub host: String,
}

impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            name: "hissrv".to_string(),
            version: "0.2.0".to_string(),
            port: 8080,
            host: "0.0.0.0".to_string(),
        }
    }
}

/// Redis 连接配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConnectionConfig {
    pub host: String,
    pub port: u16,
    pub password: Option<String>,
    pub database: u8,
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
}

fn default_timeout() -> u64 {
    5
}

impl Default for RedisConnectionConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 6379,
            password: None,
            database: 0,
            timeout_seconds: 5,
        }
    }
}

/// Redis 订阅配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisSubscriptionConfig {
    pub patterns: Vec<String>,
    #[serde(default = "default_channel_ids")]
    pub channel_ids: Option<Vec<u32>>,
}

fn default_channel_ids() -> Option<Vec<u32>> {
    None // 监控所有通道
}

impl Default for RedisSubscriptionConfig {
    fn default() -> Self {
        Self {
            patterns: vec!["comsrv:*".to_string()], // 订阅所有comsrv数据
            channel_ids: None,
        }
    }
}

/// Redis 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    pub connection: RedisConnectionConfig,
    pub subscription: RedisSubscriptionConfig,
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            connection: RedisConnectionConfig::default(),
            subscription: RedisSubscriptionConfig::default(),
        }
    }
}

/// InfluxDB 3.2 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfluxDBConfig {
    pub enabled: bool,
    pub url: String,
    pub database: String,
    pub token: Option<String>,
    pub organization: Option<String>,
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

impl Default for InfluxDBConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            url: "http://localhost:8086".to_string(),
            database: "hissrv_data".to_string(),
            token: None,
            organization: None,
            batch_size: 1000,
            flush_interval_seconds: 10,
        }
    }
}

/// 日志配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    #[serde(default = "default_log_level")]
    pub level: String,
    #[serde(default = "default_log_format")]
    pub format: String,
    pub file: Option<String>,
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_log_format() -> String {
    "text".to_string()
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            format: "text".to_string(),
            file: None,
        }
    }
}

/// 主配置结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub service: ServiceConfig,
    #[serde(default)]
    pub redis: RedisConfig,
    #[serde(default)]
    pub influxdb: InfluxDBConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub points: PointStorageConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            service: ServiceConfig::default(),
            redis: RedisConfig::default(),
            influxdb: InfluxDBConfig::default(),
            logging: LoggingConfig::default(),
            points: PointStorageConfig::default(),
        }
    }
}

impl Config {
    /// 从配置文件和环境变量加载配置
    pub fn load() -> Result<Self> {
        let config_path = std::env::var("HISSRV_CONFIG").unwrap_or_else(|_| {
            "config/hissrv.yaml".to_string()
        });

        let mut figment = Figment::new();

        // 如果配置文件存在，加载它
        if Path::new(&config_path).exists() {
            figment = figment.merge(Yaml::file(&config_path));
        } else {
            tracing::warn!("配置文件 {} 不存在，使用默认配置", config_path);
        }

        // 环境变量覆盖，使用 HISSRV_ 前缀
        figment = figment.merge(Env::prefixed("HISSRV_").split("_"));

        let mut config: Config = figment.extract()?;
        
        // 加载点位配置
        Self::load_points_config(&mut config)?;
        
        Self::validate(&config)?;
        Ok(config)
    }

    /// 加载点位配置
    fn load_points_config(config: &mut Config) -> Result<()> {
        let points_config_path = std::env::var("HISSRV_POINTS_CONFIG").unwrap_or_else(|_| {
            "config/points.yaml".to_string()
        });

        if Path::new(&points_config_path).exists() {
            tracing::info!("加载点位配置文件: {}", points_config_path);
            config.points = PointStorageConfig::from_file(&points_config_path)?;
        } else {
            tracing::info!("点位配置文件 {} 不存在，使用默认配置", points_config_path);
        }

        Ok(())
    }

    /// 从指定文件加载配置
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let figment = Figment::new()
            .merge(Yaml::file(path))
            .merge(Env::prefixed("HISSRV_").split("_"));

        let mut config: Config = figment.extract()?;
        
        // 加载点位配置
        Self::load_points_config(&mut config)?;
        
        Self::validate(&config)?;
        Ok(config)
    }

    /// 验证配置的有效性
    fn validate(config: &Config) -> Result<()> {
        // 验证 Redis 配置
        if config.redis.connection.host.is_empty() {
            return Err(HisSrvError::config("Redis host 不能为空"));
        }

        // 验证 InfluxDB 配置
        if config.influxdb.enabled {
            if config.influxdb.url.is_empty() {
                return Err(HisSrvError::config("InfluxDB URL 不能为空"));
            }
            if config.influxdb.database.is_empty() {
                return Err(HisSrvError::config("InfluxDB database 不能为空"));
            }
        }

        // 验证服务配置
        if config.service.port == 0 {
            return Err(HisSrvError::config("服务端口必须大于 0"));
        }

        // 验证日志级别
        match config.logging.level.to_lowercase().as_str() {
            "trace" | "debug" | "info" | "warn" | "error" => {}
            _ => return Err(HisSrvError::config("无效的日志级别")),
        }

        // 验证点位配置
        config.points.validate()?;

        Ok(())
    }

    /// 获取 Redis URL
    pub fn redis_url(&self) -> String {
        let auth = if let Some(ref password) = self.redis.connection.password {
            format!(":{}@", password)
        } else {
            String::new()
        };

        format!(
            "redis://{}{}:{}/{}",
            auth, self.redis.connection.host, self.redis.connection.port, self.redis.connection.database
        )
    }

    /// 获取服务监听地址
    pub fn listen_addr(&self) -> String {
        format!("{}:{}", self.service.host, self.service.port)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Write;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.service.name, "hissrv");
        assert_eq!(config.redis.connection.host, "127.0.0.1");
        assert!(config.influxdb.enabled);
        assert!(config.points.enabled);
    }

    #[test]
    fn test_config_validation() {
        let mut config = Config::default();
        config.redis.connection.host = String::new();
        assert!(Config::validate(&config).is_err());
    }

    #[test]
    fn test_redis_url() {
        let config = Config::default();
        assert_eq!(config.redis_url(), "redis://127.0.0.1:6379/0");

        let mut config_with_auth = Config::default();
        config_with_auth.redis.connection.password = Some("secret".to_string());
        assert_eq!(config_with_auth.redis_url(), "redis://:secret@127.0.0.1:6379/0");
    }

    #[tokio::test]
    async fn test_load_from_file() {
        let yaml_content = r#"
service:
  name: "test-hissrv"
  version: "0.2.0"
  port: 9090
  host: "0.0.0.0"
redis:
  connection:
    host: "redis-server"
    port: 6380
    database: 0
  subscription:
    patterns:
      - "*:m:*"
      - "*:s:*"
influxdb:
  enabled: true
  url: "http://influx:8086"
  database: "test_db"
points:
  enabled: true
  default_policy: "allow_all"
"#;

        let mut temp_file = NamedTempFile::new().unwrap();
        write!(temp_file, "{}", yaml_content).unwrap();

        let config = Config::from_file(temp_file.path()).unwrap();
        assert_eq!(config.service.name, "test-hissrv");
        assert_eq!(config.service.port, 9090);
        assert_eq!(config.redis.connection.host, "redis-server");
        assert_eq!(config.redis.connection.port, 6380);
        assert_eq!(config.influxdb.url, "http://influx:8086");
        assert_eq!(config.influxdb.database, "test_db");
        assert!(config.points.enabled);
    }
}