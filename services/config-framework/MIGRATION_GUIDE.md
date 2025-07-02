# VoltageEMS Configuration Migration Guide

This guide helps you migrate VoltageEMS services from their current configuration approach to the unified `voltage-config` framework.

## Overview

The `voltage-config` framework provides:
- ✅ Unified configuration management across all services
- ✅ Type-safe configuration with validation
- ✅ Multi-format support (YAML, TOML, JSON, ENV)
- ✅ Configuration hot-reloading
- ✅ Hierarchical configuration with environment overrides

## Migration Status

| Service | Current Approach | Migration Status | Notes |
|---------|-----------------|------------------|-------|
| alarmsrv | Environment variables | ✅ Complete | Example implementation available |
| hissrv | YAML + CLI args | ✅ Complete | Example implementation available |
| modsrv | Custom file parsing | 🔄 Pending | |
| netsrv | config crate | 🔄 Pending | |
| comsrv | Figment | 🔄 Pending | Most complex migration |

## Step-by-Step Migration

### 1. Add Dependency

Add `voltage-config` to your service's `Cargo.toml`:

```toml
[dependencies]
voltage-config = { path = "../config-framework" }
```

### 2. Create New Configuration Structure

Create a new configuration module (`config_new.rs`) that extends `BaseServiceConfig`:

```rust
use serde::{Deserialize, Serialize};
use std::any::Any;
use voltage_config::prelude::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MyServiceConfig {
    #[serde(flatten)]
    pub base: BaseServiceConfig,
    
    // Add your service-specific fields here
    pub my_service: MyServiceSpecificConfig,
}

impl Configurable for MyServiceConfig {
    fn validate(&self) -> voltage_config::Result<()> {
        // Add service-specific validation
        Ok(())
    }
    
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl ServiceConfig for MyServiceConfig {
    fn base(&self) -> &BaseServiceConfig {
        &self.base
    }
    
    fn base_mut(&mut self) -> &mut BaseServiceConfig {
        &mut self.base
    }
}
```

### 3. Implement Configuration Loading

Add a `load()` method to your configuration:

```rust
impl MyServiceConfig {
    pub async fn load() -> Result<Self> {
        let loader = ConfigLoaderBuilder::new()
            .base_path("config")
            .add_file("myservice.yml")
            .environment(Environment::from_env())
            .env_prefix("MYSERVICE")
            .defaults(serde_json::json!({
                // Default configuration values
            }))?
            .build()?;
        
        let config: Self = loader.load_async().await?;
        config.validate_all()?;
        
        Ok(config)
    }
}
```

### 4. Create Configuration File

Create a YAML configuration file at `config/myservice.yml`:

```yaml
# Service identification
service:
  name: myservice
  version: 1.0.0
  description: My Service Description

# Redis configuration
redis:
  url: redis://localhost:6379
  prefix: "voltage:myservice:"
  pool_size: 20

# Logging configuration
logging:
  level: info
  console: true
  file:
    path: logs/myservice.log
    rotation: daily

# Monitoring configuration
monitoring:
  metrics_enabled: true
  metrics_port: 9090
  health_check_enabled: true
  health_check_port: 8080

# Service-specific configuration
my_service:
  # Add your service-specific settings here
```

### 5. Update Main Application

Update your `main.rs` to use the new configuration:

```rust
// Replace old import
// use crate::config::Config;

// With new import
use crate::config_new::MyServiceConfig;

#[tokio::main]
async fn main() -> Result<()> {
    // Load configuration
    let config = MyServiceConfig::load().await?;
    
    // Initialize logging based on config
    init_logging(&config.base.logging);
    
    // Use configuration
    info!("Starting {} v{}", 
        config.base.service.name, 
        config.base.service.version
    );
    
    // ... rest of your application
}
```

### 6. Migration Tools

Use the provided migration helpers:

```rust
use voltage_config::ConfigMigrator;

// Migrate from old format to new format
let migrator = ConfigMigrator::new("old_config.json", "config/myservice.yml");
migrator.backup()?; // Create backup first
migrator.migrate(transformer_fn)?;
```

## Service-Specific Migration Notes

### alarmsrv

- Previously used environment variables exclusively
- Now supports both file-based and environment variable configuration
- Migration example: `services/alarmsrv/examples/migrate_config.rs`

### hissrv

- Previously used YAML with CLI argument overrides
- Complex nested configuration with multiple storage backends
- Filter rules and data transformations migrated successfully

### modsrv

- Uses custom `Config::from_file()` method
- Will need to migrate model templates and control logic configuration
- Consider backward compatibility for existing model definitions

### netsrv

- Currently uses the `config` crate
- Straightforward migration as it already uses structured configuration
- Focus on preserving network endpoint configurations

### comsrv

- Already uses Figment, so conceptually similar
- Most complex due to channel configurations and protocol settings
- Consider creating a migration tool for CSV point tables

## Environment Variable Mapping

The framework automatically maps environment variables to configuration fields:

| Environment Variable | Configuration Path | Example |
|---------------------|-------------------|---------|
| `MYSERVICE_REDIS_URL` | `redis.url` | `redis://localhost:6379` |
| `MYSERVICE_LOGGING_LEVEL` | `logging.level` | `debug` |
| `MYSERVICE_API_PORT` | `api.port` | `8080` |

## Configuration Validation

The framework provides multiple validation levels:

1. **Type validation** - Automatic via serde
2. **Base validation** - Common fields like service name, Redis URL
3. **Service validation** - Custom validation in `Configurable::validate()`
4. **Field validation** - Using validation rules:

```rust
let loader = ConfigLoaderBuilder::new()
    .add_validation_rule(
        "api.port",
        Box::new(RangeRule::new("port_range", Some(1024), Some(65535), "api.port"))
    )
    .build()?;
```

## Testing Your Migration

1. **Unit Tests**: Test configuration loading and validation
2. **Integration Tests**: Test with actual configuration files
3. **Migration Tests**: Verify old configs convert correctly
4. **Backwards Compatibility**: Ensure existing deployments work

Example test:

```rust
#[tokio::test]
async fn test_config_migration() {
    // Load old configuration
    let old_config = OldConfig::load().await.unwrap();
    
    // Convert to new format
    let new_config = MyServiceConfig::from_old(old_config);
    
    // Validate
    assert!(new_config.validate_all().is_ok());
    
    // Verify values preserved
    assert_eq!(new_config.base.redis.url, "redis://localhost:6379");
}
```

## Best Practices

1. **Gradual Migration**: Keep old configuration module during transition
2. **Feature Flags**: Use feature flags to switch between old/new config
3. **Documentation**: Update service documentation with new config format
4. **Defaults**: Provide sensible defaults for all configuration values
5. **Validation**: Add comprehensive validation rules
6. **Monitoring**: Log configuration changes and validation results

## Troubleshooting

### Common Issues

1. **Missing configuration file**
   - Solution: Ensure `config/` directory exists with appropriate YAML files

2. **Environment variable conflicts**
   - Solution: Use unique prefixes for each service (e.g., `ALARM_`, `HIS_`)

3. **Validation failures**
   - Solution: Check logs for specific validation errors
   - Use `ConfigValidator` to test configurations

4. **Type mismatches**
   - Solution: Ensure serde attributes match expected formats
   - Use `#[serde(default)]` for optional fields

## Future Enhancements

- [ ] Configuration UI/CLI tool
- [ ] Automatic configuration documentation generation
- [ ] Configuration versioning and migration tracking
- [ ] Integration with service discovery
- [ ] Encrypted configuration values support

## Support

For questions or issues with migration:
1. Check service-specific examples in `examples/` directories
2. Review test cases for configuration scenarios
3. Open an issue in the VoltageEMS repository