//! Modbus + Redis 大规模压力测试
//! 
//! 此程序用于测试comsrv在高负载下的性能表现：
//! - 10个或更多Modbus TCP通道
//! - 每个通道1000个或更多数据点
//! - 实时性能监控和统计
//! - Redis批量数据存储

use comsrv::core::protocol_factory::{create_default_factory, ProtocolFactory};
use comsrv::core::config::config_manager::{ChannelConfig, ChannelParameters};
use comsrv::core::config::config_manager::ProtocolType;
use comsrv::utils::error::Result;
use redis::{Commands, Connection};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::time::{interval, sleep};
use tokio::task::JoinHandle;
use log::{info, warn, error};

/// 压力测试配置
#[derive(Debug, Clone)]
struct StressTestConfig {
    pub channel_count: usize,
    pub points_per_channel: usize,
    pub base_port: u16,
    pub poll_interval_ms: u64,
    pub redis_batch_size: usize,
    pub stats_interval_sec: u64,
    pub test_duration_sec: u64,
}

impl Default for StressTestConfig {
    fn default() -> Self {
        Self {
            channel_count: 10,
            points_per_channel: 1000,
            base_port: 5020,  // 从5020开始，避免和502冲突
            poll_interval_ms: 1000,  // 1秒采集间隔
            redis_batch_size: 100,   // Redis批量写入大小
            stats_interval_sec: 10,  // 10秒统计间隔
            test_duration_sec: 300,  // 5分钟测试时间
        }
    }
}

/// 性能统计数据
#[derive(Debug, Clone)]
struct PerformanceStats {
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
}

impl Default for PerformanceStats {
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
        }
    }
}

impl PerformanceStats {
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            start_time: now,
            last_update: now,
            min_read_time_ms: f64::MAX,
            ..Default::default()
        }
    }

    pub fn update_read_time(&mut self, duration_ms: f64) {
        if self.successful_reads > 0 {
            self.avg_read_time_ms = (self.avg_read_time_ms * self.successful_reads as f64 + duration_ms) / (self.successful_reads as f64 + 1.0);
        } else {
            self.avg_read_time_ms = duration_ms;
        }
        
        if duration_ms > self.max_read_time_ms {
            self.max_read_time_ms = duration_ms;
        }
        if duration_ms < self.min_read_time_ms {
            self.min_read_time_ms = duration_ms;
        }
    }

    pub fn throughput_per_sec(&self) -> f64 {
        let elapsed = self.start_time.elapsed().as_secs_f64();
        if elapsed > 0.0 {
            self.total_points as f64 / elapsed
        } else {
            0.0
        }
    }

    pub fn success_rate(&self) -> f64 {
        if self.total_reads > 0 {
            (self.successful_reads as f64 / self.total_reads as f64) * 100.0
        } else {
            0.0
        }
    }
}

/// 数据点定义
#[derive(Debug, Clone)]
struct DataPoint {
    pub name: String,
    pub address: u16,
    pub data_type: String,
    pub unit: String,
    pub description: String,
}

/// 压力测试主程序
#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();
    
    println!("🚀 Modbus + Redis 大规模压力测试");
    println!("================================");
    
    let mut config = StressTestConfig::default();
    
    // 从环境变量或命令行参数调整配置
    if let Ok(channels) = std::env::var("STRESS_CHANNELS") {
        config.channel_count = channels.parse().unwrap_or(10);
    }
    if let Ok(points) = std::env::var("STRESS_POINTS_PER_CHANNEL") {
        config.points_per_channel = points.parse().unwrap_or(1000);
    }
    if let Ok(duration) = std::env::var("STRESS_DURATION") {
        config.test_duration_sec = duration.parse().unwrap_or(300);
    }
    
    println!("📋 测试配置:");
    println!("  通道数量: {}", config.channel_count);
    println!("  每通道点位: {}", config.points_per_channel);
    println!("  总点位数: {}", config.channel_count * config.points_per_channel);
    println!("  采集间隔: {}ms", config.poll_interval_ms);
    println!("  测试时长: {}秒", config.test_duration_sec);
    println!();

    // 1. 连接Redis
    let redis_client = redis::Client::open("redis://127.0.0.1:6379/")?;
    let mut redis_conn = redis_client.get_connection()?;
    
    // 清理Redis
    redis::cmd("FLUSHDB").execute(&mut redis_conn);
    println!("✅ Redis连接成功，数据库已清理");

    // 2. 创建协议工厂
    let factory = create_default_factory();
    println!("✅ 协议工厂创建成功");

    // 3. 创建多个通道
    let mut channels = Vec::new();
    for i in 0..config.channel_count {
        let channel_config = create_channel_config(i, config.base_port + i as u16, &config);
        
        // 验证配置
        factory.validate_config(&channel_config)?;
        
        // 创建通道
        factory.create_channel(channel_config.clone())?;
        channels.push(channel_config);
        
        println!("✅ 通道 {} 创建成功 (端口: {})", i + 1, config.base_port + i as u16);
    }

    // 4. 启动所有通道
    factory.start_all_channels().await?;
    println!("✅ 所有通道已启动");

    // 5. 创建性能统计
    let stats = Arc::new(tokio::sync::RwLock::new(PerformanceStats::new()));
    
    // 6. 启动数据采集任务
    let mut collection_tasks = Vec::new();
    for (channel_id, channel_config) in channels.iter().enumerate() {
        let stats_clone = stats.clone();
        let redis_client_clone = redis_client.clone();
        let config_clone = config.clone();
        let channel_config_clone = channel_config.clone();
        
        let task = tokio::spawn(async move {
            run_channel_collection(
                channel_id,
                channel_config_clone,
                redis_client_clone,
                stats_clone,
                config_clone,
            ).await
        });
        
        collection_tasks.push(task);
    }

    // 7. 启动性能监控任务
    let stats_monitor = stats.clone();
    let config_monitor = config.clone();
    let redis_monitor = redis_client.clone();
    let monitor_task = tokio::spawn(async move {
        run_performance_monitor(stats_monitor, config_monitor, redis_monitor).await
    });

    // 8. 启动Redis内存监控
    let redis_memory_monitor = redis_client.clone();
    let memory_task = tokio::spawn(async move {
        run_memory_monitor(redis_memory_monitor).await
    });

    println!();
    println!("🔥 压力测试开始！");
    println!("测试将持续 {} 秒...", config.test_duration_sec);
    println!("提示: 使用环境变量可调整配置:");
    println!("  STRESS_CHANNELS=20 STRESS_POINTS_PER_CHANNEL=2000 STRESS_DURATION=600");
    println!();

    // 9. 等待测试完成
    sleep(Duration::from_secs(config.test_duration_sec)).await;

    println!();
    println!("⏹️  测试时间结束，正在停止...");

    // 10. 停止所有任务
    for task in collection_tasks {
        task.abort();
    }
    monitor_task.abort();
    memory_task.abort();

    // 11. 停止所有通道
    factory.stop_all_channels().await?;

    // 12. 生成最终报告
    let final_stats = stats.read().await.clone();
    generate_final_report(&final_stats, &config);

    Ok(())
}

/// 创建通道配置
fn create_channel_config(channel_id: usize, port: u16, config: &StressTestConfig) -> ChannelConfig {
    let mut parameters = HashMap::new();
    parameters.insert("address".to_string(), serde_yaml::Value::String("127.0.0.1".to_string()));
    parameters.insert("port".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(port)));
    parameters.insert("timeout".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(5000)));
    parameters.insert("slave_id".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(1)));
    parameters.insert("poll_rate".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(config.poll_interval_ms)));
    parameters.insert("max_retries".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(3)));
    
    ChannelConfig {
        id: (channel_id + 1) as u16,
        name: format!("压力测试通道_{}", channel_id + 1),
        description: format!("压力测试通道 {} - {} 个数据点", channel_id + 1, config.points_per_channel),
        protocol: ProtocolType::ModbusTcp,
        parameters: ChannelParameters::Generic(parameters),
    }
}

/// 生成数据点定义
fn generate_data_points(channel_id: usize, count: usize) -> Vec<DataPoint> {
    let mut points = Vec::new();
    
    for i in 0..count {
        let address = (i as u16) % 10000; // 防止地址溢出
        let point_type = i % 8;
        
        let (name, data_type, unit, description) = match point_type {
            0 => (format!("temperature_{}_{}", channel_id + 1, i), "FLOAT32".to_string(), "°C".to_string(), "温度传感器".to_string()),
            1 => (format!("pressure_{}_{}", channel_id + 1, i), "FLOAT32".to_string(), "kPa".to_string(), "压力传感器".to_string()),
            2 => (format!("flow_{}_{}", channel_id + 1, i), "FLOAT32".to_string(), "L/min".to_string(), "流量计".to_string()),
            3 => (format!("voltage_{}_{}", channel_id + 1, i), "FLOAT32".to_string(), "V".to_string(), "电压传感器".to_string()),
            4 => (format!("current_{}_{}", channel_id + 1, i), "FLOAT32".to_string(), "A".to_string(), "电流传感器".to_string()),
            5 => (format!("power_{}_{}", channel_id + 1, i), "FLOAT32".to_string(), "kW".to_string(), "功率计".to_string()),
            6 => (format!("frequency_{}_{}", channel_id + 1, i), "FLOAT32".to_string(), "Hz".to_string(), "频率计".to_string()),
            7 => (format!("status_{}_{}", channel_id + 1, i), "BOOL".to_string(), "".to_string(), "状态位".to_string()),
            _ => unreachable!(),
        };
        
        points.push(DataPoint {
            name,
            address,
            data_type,
            unit,
            description,
        });
    }
    
    points
}

/// 通道数据采集任务
async fn run_channel_collection(
    channel_id: usize,
    _channel_config: ChannelConfig,
    redis_client: redis::Client,
    stats: Arc<tokio::sync::RwLock<PerformanceStats>>,
    config: StressTestConfig,
) {
    let mut redis_conn = match redis_client.get_connection() {
        Ok(conn) => conn,
        Err(e) => {
            error!("通道 {} Redis连接失败: {}", channel_id + 1, e);
            return;
        }
    };

    let data_points = generate_data_points(channel_id, config.points_per_channel);
    let channel_key = channel_id + 1;
    
    let mut interval = interval(Duration::from_millis(config.poll_interval_ms));
    let mut collection_count = 0u64;

    info!("通道 {} 开始数据采集 ({} 个点位)", channel_id + 1, data_points.len());

    loop {
        interval.tick().await;
        collection_count += 1;
        
        let read_start = Instant::now();
        
        // 模拟Modbus数据读取
        let mut successful_points = 0;
        let mut data_batch = Vec::new();
        
        for (point_idx, point) in data_points.iter().enumerate() {
            // 模拟真实的Modbus读取延迟
            if point_idx % 100 == 0 {
                tokio::task::yield_now().await;
            }
            
            // 生成模拟数据
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs();
            
            let value = generate_simulated_value(&point.data_type, channel_id, point_idx, collection_count);
            
            let data_entry = json!({
                "channel_id": channel_key,
                "point_name": point.name,
                "address": point.address,
                "value": value,
                "unit": point.unit,
                "timestamp": timestamp,
                "quality": "good",
                "description": point.description
            });
            
            data_batch.push((format!("data:{}:{}", channel_key, point.name), data_entry.to_string()));
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
            stats_guard.update_read_time(read_duration.as_millis() as f64);
        }
        
        if collection_count % 50 == 0 {
            info!("通道 {} 完成第 {} 轮采集，读取 {} 个点位，耗时 {:.2}ms", 
                  channel_id + 1, collection_count, successful_points, read_duration.as_millis());
        }
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

/// 生成模拟数据值
fn generate_simulated_value(data_type: &str, channel_id: usize, point_idx: usize, cycle: u64) -> serde_json::Value {
    let time_factor = (cycle as f64 * 0.1) + (channel_id as f64 * 0.05) + (point_idx as f64 * 0.01);
    
    match data_type {
        "FLOAT32" => {
            let base_value = match point_idx % 7 {
                0 => 25.0 + (time_factor * 0.5).sin() * 5.0,      // 温度
                1 => 101.3 + (time_factor * 0.3).cos() * 2.0,     // 压力
                2 => 50.0 + (time_factor * 0.7).sin() * 10.0,     // 流量
                3 => 220.0 + (time_factor * 0.2).sin() * 5.0,     // 电压
                4 => 15.0 + (time_factor * 0.4).cos() * 3.0,      // 电流
                5 => 10.0 + (time_factor * 0.6).sin() * 5.0,      // 功率
                6 => 50.0 + (time_factor * 0.8).cos() * 0.5,      // 频率
                _ => 0.0,
            };
            json!((base_value * 100.0).round() / 100.0)
        },
        "BOOL" => {
            json!((time_factor.sin() + channel_id as f64) > 0.0)
        },
        _ => json!(0)
    }
}

/// 性能监控任务
async fn run_performance_monitor(
    stats: Arc<tokio::sync::RwLock<PerformanceStats>>,
    config: StressTestConfig,
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
        let throughput = stats_snapshot.throughput_per_sec();
        let success_rate = stats_snapshot.success_rate();
        
        println!();
        println!("📊 性能监控报告 (运行时间: {}秒)", elapsed);
        println!("  ├─ 总采集次数: {}", stats_snapshot.total_reads);
        println!("  ├─ 成功采集: {} ({:.1}%)", stats_snapshot.successful_reads, success_rate);
        println!("  ├─ 失败采集: {}", stats_snapshot.failed_reads);
        println!("  ├─ 总数据点: {}", stats_snapshot.total_points);
        println!("  ├─ 数据吞吐量: {:.1} 点位/秒", throughput);
        println!("  ├─ 活跃通道: {}", stats_snapshot.channels_active);
        println!("  ├─ Redis写入: {}", stats_snapshot.redis_writes);
        println!("  ├─ Redis错误: {}", stats_snapshot.redis_errors);
        println!("  ├─ 平均读取时间: {:.1}ms", stats_snapshot.avg_read_time_ms);
        println!("  ├─ 最大读取时间: {:.1}ms", stats_snapshot.max_read_time_ms);
        println!("  └─ 最小读取时间: {:.1}ms", if stats_snapshot.min_read_time_ms == f64::MAX { 0.0 } else { stats_snapshot.min_read_time_ms });

        // 检查Redis连接状态
        if let Ok(mut conn) = redis_client.get_connection() {
            if let Ok(info) = redis::cmd("INFO").arg("memory").query::<String>(&mut conn) {
                if let Some(used_memory_line) = info.lines().find(|line| line.starts_with("used_memory_human:")) {
                    if let Some(memory) = used_memory_line.split(':').nth(1) {
                        println!("  Redis内存使用: {}", memory.trim());
                    }
                }
            }
        }
    }
}

/// Redis内存监控
async fn run_memory_monitor(redis_client: redis::Client) {
    let mut interval = interval(Duration::from_secs(30));
    
    loop {
        interval.tick().await;
        
        if let Ok(mut conn) = redis_client.get_connection() {
            if let Ok(dbsize) = redis::cmd("DBSIZE").query::<i64>(&mut conn) {
                info!("Redis数据库大小: {} 个键", dbsize);
            }
            
            if let Ok(info) = redis::cmd("INFO").arg("stats").query::<String>(&mut conn) {
                for line in info.lines() {
                    if line.starts_with("total_commands_processed:") {
                        if let Some(commands) = line.split(':').nth(1) {
                            info!("Redis总命令数: {}", commands.trim());
                        }
                    }
                }
            }
        }
    }
}

/// 生成最终测试报告
fn generate_final_report(stats: &PerformanceStats, config: &StressTestConfig) {
    let total_test_time = stats.start_time.elapsed().as_secs_f64();
    let throughput = stats.throughput_per_sec();
    let success_rate = stats.success_rate();
    
    println!();
    println!("🎯 最终压力测试报告");
    println!("=======================================");
    println!("测试配置:");
    println!("  通道数量: {}", config.channel_count);
    println!("  每通道点位: {}", config.points_per_channel);
    println!("  总点位数: {}", config.channel_count * config.points_per_channel);
    println!("  测试时长: {:.1}秒", total_test_time);
    println!();
    println!("性能指标:");
    println!("  总采集次数: {}", stats.total_reads);
    println!("  成功采集: {} ({:.2}%)", stats.successful_reads, success_rate);
    println!("  失败采集: {}", stats.failed_reads);
    println!("  总数据点: {}", stats.total_points);
    println!("  平均吞吐量: {:.1} 点位/秒", throughput);
    println!("  峰值吞吐量: {:.1} 点位/秒", stats.total_points as f64 / total_test_time);
    println!();
    println!("响应时间:");
    println!("  平均读取时间: {:.1}ms", stats.avg_read_time_ms);
    println!("  最大读取时间: {:.1}ms", stats.max_read_time_ms);
    println!("  最小读取时间: {:.1}ms", if stats.min_read_time_ms == f64::MAX { 0.0 } else { stats.min_read_time_ms });
    println!();
    println!("数据库操作:");
    println!("  Redis写入次数: {}", stats.redis_writes);
    println!("  Redis错误次数: {}", stats.redis_errors);
    println!("  Redis成功率: {:.2}%", if stats.redis_writes > 0 { 
        (stats.redis_writes as f64 / (stats.redis_writes + stats.redis_errors) as f64) * 100.0 
    } else { 0.0 });
    println!();
    
    // 性能等级评估
    let performance_grade = if throughput > 5000.0 && success_rate > 99.0 {
        "A+ (优秀)"
    } else if throughput > 3000.0 && success_rate > 95.0 {
        "A (良好)"
    } else if throughput > 1000.0 && success_rate > 90.0 {
        "B (一般)"
    } else {
        "C (需优化)"
    };
    
    println!("🏆 性能等级: {}", performance_grade);
    println!("=======================================");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stress_config_creation() {
        let config = StressTestConfig::default();
        assert_eq!(config.channel_count, 10);
        assert_eq!(config.points_per_channel, 1000);
        assert_eq!(config.base_port, 5020);
    }

    #[test]
    fn test_data_point_generation() {
        let points = generate_data_points(0, 10);
        assert_eq!(points.len(), 10);
        assert!(points[0].name.contains("temperature"));
        assert!(points[1].name.contains("pressure"));
    }

    #[test]
    fn test_performance_stats() {
        let mut stats = PerformanceStats::new();
        stats.total_reads = 100;
        stats.successful_reads = 95;
        assert_eq!(stats.success_rate(), 95.0);
    }

    #[test]
    fn test_simulated_value_generation() {
        let value = generate_simulated_value("FLOAT32", 0, 0, 1);
        assert!(value.is_number());
        
        let bool_value = generate_simulated_value("BOOL", 0, 0, 1);
        assert!(bool_value.is_boolean());
    }
} 