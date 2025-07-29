//! 基础配置模块

use serde::{Deserialize, Serialize};

/// 基础服务配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServiceConfig {
    /// 服务名称
    pub name: String,
    /// 服务版本
    pub version: String,
    /// 服务地址
    pub host: String,
    /// 服务端口
    pub port: u16,
    /// 环境
    pub environment: Environment,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Environment {
    Development,
    Staging,
    Production,
}

impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            name: "unknown".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            host: "0.0.0.0".to_string(),
            port: 8080,
            environment: Environment::Development,
        }
    }
}

/// Redis 配置
#[cfg(feature = "redis")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    pub host: String,
    pub port: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    #[serde(default)]
    pub database: u8,
    #[serde(default = "default_pool_size")]
    pub pool_size: u32,
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
}

#[cfg(feature = "redis")]
impl RedisConfig {
    pub fn to_url(&self) -> String {
        if let Some(ref password) = self.password {
            format!(
                "redis://:{}@{}:{}/{}",
                password, self.host, self.port, self.database
            )
        } else {
            format!("redis://{}:{}/{}", self.host, self.port, self.database)
        }
    }
}

#[cfg(feature = "redis")]
impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            host: "127.0.0.1".to_string(),
            port: 6379,
            password: None,
            database: 0,
            pool_size: default_pool_size(),
            timeout_seconds: default_timeout(),
        }
    }
}

/// `InfluxDB` 2.x 配置
#[cfg(feature = "influxdb")]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InfluxConfig {
    pub url: String,
    pub org: String,
    pub bucket: String,
    pub token: String,
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,

    // 向下兼容字段
    #[serde(skip_serializing_if = "Option::is_none")]
    pub database: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
}

#[cfg(feature = "influxdb")]
impl InfluxConfig {
    /// 向下兼容：如果没有设置bucket，使用database字段
    pub fn get_bucket(&self) -> &str {
        if let Some(ref database) = self.database {
            database
        } else {
            &self.bucket
        }
    }
}

fn default_pool_size() -> u32 {
    10
}

fn default_timeout() -> u64 {
    30
}
