use crate::config::Config;
use crate::error::{HisSrvError, Result};
use crate::influxdb_handler::{InfluxDBConnection, try_parse_numeric};
use redis::{Client, Connection, Commands, RedisResult, Value};
use std::collections::{HashMap, HashSet};

pub struct RedisConnection {
    client: Option<Client>,
    conn: Option<Connection>,
    connected: bool,
}

#[derive(Debug, PartialEq)]
pub enum RedisType {
    String,
    List,
    Set,
    Hash,
    ZSet,
    None,
}

impl RedisConnection {
    pub fn new() -> Self {
        RedisConnection {
            client: None,
            conn: None,
            connected: false,
        }
    }

    pub fn connect(&mut self, config: &Config) -> Result<()> {
        // Disconnect if already connected
        self.disconnect();

        let client = if !config.redis_socket.is_empty() {
            // Connect using Unix socket
            Client::open(format!("unix://{}", config.redis_socket))?
        } else {
            // Connect using TCP
            let redis_url = if config.redis_password.is_empty() {
                format!("redis://{}:{}", config.redis_host, config.redis_port)
            } else {
                format!(
                    "redis://:{}@{}:{}",
                    config.redis_password, config.redis_host, config.redis_port
                )
            };
            Client::open(redis_url)?
        };

        let mut conn = client.get_connection()?;

        // Test connection with PING
        let ping_result: String = redis::cmd("PING").query(&mut conn)?;
        if ping_result != "PONG" {
            return Err(HisSrvError::ConnectionError(
                "Redis connection test failed".to_string(),
            ));
        }

        if !config.redis_socket.is_empty() {
            println!(
                "Successfully connected to Redis via Unix socket: {}",
                config.redis_socket
            );
        } else {
            println!(
                "Successfully connected to Redis at {}:{}",
                config.redis_host, config.redis_port
            );
        }

        self.client = Some(client);
        self.conn = Some(conn);
        self.connected = true;

        Ok(())
    }

    pub fn disconnect(&mut self) {
        self.conn = None;
        self.client = None;
        self.connected = false;
    }

    pub fn is_connected(&self) -> bool {
        self.connected
    }

    pub fn get_keys(&mut self, pattern: &str) -> Result<Vec<String>> {
        if !self.connected || self.conn.is_none() {
            return Err(HisSrvError::ConnectionError(
                "Not connected to Redis".to_string(),
            ));
        }

        let conn = self.conn.as_mut().unwrap();
        let keys: Vec<String> = redis::cmd("KEYS")
            .arg(pattern)
            .query(conn)?;

        Ok(keys)
    }

    pub fn get_type(&mut self, key: &str) -> Result<RedisType> {
        if !self.connected || self.conn.is_none() {
            return Err(HisSrvError::ConnectionError(
                "Not connected to Redis".to_string(),
            ));
        }

        let conn = self.conn.as_mut().unwrap();
        let type_str: String = redis::cmd("TYPE")
            .arg(key)
            .query(conn)?;

        match type_str.as_str() {
            "string" => Ok(RedisType::String),
            "list" => Ok(RedisType::List),
            "set" => Ok(RedisType::Set),
            "hash" => Ok(RedisType::Hash),
            "zset" => Ok(RedisType::ZSet),
            _ => Ok(RedisType::None),
        }
    }

    pub fn get_string(&mut self, key: &str) -> Result<String> {
        if !self.connected || self.conn.is_none() {
            return Err(HisSrvError::ConnectionError(
                "Not connected to Redis".to_string(),
            ));
        }

        let conn = self.conn.as_mut().unwrap();
        let value: String = conn.get(key)?;
        Ok(value)
    }

    pub fn get_hash(&mut self, key: &str) -> Result<HashMap<String, String>> {
        if !self.connected || self.conn.is_none() {
            return Err(HisSrvError::ConnectionError(
                "Not connected to Redis".to_string(),
            ));
        }

        let conn = self.conn.as_mut().unwrap();
        let hash: HashMap<String, String> = conn.hgetall(key)?;
        Ok(hash)
    }

    pub fn get_list(&mut self, key: &str) -> Result<Vec<String>> {
        if !self.connected || self.conn.is_none() {
            return Err(HisSrvError::ConnectionError(
                "Not connected to Redis".to_string(),
            ));
        }

        let conn = self.conn.as_mut().unwrap();
        let list: Vec<String> = conn.lrange(key, 0, -1)?;
        Ok(list)
    }

    pub fn get_set(&mut self, key: &str) -> Result<HashSet<String>> {
        if !self.connected || self.conn.is_none() {
            return Err(HisSrvError::ConnectionError(
                "Not connected to Redis".to_string(),
            ));
        }

        let conn = self.conn.as_mut().unwrap();
        let set: HashSet<String> = conn.smembers(key)?;
        Ok(set)
    }

    pub fn get_zset(&mut self, key: &str) -> Result<Vec<(String, f64)>> {
        if !self.connected || self.conn.is_none() {
            return Err(HisSrvError::ConnectionError(
                "Not connected to Redis".to_string(),
            ));
        }

        let conn = self.conn.as_mut().unwrap();
        let zset: Vec<(String, f64)> = conn.zrange_withscores(key, 0, -1)?;
        Ok(zset)
    }
}

pub async fn process_redis_data(
    redis: &mut RedisConnection,
    influxdb: &mut InfluxDBConnection,
    config: &Config,
) -> Result<()> {
    if !config.enable_influxdb || !influxdb.is_connected() {
        if config.verbose {
            println!(
                "InfluxDB writing is disabled. Waiting {} seconds...",
                config.interval_seconds
            );
        }
        return Ok(());
    }

    if !redis.is_connected() {
        println!("Redis connection lost. Attempting to reconnect...");
        if let Err(e) = redis.connect(config) {
            println!("Failed to reconnect to Redis: {}", e);
            return Err(HisSrvError::ConnectionError(
                "Failed to reconnect to Redis".to_string(),
            ));
        }
    }

    // Get matching Redis keys
    let keys = redis.get_keys(&config.redis_key_pattern)?;

    if config.verbose {
        println!(
            "Found {} keys matching pattern: {}",
            keys.len(),
            config.redis_key_pattern
        );
    }

    let mut stored_points = 0;
    let mut skipped_points = 0;

    // Process each key
    for key in &keys {
        // Check if this point should be stored
        if !config.should_store_point(key) {
            skipped_points += 1;
            if config.verbose {
                println!("Skipping key (not configured for storage): {}", key);
            }
            continue;
        }

        // Get key type
        match redis.get_type(key) {
            Ok(RedisType::String) => {
                // Process string type
                match redis.get_string(key) {
                    Ok(value) => {
                        // Try to parse value as numeric
                        let (is_numeric, numeric_value) = try_parse_numeric(&value);

                        // Write to InfluxDB
                        if let Err(e) = influxdb
                            .write_point(
                                key,
                                "string",
                                None,
                                is_numeric,
                                numeric_value,
                                if is_numeric { None } else { Some(&value) },
                                None,
                            )
                            .await
                        {
                            println!("Error writing string point to InfluxDB: {}", e);
                        } else {
                            stored_points += 1;
                            if config.verbose {
                                println!("Transferred string key: {}", key);
                            }
                        }
                    }
                    Err(e) => {
                        println!("Error getting string value for key {}: {}", key, e);
                    }
                }
            }
            Ok(RedisType::Hash) => {
                // Process hash type
                match redis.get_hash(key) {
                    Ok(hash_values) => {
                        let hash_len = hash_values.len();
                        for (field, value) in hash_values {
                            // Try to parse value as numeric
                            let (is_numeric, numeric_value) = try_parse_numeric(&value);

                            // Write to InfluxDB
                            if let Err(e) = influxdb
                                .write_point(
                                    key,
                                    "hash",
                                    Some(&field),
                                    is_numeric,
                                    numeric_value,
                                    if is_numeric { None } else { Some(&value) },
                                    None,
                                )
                                .await
                            {
                                println!("Error writing hash point to InfluxDB: {}", e);
                            }
                        }
                        stored_points += 1;
                        if config.verbose {
                            println!(
                                "Transferred hash key: {} with {} fields",
                                key,
                                hash_len
                            );
                        }
                    }
                    Err(e) => {
                        println!("Error getting hash values for key {}: {}", key, e);
                    }
                }
            }
            Ok(RedisType::List) => {
                // Process list type
                match redis.get_list(key) {
                    Ok(list_values) => {
                        for (i, value) in list_values.iter().enumerate() {
                            // Try to parse value as numeric
                            let (is_numeric, numeric_value) = try_parse_numeric(value);

                            // Write to InfluxDB
                            if let Err(e) = influxdb
                                .write_point(
                                    key,
                                    "list",
                                    Some(&i.to_string()),
                                    is_numeric,
                                    numeric_value,
                                    if is_numeric { None } else { Some(value) },
                                    None,
                                )
                                .await
                            {
                                println!("Error writing list point to InfluxDB: {}", e);
                            }
                        }
                        stored_points += 1;
                        if config.verbose {
                            println!(
                                "Transferred list key: {} with {} items",
                                key,
                                list_values.len()
                            );
                        }
                    }
                    Err(e) => {
                        println!("Error getting list values for key {}: {}", key, e);
                    }
                }
            }
            Ok(RedisType::Set) => {
                // Process set type
                match redis.get_set(key) {
                    Ok(set_values) => {
                        let set_len = set_values.len();
                        for value in set_values {
                            // Try to parse value as numeric
                            let (is_numeric, numeric_value) = try_parse_numeric(&value);

                            // Write to InfluxDB
                            if let Err(e) = influxdb
                                .write_point(
                                    key,
                                    "set",
                                    None,
                                    is_numeric,
                                    numeric_value,
                                    if is_numeric { None } else { Some(&value) },
                                    None,
                                )
                                .await
                            {
                                println!("Error writing set point to InfluxDB: {}", e);
                            }
                        }
                        stored_points += 1;
                        if config.verbose {
                            println!(
                                "Transferred set key: {} with {} members",
                                key,
                                set_len
                            );
                        }
                    }
                    Err(e) => {
                        println!("Error getting set values for key {}: {}", key, e);
                    }
                }
            }
            Ok(RedisType::ZSet) => {
                // Process sorted set type
                match redis.get_zset(key) {
                    Ok(zset_values) => {
                        let zset_len = zset_values.len();
                        for (member, score) in zset_values {
                            // Try to parse value as numeric
                            let (is_numeric, numeric_value) = try_parse_numeric(&member);

                            // Write to InfluxDB
                            if let Err(e) = influxdb
                                .write_point(
                                    key,
                                    "zset",
                                    None,
                                    is_numeric,
                                    numeric_value,
                                    if is_numeric { None } else { Some(&member) },
                                    Some(score),
                                )
                                .await
                            {
                                println!("Error writing zset point to InfluxDB: {}", e);
                            }
                        }
                        stored_points += 1;
                        if config.verbose {
                            println!(
                                "Transferred sorted set key: {} with {} members",
                                key,
                                zset_len
                            );
                        }
                    }
                    Err(e) => {
                        println!("Error getting zset values for key {}: {}", key, e);
                    }
                }
            }
            Ok(RedisType::None) => {
                println!("Key {} has no type or does not exist", key);
            }
            Err(e) => {
                println!("Error getting type for key {}: {}", key, e);
            }
        }
    }

    println!(
        "Completed data transfer cycle. Found {} keys, stored {}, skipped {}. Waiting {} seconds for next cycle...",
        keys.len(),
        stored_points,
        skipped_points,
        config.interval_seconds
    );

    Ok(())
} 