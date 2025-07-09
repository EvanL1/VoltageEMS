use alarmsrv::config::AlarmConfig;
use alarmsrv::config_new::{generate_default_config, AlarmServiceConfig};
use anyhow::Result;
use std::fs;
use std::path::Path;

#[tokio::main]
async fn main() -> Result<()> {
    println!("Alarm Service Configuration Migration Tool");
    println!("=========================================");

    // Generate default configuration
    let yaml_content = generate_default_config();

    // Create config directory if it doesn't exist
    fs::create_dir_all("config")?;

    // Write default configuration
    let config_path = Path::new("config/alarmsrv.yml");
    if config_path.exists() {
        println!(
            "Configuration file already exists at: {}",
            config_path.display()
        );
        println!("Do you want to overwrite it? (y/N)");

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;

        if input.trim().to_lowercase() != "y" {
            println!("Skipping configuration file creation.");
            return Ok(());
        }
    }

    fs::write(config_path, yaml_content)?;
    println!("✓ Created configuration file: {}", config_path.display());

    // Try to load existing configuration from environment
    println!("\nChecking for existing environment configuration...");
    match AlarmConfig::load().await {
        Ok(old_config) => {
            println!("✓ Found existing configuration from environment variables");
            println!("\nExisting configuration:");
            println!(
                "  Redis: {}:{}",
                old_config.redis.host, old_config.redis.port
            );
            println!("  API: {}:{}", old_config.api.host, old_config.api.port);
            println!(
                "  Storage retention: {} days",
                old_config.storage.retention_days
            );

            println!("\nTo migrate these settings, set the following environment variables:");
            println!("  export ALARMSRV_REDIS__HOST={}", old_config.redis.host);
            println!("  export ALARMSRV_REDIS__PORT={}", old_config.redis.port);
            if let Some(password) = &old_config.redis.password {
                println!("  export ALARMSRV_REDIS__PASSWORD={}", password);
            }
            println!("  export ALARMSRV_API__HOST={}", old_config.api.host);
            println!("  export ALARMSRV_API__PORT={}", old_config.api.port);
            println!(
                "  export ALARMSRV_STORAGE__RETENTION_DAYS={}",
                old_config.storage.retention_days
            );
            println!(
                "  export ALARMSRV_STORAGE__AUTO_CLEANUP={}",
                old_config.storage.auto_cleanup
            );
            println!(
                "  export ALARMSRV_STORAGE__CLEANUP_INTERVAL_HOURS={}",
                old_config.storage.cleanup_interval_hours
            );
        }
        Err(_) => {
            println!("No existing configuration found in environment variables.");
            println!("Using default configuration values.");
        }
    }

    println!("\nMigration complete!");
    println!("\nNext steps:");
    println!("1. Review the generated configuration file: config/alarmsrv.yml");
    println!("2. Update your service to use the new configuration:");
    println!("   - Replace `use config::*` with `use config_new::*`");
    println!("   - Replace `AlarmConfig::load()` with `config_new::load_config()`");
    println!("3. Test the service with the new configuration");

    Ok(())
}
