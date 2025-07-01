use serde::{Deserialize, Serialize};
use std::any::Any;
use std::collections::HashMap;
use voltage_config::prelude::*;

#[derive(Debug, Serialize, Deserialize)]
struct ServiceConfig {
    service: ServiceInfo,
    redis: RedisConfig,
    channels: Vec<ChannelConfig>,
    monitoring: MonitoringConfig,
}

#[derive(Debug, Serialize, Deserialize)]
struct ServiceInfo {
    name: String,
    version: String,
    description: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct RedisConfig {
    url: String,
    prefix: String,
    pool_size: u32,
}

#[derive(Debug, Serialize, Deserialize)]
struct ChannelConfig {
    id: String,
    protocol: String,
    enabled: bool,
    parameters: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
struct MonitoringConfig {
    metrics_enabled: bool,
    metrics_port: u16,
    health_check_interval: u64,
}

impl Configurable for ServiceConfig {
    fn validate(&self) -> Result<()> {
        if self.service.name.is_empty() {
            return Err(ConfigError::Validation("Service name cannot be empty".into()));
        }
        
        if self.redis.pool_size == 0 {
            return Err(ConfigError::Validation("Redis pool size must be greater than 0".into()));
        }
        
        for channel in &self.channels {
            if channel.id.is_empty() {
                return Err(ConfigError::Validation("Channel ID cannot be empty".into()));
            }
        }
        
        Ok(())
    }
    
    fn as_any(&self) -> &dyn Any {
        self
    }
}

struct ProtocolValidator;

#[async_trait::async_trait]
impl ConfigValidator for ProtocolValidator {
    async fn validate(&self, config: &(dyn Any + Send + Sync)) -> Result<()> {
        if let Some(service_config) = config.downcast_ref::<ServiceConfig>() {
            for channel in &service_config.channels {
                match channel.protocol.as_str() {
                    "modbus" | "can" | "iec104" | "gpio" => {}
                    _ => {
                        return Err(ConfigError::Validation(format!(
                            "Unknown protocol: {}",
                            channel.protocol
                        )));
                    }
                }
            }
        }
        Ok(())
    }
    
    fn name(&self) -> &str {
        "ProtocolValidator"
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    
    let loader = ConfigLoaderBuilder::new()
        .base_path("config")
        .add_file("service.yml")
        .environment(Environment::from_env())
        .env_prefix("VOLTAGE")
        .add_validator(Box::new(ProtocolValidator))
        .add_validation_rule(
            "redis.url",
            Box::new(RegexRule::new(
                "redis_url",
                r"^redis://.*",
                "redis.url",
            )?),
        )
        .add_validation_rule(
            "monitoring.metrics_port",
            Box::new(RangeRule::new(
                "metrics_port_range",
                Some(1024),
                Some(65535),
                "monitoring.metrics_port",
            )),
        )
        .defaults(serde_json::json!({
            "service": {
                "name": "comsrv",
                "version": "1.0.0",
                "description": "Communication Service"
            },
            "redis": {
                "url": "redis://localhost:6379",
                "prefix": "voltage:",
                "pool_size": 10
            },
            "channels": [],
            "monitoring": {
                "metrics_enabled": true,
                "metrics_port": 9090,
                "health_check_interval": 30
            }
        }))?
        .build()?;
    
    let config: ServiceConfig = loader.load_async().await?;
    
    println!("Service Configuration:");
    println!("  Name: {}", config.service.name);
    println!("  Version: {}", config.service.version);
    println!("  Description: {}", config.service.description);
    println!("\nRedis Configuration:");
    println!("  URL: {}", config.redis.url);
    println!("  Prefix: {}", config.redis.prefix);
    println!("  Pool Size: {}", config.redis.pool_size);
    println!("\nMonitoring:");
    println!("  Metrics Enabled: {}", config.monitoring.metrics_enabled);
    println!("  Metrics Port: {}", config.monitoring.metrics_port);
    println!("\nChannels: {}", config.channels.len());
    
    for channel in &config.channels {
        println!("  - ID: {}, Protocol: {}, Enabled: {}", 
            channel.id, channel.protocol, channel.enabled);
    }
    
    Ok(())
}