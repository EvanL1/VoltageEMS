/// Modbus + Redis 完整测试演示
/// 
/// 这个示例展示如何：
/// 1. 连接并读取Modbus设备数据
/// 2. 将数据存储到Redis数据库
/// 3. 监控数据变化
/// 4. 提供实时数据查询

use std::collections::HashMap;
use std::time::Duration;
use std::sync::Arc;
use tokio::time::interval;
use serde_json::json;
use redis::{Client as RedisClient, Commands, Connection};
use comsrv::core::protocols::common::{create_default_factory, ProtocolFactory};
use comsrv::core::config::config_manager::{ChannelConfig, ProtocolType, ChannelParameters};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志
    env_logger::init();
    
    println!("🔥 Modbus + Redis 完整测试演示");
    println!("{}", "=".repeat(60));
    
    // 创建Redis连接
    let redis_client = connect_redis().await?;
    println!("✅ Redis连接成功: localhost:6379");
    
    // 创建协议工厂
    let factory = create_default_factory();
    println!("✅ 协议工厂创建成功");
    
    // 创建Modbus TCP配置
    let modbus_config = create_modbus_tcp_config();
    // 从配置参数中提取主机和端口信息进行显示
    let host = match &modbus_config.parameters {
        ChannelParameters::Generic(params) => {
            params.get("address")
                .and_then(|v| v.as_str())
                .unwrap_or("localhost")
        }
        ChannelParameters::ModbusTcp { host, .. } => host.as_str(),
        ChannelParameters::ModbusRtu { .. } => "localhost", // RTU doesn't use host
    };
    let port = match &modbus_config.parameters {
        ChannelParameters::Generic(params) => {
            params.get("port")
                .and_then(|v| v.as_u64())
                .unwrap_or(502)
        }
        ChannelParameters::ModbusTcp { port, .. } => *port as u64,
        ChannelParameters::ModbusRtu { .. } => 502, // RTU doesn't use TCP port
    };
    println!("✅ Modbus TCP配置创建: {}:{}", host, port);
    
    // 验证配置
    match factory.validate_config(&modbus_config) {
        Ok(_) => println!("✅ 配置验证通过"),
        Err(e) => {
            println!("❌ 配置验证失败: {}", e);
            return Ok(());
        }
    }
    
    // 创建通道
    factory.create_channel(modbus_config.clone())?;
    println!("✅ Modbus通道创建成功");
    
    // 启动演示任务
    let factory_clone = Arc::new(factory);
    let demo_tasks = vec![
        tokio::spawn(run_data_collection(factory_clone.clone(), redis_client.clone())),
        tokio::spawn(run_data_monitoring(redis_client.clone())),
        tokio::spawn(run_statistics_reporter(redis_client.clone())),
    ];
    
    println!("\n🚀 启动数据采集和监控...");
    println!("按 Ctrl+C 停止程序");
    
    // 等待用户中断或任务完成
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            println!("\n🛑 收到中断信号，正在停止...");
        }
        _ = futures::future::try_join_all(demo_tasks) => {
            println!("\n✅ 所有任务完成");
        }
    }
    
    // 清理资源
    factory_clone.stop_all_channels().await?;
    println!("✅ 所有通道已停止");
    
    Ok(())
}

/// 连接Redis数据库
async fn connect_redis() -> anyhow::Result<RedisClient> {
    let client = RedisClient::open("redis://127.0.0.1:6379/")?;
    
    // 测试连接
    let mut conn = client.get_connection()?;
    redis::cmd("PING").query::<String>(&mut conn)?;
    
    Ok(client)
}

/// 创建Modbus TCP配置
fn create_modbus_tcp_config() -> ChannelConfig {
    let mut parameters = HashMap::new();
    parameters.insert("address".to_string(), serde_yaml::Value::String("127.0.0.1".to_string())); // 使用address而不是host
    parameters.insert("port".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(502)));
    parameters.insert("timeout".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(5000))); // 使用timeout而不是timeout_ms
    parameters.insert("slave_id".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(1)));
    parameters.insert("poll_rate".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(2000)));
    parameters.insert("max_retries".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(3)));
    
    ChannelConfig {
        id: 1,
        name: "Modbus TCP Demo".to_string(),
        description: "Modbus TCP演示通道".to_string(),
        protocol: ProtocolType::ModbusTcp,
        parameters: ChannelParameters::Generic(parameters),
    }
}

/// 数据采集任务
async fn run_data_collection(factory: Arc<ProtocolFactory>, redis_client: RedisClient) -> anyhow::Result<()> {
    let mut conn = redis_client.get_connection()?;
    let mut interval = interval(Duration::from_secs(2));
    let mut counter = 0u32;
    
    println!("🔄 数据采集任务已启动");
    
    loop {
        interval.tick().await;
        counter += 1;
        
        // 获取通道
        if let Some(channel) = factory.get_channel(1).await {
            let channel_guard = channel.read().await;
            
            // 模拟读取Modbus数据
            let simulated_data = generate_simulated_data(counter);
            
            // 存储到Redis
            for (key, value) in simulated_data.iter() {
                let redis_key = format!("modbus:data:{}", key);
                let data_json = json!({
                    "value": value,
                    "timestamp": chrono::Utc::now().timestamp(),
                    "counter": counter,
                    "status": "ok"
                });
                
                let _: () = conn.set(&redis_key, data_json.to_string())?;
                
                // 添加到时间序列（用于历史数据）
                let ts_key = format!("modbus:ts:{}", key);
                let ts_value = format!("{}:{}", chrono::Utc::now().timestamp(), value);
                let _: () = conn.lpush(&ts_key, &ts_value)?;
                let _: () = conn.ltrim(&ts_key, 0, 99)?; // 保持最近100个值
            }
            
            // 更新统计信息
            let _: () = conn.incr("modbus:stats:total_reads", 1)?;
            let _: () = conn.set("modbus:stats:last_update", chrono::Utc::now().timestamp())?;
            
            println!("📊 数据采集 #{}: {} 个数据点已存储到Redis", counter, simulated_data.len());
        }
        
        // 每隔20次采集显示详细信息
        if counter % 10 == 0 {
            show_data_summary(&mut conn, counter)?;
        }
    }
}

/// 数据监控任务
async fn run_data_monitoring(redis_client: RedisClient) -> anyhow::Result<()> {
    let mut conn = redis_client.get_connection()?;
    let mut interval = interval(Duration::from_secs(5));
    
    println!("👁️  数据监控任务已启动");
    
    loop {
        interval.tick().await;
        
        // 监控数据变化
        let keys: Vec<String> = conn.keys("modbus:data:*")?;
        
        if !keys.is_empty() {
            println!("\n📈 数据监控报告:");
            
            for key in keys.iter().take(5) { // 只显示前5个
                let value: String = conn.get(key)?;
                
                if let Ok(data) = serde_json::from_str::<serde_json::Value>(&value) {
                    let point_name = key.replace("modbus:data:", "");
                    let value = data["value"].as_f64().unwrap_or(0.0);
                    let timestamp = data["timestamp"].as_i64().unwrap_or(0);
                    let status = data["status"].as_str().unwrap_or("unknown");
                    
                    println!("  {} = {:.2} [{}] @{}", 
                            point_name, value, status, 
                            chrono::DateTime::from_timestamp(timestamp, 0)
                                .unwrap_or_default()
                                .format("%H:%M:%S"));
                }
            }
            
            if keys.len() > 5 {
                println!("  ... 还有 {} 个数据点", keys.len() - 5);
            }
        }
    }
}

/// 统计报告任务
async fn run_statistics_reporter(redis_client: RedisClient) -> anyhow::Result<()> {
    let mut conn = redis_client.get_connection()?;
    let mut interval = interval(Duration::from_secs(15));
    
    println!("📊 统计报告任务已启动");
    
    loop {
        interval.tick().await;
        
        println!("\n📋 系统统计报告:");
        
        // Redis信息
        let info: String = redis::cmd("INFO").arg("memory").query(&mut conn)?;
        if let Some(memory_line) = info.lines().find(|line| line.starts_with("used_memory_human:")) {
            println!("  Redis内存使用: {}", memory_line.split(':').nth(1).unwrap_or("unknown"));
        }
        
        // 数据统计
        let total_reads: i64 = conn.get("modbus:stats:total_reads").unwrap_or(0);
        let last_update: i64 = conn.get("modbus:stats:last_update").unwrap_or(0);
        let data_keys: Vec<String> = conn.keys("modbus:data:*")?;
        
        println!("  总读取次数: {}", total_reads);
        println!("  数据点数量: {}", data_keys.len());
        
        if last_update > 0 {
            let last_update_time = chrono::DateTime::from_timestamp(last_update, 0)
                .unwrap_or_default();
            println!("  最后更新: {}", last_update_time.format("%Y-%m-%d %H:%M:%S"));
        }
        
        // 检查数据新鲜度
        let now = chrono::Utc::now().timestamp();
        if last_update > 0 && now - last_update > 10 {
            println!("  ⚠️  警告: 数据超过10秒未更新");
        }
    }
}

/// 生成模拟数据
fn generate_simulated_data(counter: u32) -> HashMap<String, f64> {
    let mut data = HashMap::new();
    let time_factor = (counter as f64) * 0.1;
    
    // 模拟不同类型的工业数据
    data.insert("temperature_1".to_string(), 25.0 + (time_factor * 0.5).sin() * 5.0);
    data.insert("pressure_1".to_string(), 101.3 + (time_factor * 0.3).cos() * 2.0);
    data.insert("flow_rate_1".to_string(), 50.0 + (time_factor * 0.7).sin() * 10.0);
    data.insert("voltage_a".to_string(), 220.0 + (time_factor * 0.2).sin() * 5.0);
    data.insert("current_a".to_string(), 15.0 + (time_factor * 0.4).cos() * 3.0);
    data.insert("power_factor".to_string(), 0.85 + (time_factor * 0.1).sin() * 0.1);
    data.insert("frequency".to_string(), 50.0 + (time_factor * 0.05).cos() * 0.2);
    
    data
}

/// 显示数据摘要
fn show_data_summary(conn: &mut Connection, counter: u32) -> anyhow::Result<()> {
    println!("\n📊 数据摘要 (采集轮次 #{}):", counter);
    
    let keys: Vec<String> = conn.keys("modbus:data:*")?;
    println!("  活跃数据点: {}", keys.len());
    
    // 显示一些示例数据
    for key in keys.iter().take(3) {
        let value: String = conn.get(key)?;
        if let Ok(data) = serde_json::from_str::<serde_json::Value>(&value) {
            let point_name = key.replace("modbus:data:", "");
            let value = data["value"].as_f64().unwrap_or(0.0);
            println!("    {}: {:.2}", point_name, value);
        }
    }
    
    // Redis键统计
    let all_keys: Vec<String> = conn.keys("modbus:*")?;
    println!("  Redis键总数: {}", all_keys.len());
    
    Ok(())
}

/// 工具函数：查询特定数据点的历史数据
#[allow(dead_code)]
async fn query_historical_data(redis_client: &RedisClient, point_name: &str, limit: usize) -> anyhow::Result<Vec<(i64, f64)>> {
    let mut conn = redis_client.get_connection()?;
    let ts_key = format!("modbus:ts:{}", point_name);
    
    let values: Vec<String> = conn.lrange(&ts_key, 0, limit as isize - 1)?;
    let mut result = Vec::new();
    
    for value in values {
        if let Some((timestamp_str, value_str)) = value.split_once(':') {
            if let (Ok(timestamp), Ok(value)) = (timestamp_str.parse::<i64>(), value_str.parse::<f64>()) {
                result.push((timestamp, value));
            }
        }
    }
    
    Ok(result)
}

/// 工具函数：获取实时数据快照
#[allow(dead_code)]
async fn get_realtime_snapshot(redis_client: &RedisClient) -> anyhow::Result<HashMap<String, serde_json::Value>> {
    let mut conn = redis_client.get_connection()?;
    let keys: Vec<String> = conn.keys("modbus:data:*")?;
    
    let mut snapshot = HashMap::new();
    
    for key in keys {
        let value: String = conn.get(&key)?;
        if let Ok(data) = serde_json::from_str::<serde_json::Value>(&value) {
            let point_name = key.replace("modbus:data:", "");
            snapshot.insert(point_name, data);
        }
    }
    
    Ok(snapshot)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_config_creation() {
        let config = create_modbus_tcp_config();
        assert_eq!(config.id, 1);
        assert_eq!(config.protocol, ProtocolType::ModbusTcp);
        assert_eq!(config.name, "Modbus TCP Demo");
    }
    
    #[test]
    fn test_data_generation() {
        let data = generate_simulated_data(0);
        assert!(!data.is_empty());
        assert!(data.contains_key("temperature_1"));
        assert!(data.contains_key("pressure_1"));
    }
    
    #[tokio::test]
    async fn test_redis_connection() {
        // 仅在Redis可用时运行此测试
        if let Ok(client) = RedisClient::open("redis://127.0.0.1:6379/") {
            if let Ok(mut conn) = client.get_connection() {
                let result = redis::cmd("PING").query::<String>(&mut conn);
                assert!(result.is_ok());
            }
        }
    }
} 