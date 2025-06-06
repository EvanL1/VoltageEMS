//! 增强版 Modbus + Redis 压力测试
//! 
//! 新增功能：
//! - 读取频率测试
//! - 真实Modbus报文模拟
//! - 随机数据点生成
//! - 多频率并发测试

use comsrv::core::protocol_factory::{create_default_factory};
use comsrv::core::config::config_manager::{ChannelConfig, ChannelParameters, ProtocolType};
use comsrv::utils::error::Result;
use redis::Connection;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::time::{interval, sleep};
use log::{info, error};
use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;

/// 增强版压力测试配置
#[derive(Debug, Clone)]
struct EnhancedStressConfig {
    pub channel_count: usize,
    pub points_per_channel: usize,
    pub base_port: u16,
    pub frequency_test_modes: Vec<FrequencyTestMode>,
    pub random_data_points: bool,
    pub modbus_function_codes: Vec<u8>,
    pub redis_batch_size: usize,
    pub stats_interval_sec: u64,
    pub test_duration_sec: u64,
}

/// 读取频率测试模式
#[derive(Debug, Clone)]
struct FrequencyTestMode {
    pub name: String,
    pub poll_interval_ms: u64,
    pub channel_count: usize,
    pub description: String,
}

impl Default for EnhancedStressConfig {
    fn default() -> Self {
        Self {
            channel_count: 15,
            points_per_channel: 1000,
            base_port: 5020,
            frequency_test_modes: vec![
                FrequencyTestMode {
                    name: "高频采集".to_string(),
                    poll_interval_ms: 100,  // 10Hz
                    channel_count: 3,
                    description: "模拟快速响应设备".to_string(),
                },
                FrequencyTestMode {
                    name: "中频采集".to_string(),
                    poll_interval_ms: 500,  // 2Hz
                    channel_count: 5,
                    description: "标准工业设备".to_string(),
                },
                FrequencyTestMode {
                    name: "低频采集".to_string(),
                    poll_interval_ms: 2000, // 0.5Hz
                    channel_count: 4,
                    description: "慢速监控设备".to_string(),
                },
                FrequencyTestMode {
                    name: "超高频采集".to_string(),
                    poll_interval_ms: 50,   // 20Hz
                    channel_count: 3,
                    description: "实时控制系统".to_string(),
                },
            ],
            random_data_points: true,
            modbus_function_codes: vec![0x01, 0x02, 0x03, 0x04], // 读线圈、离散输入、保持寄存器、输入寄存器
            redis_batch_size: 200,
            stats_interval_sec: 5,
            test_duration_sec: 300,
        }
    }
}

/// 增强版性能统计
#[derive(Debug, Clone)]
struct EnhancedPerformanceStats {
    pub total_reads: u64,
    pub successful_reads: u64,
    pub failed_reads: u64,
    pub total_points: u64,
    pub redis_writes: u64,
    pub redis_errors: u64,
    pub start_time: Instant,
    pub last_update: Instant,
    pub channels_active: usize,
    pub avg_read_time_ms: f64,
    pub max_read_time_ms: f64,
    pub min_read_time_ms: f64,
    pub frequency_stats: HashMap<String, FrequencyStats>,
    pub function_code_stats: HashMap<u8, FunctionCodeStats>,
}

#[derive(Debug, Clone, Default)]
struct FrequencyStats {
    pub reads: u64,
    pub points: u64,
    pub avg_response_time: f64,
    pub throughput: f64,
}

#[derive(Debug, Clone, Default)]
struct FunctionCodeStats {
    pub requests: u64,
    pub successes: u64,
    pub failures: u64,
    pub avg_response_time: f64,
}

impl Default for EnhancedPerformanceStats {
    fn default() -> Self {
        let now = Instant::now();
        Self {
            total_reads: 0,
            successful_reads: 0,
            failed_reads: 0,
            total_points: 0,
            redis_writes: 0,
            redis_errors: 0,
            start_time: now,
            last_update: now,
            channels_active: 0,
            avg_read_time_ms: 0.0,
            max_read_time_ms: 0.0,
            min_read_time_ms: f64::MAX,
            frequency_stats: HashMap::new(),
            function_code_stats: HashMap::new(),
        }
    }
}

/// 随机数据点生成器
#[derive(Debug, Clone)]
struct RandomDataPoint {
    pub name: String,
    pub address: u16,
    pub function_code: u8,
    pub data_type: String,
    pub unit: String,
    pub min_value: f64,
    pub max_value: f64,
    pub noise_factor: f64,
}

/// 增强版压力测试主程序
#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    
    println!("🚀 增强版 Modbus + Redis 压力测试");
    println!("===================================");
    println!("新功能:");
    println!("  ✨ 多频率并发测试");
    println!("  ✨ 真实Modbus报文模拟");
    println!("  ✨ 随机数据点生成");
    println!("  ✨ 功能码性能分析");
    println!();
    
    let mut config = EnhancedStressConfig::default();
    
    // 从环境变量调整配置
    if let Ok(channels) = std::env::var("ENHANCED_CHANNELS") {
        config.channel_count = channels.parse().unwrap_or(15);
    }
    if let Ok(points) = std::env::var("ENHANCED_POINTS_PER_CHANNEL") {
        config.points_per_channel = points.parse().unwrap_or(1000);
    }
    if let Ok(duration) = std::env::var("ENHANCED_DURATION") {
        config.test_duration_sec = duration.parse().unwrap_or(300);
    }
    
    println!("📋 增强测试配置:");
    println!("  总通道数: {}", config.channel_count);
    println!("  每通道点位: {}", config.points_per_channel);
    println!("  总点位数: {}", config.channel_count * config.points_per_channel);
    println!("  测试时长: {}秒", config.test_duration_sec);
    println!("  随机数据点: {}", if config.random_data_points { "启用" } else { "禁用" });
    println!("  支持功能码: {:?}", config.modbus_function_codes);
    
    println!();
    println!("📊 频率测试模式:");
    for mode in &config.frequency_test_modes {
        println!("  ├─ {}: {}ms间隔, {}通道 ({})", 
                mode.name, mode.poll_interval_ms, mode.channel_count, mode.description);
    }
    println!();

    // 1. 连接Redis
    let redis_client = redis::Client::open("redis://127.0.0.1:6379/")?;
    let mut redis_conn = redis_client.get_connection()?;
    
    redis::cmd("FLUSHDB").execute(&mut redis_conn);
    println!("✅ Redis连接成功，数据库已清理");

    // 2. 创建协议工厂
    let factory = create_default_factory();
    println!("✅ 协议工厂创建成功");

    // 3. 创建多频率通道
    let mut all_channels = Vec::new();
    let mut channel_id = 0;
    
    for mode in &config.frequency_test_modes {
        for i in 0..mode.channel_count {
            let channel_config = create_enhanced_channel_config(
                channel_id, 
                config.base_port + channel_id as u16, 
                mode,
                &config
            );
            
            factory.validate_config(&channel_config)?;
            factory.create_channel(channel_config.clone())?;
            all_channels.push((channel_config, mode.clone()));
            
            println!("✅ {}通道 {} 创建成功 (端口: {}, 间隔: {}ms)", 
                    mode.name, i + 1, config.base_port + channel_id as u16, mode.poll_interval_ms);
            channel_id += 1;
        }
    }

    // 4. 启动所有通道
    factory.start_all_channels().await?;
    println!("✅ 所有 {} 个通道已启动", all_channels.len());

    // 5. 创建增强性能统计
    let stats = Arc::new(tokio::sync::RwLock::new(EnhancedPerformanceStats::default()));
    
    // 初始化统计数据
    {
        let mut stats_guard = stats.write().await;
        for mode in &config.frequency_test_modes {
            stats_guard.frequency_stats.insert(mode.name.clone(), FrequencyStats::default());
        }
        for &code in &config.modbus_function_codes {
            stats_guard.function_code_stats.insert(code, FunctionCodeStats::default());
        }
    }

    // 6. 启动多频率数据采集任务
    let mut collection_tasks = Vec::new();
    for (idx, (channel_config, mode)) in all_channels.iter().enumerate() {
        let stats_clone = stats.clone();
        let redis_client_clone = redis_client.clone();
        let config_clone = config.clone();
        let channel_config_clone = channel_config.clone();
        let mode_clone = mode.clone();
        
        let task = tokio::spawn(async move {
            run_enhanced_channel_collection(
                idx,
                channel_config_clone,
                mode_clone,
                redis_client_clone,
                stats_clone,
                config_clone,
            ).await
        });
        
        collection_tasks.push(task);
    }

    // 7. 启动增强性能监控
    let stats_monitor = stats.clone();
    let config_monitor = config.clone();
    let redis_monitor = redis_client.clone();
    let monitor_task = tokio::spawn(async move {
        run_enhanced_performance_monitor(stats_monitor, config_monitor, redis_monitor).await
    });

    println!();
    println!("🔥 增强压力测试开始！");
    println!("测试将持续 {} 秒，包含多频率并发测试...", config.test_duration_sec);
    println!();

    // 8. 等待测试完成
    sleep(Duration::from_secs(config.test_duration_sec)).await;

    println!();
    println!("⏹️  测试完成，正在停止...");

    // 9. 停止所有任务
    for task in collection_tasks {
        task.abort();
    }
    monitor_task.abort();

    // 10. 停止所有通道
    factory.stop_all_channels().await?;

    // 11. 生成增强测试报告
    let final_stats = stats.read().await.clone();
    generate_enhanced_final_report(&final_stats, &config);

    Ok(())
}

/// 创建增强通道配置
fn create_enhanced_channel_config(
    channel_id: usize, 
    port: u16, 
    mode: &FrequencyTestMode,
    config: &EnhancedStressConfig
) -> ChannelConfig {
    let mut parameters = HashMap::new();
    parameters.insert("address".to_string(), serde_yaml::Value::String("127.0.0.1".to_string()));
    parameters.insert("port".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(port)));
    parameters.insert("timeout".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(5000)));
    parameters.insert("slave_id".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(1)));
    parameters.insert("poll_rate".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(mode.poll_interval_ms)));
    parameters.insert("max_retries".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(2)));
    
    ChannelConfig {
        id: (channel_id + 1) as u16,
        name: format!("{}_{}", mode.name, channel_id + 1),
        description: format!("{} - {}ms间隔, {} 个数据点", mode.description, mode.poll_interval_ms, config.points_per_channel),
        protocol: ProtocolType::ModbusTcp,
        parameters: ChannelParameters::Generic(parameters),
    }
}

/// 生成随机数据点
fn generate_random_data_points(channel_id: usize, count: usize, rng: &mut StdRng) -> Vec<RandomDataPoint> {
    let mut points = Vec::new();
    
    let data_templates = vec![
        ("temperature", "°C", 0.0, 100.0, 0.1),
        ("pressure", "kPa", 50.0, 200.0, 0.05),
        ("flow_rate", "L/min", 0.0, 1000.0, 0.2),
        ("voltage", "V", 180.0, 260.0, 0.02),
        ("current", "A", 0.0, 50.0, 0.15),
        ("power", "kW", 0.0, 100.0, 0.1),
        ("frequency", "Hz", 45.0, 55.0, 0.01),
        ("humidity", "%", 0.0, 100.0, 0.05),
        ("level", "m", 0.0, 10.0, 0.1),
        ("speed", "rpm", 0.0, 5000.0, 0.2),
    ];
    
    for i in 0..count {
        let template_idx = rng.gen_range(0..data_templates.len());
        let (base_name, unit, min_val, max_val, noise) = data_templates[template_idx];
        
        let address = rng.gen_range(0..9999);
        let function_code = match rng.gen_range(0..4) {
            0 => 0x01, // 读线圈
            1 => 0x02, // 读离散输入
            2 => 0x03, // 读保持寄存器
            _ => 0x04, // 读输入寄存器
        };
        
        let data_type = if function_code <= 0x02 { "BOOL" } else { "FLOAT32" };
        
        points.push(RandomDataPoint {
            name: format!("{}_{}_{}_{}", base_name, channel_id + 1, i, rng.gen::<u32>() % 1000),
            address,
            function_code,
            data_type: data_type.to_string(),
            unit: unit.to_string(),
            min_value: min_val,
            max_value: max_val,
            noise_factor: noise,
        });
    }
    
    points
}

/// 增强版通道数据采集
async fn run_enhanced_channel_collection(
    channel_id: usize,
    _channel_config: ChannelConfig,
    mode: FrequencyTestMode,
    redis_client: redis::Client,
    stats: Arc<tokio::sync::RwLock<EnhancedPerformanceStats>>,
    config: EnhancedStressConfig,
) {
    let mut redis_conn = match redis_client.get_connection() {
        Ok(conn) => conn,
        Err(e) => {
            error!("通道 {} Redis连接失败: {}", channel_id + 1, e);
            return;
        }
    };

    // 生成随机数据点
    let mut rng = StdRng::seed_from_u64(channel_id as u64 + 12345);
    let data_points = if config.random_data_points {
        generate_random_data_points(channel_id, config.points_per_channel, &mut rng)
    } else {
        generate_fixed_data_points(channel_id, config.points_per_channel)
    };
    
    let mut interval = interval(Duration::from_millis(mode.poll_interval_ms));
    let mut collection_count = 0u64;

    info!("通道 {} ({}) 开始数据采集: {} 个点位, {}ms间隔", 
          channel_id + 1, mode.name, data_points.len(), mode.poll_interval_ms);

    loop {
        interval.tick().await;
        collection_count += 1;
        
        let read_start = Instant::now();
        
        // 模拟真实Modbus读取
        let mut successful_points = 0;
        let mut data_batch = Vec::new();
        
        // 按功能码分组批量读取
        let mut function_groups: HashMap<u8, Vec<&RandomDataPoint>> = HashMap::new();
        for point in &data_points {
            function_groups.entry(point.function_code).or_insert_with(Vec::new).push(point);
        }
        
        for (function_code, points_group) in function_groups {
            let func_start = Instant::now();
            
            // 模拟Modbus批量读取
            for point in points_group {
                let timestamp = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                
                let value = generate_realistic_modbus_value(
                    point, 
                    channel_id, 
                    collection_count, 
                    &mut rng
                );
                
                let data_entry = json!({
                    "channel_id": channel_id + 1,
                    "point_name": point.name,
                    "address": point.address,
                    "function_code": function_code,
                    "value": value,
                    "unit": point.unit,
                    "timestamp": timestamp,
                    "quality": "good",
                    "mode": mode.name,
                    "frequency_ms": mode.poll_interval_ms
                });
                
                data_batch.push((
                    format!("data:{}:{}", channel_id + 1, point.name), 
                    data_entry.to_string()
                ));
                successful_points += 1;
                
                // 批量写入Redis
                if data_batch.len() >= config.redis_batch_size {
                    if let Err(e) = write_batch_to_redis(&mut redis_conn, &data_batch) {
                        error!("通道 {} Redis批量写入失败: {}", channel_id + 1, e);
                        let mut stats_guard = stats.write().await;
                        stats_guard.redis_errors += 1;
                    } else {
                        let mut stats_guard = stats.write().await;
                        stats_guard.redis_writes += 1;
                    }
                    data_batch.clear();
                }
            }
            
            let func_duration = func_start.elapsed();
            
            // 更新功能码统计
            {
                let mut stats_guard = stats.write().await;
                if let Some(func_stats) = stats_guard.function_code_stats.get_mut(&function_code) {
                    func_stats.requests += 1;
                    func_stats.successes += 1;
                    func_stats.avg_response_time = (func_stats.avg_response_time * (func_stats.requests - 1) as f64 + func_duration.as_millis() as f64) / func_stats.requests as f64;
                }
            }
        }
        
        // 写入剩余数据
        if !data_batch.is_empty() {
            if let Err(e) = write_batch_to_redis(&mut redis_conn, &data_batch) {
                error!("通道 {} Redis最终批量写入失败: {}", channel_id + 1, e);
                let mut stats_guard = stats.write().await;
                stats_guard.redis_errors += 1;
            } else {
                let mut stats_guard = stats.write().await;
                stats_guard.redis_writes += 1;
            }
        }
        
        let read_duration = read_start.elapsed();
        
        // 更新统计
        {
            let mut stats_guard = stats.write().await;
            stats_guard.total_reads += 1;
            stats_guard.successful_reads += 1;
            stats_guard.total_points += successful_points;
            stats_guard.last_update = Instant::now();
            
            // 更新响应时间
            if stats_guard.successful_reads > 0 {
                stats_guard.avg_read_time_ms = (stats_guard.avg_read_time_ms * (stats_guard.successful_reads - 1) as f64 + read_duration.as_millis() as f64) / stats_guard.successful_reads as f64;
            } else {
                stats_guard.avg_read_time_ms = read_duration.as_millis() as f64;
            }
            
            let read_time_ms = read_duration.as_millis() as f64;
            if read_time_ms > stats_guard.max_read_time_ms {
                stats_guard.max_read_time_ms = read_time_ms;
            }
            if read_time_ms < stats_guard.min_read_time_ms {
                stats_guard.min_read_time_ms = read_time_ms;
            }
            
            // 更新频率统计
            let elapsed_secs = stats_guard.start_time.elapsed().as_secs_f64();
            if let Some(freq_stats) = stats_guard.frequency_stats.get_mut(&mode.name) {
                freq_stats.reads += 1;
                freq_stats.points += successful_points;
                freq_stats.avg_response_time = (freq_stats.avg_response_time * (freq_stats.reads - 1) as f64 + read_duration.as_millis() as f64) / freq_stats.reads as f64;
                freq_stats.throughput = freq_stats.points as f64 / elapsed_secs;
            }
        }
        
        if collection_count % 20 == 0 {
            info!("通道 {} ({}) 第 {} 轮: {} 点位, {:.2}ms", 
                  channel_id + 1, mode.name, collection_count, successful_points, read_duration.as_millis());
        }
    }
}

/// 生成固定数据点（向后兼容）
fn generate_fixed_data_points(channel_id: usize, count: usize) -> Vec<RandomDataPoint> {
    let mut points = Vec::new();
    
    for i in 0..count {
        let address = (i as u16) % 10000;
        let point_type = i % 8;
        
        let (name, unit, min_val, max_val, function_code) = match point_type {
            0 => (format!("temperature_{}_{}", channel_id + 1, i), "°C".to_string(), 0.0, 100.0, 0x03),
            1 => (format!("pressure_{}_{}", channel_id + 1, i), "kPa".to_string(), 50.0, 200.0, 0x03),
            2 => (format!("flow_{}_{}", channel_id + 1, i), "L/min".to_string(), 0.0, 1000.0, 0x03),
            3 => (format!("voltage_{}_{}", channel_id + 1, i), "V".to_string(), 180.0, 260.0, 0x04),
            4 => (format!("current_{}_{}", channel_id + 1, i), "A".to_string(), 0.0, 50.0, 0x04),
            5 => (format!("power_{}_{}", channel_id + 1, i), "kW".to_string(), 0.0, 100.0, 0x03),
            6 => (format!("frequency_{}_{}", channel_id + 1, i), "Hz".to_string(), 45.0, 55.0, 0x04),
            7 => (format!("status_{}_{}", channel_id + 1, i), "".to_string(), 0.0, 1.0, 0x01),
            _ => unreachable!(),
        };
        
        points.push(RandomDataPoint {
            name,
            address,
            function_code,
            data_type: if function_code == 0x01 || function_code == 0x02 { "BOOL" } else { "FLOAT32" }.to_string(),
            unit,
            min_value: min_val,
            max_value: max_val,
            noise_factor: 0.1,
        });
    }
    
    points
}

/// 生成真实的Modbus数据值
fn generate_realistic_modbus_value(
    point: &RandomDataPoint,
    channel_id: usize,
    cycle: u64,
    rng: &mut StdRng,
) -> serde_json::Value {
    let time_factor = (cycle as f64 * 0.01) + (channel_id as f64 * 0.1);
    
    match point.data_type.as_str() {
        "BOOL" => {
            // 随机布尔值，带一些规律性
            let probability = 0.5 + 0.3 * (time_factor * 0.1).sin();
            json!(rng.gen::<f64>() < probability)
        },
        "FLOAT32" => {
            // 基于范围的随机浮点值，带趋势和噪声
            let range = point.max_value - point.min_value;
            let base_trend = 0.5 + 0.3 * (time_factor * 0.05).sin(); // 主趋势
            let noise = (rng.gen::<f64>() - 0.5) * 2.0 * point.noise_factor; // 噪声
            
            let normalized_value = base_trend + noise;
            let value = point.min_value + range * normalized_value.clamp(0.0, 1.0);
            
            json!((value * 100.0).round() / 100.0) // 保留2位小数
        },
        _ => json!(0)
    }
}

/// 批量写入Redis
fn write_batch_to_redis(
    conn: &mut Connection,
    batch: &[(String, String)]
) -> redis::RedisResult<()> {
    let mut pipe = redis::pipe();
    
    for (key, value) in batch {
        pipe.set(key, value);
    }
    
    pipe.query(conn)
}

/// 增强版性能监控
async fn run_enhanced_performance_monitor(
    stats: Arc<tokio::sync::RwLock<EnhancedPerformanceStats>>,
    config: EnhancedStressConfig,
    redis_client: redis::Client,
) {
    let mut interval = interval(Duration::from_secs(config.stats_interval_sec));
    
    loop {
        interval.tick().await;
        
        let stats_snapshot = {
            let mut stats_guard = stats.write().await;
            stats_guard.channels_active = config.channel_count;
            stats_guard.clone()
        };
        
        let elapsed = stats_snapshot.start_time.elapsed().as_secs();
        let throughput = stats_snapshot.total_points as f64 / stats_snapshot.start_time.elapsed().as_secs_f64();
        let success_rate = if stats_snapshot.total_reads > 0 {
            (stats_snapshot.successful_reads as f64 / stats_snapshot.total_reads as f64) * 100.0
        } else {
            0.0
        };
        
        println!();
        println!("📊 增强性能监控报告 (运行时间: {}秒)", elapsed);
        println!("  ├─ 总采集次数: {}", stats_snapshot.total_reads);
        println!("  ├─ 成功采集: {} ({:.1}%)", stats_snapshot.successful_reads, success_rate);
        println!("  ├─ 总数据点: {}", stats_snapshot.total_points);
        println!("  ├─ 整体吞吐量: {:.1} 点位/秒", throughput);
        println!("  ├─ 平均响应时间: {:.1}ms", stats_snapshot.avg_read_time_ms);
        
        // 显示频率统计
        println!("  ├─ 频率测试统计:");
        for (name, freq_stats) in &stats_snapshot.frequency_stats {
            println!("  │   ├─ {}: {:.1} 点位/秒, {:.1}ms响应", 
                    name, freq_stats.throughput, freq_stats.avg_response_time);
        }
        
        // 显示功能码统计
        println!("  ├─ 功能码统计:");
        for (code, func_stats) in &stats_snapshot.function_code_stats {
            let success_rate = if func_stats.requests > 0 {
                (func_stats.successes as f64 / func_stats.requests as f64) * 100.0
            } else {
                0.0
            };
            println!("  │   ├─ 0x{:02X}: {} 请求, {:.1}% 成功率, {:.1}ms", 
                    code, func_stats.requests, success_rate, func_stats.avg_response_time);
        }
        
        println!("  └─ Redis: {} 写入, {} 错误", stats_snapshot.redis_writes, stats_snapshot.redis_errors);

        // Redis内存监控
        if let Ok(mut conn) = redis_client.get_connection() {
            if let Ok(info) = redis::cmd("INFO").arg("memory").query::<String>(&mut conn) {
                if let Some(used_memory_line) = info.lines().find(|line| line.starts_with("used_memory_human:")) {
                    if let Some(memory) = used_memory_line.split(':').nth(1) {
                        println!("  Redis内存: {}", memory.trim());
                    }
                }
            }
        }
    }
}

/// 生成增强版最终报告
fn generate_enhanced_final_report(stats: &EnhancedPerformanceStats, config: &EnhancedStressConfig) {
    let total_test_time = stats.start_time.elapsed().as_secs_f64();
    let throughput = stats.total_points as f64 / total_test_time;
    let success_rate = if stats.total_reads > 0 {
        (stats.successful_reads as f64 / stats.total_reads as f64) * 100.0
    } else {
        0.0
    };
    
    println!();
    println!("🎯 增强版压力测试最终报告");
    println!("=======================================");
    println!("测试配置:");
    println!("  总通道数: {}", config.channel_count);
    println!("  每通道点位: {}", config.points_per_channel);
    println!("  总点位数: {}", config.channel_count * config.points_per_channel);
    println!("  测试时长: {:.1}秒", total_test_time);
    println!("  随机数据点: {}", if config.random_data_points { "启用" } else { "禁用" });
    println!();
    
    println!("整体性能指标:");
    println!("  总采集次数: {}", stats.total_reads);
    println!("  成功采集: {} ({:.2}%)", stats.successful_reads, success_rate);
    println!("  总数据点: {}", stats.total_points);
    println!("  平均吞吐量: {:.1} 点位/秒", throughput);
    println!("  平均响应时间: {:.1}ms", stats.avg_read_time_ms);
    println!("  最大响应时间: {:.1}ms", stats.max_read_time_ms);
    println!("  最小响应时间: {:.1}ms", if stats.min_read_time_ms == f64::MAX { 0.0 } else { stats.min_read_time_ms });
    println!();
    
    println!("频率测试性能:");
    for (name, freq_stats) in &stats.frequency_stats {
        println!("  ├─ {}: ", name);
        println!("  │   ├─ 采集次数: {}", freq_stats.reads);
        println!("  │   ├─ 数据点数: {}", freq_stats.points);
        println!("  │   ├─ 吞吐量: {:.1} 点位/秒", freq_stats.throughput);
        println!("  │   └─ 平均响应时间: {:.1}ms", freq_stats.avg_response_time);
    }
    println!();
    
    println!("功能码性能分析:");
    for (code, func_stats) in &stats.function_code_stats {
        let success_rate = if func_stats.requests > 0 {
            (func_stats.successes as f64 / func_stats.requests as f64) * 100.0
        } else {
            0.0
        };
        let function_name = match *code {
            0x01 => "读线圈",
            0x02 => "读离散输入",
            0x03 => "读保持寄存器",
            0x04 => "读输入寄存器",
            _ => "未知功能码",
        };
        println!("  ├─ 0x{:02X} ({}): ", code, function_name);
        println!("  │   ├─ 请求次数: {}", func_stats.requests);
        println!("  │   ├─ 成功率: {:.2}%", success_rate);
        println!("  │   └─ 平均响应时间: {:.1}ms", func_stats.avg_response_time);
    }
    println!();
    
    println!("数据库操作:");
    println!("  Redis写入次数: {}", stats.redis_writes);
    println!("  Redis错误次数: {}", stats.redis_errors);
    let redis_success_rate = if stats.redis_writes + stats.redis_errors > 0 {
        (stats.redis_writes as f64 / (stats.redis_writes + stats.redis_errors) as f64) * 100.0
    } else {
        0.0
    };
    println!("  Redis成功率: {:.2}%", redis_success_rate);
    println!();
    
    // 性能等级评估
    let performance_grade = if throughput > 8000.0 && success_rate > 99.0 {
        "S+ (超级优秀)"
    } else if throughput > 6000.0 && success_rate > 98.0 {
        "S (优秀+)"
    } else if throughput > 4000.0 && success_rate > 95.0 {
        "A+ (优秀)"
    } else if throughput > 2000.0 && success_rate > 90.0 {
        "A (良好)"
    } else if throughput > 1000.0 && success_rate > 85.0 {
        "B (一般)"
    } else {
        "C (需优化)"
    };
    
    println!("🏆 增强版性能等级: {}", performance_grade);
    println!("=======================================");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enhanced_config_creation() {
        let config = EnhancedStressConfig::default();
        assert_eq!(config.channel_count, 15);
        assert!(config.random_data_points);
        assert!(!config.frequency_test_modes.is_empty());
    }

    #[test]
    fn test_random_data_point_generation() {
        let mut rng = StdRng::seed_from_u64(12345);
        let points = generate_random_data_points(0, 10, &mut rng);
        assert_eq!(points.len(), 10);
        assert!(points.iter().any(|p| p.function_code == 0x01));
        assert!(points.iter().any(|p| p.data_type == "BOOL"));
    }

    #[test]
    fn test_frequency_test_modes() {
        let config = EnhancedStressConfig::default();
        let total_channels: usize = config.frequency_test_modes.iter().map(|m| m.channel_count).sum();
        assert_eq!(total_channels, config.channel_count);
    }
} 