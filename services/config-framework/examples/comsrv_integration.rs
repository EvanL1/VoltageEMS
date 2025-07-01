use serde::{Deserialize, Serialize};
use std::any::Any;
use std::collections::HashMap;
use voltage_config::prelude::*;

#[derive(Debug, Serialize, Deserialize)]
struct ComsrvConfig {
    service: ServiceInfo,
    redis: RedisConfig,
    channels: Vec<ChannelConfig>,
    point_map: PointMapConfig,
    telemetry: TelemetryConfig,
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
    command_timeout_ms: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct ChannelConfig {
    id: String,
    protocol: String,
    enabled: bool,
    parameters: HashMap<String, serde_json::Value>,
    point_table_path: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PointMapConfig {
    telemetry_csv: String,
    control_csv: String,
    adjustment_csv: String,
    signal_csv: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct TelemetryConfig {
    metrics_enabled: bool,
    metrics_port: u16,
    log_level: String,
}

impl Configurable for ComsrvConfig {
    fn validate(&self) -> Result<()> {
        if self.service.name.is_empty() {
            return Err(ConfigError::Validation("Service name cannot be empty".into()));
        }
        
        if !self.redis.url.starts_with("redis://") {
            return Err(ConfigError::Validation("Redis URL must start with redis://".into()));
        }
        
        if self.redis.pool_size == 0 {
            return Err(ConfigError::Validation("Redis pool size must be greater than 0".into()));
        }
        
        for channel in &self.channels {
            if channel.id.is_empty() {
                return Err(ConfigError::Validation("Channel ID cannot be empty".into()));
            }
            
            match channel.protocol.as_str() {
                "modbus_tcp" | "modbus_rtu" | "can" | "iec104" | "gpio" => {}
                _ => {
                    return Err(ConfigError::Validation(format!(
                        "Unknown protocol: {}",
                        channel.protocol
                    )));
                }
            }
        }
        
        Ok(())
    }
    
    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    
    let config_dir = std::env::current_dir()?.join("config");
    
    let loader = ConfigLoaderBuilder::new()
        .base_path(&config_dir)
        .add_file("comsrv.yml")
        .add_file(config_dir.join("channels.yml"))
        .environment(Environment::from_env())
        .env_prefix("COMSRV")
        .add_validation_rule(
            "redis.command_timeout_ms",
            Box::new(RangeRule::new(
                "redis_timeout",
                Some(100),
                Some(60000),
                "redis.command_timeout_ms",
            )),
        )
        .add_validation_rule(
            "telemetry.metrics_port",
            Box::new(RangeRule::new(
                "metrics_port",
                Some(1024),
                Some(65535),
                "telemetry.metrics_port",
            )),
        )
        .defaults(serde_json::json!({
            "service": {
                "name": "comsrv",
                "version": "1.0.0",
                "description": "Industrial Communication Service"
            },
            "redis": {
                "url": "redis://localhost:6379",
                "prefix": "voltage:",
                "pool_size": 10,
                "command_timeout_ms": 5000
            },
            "channels": [],
            "point_map": {
                "telemetry_csv": "config/Modbus_Test_1/telemetry.csv",
                "control_csv": "config/Modbus_Test_1/control.csv",
                "adjustment_csv": "config/Modbus_Test_1/adjustment.csv",
                "signal_csv": "config/Modbus_Test_1/signal.csv"
            },
            "telemetry": {
                "metrics_enabled": true,
                "metrics_port": 9090,
                "log_level": "info"
            }
        }))?
        .build()?;
    
    let config: ComsrvConfig = loader.load_async().await?;
    
    println!("=== Comsrv Configuration ===");
    println!("Service: {} v{}", config.service.name, config.service.version);
    println!("Description: {}", config.service.description);
    println!("\nRedis:");
    println!("  URL: {}", config.redis.url);
    println!("  Prefix: {}", config.redis.prefix);
    println!("  Pool Size: {}", config.redis.pool_size);
    println!("  Command Timeout: {}ms", config.redis.command_timeout_ms);
    println!("\nTelemetry:");
    println!("  Metrics Enabled: {}", config.telemetry.metrics_enabled);
    println!("  Metrics Port: {}", config.telemetry.metrics_port);
    println!("  Log Level: {}", config.telemetry.log_level);
    println!("\nPoint Tables:");
    println!("  Telemetry: {}", config.point_map.telemetry_csv);
    println!("  Control: {}", config.point_map.control_csv);
    println!("  Adjustment: {}", config.point_map.adjustment_csv);
    println!("  Signal: {}", config.point_map.signal_csv);
    println!("\nChannels: {}", config.channels.len());
    
    for channel in &config.channels {
        println!("  - Channel ID: {}", channel.id);
        println!("    Protocol: {}", channel.protocol);
        println!("    Enabled: {}", channel.enabled);
        if let Some(path) = &channel.point_table_path {
            println!("    Point Table: {}", path);
        }
    }
    
    let watcher = ConfigWatcher::new(loader, vec![config_dir])
        .with_interval(std::time::Duration::from_secs(5));
    
    watcher.start().await?;
    
    println!("\nWatching for configuration changes (press Ctrl+C to exit)...");
    
    while let Some(event) = watcher.wait_for_change().await {
        match event {
            WatchEvent::Modified(path) => {
                println!("\nConfiguration file modified: {}", path.display());
                match watcher.reload::<ComsrvConfig>().await {
                    Ok(new_config) => {
                        println!("Reloaded configuration successfully");
                        println!("Active channels: {}", new_config.channels.len());
                    }
                    Err(e) => eprintln!("Failed to reload configuration: {}", e),
                }
            }
            _ => {}
        }
    }
    
    Ok(())
}