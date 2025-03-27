use crate::config::Config;
use crate::error::Result;
use crate::influxdb_handler::InfluxDBConnection;
use crate::redis_handler::{RedisConnection, process_redis_data};
use std::time::SystemTime;
use tokio::time::{sleep, Duration};

mod config;
mod error;
mod influxdb_handler;
mod redis_handler;

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command line arguments
    let mut config = Config::from_args()?;
    
    // Record the last modification time of the configuration file
    let mut last_config_mod_time = SystemTime::now();
    
    // Connect to RTDB
    let mut redis = RedisConnection::new();
    if !redis.connect(&config)? {
        println!("Failed to connect to RTDB. Exiting.");
        return Ok(());
    }
    
    // Connect to InfluxDB (if enabled)
    let mut influxdb = InfluxDBConnection::new();
    influxdb.connect(&config).await?;
    
    println!("Starting data transfer service...");
    if config.enable_influxdb {
        println!(
            "Data will be transferred from RTDB to InfluxDB every {} seconds.",
            config.interval_seconds
        );
        println!(
            "Default point storage policy: {}",
            if config.default_point_storage { "Store" } else { "Ignore" }
        );
        println!(
            "Number of specific point patterns: {}",
            config.point_storage_patterns.len()
        );
    } else {
        println!("Data transfer to InfluxDB is currently disabled.");
    }
    println!("Press Ctrl+C to stop");
    
    // Main loop
    loop {
        // Check if configuration file has changed
        if let Ok(true) = config.config_file_changed(&mut last_config_mod_time) {
            println!("Configuration file changed. Reloading...");
            
            // Save old configuration to detect critical changes
            let old_enable_influxdb = config.enable_influxdb;
            let old_retention_days = config.retention_days;
            let old_influxdb_url = config.influxdb_url.clone();
            let old_influxdb_db = config.influxdb_db.clone();
            let old_redis_host = config.redis_host.clone();
            let old_redis_port = config.redis_port;
            let old_redis_password = config.redis_password.clone();
            let old_redis_socket = config.redis_socket.clone();
            
            // Reload configuration
            let mut new_config = config.clone();
            if let Ok(()) = new_config.parse_config_file(&config.config_file) {
                config = new_config;
                
                // Check if RTDB connection settings have changed
                let reconnect_redis = old_redis_host != config.redis_host
                    || old_redis_port != config.redis_port
                    || old_redis_password != config.redis_password
                    || old_redis_socket != config.redis_socket;
                
                if reconnect_redis {
                    println!("RTDB connection settings changed. Reconnecting...");
                    if !redis.connect(&config)? {
                        println!("Failed to reconnect to RTDB with new settings.");
                    }
                }
                
                // If InfluxDB settings have changed, need to reconnect
                let reconnect_influxdb = config.enable_influxdb && (
                    !old_enable_influxdb
                    || old_influxdb_url != config.influxdb_url
                    || old_influxdb_db != config.influxdb_db
                );
                
                if !config.enable_influxdb && old_enable_influxdb {
                    println!("InfluxDB writing has been disabled.");
                } else if config.enable_influxdb && !old_enable_influxdb {
                    println!("InfluxDB writing has been enabled.");
                    influxdb.connect(&config).await?;
                } else if reconnect_influxdb {
                    println!("InfluxDB connection settings changed.");
                    influxdb.connect(&config).await?;
                } else if config.enable_influxdb && old_retention_days != config.retention_days {
                    println!(
                        "Retention policy changed from {} to {} days.",
                        old_retention_days, config.retention_days
                    );
                    influxdb.create_retention_policy(config.retention_days).await?;
                }
                
                // Log point storage configuration changes
                println!(
                    "Updated point storage configuration. Default: {}, Patterns: {}",
                    if config.default_point_storage { "Store" } else { "Ignore" },
                    config.point_storage_patterns.len()
                );
            }
        }
        
        // Process RTDB data and write to InfluxDB
        process_redis_data(&mut redis, &mut influxdb, &config).await?;
        
        // Wait for next sync cycle
        sleep(Duration::from_secs(config.interval_seconds)).await;
    }
} 