//! Redis发布订阅集成测试
//!
//! 测试comsrv的Redis数据发布和命令订阅功能
//! 包括：
//! 1. 数据发布格式验证
//! 2. 高频率数据发布性能
//! 3. 命令订阅机制测试
//! 4. Redis连接恢复测试

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};
use tokio::time::{sleep, timeout};
use tracing::{debug, error, info, warn};

use redis::aio::{MultiplexedConnection, PubSub};
use redis::{AsyncCommands, Client, RedisResult};
use serde::{Deserialize, Serialize};
use serde_json::json;

/// 点位数据结构
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PointData {
    channel_id: u16,
    point_id: u32,
    value: f64,
    timestamp: i64,
    quality: u8,
}

/// 控制命令结构
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ControlCommand {
    channel_id: u16,
    point_id: u32,
    value: f64,
    command_type: String,
    timestamp: i64,
}

/// 测试配置
struct TestConfig {
    redis_url: String,
    test_channel_id: u16,
    test_duration: Duration,
    publish_rate_hz: u32,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            redis_url: std::env::var("TEST_REDIS_URL")
                .unwrap_or_else(|_| "redis://localhost:6379".to_string()),
            test_channel_id: 9002,
            test_duration: Duration::from_secs(30),
            publish_rate_hz: 100, // 100Hz发布频率
        }
    }
}

#[cfg(feature = "integration")]
#[tokio::test]
async fn test_redis_data_publish_format() {
    init_logging();
    let config = TestConfig::default();
    
    info!("Testing Redis data publish format");
    
    // 创建Redis连接
    let client = Client::open(config.redis_url.as_str())
        .expect("Failed to create Redis client");
    let mut conn = client.get_multiplexed_async_connection().await
        .expect("Failed to connect to Redis");
    
    // 测试各种数据类型的发布格式
    let test_cases = vec![
        // (key_pattern, point_type, point_id, value)
        (format!("{}:m:{}", config.test_channel_id, 10001), "telemetry", 10001, 42.5),
        (format!("{}:m:{}", config.test_channel_id, 10002), "telemetry", 10002, 380.0),
        (format!("{}:s:{}", config.test_channel_id, 20001), "signal", 20001, 1.0),
        (format!("{}:s:{}", config.test_channel_id, 20002), "signal", 20002, 0.0),
        (format!("{}:c:{}", config.test_channel_id, 30001), "control", 30001, 1.0),
        (format!("{}:a:{}", config.test_channel_id, 40001), "adjustment", 40001, 75.5),
    ];
    
    // 发布测试数据
    for (key, point_type, point_id, value) in &test_cases {
        let point_data = PointData {
            channel_id: config.test_channel_id,
            point_id: *point_id,
            value: *value,
            timestamp: chrono::Utc::now().timestamp_millis(),
            quality: 192, // Good quality
        };
        
        // 存储为JSON格式
        let json_value = serde_json::to_string(&point_data)
            .expect("Failed to serialize point data");
        
        let _: () = conn.set(key, &json_value).await
            .expect("Failed to set Redis key");
        
        info!("Published {} point {}: {}", point_type, point_id, value);
    }
    
    // 验证数据格式
    for (key, point_type, point_id, expected_value) in &test_cases {
        let json_str: String = conn.get(key).await
            .expect("Failed to get Redis key");
        
        let point_data: PointData = serde_json::from_str(&json_str)
            .expect("Failed to deserialize point data");
        
        assert_eq!(point_data.channel_id, config.test_channel_id);
        assert_eq!(point_data.point_id, *point_id);
        assert_eq!(point_data.value, *expected_value);
        assert_eq!(point_data.quality, 192);
        
        info!("Verified {} point {} format", point_type, point_id);
    }
    
    // 测试批量获取
    let pattern = format!("{}:*", config.test_channel_id);
    let keys: Vec<String> = redis::cmd("KEYS")
        .arg(&pattern)
        .query_async(&mut conn)
        .await
        .expect("Failed to get keys");
    
    assert_eq!(keys.len(), test_cases.len(), "Incorrect number of keys");
    info!("Successfully verified {} data points", keys.len());
}

#[cfg(feature = "integration")]
#[tokio::test]
async fn test_high_frequency_data_publish() {
    init_logging();
    let config = TestConfig {
        publish_rate_hz: 1000, // 1000Hz
        test_duration: Duration::from_secs(10),
        ..Default::default()
    };
    
    info!("Testing high frequency data publishing at {}Hz", config.publish_rate_hz);
    
    let client = Client::open(config.redis_url.as_str())
        .expect("Failed to create Redis client");
    let conn = Arc::new(RwLock::new(
        client.get_multiplexed_async_connection().await
            .expect("Failed to connect to Redis")
    ));
    
    // 统计信息
    let stats = Arc::new(RwLock::new(PublishStats::default()));
    
    // 启动发布任务
    let publish_interval = Duration::from_micros(1_000_000 / config.publish_rate_hz as u64);
    let start_time = Instant::now();
    
    let conn_clone = Arc::clone(&conn);
    let stats_clone = Arc::clone(&stats);
    let channel_id = config.test_channel_id;
    
    let publish_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(publish_interval);
        let mut sequence = 0u64;
        
        while start_time.elapsed() < config.test_duration {
            interval.tick().await;
            
            let point_data = PointData {
                channel_id,
                point_id: 10001,
                value: (sequence as f64).sin() * 100.0,
                timestamp: chrono::Utc::now().timestamp_millis(),
                quality: 192,
            };
            
            let key = format!("{}:m:{}", channel_id, 10001);
            let json_value = serde_json::to_string(&point_data).unwrap();
            
            let publish_start = Instant::now();
            
            match conn_clone.write().await.set::<_, _, ()>(&key, &json_value).await {
                Ok(_) => {
                    let mut stats_guard = stats_clone.write().await;
                    stats_guard.successful_publishes += 1;
                    stats_guard.total_latency_ms += publish_start.elapsed().as_micros() as f64 / 1000.0;
                }
                Err(e) => {
                    let mut stats_guard = stats_clone.write().await;
                    stats_guard.failed_publishes += 1;
                    debug!("Publish failed: {}", e);
                }
            }
            
            sequence += 1;
        }
    });
    
    // 同时启动订阅任务来验证数据
    let conn_clone = Arc::clone(&conn);
    let stats_clone = Arc::clone(&stats);
    
    let verify_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(100));
        let key = format!("{}:m:{}", config.test_channel_id, 10001);
        
        while start_time.elapsed() < config.test_duration {
            interval.tick().await;
            
            if let Ok(json_str) = conn_clone.read().await.get::<_, String>(&key).await {
                if let Ok(point_data) = serde_json::from_str::<PointData>(&json_str) {
                    let mut stats_guard = stats_clone.write().await;
                    stats_guard.verified_points += 1;
                    
                    // 检查数据新鲜度
                    let now = chrono::Utc::now().timestamp_millis();
                    let age_ms = now - point_data.timestamp;
                    if age_ms < 100 {
                        stats_guard.fresh_points += 1;
                    }
                }
            }
        }
    });
    
    // 等待测试完成
    let _ = tokio::join!(publish_task, verify_task);
    
    // 打印统计结果
    let final_stats = stats.read().await;
    final_stats.print_summary();
    
    // 验证性能
    let total_publishes = final_stats.successful_publishes + final_stats.failed_publishes;
    let success_rate = final_stats.successful_publishes as f64 / total_publishes as f64;
    let avg_latency = final_stats.total_latency_ms / final_stats.successful_publishes as f64;
    
    assert!(success_rate > 0.99, "Publish success rate below 99%");
    assert!(avg_latency < 10.0, "Average latency above 10ms");
    assert!(final_stats.fresh_points > final_stats.verified_points * 90 / 100, 
        "Data freshness below 90%");
}

#[cfg(feature = "integration")]
#[tokio::test]
async fn test_command_subscription() {
    init_logging();
    let config = TestConfig::default();
    
    info!("Testing command subscription mechanism");
    
    let client = Client::open(config.redis_url.as_str())
        .expect("Failed to create Redis client");
    
    // 创建发布者连接
    let mut publisher = client.get_async_connection().await
        .expect("Failed to connect to Redis");
    
    // 创建订阅者
    let mut pubsub = client.get_async_pubsub().await
        .expect("Failed to create pubsub");
    
    // 订阅控制和调节命令通道
    let control_channel = format!("cmd:{}:control", config.test_channel_id);
    let adjustment_channel = format!("cmd:{}:adjustment", config.test_channel_id);
    
    pubsub.subscribe(&control_channel).await
        .expect("Failed to subscribe to control channel");
    pubsub.subscribe(&adjustment_channel).await
        .expect("Failed to subscribe to adjustment channel");
    
    info!("Subscribed to command channels");
    
    // 启动接收任务
    let (tx, mut rx) = mpsc::channel(100);
    
    let receive_task = tokio::spawn(async move {
        let mut pubsub_stream = pubsub.into_on_message();
        
        while let Some(msg) = pubsub_stream.next().await {
            if let Ok(payload) = msg.get_payload::<String>() {
                if let Ok(command) = serde_json::from_str::<ControlCommand>(&payload) {
                    info!("Received command: {:?}", command);
                    let _ = tx.send(command).await;
                }
            }
        }
    });
    
    // 等待订阅建立
    sleep(Duration::from_millis(100)).await;
    
    // 发布测试命令
    let test_commands = vec![
        ControlCommand {
            channel_id: config.test_channel_id,
            point_id: 30001,
            value: 1.0,
            command_type: "control".to_string(),
            timestamp: chrono::Utc::now().timestamp_millis(),
        },
        ControlCommand {
            channel_id: config.test_channel_id,
            point_id: 40001,
            value: 75.5,
            command_type: "adjustment".to_string(),
            timestamp: chrono::Utc::now().timestamp_millis(),
        },
    ];
    
    for cmd in &test_commands {
        let channel = if cmd.command_type == "control" {
            &control_channel
        } else {
            &adjustment_channel
        };
        
        let payload = serde_json::to_string(cmd)
            .expect("Failed to serialize command");
        
        let _: () = publisher.publish(channel, &payload).await
            .expect("Failed to publish command");
        
        info!("Published {} command to point {}", cmd.command_type, cmd.point_id);
    }
    
    // 接收并验证命令
    let mut received_commands = Vec::new();
    let receive_timeout = Duration::from_secs(5);
    let deadline = Instant::now() + receive_timeout;
    
    while received_commands.len() < test_commands.len() && Instant::now() < deadline {
        match timeout(Duration::from_millis(100), rx.recv()).await {
            Ok(Some(cmd)) => {
                received_commands.push(cmd);
            }
            _ => {}
        }
    }
    
    // 验证接收到的命令
    assert_eq!(received_commands.len(), test_commands.len(), 
        "Not all commands were received");
    
    for (sent, received) in test_commands.iter().zip(received_commands.iter()) {
        assert_eq!(sent.channel_id, received.channel_id);
        assert_eq!(sent.point_id, received.point_id);
        assert_eq!(sent.value, received.value);
        assert_eq!(sent.command_type, received.command_type);
    }
    
    info!("Successfully verified command subscription");
}

#[cfg(feature = "integration")]
#[tokio::test]
async fn test_redis_connection_recovery() {
    init_logging();
    let config = TestConfig::default();
    
    info!("Testing Redis connection recovery");
    
    // 测试重连机制
    let mut retry_count = 0;
    let max_retries = 5;
    let mut conn: Option<MultiplexedConnection> = None;
    
    while retry_count < max_retries {
        match Client::open(config.redis_url.as_str()) {
            Ok(client) => {
                match client.get_multiplexed_async_connection().await {
                    Ok(c) => {
                        conn = Some(c);
                        break;
                    }
                    Err(e) => {
                        warn!("Connection attempt {} failed: {}", retry_count + 1, e);
                        retry_count += 1;
                        sleep(Duration::from_millis(500)).await;
                    }
                }
            }
            Err(e) => {
                error!("Failed to create client: {}", e);
                retry_count += 1;
                sleep(Duration::from_millis(500)).await;
            }
        }
    }
    
    assert!(conn.is_some(), "Failed to establish Redis connection after {} retries", max_retries);
    
    let mut conn = conn.unwrap();
    
    // 测试连接健康检查
    let ping_result: RedisResult<String> = redis::cmd("PING").query_async(&mut conn).await;
    assert!(ping_result.is_ok(), "Redis PING failed");
    
    // 模拟连接中断后的恢复
    // 注意：在真实环境中，这需要实际断开网络连接
    // 这里我们通过发送大量请求来测试连接池的稳定性
    
    let mut success_count = 0;
    let test_iterations = 100;
    
    for i in 0..test_iterations {
        let key = format!("test:recovery:{}", i);
        let value = format!("value_{}", i);
        
        match conn.set::<_, _, ()>(&key, &value).await {
            Ok(_) => success_count += 1,
            Err(e) => {
                warn!("Request {} failed: {}, attempting recovery", i, e);
                
                // 尝试重新建立连接
                if let Ok(client) = Client::open(config.redis_url.as_str()) {
                    if let Ok(new_conn) = client.get_multiplexed_async_connection().await {
                        conn = new_conn;
                        info!("Successfully recovered connection");
                    }
                }
            }
        }
        
        if i % 10 == 0 {
            sleep(Duration::from_millis(10)).await;
        }
    }
    
    let success_rate = success_count as f64 / test_iterations as f64;
    info!("Connection recovery test completed: {:.2}% success rate", success_rate * 100.0);
    
    assert!(success_rate > 0.95, "Connection recovery success rate below 95%");
}

// 辅助结构和函数

#[derive(Debug, Default)]
struct PublishStats {
    successful_publishes: u64,
    failed_publishes: u64,
    total_latency_ms: f64,
    verified_points: u64,
    fresh_points: u64,
}

impl PublishStats {
    fn print_summary(&self) {
        println!("\n=== High Frequency Publish Statistics ===");
        println!("Successful publishes: {}", self.successful_publishes);
        println!("Failed publishes: {}", self.failed_publishes);
        println!("Success rate: {:.2}%", 
            self.successful_publishes as f64 / 
            (self.successful_publishes + self.failed_publishes) as f64 * 100.0
        );
        println!("Average latency: {:.2}ms", 
            self.total_latency_ms / self.successful_publishes as f64
        );
        println!("Verified points: {}", self.verified_points);
        println!("Fresh points (<100ms old): {} ({:.2}%)", 
            self.fresh_points,
            self.fresh_points as f64 / self.verified_points as f64 * 100.0
        );
    }
}

fn init_logging() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter("comsrv=debug,redis_pubsub_test=debug")
        .try_init();
}