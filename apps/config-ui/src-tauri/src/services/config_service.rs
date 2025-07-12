use crate::error::Result;
use crate::models::*;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct ConfigService {
    redis_client: Option<Arc<RwLock<redis::aio::MultiplexedConnection>>>,
    services: Vec<&'static str>,
}

impl ConfigService {
    pub async fn new(_redis_url: &str) -> Result<Self> {
        // 暂时不连接 Redis，使用模拟数据
        // let client = redis::Client::open(redis_url)?;
        // let con = client.get_multiplexed_tokio_connection().await?;
        
        Ok(Self {
            redis_client: None,
            services: vec!["comsrv", "modsrv", "hissrv", "netsrv", "alarmsrv", "rulesrv"],
        })
    }
    
    pub async fn get_all_services(&self) -> Result<Vec<ServiceInfo>> {
        let mut result = Vec::new();
        
        for service_name in &self.services {
            match self.get_service_info(service_name).await {
                Ok(info) => result.push(info),
                Err(_) => {
                    // 如果获取失败，创建一个默认的服务信息
                    result.push(ServiceInfo {
                        name: service_name.to_string(),
                        version: "unknown".to_string(),
                        status: ServiceStatus::Unknown,
                        uptime: "0s".to_string(),
                        memory: "0MB".to_string(),
                        connections: 0,
                    });
                }
            }
        }
        
        Ok(result)
    }
    
    pub async fn get_service_info(&self, service_name: &str) -> Result<ServiceInfo> {
        // 暂时返回模拟数据
        let status = if service_name == "comsrv" || service_name == "modsrv" {
            ServiceStatus::Running
        } else {
            ServiceStatus::Stopped
        };
        
        Ok(ServiceInfo {
            name: service_name.to_string(),
            version: "0.1.0".to_string(),
            status,
            uptime: "1h 23m".to_string(),
            memory: "45.2MB".to_string(),
            connections: 12,
        })
    }
    
    pub async fn get_service_config(&self, service_name: &str) -> Result<ServiceConfig> {
        // 从文件系统读取配置
        let _config_path = format!("../../services/{}/config/default.yml", service_name);
        
        // TODO: 实际实现配置读取
        // 这里返回一个示例配置
        Ok(ServiceConfig {
            name: service_name.to_string(),
            version: "0.1.0".to_string(),
            redis: RedisConfig {
                url: "redis://localhost:6379".to_string(),
                prefix: format!("voltage:{}:", service_name),
                pool_size: 10,
            },
            channels: None,
            logging: LoggingConfig {
                level: "info".to_string(),
                file: format!("logs/{}.log", service_name),
                rotation: "daily".to_string(),
            },
        })
    }
    
    pub async fn update_service_config(
        &self,
        service_name: &str,
        config: serde_json::Value,
    ) -> Result<()> {
        // 验证配置
        self.validate_config(service_name, &config).await?;
        
        // TODO: 保存配置到文件系统
        
        // TODO: 通知服务重新加载配置
        // if let Some(redis_client) = &self.redis_client {
        //     let mut con = redis_client.write().await;
        //     let channel = format!("config:{}:update", service_name);
        //     con.publish(channel, "reload").await?;
        // }
        
        Ok(())
    }
    
    pub async fn validate_config(
        &self,
        _service_name: &str,
        config: &serde_json::Value,
    ) -> Result<ValidationResult> {
        let mut errors = Vec::new();
        let warnings = Vec::new();
        
        // 基本验证
        if config.get("redis").is_none() {
            errors.push(ValidationError {
                field: "redis".to_string(),
                message: "Redis configuration is required".to_string(),
            });
        }
        
        if let Some(redis) = config.get("redis") {
            if redis.get("url").is_none() {
                errors.push(ValidationError {
                    field: "redis.url".to_string(),
                    message: "Redis URL is required".to_string(),
                });
            }
        }
        
        Ok(ValidationResult {
            valid: errors.is_empty(),
            errors,
            warnings,
        })
    }
    
    pub async fn get_config_diff(
        &self,
        _service_name: &str,
        _version1: &str,
        _version2: &str,
    ) -> Result<DiffResult> {
        // TODO: 实现配置版本对比
        Ok(DiffResult {
            added: vec![],
            removed: vec![],
            modified: vec![],
        })
    }
}