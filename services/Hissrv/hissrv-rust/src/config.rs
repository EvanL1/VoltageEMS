use crate::error::{HisSrvError, Result};
use clap::{Parser, ArgAction};
use regex::Regex;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::time::SystemTime;

#[derive(Clone, Debug)]
pub struct Config {
    // Redis configuration
    pub redis_host: String,
    pub redis_port: u16,
    pub redis_password: String,
    pub redis_key_pattern: String,
    pub redis_socket: String,

    // InfluxDB configuration
    pub influxdb_url: String,
    pub influxdb_db: String,
    pub influxdb_user: String,
    pub influxdb_password: String,

    // Program configuration
    pub interval_seconds: u64,
    pub verbose: bool,
    pub enable_influxdb: bool,
    pub retention_days: u32,
    pub config_file: String,

    // Point storage configuration
    pub point_storage_patterns: Vec<(String, bool)>,
    pub default_point_storage: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            redis_host: "127.0.0.1".to_string(),
            redis_port: 6379,
            redis_password: String::new(),
            redis_key_pattern: "*".to_string(),
            redis_socket: "/var/run/redis/redis.sock".to_string(),

            influxdb_url: "http://localhost:8086".to_string(),
            influxdb_db: "mydb".to_string(),
            influxdb_user: String::new(),
            influxdb_password: String::new(),

            interval_seconds: 10,
            verbose: false,
            enable_influxdb: true,
            retention_days: 30,
            config_file: "hissrv.conf".to_string(),

            point_storage_patterns: Vec::new(),
            default_point_storage: true,
        }
    }
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    #[arg(long, help = "Configuration file path")]
    config: Option<String>,

    #[arg(long, help = "Redis host (default: 127.0.0.1)")]
    redis_host: Option<String>,

    #[arg(long, help = "Redis port (default: 6379)")]
    redis_port: Option<u16>,

    #[arg(long, help = "Redis password")]
    redis_password: Option<String>,

    #[arg(long, help = "Redis key pattern to match (default: *)")]
    redis_key_pattern: Option<String>,

    #[arg(long, help = "Redis Unix socket path (if specified, TCP is not used)")]
    redis_socket: Option<String>,

    #[arg(long, help = "InfluxDB URL (default: http://localhost:8086)")]
    influxdb_url: Option<String>,

    #[arg(long, help = "InfluxDB database name (default: mydb)")]
    influxdb_db: Option<String>,

    #[arg(long, help = "InfluxDB username")]
    influxdb_user: Option<String>,

    #[arg(long, help = "InfluxDB password")]
    influxdb_password: Option<String>,

    #[arg(long, help = "Sync interval in seconds (default: 10)")]
    interval: Option<u64>,

    #[arg(long, action = ArgAction::SetTrue, help = "Enable verbose logging")]
    verbose: bool,

    #[arg(long, action = ArgAction::SetTrue, help = "Enable writing to InfluxDB (default)")]
    enable_influxdb: bool,

    #[arg(long, action = ArgAction::SetTrue, help = "Disable writing to InfluxDB")]
    disable_influxdb: bool,

    #[arg(long, help = "Data retention period in days (default: 30)")]
    retention_days: Option<u32>,
}

impl Config {
    pub fn new() -> Self {
        Config::default()
    }

    pub fn from_args() -> Result<Self> {
        let args = Args::parse();
        let mut config = Config::default();

        // First load from config file if specified
        if let Some(config_file) = &args.config {
            config.config_file = config_file.clone();
            config.parse_config_file(&config_file)?;
        }

        // Then override with command line arguments
        if let Some(host) = args.redis_host {
            config.redis_host = host;
        }
        if let Some(port) = args.redis_port {
            config.redis_port = port;
        }
        if let Some(password) = args.redis_password {
            config.redis_password = password;
        }
        if let Some(pattern) = args.redis_key_pattern {
            config.redis_key_pattern = pattern;
        }
        if let Some(socket) = args.redis_socket {
            config.redis_socket = socket;
        }
        if let Some(url) = args.influxdb_url {
            config.influxdb_url = url;
        }
        if let Some(db) = args.influxdb_db {
            config.influxdb_db = db;
        }
        if let Some(user) = args.influxdb_user {
            config.influxdb_user = user;
        }
        if let Some(password) = args.influxdb_password {
            config.influxdb_password = password;
        }
        if let Some(interval) = args.interval {
            config.interval_seconds = interval;
        }
        if args.verbose {
            config.verbose = true;
        }
        if args.enable_influxdb {
            config.enable_influxdb = true;
        }
        if args.disable_influxdb {
            config.enable_influxdb = false;
        }
        if let Some(days) = args.retention_days {
            config.retention_days = days;
        }

        Ok(config)
    }

    pub fn parse_config_file(&mut self, filename: &str) -> Result<()> {
        let file = File::open(filename).map_err(|e| {
            HisSrvError::ConfigError(format!("Could not open config file: {}: {}", filename, e))
        })?;

        let reader = BufReader::new(file);
        self.point_storage_patterns.clear();

        for line in reader.lines() {
            let line = line?;
            let line = line.trim();

            // Skip comments and empty lines
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if let Some(pos) = line.find('=') {
                let key = line[..pos].trim();
                let value = line[pos + 1..].trim();

                match key {
                    "redis_host" => self.redis_host = value.to_string(),
                    "redis_port" => {
                        self.redis_port = value.parse().map_err(|_| {
                            HisSrvError::ParseError(format!("Invalid redis_port: {}", value))
                        })?
                    }
                    "redis_password" => self.redis_password = value.to_string(),
                    "redis_key_pattern" => self.redis_key_pattern = value.to_string(),
                    "redis_socket" => self.redis_socket = value.to_string(),
                    "influxdb_url" => self.influxdb_url = value.to_string(),
                    "influxdb_db" => self.influxdb_db = value.to_string(),
                    "influxdb_user" => self.influxdb_user = value.to_string(),
                    "influxdb_password" => self.influxdb_password = value.to_string(),
                    "interval_seconds" => {
                        self.interval_seconds = value.parse().map_err(|_| {
                            HisSrvError::ParseError(format!("Invalid interval_seconds: {}", value))
                        })?
                    }
                    "verbose" => {
                        self.verbose = value == "true" || value == "1" || value == "yes";
                    }
                    "enable_influxdb" => {
                        self.enable_influxdb = value == "true" || value == "1" || value == "yes";
                    }
                    "retention_days" => {
                        self.retention_days = value.parse().map_err(|_| {
                            HisSrvError::ParseError(format!("Invalid retention_days: {}", value))
                        })?
                    }
                    "default_point_storage" => {
                        self.default_point_storage = value == "true" || value == "1" || value == "yes";
                    }
                    "point_storage" => {
                        // Parse point storage configuration
                        // Format: point_pattern:true/false
                        if let Some(colon_pos) = value.rfind(':') {
                            let pattern = value[..colon_pos].to_string();
                            let storage_str = &value[colon_pos + 1..];
                            let storage = storage_str == "true" || storage_str == "1" || storage_str == "yes";

                            self.point_storage_patterns.push((pattern.clone(), storage));

                            if self.verbose {
                                println!(
                                    "Added point storage pattern: {} -> {}",
                                    pattern,
                                    if storage { "store" } else { "ignore" }
                                );
                            }
                        }
                    }
                    _ => {
                        // Unknown key, just ignore
                    }
                }
            }
        }

        Ok(())
    }

    pub fn should_store_point(&self, key: &str) -> bool {
        // If global InfluxDB writing is disabled, don't store any points
        if !self.enable_influxdb {
            return false;
        }

        // Check against specific patterns
        for (pattern, storage) in &self.point_storage_patterns {
            // Convert Redis glob pattern to regex
            let regex_pattern = pattern
                .replace("*", ".*")
                .replace("?", ".");

            // Check if key matches pattern
            if let Ok(regex) = Regex::new(&regex_pattern) {
                if regex.is_match(key) {
                    return *storage;
                }
            }
        }

        // If no pattern matched, use default
        self.default_point_storage
    }

    pub fn config_file_changed(&self, last_mod_time: &mut SystemTime) -> Result<bool> {
        let metadata = fs::metadata(&self.config_file)?;
        
        if let Ok(modified) = metadata.modified() {
            if modified > *last_mod_time {
                *last_mod_time = modified;
                return Ok(true);
            }
        }
        
        Ok(false)
    }
} 