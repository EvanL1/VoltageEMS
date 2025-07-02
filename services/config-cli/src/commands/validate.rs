use anyhow::{Context, Result};
use colored::Colorize;
use std::path::Path;
use voltage_config::prelude::*;

/// Execute the validate command
pub async fn execute(service: &str, file: &Path, show_warnings: bool) -> Result<()> {
    println!("🔍 {} configuration for service: {}", "Validating".bold(), service.cyan());
    println!("📄 File: {}", file.display());
    println!();

    // Load the configuration file
    let loader = ConfigLoaderBuilder::new()
        .add_file(file)
        .build()
        .context("Failed to create configuration loader")?;

    // Validate based on service type
    let validation_result = match service {
        "alarmsrv" => validate_service::<AlarmServiceConfig>(&loader).await,
        "hissrv" => validate_service::<HisServiceConfig>(&loader).await,
        "comsrv" => validate_service::<ComServiceConfig>(&loader).await,
        "modsrv" => validate_service::<ModServiceConfig>(&loader).await,
        "netsrv" => validate_service::<NetServiceConfig>(&loader).await,
        _ => anyhow::bail!("Unknown service: {}. Valid services are: alarmsrv, hissrv, comsrv, modsrv, netsrv", service),
    };

    // Display results
    match validation_result {
        Ok((warnings, info)) => {
            println!("{} Configuration is valid!", "✅".green());
            
            if show_warnings && !warnings.is_empty() {
                println!();
                println!("{}", "⚠️  Warnings:".yellow());
                for warning in warnings {
                    println!("   • {}", warning);
                }
            }
            
            if !info.is_empty() {
                println!();
                println!("{}", "ℹ️  Information:".blue());
                for item in info {
                    println!("   • {}", item);
                }
            }
            
            Ok(())
        }
        Err(errors) => {
            println!("{} Configuration validation failed!", "❌".red());
            println!();
            println!("{}", "Errors:".red().bold());
            for error in errors {
                println!("   • {}", error);
            }
            anyhow::bail!("Configuration validation failed")
        }
    }
}

/// Validate a specific service configuration
async fn validate_service<T>(loader: &ConfigLoader) -> Result<(Vec<String>, Vec<String>), Vec<String>>
where
    T: ServiceConfig + for<'de> serde::Deserialize<'de> + 'static,
{
    let mut warnings = Vec::new();
    let mut info = Vec::new();
    let mut errors = Vec::new();

    // Try to load and validate the configuration
    match loader.load::<T>() {
        Ok(config) => {
            // Validate base configuration
            if let Err(e) = config.base().validate() {
                errors.push(format!("Base configuration error: {}", e));
            }
            
            // Validate service-specific configuration
            if let Err(e) = config.validate() {
                errors.push(format!("Service configuration error: {}", e));
            }
            
            // Validate complete configuration
            if let Err(e) = config.validate_all() {
                errors.push(format!("Complete configuration error: {}", e));
            }
            
            // Add service-specific checks and warnings
            check_service_specifics(&config, &mut warnings, &mut info);
            
            if errors.is_empty() {
                Ok((warnings, info))
            } else {
                Err(errors)
            }
        }
        Err(e) => {
            errors.push(format!("Failed to load configuration: {}", e));
            Err(errors)
        }
    }
}

/// Check service-specific configuration details
fn check_service_specifics<T: ServiceConfig>(config: &T, warnings: &mut Vec<String>, info: &mut Vec<String>) {
    let base = config.base();
    
    // Check Redis configuration
    if base.redis.pool_size < 10 {
        warnings.push("Redis pool size is less than 10, which might impact performance".to_string());
    }
    
    if base.redis.password.is_none() {
        warnings.push("Redis password is not set. Consider setting it for production".to_string());
    }
    
    // Check logging configuration
    if base.logging.level == "trace" || base.logging.level == "debug" {
        warnings.push(format!("Log level is set to '{}', which may impact performance", base.logging.level));
    }
    
    if base.logging.file.is_none() && !base.logging.console {
        warnings.push("No logging output configured (neither file nor console)".to_string());
    }
    
    // Check monitoring configuration
    if !base.monitoring.metrics_enabled {
        info.push("Metrics are disabled. Consider enabling for production monitoring".to_string());
    }
    
    if !base.monitoring.health_check_enabled {
        info.push("Health checks are disabled. Consider enabling for service monitoring".to_string());
    }
    
    // Add general information
    info.push(format!("Service: {} v{}", base.service.name, base.service.version));
    info.push(format!("Redis URL: {}", base.redis.url));
    info.push(format!("Metrics port: {}", base.monitoring.metrics_port));
}

// Service configuration types (these would be imported from actual service modules)
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct AlarmServiceConfig {
    #[serde(flatten)]
    base: BaseServiceConfig,
}

impl Configurable for AlarmServiceConfig {
    fn validate(&self) -> voltage_config::Result<()> { Ok(()) }
    fn as_any(&self) -> &dyn std::any::Any { self }
}

impl ServiceConfig for AlarmServiceConfig {
    fn base(&self) -> &BaseServiceConfig { &self.base }
    fn base_mut(&mut self) -> &mut BaseServiceConfig { &mut self.base }
}

#[derive(Debug, Serialize, Deserialize)]
struct HisServiceConfig {
    #[serde(flatten)]
    base: BaseServiceConfig,
}

impl Configurable for HisServiceConfig {
    fn validate(&self) -> voltage_config::Result<()> { Ok(()) }
    fn as_any(&self) -> &dyn std::any::Any { self }
}

impl ServiceConfig for HisServiceConfig {
    fn base(&self) -> &BaseServiceConfig { &self.base }
    fn base_mut(&mut self) -> &mut BaseServiceConfig { &mut self.base }
}

#[derive(Debug, Serialize, Deserialize)]
struct ComServiceConfig {
    #[serde(flatten)]
    base: BaseServiceConfig,
}

impl Configurable for ComServiceConfig {
    fn validate(&self) -> voltage_config::Result<()> { Ok(()) }
    fn as_any(&self) -> &dyn std::any::Any { self }
}

impl ServiceConfig for ComServiceConfig {
    fn base(&self) -> &BaseServiceConfig { &self.base }
    fn base_mut(&mut self) -> &mut BaseServiceConfig { &mut self.base }
}

#[derive(Debug, Serialize, Deserialize)]
struct ModServiceConfig {
    #[serde(flatten)]
    base: BaseServiceConfig,
}

impl Configurable for ModServiceConfig {
    fn validate(&self) -> voltage_config::Result<()> { Ok(()) }
    fn as_any(&self) -> &dyn std::any::Any { self }
}

impl ServiceConfig for ModServiceConfig {
    fn base(&self) -> &BaseServiceConfig { &self.base }
    fn base_mut(&mut self) -> &mut BaseServiceConfig { &mut self.base }
}

#[derive(Debug, Serialize, Deserialize)]
struct NetServiceConfig {
    #[serde(flatten)]
    base: BaseServiceConfig,
}

impl Configurable for NetServiceConfig {
    fn validate(&self) -> voltage_config::Result<()> { Ok(()) }
    fn as_any(&self) -> &dyn std::any::Any { self }
}

impl ServiceConfig for NetServiceConfig {
    fn base(&self) -> &BaseServiceConfig { &self.base }
    fn base_mut(&mut self) -> &mut BaseServiceConfig { &mut self.base }
}