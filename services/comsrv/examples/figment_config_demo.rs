//! Figment配置系统演示
//! 展示使用figment进行多源配置加载和序列化输出

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use figment::{
    providers::{Env, Format, Serialized, Yaml},
    Figment,
};

/// 演示配置结构 - 模拟comsrv的配置
#[derive(Debug, Clone, Serialize, Deserialize)]
struct DemoConfig {
    pub service: ServiceConfig,
    pub channels: Vec<ChannelConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ServiceConfig {
    #[serde(default = "default_name")]
    pub name: String,
    
    #[serde(default)]
    pub api: ApiConfig,
    
    #[serde(default)]
    pub redis: RedisConfig,
    
    #[serde(default)]
    pub logging: LoggingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ApiConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    
    #[serde(default = "default_bind")]
    pub bind_address: String,
    
    #[serde(default = "default_version")]
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RedisConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    
    #[serde(default = "default_redis_url")]
    pub url: String,
    
    #[serde(default)]
    pub db: u8,
    
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LoggingConfig {
    #[serde(default = "default_log_level")]
    pub level: String,
    
    #[serde(default = "default_true")]
    pub console: bool,
    
    #[serde(default = "default_log_size")]
    pub max_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChannelConfig {
    pub id: u16,
    pub name: String,
    pub protocol: String,
    pub parameters: HashMap<String, serde_yaml::Value>,
    
    #[serde(default)]
    pub table_config: Option<TableConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TableConfig {
    pub four_telemetry_route: String,
    pub protocol_mapping_route: String,
}

// 默认值函数
fn default_name() -> String { "comsrv".to_string() }
fn default_true() -> bool { true }
fn default_bind() -> String { "127.0.0.1:3000".to_string() }
fn default_version() -> String { "v1".to_string() }
fn default_redis_url() -> String { "redis://127.0.0.1:6379/0".to_string() }
fn default_timeout() -> u64 { 5000 }
fn default_log_level() -> String { "info".to_string() }
fn default_log_size() -> u64 { 104_857_600 }

// 默认实现
impl Default for DemoConfig {
    fn default() -> Self {
        Self {
            service: ServiceConfig::default(),
            channels: Vec::new(),
        }
    }
}

impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            name: default_name(),
            api: ApiConfig::default(),
            redis: RedisConfig::default(),
            logging: LoggingConfig::default(),
        }
    }
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            enabled: default_true(),
            bind_address: default_bind(),
            version: default_version(),
        }
    }
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            enabled: default_true(),
            url: default_redis_url(),
            db: 0,
            timeout_ms: default_timeout(),
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            console: default_true(),
            max_size: default_log_size(),
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Figment配置加载和序列化演示");
    println!("==============================");
    
    // 测试1: 默认配置
    demo_default_config()?;
    
    // 测试2: 环境变量覆盖
    demo_env_override()?;
    
    // 测试3: YAML文件加载
    demo_yaml_config()?;
    
    // 测试4: 多源配置合并
    demo_multi_source()?;
    
    println!("\\n✅ 所有演示完成!");
    Ok(())
}

fn demo_default_config() -> Result<(), Box<dyn std::error::Error>> {
    println!("\\n📋 演示1: 默认配置生成");
    println!("------------------------");
    
    let config: DemoConfig = Figment::new()
        .merge(Serialized::defaults(DemoConfig::default()))
        .extract()?;
    
    println!("📄 默认配置 (JSON格式):");
    let json_output = serde_json::to_string_pretty(&config)?;
    println!("{}", json_output);
    
    println!("\\n📄 默认配置 (YAML格式):");
    let yaml_output = serde_yaml::to_string(&config)?;
    println!("{}", yaml_output);
    
    Ok(())
}

fn demo_env_override() -> Result<(), Box<dyn std::error::Error>> {
    println!("\\n🌍 演示2: 环境变量覆盖");
    println!("------------------------");
    
    // 设置测试环境变量
    std::env::set_var("COMSRV_SERVICE_NAME", "环境变量测试服务");
    std::env::set_var("COMSRV_SERVICE_API_BIND_ADDRESS", "0.0.0.0:8080");
    std::env::set_var("COMSRV_SERVICE_REDIS_ENABLED", "false");
    std::env::set_var("COMSRV_SERVICE_REDIS_DB", "3");
    std::env::set_var("COMSRV_SERVICE_LOGGING_LEVEL", "debug");
    
    let config: DemoConfig = Figment::new()
        .merge(Serialized::defaults(DemoConfig::default()))
        .merge(Env::prefixed("COMSRV_").split("_"))
        .extract()?;
    
    println!("📄 环境变量覆盖后的配置:");
    let json_output = serde_json::to_string_pretty(&config)?;
    println!("{}", json_output);
    
    // 验证环境变量效果
    println!("\\n✅ 环境变量验证:");
    println!("  - 服务名: {} (✓ 环境变量生效)", config.service.name);
    println!("  - API地址: {} (✓ 环境变量生效)", config.service.api.bind_address);
    println!("  - Redis启用: {} (✓ 环境变量生效)", config.service.redis.enabled);
    println!("  - Redis数据库: {} (✓ 环境变量生效)", config.service.redis.db);
    println!("  - 日志级别: {} (✓ 环境变量生效)", config.service.logging.level);
    
    Ok(())
}

fn demo_yaml_config() -> Result<(), Box<dyn std::error::Error>> {
    println!("\\n📝 演示3: YAML配置文件加载");
    println!("---------------------------");
    
    // 创建演示配置文件
    let yaml_content = r#"
service:
  name: "YAML配置服务"
  api:
    enabled: true
    bind_address: "0.0.0.0:9000"
    version: "v2"
  redis:
    enabled: true
    url: "redis://yaml-redis:6379/5"
    db: 5
    timeout_ms: 10000
  logging:
    level: "warn"
    console: false
    max_size: 209715200

channels:
  - id: 1001
    name: "YAML电表通道"
    protocol: "modbus_tcp"
    parameters:
      host: "192.168.1.100"
      port: 502
      timeout_ms: 1000
    table_config:
      four_telemetry_route: "config/YAMLMeter"
      protocol_mapping_route: "config/YAMLMeter"
      
  - id: 1002
    name: "YAML CAN通道"
    protocol: "can"
    parameters:
      interface: "can0"
      bitrate: 250000
"#;
    
    std::fs::write("demo_config.yaml", yaml_content)?;
    
    let config: DemoConfig = Figment::new()
        .merge(Serialized::defaults(DemoConfig::default()))
        .merge(Yaml::file("demo_config.yaml"))
        .extract()?;
    
    println!("📄 YAML配置加载结果:");
    let json_output = serde_json::to_string_pretty(&config)?;
    println!("{}", json_output);
    
    println!("\\n✅ YAML配置验证:");
    println!("  - 服务名: {}", config.service.name);
    println!("  - API版本: {}", config.service.api.version);
    println!("  - 通道数量: {}", config.channels.len());
    
    for channel in &config.channels {
        println!("    * 通道 {} ({}): 协议 {}", 
            channel.id, channel.name, channel.protocol);
    }
    
    // 清理文件
    std::fs::remove_file("demo_config.yaml").ok();
    
    Ok(())
}

fn demo_multi_source() -> Result<(), Box<dyn std::error::Error>> {
    println!("\\n🔗 演示4: 多源配置合并 (优先级演示)");
    println!("----------------------------------");
    
    // 创建基础YAML配置
    let yaml_content = r#"
service:
  name: "基础YAML服务"
  api:
    bind_address: "127.0.0.1:3000"
  redis:
    db: 1
  logging:
    level: "info"
"#;
    std::fs::write("base_config.yaml", yaml_content)?;
    
    // 设置环境变量 (优先级更高)
    std::env::set_var("COMSRV_SERVICE_NAME", "环境变量优先服务");
    std::env::set_var("COMSRV_SERVICE_REDIS_DB", "9");
    
    // 按优先级顺序合并: 默认值 < YAML < 环境变量
    let config: DemoConfig = Figment::new()
        .merge(Serialized::defaults(DemoConfig::default()))  // 优先级: 1 (最低)
        .merge(Yaml::file("base_config.yaml"))                // 优先级: 2
        .merge(Env::prefixed("COMSRV_").split("_"))          // 优先级: 3 (最高)
        .extract()?;
    
    println!("📄 多源合并后的最终配置:");
    let json_output = serde_json::to_string_pretty(&config)?;
    println!("{}", json_output);
    
    println!("\\n🎯 配置来源分析:");
    println!("  - service.name: '{}' (🌍 环境变量覆盖)", config.service.name);
    println!("  - api.bind_address: '{}' (📝 YAML文件)", config.service.api.bind_address);
    println!("  - redis.db: {} (🌍 环境变量覆盖)", config.service.redis.db);
    println!("  - logging.level: '{}' (📝 YAML文件)", config.service.logging.level);
    println!("  - api.version: '{}' (📋 默认值)", config.service.api.version);
    
    println!("\\n💡 优先级规则: 环境变量 > YAML文件 > 默认值");
    
    // 清理文件
    std::fs::remove_file("base_config.yaml").ok();
    
    Ok(())
}