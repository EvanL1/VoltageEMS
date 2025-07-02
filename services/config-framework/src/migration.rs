use std::path::{Path, PathBuf};
use std::fs;
use serde::{Serialize, Deserialize};
use serde_json::Value;
use crate::{Result, ConfigError};

/// Configuration migration helper for transitioning services to the unified config framework
pub struct ConfigMigrator {
    source_path: PathBuf,
    target_path: PathBuf,
}

impl ConfigMigrator {
    pub fn new<P: AsRef<Path>>(source_path: P, target_path: P) -> Self {
        Self {
            source_path: source_path.as_ref().to_path_buf(),
            target_path: target_path.as_ref().to_path_buf(),
        }
    }
    
    /// Migrate configuration from old format to new format
    pub fn migrate<S, T>(&self, transformer: impl Fn(S) -> Result<T>) -> Result<()>
    where
        S: for<'de> Deserialize<'de>,
        T: Serialize,
    {
        // Read source configuration
        let source_content = fs::read_to_string(&self.source_path)
            .map_err(|e| ConfigError::Io(e))?;
        
        // Parse based on file extension
        let source_value: S = match self.source_path.extension().and_then(|e| e.to_str()) {
            Some("yaml") | Some("yml") => {
                serde_yaml::from_str(&source_content)
                    .map_err(|e| ConfigError::Parse(e.to_string()))?
            }
            Some("toml") => {
                toml::from_str(&source_content)
                    .map_err(|e| ConfigError::Parse(e.to_string()))?
            }
            Some("json") => {
                serde_json::from_str(&source_content)
                    .map_err(|e| ConfigError::Parse(e.to_string()))?
            }
            _ => return Err(ConfigError::Parse("Unsupported file format".into())),
        };
        
        // Transform to new format
        let target_config = transformer(source_value)?;
        
        // Write to target file
        let target_content = match self.target_path.extension().and_then(|e| e.to_str()) {
            Some("yaml") | Some("yml") => {
                serde_yaml::to_string(&target_config)
                    .map_err(|e| ConfigError::Parse(e.to_string()))?
            }
            Some("toml") => {
                toml::to_string_pretty(&target_config)
                    .map_err(|e| ConfigError::Parse(e.to_string()))?
            }
            Some("json") => {
                serde_json::to_string_pretty(&target_config)
                    .map_err(|e| ConfigError::Parse(e.to_string()))?
            }
            _ => return Err(ConfigError::Parse("Unsupported file format".into())),
        };
        
        // Create target directory if it doesn't exist
        if let Some(parent) = self.target_path.parent() {
            fs::create_dir_all(parent).map_err(|e| ConfigError::Io(e))?;
        }
        
        fs::write(&self.target_path, target_content)
            .map_err(|e| ConfigError::Io(e))?;
        
        Ok(())
    }
    
    /// Create a backup of the original configuration
    pub fn backup(&self) -> Result<PathBuf> {
        let backup_path = self.source_path.with_extension(
            format!("{}.backup", 
                self.source_path.extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("conf")
            )
        );
        
        fs::copy(&self.source_path, &backup_path)
            .map_err(|e| ConfigError::Io(e))?;
        
        Ok(backup_path)
    }
}

/// Helper functions for common migration patterns
pub mod transformers {
    use super::*;
    use crate::base::{BaseServiceConfig, ServiceInfo, RedisConfig, LoggingConfig, MonitoringConfig};
    use std::collections::HashMap;
    
    /// Transform environment variables to BaseServiceConfig
    pub fn env_to_base_config(env_map: HashMap<String, String>) -> Result<BaseServiceConfig> {
        Ok(BaseServiceConfig {
            service: ServiceInfo {
                name: env_map.get("SERVICE_NAME")
                    .cloned()
                    .unwrap_or_else(|| "unknown".to_string()),
                version: env_map.get("SERVICE_VERSION")
                    .cloned()
                    .unwrap_or_else(|| "1.0.0".to_string()),
                description: env_map.get("SERVICE_DESCRIPTION")
                    .cloned()
                    .unwrap_or_default(),
                instance_id: env_map.get("INSTANCE_ID")
                    .cloned()
                    .unwrap_or_default(),
            },
            redis: RedisConfig {
                url: env_map.get("REDIS_URL")
                    .cloned()
                    .unwrap_or_else(|| "redis://localhost:6379".to_string()),
                prefix: env_map.get("REDIS_PREFIX")
                    .cloned()
                    .unwrap_or_else(|| "voltage:".to_string()),
                pool_size: env_map.get("REDIS_POOL_SIZE")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(10),
                database: env_map.get("REDIS_DATABASE")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0),
                password: env_map.get("REDIS_PASSWORD").cloned(),
            },
            logging: LoggingConfig {
                level: env_map.get("LOG_LEVEL")
                    .cloned()
                    .unwrap_or_else(|| "info".to_string()),
                console: env_map.get("LOG_CONSOLE")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(true),
                file: None,
                json_format: env_map.get("LOG_JSON")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(false),
            },
            monitoring: MonitoringConfig {
                metrics_enabled: env_map.get("METRICS_ENABLED")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(true),
                metrics_port: env_map.get("METRICS_PORT")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(9090),
                health_check_enabled: env_map.get("HEALTH_CHECK_ENABLED")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(true),
                health_check_port: env_map.get("HEALTH_CHECK_PORT")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(8080),
                health_check_interval: env_map.get("HEALTH_CHECK_INTERVAL")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(30),
            },
        })
    }
    
    /// Merge old config format with base config
    pub fn merge_with_base(base: BaseServiceConfig, service_config: Value) -> Result<Value> {
        let mut result = serde_json::to_value(base)?;
        
        if let (Some(result_obj), Some(service_obj)) = (result.as_object_mut(), service_config.as_object()) {
            for (key, value) in service_obj {
                // Don't override base config fields
                if !["service", "redis", "logging", "monitoring"].contains(&key.as_str()) {
                    result_obj.insert(key.clone(), value.clone());
                }
            }
        }
        
        Ok(result)
    }
    
    /// Extract Redis config from various old formats
    pub fn extract_redis_config(old_config: &Value) -> Option<RedisConfig> {
        // Try different common patterns
        if let Some(redis) = old_config.get("redis") {
            return serde_json::from_value(redis.clone()).ok();
        }
        
        // Try extracting from flat structure
        if let Some(obj) = old_config.as_object() {
            let url = obj.get("redis_url")
                .or_else(|| obj.get("redis_host").map(|host| {
                    let port = obj.get("redis_port")
                        .and_then(|p| p.as_u64())
                        .unwrap_or(6379);
                    Value::String(format!("redis://{}:{}", host.as_str().unwrap_or("localhost"), port))
                }))
                .and_then(|v| v.as_str())
                .map(String::from)?;
            
            return Some(RedisConfig {
                url,
                prefix: obj.get("redis_prefix")
                    .and_then(|v| v.as_str())
                    .map(String::from)
                    .unwrap_or_else(|| "voltage:".to_string()),
                pool_size: obj.get("redis_pool_size")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(10) as u32,
                database: obj.get("redis_db")
                    .or_else(|| obj.get("redis_database"))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32,
                password: obj.get("redis_password")
                    .and_then(|v| v.as_str())
                    .map(String::from),
            });
        }
        
        None
    }
}

/// Configuration validation helper
pub struct ConfigValidator {
    warnings: Vec<String>,
    errors: Vec<String>,
}

impl ConfigValidator {
    pub fn new() -> Self {
        Self {
            warnings: Vec::new(),
            errors: Vec::new(),
        }
    }
    
    /// Validate migrated configuration
    pub fn validate<T: crate::Configurable>(&mut self, config: &T) -> bool {
        match config.validate() {
            Ok(_) => true,
            Err(e) => {
                self.errors.push(format!("Validation error: {}", e));
                false
            }
        }
    }
    
    /// Add a warning
    pub fn warn(&mut self, message: impl Into<String>) {
        self.warnings.push(message.into());
    }
    
    /// Add an error
    pub fn error(&mut self, message: impl Into<String>) {
        self.errors.push(message.into());
    }
    
    /// Get validation results
    pub fn results(&self) -> ValidationResults {
        ValidationResults {
            is_valid: self.errors.is_empty(),
            warnings: self.warnings.clone(),
            errors: self.errors.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ValidationResults {
    pub is_valid: bool,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

impl ValidationResults {
    pub fn print_summary(&self) {
        if self.is_valid {
            println!("✅ Configuration is valid");
        } else {
            println!("❌ Configuration has errors");
        }
        
        if !self.warnings.is_empty() {
            println!("\nWarnings:");
            for warning in &self.warnings {
                println!("  ⚠️  {}", warning);
            }
        }
        
        if !self.errors.is_empty() {
            println!("\nErrors:");
            for error in &self.errors {
                println!("  ❌ {}", error);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::transformers::*;
    
    #[test]
    fn test_env_to_base_config() {
        let mut env_map = HashMap::new();
        env_map.insert("SERVICE_NAME".to_string(), "test-service".to_string());
        env_map.insert("REDIS_URL".to_string(), "redis://redis:6379".to_string());
        env_map.insert("LOG_LEVEL".to_string(), "debug".to_string());
        env_map.insert("METRICS_PORT".to_string(), "9999".to_string());
        
        let config = env_to_base_config(env_map).unwrap();
        assert_eq!(config.service.name, "test-service");
        assert_eq!(config.redis.url, "redis://redis:6379");
        assert_eq!(config.logging.level, "debug");
        assert_eq!(config.monitoring.metrics_port, 9999);
    }
    
    #[test]
    fn test_extract_redis_config() {
        let old_config = serde_json::json!({
            "redis_host": "redis-server",
            "redis_port": 6380,
            "redis_prefix": "myapp:",
            "redis_password": "secret"
        });
        
        let redis_config = extract_redis_config(&old_config).unwrap();
        assert_eq!(redis_config.url, "redis://redis-server:6380");
        assert_eq!(redis_config.prefix, "myapp:");
        assert_eq!(redis_config.password, Some("secret".to_string()));
    }
}