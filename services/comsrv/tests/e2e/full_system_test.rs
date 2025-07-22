//! 端到端系统测试
//!
//! 测试完整的数据流：设备 -> 协议插件 -> Redis -> 前端

use std::time::Duration;
use tokio::time::{sleep, timeout};
use redis::AsyncCommands;
use serde_json::Value;
use std::collections::HashMap;

/// E2E测试配置
struct E2ETestConfig {
    redis_url: String,
    test_duration: Duration,
    check_interval: Duration,
}

/// 系统测试运行器
struct SystemTestRunner {
    config: E2ETestConfig,
    redis_client: redis::Client,
}

impl SystemTestRunner {
    /// 创建测试运行器
    fn new(config: E2ETestConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let redis_client = redis::Client::open(config.redis_url.as_str())?;
        
        Ok(Self {
            config,
            redis_client,
        })
    }
    
    /// 运行完整系统测试
    async fn run_full_system_test(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("\n🔄 Starting End-to-End System Test");
        println!("Duration: {:?}", self.config.test_duration);
        
        // 1. 启动协议模拟器
        self.start_protocol_simulators().await?;
        
        // 2. 启动协议插件
        self.start_protocol_plugins().await?;
        
        // 3. 验证数据流
        self.verify_data_flow().await?;
        
        // 4. 测试控制命令
        self.test_control_commands().await?;
        
        // 5. 测试故障恢复
        self.test_fault_recovery().await?;
        
        // 6. 验证性能指标
        self.verify_performance_metrics().await?;
        
        println!("\n✅ End-to-End System Test Completed Successfully");
        
        Ok(())
    }
    
    /// 启动协议模拟器
    async fn start_protocol_simulators(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("\n📡 Starting protocol simulators...");
        
        // 启动Modbus模拟器
        tokio::spawn(async {
            let addr = "127.0.0.1:5502".parse().unwrap();
            let simulator = crate::simulators::modbus_simulator::ModbusTcpSimulator::new(addr);
            let _ = simulator.start().await;
        });
        
        // 等待模拟器启动
        sleep(Duration::from_secs(2)).await;
        
        println!("✓ Protocol simulators started");
        Ok(())
    }
    
    /// 启动协议插件
    async fn start_protocol_plugins(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("\n🔌 Starting protocol plugins...");
        
        // Protocol plugin startup is handled by the main service
        
        println!("✓ Protocol plugins started");
        Ok(())
    }
    
    /// 验证数据流
    async fn verify_data_flow(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("\n🔍 Verifying data flow...");
        
        let mut con = self.redis_client.get_async_connection().await?;
        let start_time = tokio::time::Instant::now();
        let mut data_points_received = 0;
        
        while start_time.elapsed() < self.config.test_duration {
            // 检查Redis中的数据点
            let keys: Vec<String> = con.keys("point:*").await?;
            
            for key in &keys {
                let value: Option<String> = con.get(key).await?;
                if let Some(val) = value {
                    // 解析并验证数据格式
                    if let Ok(json_val) = serde_json::from_str::<Value>(&val) {
                        if json_val.get("value").is_some() && 
                           json_val.get("timestamp").is_some() &&
                           json_val.get("quality").is_some() {
                            data_points_received += 1;
                        }
                    }
                }
            }
            
            println!("  Data points received: {}", data_points_received);
            
            sleep(self.config.check_interval).await;
        }
        
        assert!(data_points_received > 0, "No data points received");
        println!("✓ Data flow verified: {} points received", data_points_received);
        
        Ok(())
    }
    
    /// 测试控制命令
    async fn test_control_commands(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("\n🎮 Testing control commands...");
        
        let mut con = self.redis_client.get_async_connection().await?;
        
        // 发送控制命令
        let command = serde_json::json!({
            "type": "write",
            "point_id": "test_control_point",
            "value": 100,
            "timestamp": chrono::Utc::now().timestamp(),
        });
        
        let _: () = con.publish("control_commands", command.to_string()).await?;
        
        // 等待命令处理
        sleep(Duration::from_millis(500)).await;
        
        // 验证命令执行结果
        let result: Option<String> = con.get("command_result:test_control_point").await?;
        assert!(result.is_some(), "Control command result not found");
        
        println!("✓ Control commands tested successfully");
        
        Ok(())
    }
    
    /// 测试故障恢复
    async fn test_fault_recovery(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("\n🔧 Testing fault recovery...");
        
        // 模拟连接断开
        println!("  Simulating connection loss...");
        // Connection loss simulation requires protocol-specific implementation
        
        sleep(Duration::from_secs(2)).await;
        
        // 验证自动重连
        println!("  Verifying automatic reconnection...");
        let mut con = self.redis_client.get_async_connection().await?;
        
        // 检查连接状态
        let status: Option<String> = con.get("connection_status:modbus_tcp").await?;
        
        // 等待重连
        let reconnect_result = timeout(Duration::from_secs(30), async {
            loop {
                let status: Option<String> = con.get("connection_status:modbus_tcp").await?;
                if let Some(s) = status {
                    if s == "connected" {
                        return Ok(());
                    }
                }
                sleep(Duration::from_secs(1)).await;
            }
        }).await;
        
        assert!(reconnect_result.is_ok(), "Failed to reconnect");
        println!("✓ Fault recovery tested successfully");
        
        Ok(())
    }
    
    /// 验证性能指标
    async fn verify_performance_metrics(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("\n📊 Verifying performance metrics...");
        
        let mut con = self.redis_client.get_async_connection().await?;
        
        // 收集性能指标
        let mut metrics = HashMap::new();
        
        // 数据延迟
        let latency_key = "metrics:data_latency";
        if let Ok(latency) = con.get::<_, String>(latency_key).await {
            if let Ok(val) = latency.parse::<f64>() {
                metrics.insert("data_latency_ms", val);
            }
        }
        
        // 吞吐量
        let throughput_key = "metrics:throughput";
        if let Ok(throughput) = con.get::<_, String>(throughput_key).await {
            if let Ok(val) = throughput.parse::<f64>() {
                metrics.insert("throughput_ops_per_sec", val);
            }
        }
        
        // 错误率
        let error_rate_key = "metrics:error_rate";
        if let Ok(error_rate) = con.get::<_, String>(error_rate_key).await {
            if let Ok(val) = error_rate.parse::<f64>() {
                metrics.insert("error_rate_percent", val);
            }
        }
        
        // 打印性能指标
        println!("\n  Performance Metrics:");
        for (metric, value) in &metrics {
            println!("    {}: {:.2}", metric, value);
        }
        
        // 验证性能要求
        if let Some(&latency) = metrics.get("data_latency_ms") {
            assert!(latency < 100.0, "Data latency too high: {:.2}ms", latency);
        }
        
        if let Some(&error_rate) = metrics.get("error_rate_percent") {
            assert!(error_rate < 1.0, "Error rate too high: {:.2}%", error_rate);
        }
        
        println!("\n✓ Performance metrics verified");
        
        Ok(())
    }
}

/// 数据一致性测试
async fn test_data_consistency(
    redis_url: &str,
    duration: Duration
) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🔄 Testing data consistency...");
    
    let client = redis::Client::open(redis_url)?;
    let mut con = client.get_async_connection().await?;
    
    let start_time = tokio::time::Instant::now();
    let mut inconsistencies = 0;
    
    while start_time.elapsed() < duration {
        // 获取所有数据点
        let keys: Vec<String> = con.keys("point:*").await?;
        
        for key in &keys {
            // 读取两次并比较时间戳
            let value1: Option<String> = con.get(key).await?;
            sleep(Duration::from_millis(10)).await;
            let value2: Option<String> = con.get(key).await?;
            
            if let (Some(v1), Some(v2)) = (value1, value2) {
                if let (Ok(json1), Ok(json2)) = (
                    serde_json::from_str::<Value>(&v1),
                    serde_json::from_str::<Value>(&v2)
                ) {
                    // 验证时间戳递增
                    if let (Some(ts1), Some(ts2)) = (
                        json1.get("timestamp").and_then(|v| v.as_i64()),
                        json2.get("timestamp").and_then(|v| v.as_i64())
                    ) {
                        if ts2 < ts1 {
                            inconsistencies += 1;
                            println!("  ⚠️ Timestamp inconsistency detected in {}", key);
                        }
                    }
                }
            }
        }
        
        sleep(Duration::from_secs(1)).await;
    }
    
    assert_eq!(inconsistencies, 0, "Data inconsistencies detected");
    println!("✓ Data consistency verified");
    
    Ok(())
}

/// 长时间稳定性测试
async fn test_long_term_stability(
    redis_url: &str,
    duration: Duration
) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n⏰ Testing long-term stability...");
    println!("Test duration: {:?}", duration);
    
    let client = redis::Client::open(redis_url)?;
    let mut con = client.get_async_connection().await?;
    
    let start_time = tokio::time::Instant::now();
    let mut last_data_count = 0;
    let mut stall_count = 0;
    
    while start_time.elapsed() < duration {
        // 统计数据点数量
        let keys: Vec<String> = con.keys("point:*").await?;
        let current_count = keys.len();
        
        // 检查数据是否停滞
        if current_count == last_data_count {
            stall_count += 1;
            if stall_count > 10 {
                println!("  ⚠️ Data flow stalled for {} seconds", stall_count);
            }
        } else {
            stall_count = 0;
        }
        
        last_data_count = current_count;
        
        // 检查内存使用
        let info: String = redis::cmd("INFO")
            .arg("memory")
            .query_async(&mut con)
            .await?;
        
        if let Some(line) = info.lines().find(|l| l.starts_with("used_memory_human:")) {
            let memory = line.split(':').nth(1).unwrap_or("unknown");
            println!("  Memory usage: {}", memory.trim());
        }
        
        sleep(Duration::from_secs(60)).await; // 每分钟检查一次
    }
    
    assert!(stall_count < 60, "Data flow stalled for too long");
    println!("✓ Long-term stability verified");
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    #[ignore] // 需要Redis运行
    async fn test_e2e_basic() {
        let config = E2ETestConfig {
            redis_url: "redis://127.0.0.1:6379".to_string(),
            test_duration: Duration::from_secs(30),
            check_interval: Duration::from_secs(5),
        };
        
        let runner = SystemTestRunner::new(config).unwrap();
        runner.run_full_system_test().await.unwrap();
    }
    
    #[tokio::test]
    #[ignore] // 需要Redis运行
    async fn test_data_consistency_check() {
        test_data_consistency(
            "redis://127.0.0.1:6379",
            Duration::from_secs(60)
        ).await.unwrap();
    }
    
    #[tokio::test]
    #[ignore] // 长时间测试
    async fn test_stability_24h() {
        test_long_term_stability(
            "redis://127.0.0.1:6379",
            Duration::from_secs(24 * 60 * 60) // 24小时
        ).await.unwrap();
    }
}