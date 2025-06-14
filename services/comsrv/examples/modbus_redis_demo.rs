/// Modbus + Redis 完整测试演示
/// 
/// 这个示例展示如何：
/// 1. 连接并读取Modbus设备数据
/// 2. 将数据存储到Redis数据库
/// 3. 监控数据变化
/// 4. 提供实时数据查询

use std::collections::HashMap;
use std::time::Duration;
use tokio::time::interval;
use serde_json::json;
use redis::{Client as RedisClient, Commands};
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use chrono::Utc;
use csv::ReaderBuilder;

use comsrv::core::protocols::modbus::common::{
    ModbusRegisterMapping, ModbusDataType, ModbusRegisterType, ByteOrder,
    PerformanceMetrics
};

/// Redis key patterns for organizing Modbus data
struct RedisKeys;

impl RedisKeys {
    const CONFIG_PREFIX: &'static str = "modbus:config";
    const POINTS_PREFIX: &'static str = "modbus:points";
    const VALUES_PREFIX: &'static str = "modbus:values";
    const STATS_PREFIX: &'static str = "modbus:stats";
    const METADATA_PREFIX: &'static str = "modbus:metadata";

    fn config_key(channel_id: u16) -> String {
        format!("{}:channel_{}", Self::CONFIG_PREFIX, channel_id)
    }

    fn point_key(channel_id: u16, point_id: &str) -> String {
        format!("{}:channel_{}:{}", Self::POINTS_PREFIX, channel_id, point_id)
    }

    fn value_key(channel_id: u16, point_id: &str) -> String {
        format!("{}:channel_{}:{}", Self::VALUES_PREFIX, channel_id, point_id)
    }

    fn stats_key(channel_id: u16) -> String {
        format!("{}:channel_{}", Self::STATS_PREFIX, channel_id)
    }

    fn metadata_key(channel_id: u16) -> String {
        format!("{}:channel_{}", Self::METADATA_PREFIX, channel_id)
    }
}

/// CSV point record for parsing
#[derive(serde::Deserialize)]
struct CsvPointRecord {
    id: String,
    name: String,
    address: u16,
    #[serde(rename = "type_")]
    type_: String,
    data_type: String,
    unit: String,
    scale: f64,
    offset: f64,
    writable: bool,
    description: String,
    byte_order: String,
}

/// Modbus channel configuration
#[derive(serde::Deserialize, serde::Serialize)]
struct ModbusChannelConfig {
    channel_id: u16,
    slave_id: u8,
    host: String,
    port: u16,
    timeout_ms: u64,
    max_retries: u32,
    poll_interval_ms: u64,
    mode: String,
    point_table_file: String,
}

/// Redis data manager for Modbus integration
struct ModbusRedisManager {
    redis_client: RedisClient,
    channel_configs: HashMap<u16, ModbusChannelConfig>,
    point_mappings: HashMap<u16, Vec<ModbusRegisterMapping>>,
    performance_metrics: PerformanceMetrics,
}

impl ModbusRedisManager {
    /// Create new manager with Redis connection
    fn new(redis_url: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let client = RedisClient::open(redis_url)?;
        
        // Test connection
    let mut conn = client.get_connection()?;
    redis::cmd("PING").query::<String>(&mut conn)?;
        println!("✅ Connected to Redis at {}", redis_url);

        Ok(Self {
            redis_client: client,
            channel_configs: HashMap::new(),
            point_mappings: HashMap::new(),
            performance_metrics: PerformanceMetrics::new(),
        })
    }

    /// Load configuration from file
    fn load_config(&mut self, config_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        println!("📂 Loading configuration from: {}", config_path.display());
        
        let config_content = fs::read_to_string(config_path)?;
        let config: ModbusChannelConfig = serde_yaml::from_str(&config_content)?;
        
        println!("✅ Loaded config for channel {}: {}:{}", 
            config.channel_id, config.host, config.port);
        
        self.channel_configs.insert(config.channel_id, config);
        Ok(())
    }

    /// Load point table from CSV file
    fn load_point_table(&mut self, channel_id: u16, csv_path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        println!("📊 Loading point table from: {}", csv_path.display());
        
        let file = File::open(csv_path)?;
        let mut reader = ReaderBuilder::new()
            .has_headers(true)
            .from_reader(file);
        
        let mut mappings = Vec::new();
        let mut point_count_by_type = HashMap::new();
        
        for result in reader.deserialize() {
            let record: CsvPointRecord = result?;
            
            // Parse data type
            let data_type = match record.data_type.to_lowercase().as_str() {
                "bool" | "boolean" => ModbusDataType::Bool,
                "uint16" | "u16" => ModbusDataType::UInt16,
                "uint32" | "u32" => ModbusDataType::UInt32,
                "int16" | "i16" => ModbusDataType::Int16,
                "int32" | "i32" => ModbusDataType::Int32,
                "float32" | "f32" => ModbusDataType::Float32,
                "float64" | "f64" => ModbusDataType::Float64,
                _ => ModbusDataType::UInt16, // default
            };
            
            // Parse register type
            let register_type = match record.type_.to_lowercase().as_str() {
                "coil" => ModbusRegisterType::Coil,
                "discrete_input" => ModbusRegisterType::DiscreteInput,
                "input_register" => ModbusRegisterType::InputRegister,
                "holding_register" => ModbusRegisterType::HoldingRegister,
                _ => ModbusRegisterType::InputRegister, // default
            };
            
            // Parse byte order
            let byte_order = match record.byte_order.to_lowercase().as_str() {
                "little_endian" => ByteOrder::LittleEndian,
                "big_endian_word_swapped" => ByteOrder::BigEndianWordSwapped,
                "little_endian_word_swapped" => ByteOrder::LittleEndianWordSwapped,
                _ => ByteOrder::BigEndian, // default
            };
            
            // Create mapping
            let mapping = ModbusRegisterMapping::builder(&record.id)
                .address(record.address)
                .register_type(register_type)
                .data_type(data_type)
                .scale(record.scale)
                .offset(record.offset)
                .unit(&record.unit)
                .description(&record.description)
                .display_name(&record.name)
                .access_mode(if record.writable { "read_write" } else { "read" })
                .byte_order(byte_order)
                .build();
            
            // Validate the mapping
            mapping.validate().map_err(|e| format!("Validation error for point {}: {}", record.id, e))?;
            
            // Count by type for statistics
            *point_count_by_type.entry(format!("{:?}", register_type)).or_insert(0) += 1;
            
            mappings.push(mapping);
        }
        
        println!("✅ Loaded {} point mappings for channel {}", mappings.len(), channel_id);
        println!("📈 Point distribution:");
        for (type_name, count) in &point_count_by_type {
            println!("   - {}: {} points", type_name, count);
        }
        
        self.point_mappings.insert(channel_id, mappings);
        Ok(())
    }

    /// Store all data to Redis
    async fn store_to_redis(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mut conn = self.redis_client.get_connection()?;
        let start_time = std::time::Instant::now();
        
        println!("\n🔴 Storing data to Redis...");
        
        for (&channel_id, config) in &self.channel_configs {
            // Store channel configuration
            let config_key = RedisKeys::config_key(channel_id);
            let config_json = json!({
                "channel_id": config.channel_id,
                "slave_id": config.slave_id,
                "host": config.host,
                "port": config.port,
                "timeout_ms": config.timeout_ms,
                "max_retries": config.max_retries,
                "poll_interval_ms": config.poll_interval_ms,
                "mode": config.mode,
                "point_table_file": config.point_table_file,
                "loaded_at": Utc::now().to_rfc3339(),
                "status": "active"
            });
            
            let _: () = conn.set(&config_key, config_json.to_string())?;
            let _: () = conn.expire(&config_key, 3600)?; // 1 hour TTL
            
            // Store point mappings if available
            if let Some(mappings) = self.point_mappings.get(&channel_id) {
                for mapping in mappings {
                    let point_key = RedisKeys::point_key(channel_id, &mapping.name);
                    let point_json = json!({
                        "id": mapping.name,
                        "display_name": mapping.display_name,
                        "address": mapping.address,
                        "register_type": format!("{:?}", mapping.register_type),
                        "data_type": format!("{:?}", mapping.data_type),
                        "scale": mapping.scale,
                        "offset": mapping.offset,
                        "unit": mapping.unit,
                        "description": mapping.description,
                        "access_mode": mapping.access_mode,
                        "byte_order": format!("{:?}", mapping.byte_order),
                        "register_count": mapping.register_count(),
                        "address_range": mapping.address_range(),
                        "stored_at": Utc::now().to_rfc3339()
                    });
                    
                    let _: () = conn.set(&point_key, point_json.to_string())?;
                    let _: () = conn.expire(&point_key, 3600)?; // 1 hour TTL
                }
                
                // Store channel statistics
                let stats_key = RedisKeys::stats_key(channel_id);
                let total_registers: u32 = mappings.iter().map(|m| m.register_count() as u32).sum();
                let stats_json = json!({
                    "channel_id": channel_id,
                    "total_points": mappings.len(),
                    "total_registers": total_registers,
                    "point_types": self.count_point_types(mappings),
                    "address_ranges": self.get_address_ranges(mappings),
                    "last_updated": Utc::now().to_rfc3339(),
                    "memory_usage_kb": (mappings.len() * 100) / 1024 // Estimate
                });
                
                let _: () = conn.set(&stats_key, stats_json.to_string())?;
                let _: () = conn.expire(&stats_key, 3600)?; // 1 hour TTL
                
                // Store metadata
                let metadata_key = RedisKeys::metadata_key(channel_id);
                let metadata_json = json!({
                    "channel_id": channel_id,
                    "data_format_version": "1.0",
                    "created_by": "modbus_redis_demo",
                    "created_at": Utc::now().to_rfc3339(),
                    "redis_keys": {
                        "config": config_key,
                        "points_prefix": format!("{}:channel_{}:", RedisKeys::POINTS_PREFIX, channel_id),
                        "values_prefix": format!("{}:channel_{}:", RedisKeys::VALUES_PREFIX, channel_id),
                        "stats": stats_key
                    }
                });
                
                let _: () = conn.set(&metadata_key, metadata_json.to_string())?;
                let _: () = conn.expire(&metadata_key, 3600)?; // 1 hour TTL
                
                println!("✅ Stored {} points for channel {}", mappings.len(), channel_id);
            }
        }
        
        let elapsed = start_time.elapsed();
        self.performance_metrics.record_operation(true, elapsed.as_millis() as u64);
        
        println!("✅ Data storage completed in {:?}", elapsed);
        Ok(())
    }

    /// Simulate point values and store to Redis
    async fn simulate_point_values(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mut conn = self.redis_client.get_connection()?;
        
        for (&channel_id, mappings) in &self.point_mappings {
            for mapping in mappings {
                // Generate realistic simulated values based on data type
                let simulated_raw_value = self.generate_simulated_value(&mapping);
                let engineering_value = mapping.convert_to_engineering_units(simulated_raw_value);
                
                let value_key = RedisKeys::value_key(channel_id, &mapping.name);
                let value_json = json!({
                    "point_id": mapping.name,
                    "raw_value": simulated_raw_value,
                    "engineering_value": engineering_value,
                    "unit": mapping.unit,
                    "quality": "good",
                    "timestamp": Utc::now().to_rfc3339(),
                    "source": "simulation",
                    "register_address": mapping.address,
                    "data_type": format!("{:?}", mapping.data_type)
                });
                
                let _: () = conn.set(&value_key, value_json.to_string())?;
                let _: () = conn.expire(&value_key, 300)?; // 5 minutes TTL for values
            }
        }
        
        Ok(())
    }

    /// Generate realistic simulated values
    fn generate_simulated_value(&self, mapping: &ModbusRegisterMapping) -> f64 {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        
        match mapping.data_type {
            ModbusDataType::Bool => if rng.gen_bool(0.3) { 1.0 } else { 0.0 },
            ModbusDataType::UInt16 => rng.gen_range(0..65535) as f64,
            ModbusDataType::Int16 => rng.gen_range(-32768..32767) as f64,
            ModbusDataType::UInt32 => rng.gen_range(0..1000000) as f64,
            ModbusDataType::Int32 => rng.gen_range(-1000000..1000000) as f64,
            ModbusDataType::Float32 => {
                // Generate values that make sense with the point's scale and offset
                if mapping.name.contains("temperature") || mapping.name.contains("Temperature") {
                    rng.gen_range(200..800) as f64 // For 0.1 scale, -40 offset = -20°C to 40°C
                } else if mapping.name.contains("pressure") || mapping.name.contains("Pressure") {
                    rng.gen_range(50000..150000) as f64 // Pressure values
                } else if mapping.name.contains("flow") || mapping.name.contains("Flow") {
                    rng.gen_range(500..2000) as f64 // For 0.01 scale = 5-20 L/min
                } else {
                    rng.gen_range(100..1000) as f64
                }
            },
            ModbusDataType::Float64 => rng.gen_range(0.0..10000.0),
            _ => rng.gen_range(0..1000) as f64,
        }
    }

    /// Count point types for statistics
    fn count_point_types(&self, mappings: &[ModbusRegisterMapping]) -> HashMap<String, u32> {
        let mut counts = HashMap::new();
        for mapping in mappings {
            *counts.entry(format!("{:?}", mapping.register_type)).or_insert(0) += 1;
        }
        counts
    }

    /// Get address ranges for statistics
    fn get_address_ranges(&self, mappings: &[ModbusRegisterMapping]) -> HashMap<String, (u16, u16)> {
        let mut ranges = HashMap::new();
        for mapping in mappings {
            ranges.insert(mapping.name.clone(), mapping.address_range());
        }
        ranges
    }

    /// Display current Redis keys
    async fn display_redis_info(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut conn = self.redis_client.get_connection()?;
        
        println!("\n📊 Current Redis Keys:");
        
        for &channel_id in self.channel_configs.keys() {
            println!("\n🔸 Channel {}:", channel_id);
            
            // List config keys
            let config_pattern = format!("{}:channel_{}*", RedisKeys::CONFIG_PREFIX, channel_id);
            let config_keys: Vec<String> = conn.keys(&config_pattern)?;
            println!("   📋 Config: {} keys", config_keys.len());
            for key in &config_keys {
                println!("      - {}", key);
            }
            
            // List point keys
            let points_pattern = format!("{}:channel_{}:*", RedisKeys::POINTS_PREFIX, channel_id);
            let point_keys: Vec<String> = conn.keys(&points_pattern)?;
            println!("   📍 Points: {} keys", point_keys.len());
            
            // List value keys
            let values_pattern = format!("{}:channel_{}:*", RedisKeys::VALUES_PREFIX, channel_id);
            let value_keys: Vec<String> = conn.keys(&values_pattern)?;
            println!("   📈 Values: {} keys", value_keys.len());
            
            // Show sample data
            if !config_keys.is_empty() {
                let sample_config: String = conn.get(&config_keys[0])?;
                println!("   📄 Sample Config Data:");
                let config_value: serde_json::Value = serde_json::from_str(&sample_config)?;
                println!("      Host: {}", config_value["host"]);
                println!("      Slave ID: {}", config_value["slave_id"]);
            }
            
            if !value_keys.is_empty() {
                let sample_value: String = conn.get(&value_keys[0])?;
                let value_data: serde_json::Value = serde_json::from_str(&sample_value)?;
                println!("   📊 Sample Value Data:");
                println!("      Point: {}", value_data["point_id"]);
                println!("      Value: {} {}", value_data["engineering_value"], 
                    value_data["unit"].as_str().unwrap_or(""));
            }
        }
    
    Ok(())
}

    /// Run the demo continuously
    async fn run_demo(&mut self, update_interval_secs: u64) -> Result<(), Box<dyn std::error::Error>> {
        println!("\n🚀 Starting Modbus Redis Demo...");
        println!("📡 Update interval: {} seconds", update_interval_secs);
        println!("🔄 Press Ctrl+C to stop\n");
        
        // Initial data store
        self.store_to_redis().await?;
        
        let mut interval = interval(Duration::from_secs(update_interval_secs));
        let mut iteration = 0;
        
        loop {
            interval.tick().await;
            iteration += 1;
            
            println!("\n⏰ Update #{} at {}", iteration, Utc::now().format("%H:%M:%S"));
            
            // Update simulated values
            if let Err(e) = self.simulate_point_values().await {
                eprintln!("❌ Error updating values: {}", e);
                continue;
            }
            
            // Display Redis info every 5 iterations
            if iteration % 5 == 0 {
                if let Err(e) = self.display_redis_info().await {
                    eprintln!("❌ Error displaying Redis info: {}", e);
                }
            }
            
            println!("✅ Updated {} channels with simulated values", self.channel_configs.len());
        }
    }
}

/// Create sample configuration files
fn create_sample_files() -> Result<(std::path::PathBuf, std::path::PathBuf), Box<dyn std::error::Error>> {
    let config_dir = std::env::temp_dir().join("modbus_demo");
    fs::create_dir_all(&config_dir)?;
    
    // Create sample config file
    let config_content = r#"channel_id: 1
slave_id: 1
host: "192.168.1.100"
port: 502
timeout_ms: 5000
max_retries: 3
poll_interval_ms: 1000
mode: "tcp"
point_table_file: "device_points.csv"
"#;
    
    let config_path = config_dir.join("modbus_config.yaml");
    let mut config_file = File::create(&config_path)?;
    config_file.write_all(config_content.as_bytes())?;
    
    // Create sample CSV file
    let csv_content = r#"id,name,address,type_,data_type,unit,scale,offset,writable,description,byte_order
PT001,Tank_Temperature,1000,input_register,float32,°C,0.1,-40.0,false,Main tank temperature sensor,big_endian
PT002,Tank_Pressure,1002,input_register,uint16,Pa,1.0,0.0,false,Tank pressure sensor,big_endian
PT003,Flow_Rate,1004,input_register,float32,L/min,0.01,0.0,false,Flow rate meter,big_endian
CT001,Pump_Control,2000,holding_register,uint16,,1.0,0.0,true,Pump speed control,big_endian
CT002,Valve_Position,2001,holding_register,uint16,%,0.1,0.0,true,Valve position setpoint,big_endian
ST001,Pump_Status,3000,coil,bool,,1.0,0.0,true,Pump on/off status,big_endian
ST002,Alarm_Status,3001,discrete_input,bool,,1.0,0.0,false,General alarm status,big_endian
AT001,System_Mode,4000,holding_register,uint16,,1.0,0.0,true,System operation mode,big_endian
AT002,Error_Code,4001,input_register,uint16,,1.0,0.0,false,Last error code,big_endian
FT001,Total_Volume,5000,input_register,uint32,L,1.0,0.0,false,Cumulative volume,big_endian
PT004,Outlet_Temperature,1006,input_register,float32,°C,0.1,-40.0,false,Outlet temperature sensor,big_endian
PT005,Flow_Velocity,1008,input_register,float32,m/s,0.001,0.0,false,Flow velocity sensor,big_endian"#;
    
    let csv_path = config_dir.join("device_points.csv");
    let mut csv_file = File::create(&csv_path)?;
    csv_file.write_all(csv_content.as_bytes())?;
    
    println!("📁 Created sample files in: {}", config_dir.display());
    println!("   📄 Config: {}", config_path.display());
    println!("   📊 CSV: {}", csv_path.display());
    
    Ok((config_path, csv_path))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🔧 Modbus Redis Integration Demo");
    println!("================================\n");
    
    // Create sample files
    let (config_path, csv_path) = create_sample_files()?;
    
    // Initialize Redis manager
    let redis_url = "redis://127.0.0.1:6379";
    let mut manager = ModbusRedisManager::new(redis_url)?;
    
    // Load configuration and point table
    manager.load_config(&config_path)?;
    manager.load_point_table(1, &csv_path)?;
    
    println!("\n🔍 Quick Redis inspection commands:");
    println!("   redis-cli KEYS \"modbus:*\"");
    println!("   redis-cli GET modbus:config:channel_1");
    println!("   redis-cli KEYS \"modbus:points:channel_1:*\"");
    println!("   redis-cli KEYS \"modbus:values:channel_1:*\"");
    println!("   redis-cli GET modbus:values:channel_1:PT001");
    
    // Run the demo
    manager.run_demo(10).await?; // Update every 10 seconds
    
    Ok(())
} 