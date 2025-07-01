# Voltage Config Framework

A shared configuration management framework for VoltageEMS microservices, built on top of Figment.

## Features

- **Multi-format support**: YAML, TOML, JSON, and environment variables
- **Hierarchical configuration**: Base configs with environment-specific overrides
- **Type-safe**: Leverages Rust's type system with serde
- **Validation**: Built-in and custom validation rules
- **Hot-reloading**: Watch configuration files for changes
- **Async support**: Both sync and async configuration loading
- **Extensible**: Custom validators and configuration sources

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
voltage-config = { path = "../config-framework" }
```

Basic example:

```rust
use serde::{Deserialize, Serialize};
use voltage_config::prelude::*;

#[derive(Debug, Serialize, Deserialize)]
struct MyConfig {
    server: ServerConfig,
    database: DatabaseConfig,
}

#[derive(Debug, Serialize, Deserialize)]
struct ServerConfig {
    host: String,
    port: u16,
}

#[derive(Debug, Serialize, Deserialize)]
struct DatabaseConfig {
    url: String,
}

impl Configurable for MyConfig {
    fn validate(&self) -> Result<()> {
        // Custom validation logic
        Ok(())
    }
    
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let loader = ConfigLoaderBuilder::new()
        .base_path("config")
        .add_file("app.yml")
        .environment(Environment::from_env())
        .env_prefix("MYAPP")
        .build()?;
    
    let config: MyConfig = loader.load()?;
    println!("Server: {}:{}", config.server.host, config.server.port);
    
    Ok(())
}
```

## Configuration Loading Order

1. Default values (if provided)
2. `config/default.yml`
3. `config/{environment}.yml`
4. Additional files (in order added)
5. Environment variables

Later sources override earlier ones.

## Environment Variables

Environment variables are automatically mapped to configuration keys:
- `MYAPP_SERVER_HOST` → `server.host`
- `MYAPP_DATABASE_URL` → `database.url`

## File Watching

```rust
let watcher = ConfigWatcher::new(loader, vec!["config".into()])
    .with_interval(Duration::from_secs(5));

watcher.start().await?;

while let Some(event) = watcher.wait_for_change().await {
    match event {
        WatchEvent::Modified(path) => {
            let new_config = watcher.reload::<MyConfig>().await?;
            // Handle reloaded configuration
        }
        _ => {}
    }
}
```

## Custom Validators

```rust
struct MyValidator;

#[async_trait::async_trait]
impl ConfigValidator for MyValidator {
    async fn validate(&self, config: &dyn Any) -> Result<()> {
        // Custom validation logic
        Ok(())
    }
    
    fn name(&self) -> &str {
        "MyValidator"
    }
}

let loader = ConfigLoaderBuilder::new()
    .add_validator(Box::new(MyValidator))
    .build()?;
```

## Integration with VoltageEMS Services

This framework is designed to standardize configuration management across all VoltageEMS services:

- **comsrv**: Communication service configuration
- **modsrv**: Model service configuration  
- **hissrv**: Historical data service configuration
- **netsrv**: Network service configuration
- **alarmsrv**: Alarm service configuration

Each service can extend the base configuration with service-specific settings while maintaining consistency in configuration loading and validation.