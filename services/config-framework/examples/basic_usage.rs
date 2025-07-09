use serde::{Deserialize, Serialize};
use std::any::Any;
use voltage_config::prelude::*;

#[derive(Debug, Serialize, Deserialize)]
struct AppConfig {
    server: ServerConfig,
    database: DatabaseConfig,
    logging: LoggingConfig,
}

#[derive(Debug, Serialize, Deserialize)]
struct ServerConfig {
    host: String,
    port: u16,
    workers: usize,
}

#[derive(Debug, Serialize, Deserialize)]
struct DatabaseConfig {
    url: String,
    max_connections: u32,
    timeout_seconds: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct LoggingConfig {
    level: String,
    format: String,
    output: String,
}

impl Configurable for AppConfig {
    fn validate(&self) -> Result<()> {
        if self.server.port == 0 {
            return Err(ConfigError::Validation("Server port cannot be 0".into()));
        }

        if self.database.max_connections == 0 {
            return Err(ConfigError::Validation(
                "Max connections must be greater than 0".into(),
            ));
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

    let loader = ConfigLoaderBuilder::new()
        .base_path("config")
        .add_file("app.yml")
        .add_file_with_format("overrides.toml", ConfigFormat::Toml)
        .environment(Environment::Development)
        .env_prefix("APP")
        .defaults(serde_json::json!({
            "server": {
                "host": "127.0.0.1",
                "port": 8080,
                "workers": 4
            },
            "database": {
                "url": "postgres://localhost/myapp",
                "max_connections": 10,
                "timeout_seconds": 30
            },
            "logging": {
                "level": "info",
                "format": "json",
                "output": "stdout"
            }
        }))?
        .build()?;

    let config: AppConfig = loader.load()?;

    println!("Loaded configuration:");
    println!("Server: {}:{}", config.server.host, config.server.port);
    println!("Database URL: {}", config.database.url);
    println!("Log level: {}", config.logging.level);

    let watcher = ConfigWatcher::new(loader, vec!["config".into()])
        .with_interval(std::time::Duration::from_secs(2));

    watcher.start().await?;

    println!("\nWatching for configuration changes (press Ctrl+C to exit)...");

    while let Some(event) = watcher.wait_for_change().await {
        match event {
            WatchEvent::Modified(path) => {
                println!("Configuration file modified: {}", path.display());
                match watcher.reload::<AppConfig>().await {
                    Ok(new_config) => {
                        println!("Reloaded configuration successfully");
                        println!("New server port: {}", new_config.server.port);
                    }
                    Err(e) => eprintln!("Failed to reload configuration: {}", e),
                }
            }
            WatchEvent::Created(path) => {
                println!("New configuration file created: {}", path.display());
            }
            WatchEvent::Deleted(path) => {
                println!("Configuration file deleted: {}", path.display());
            }
            WatchEvent::Reloaded => {
                println!("Configuration reloaded");
            }
        }
    }

    Ok(())
}
