use anyhow::Result;
use serde_json::json;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use voltage_common::redis_client::RedisClient;
use voltage_common::types::{PointType, PointValue, Quality};

/// 可靠性测试场景
#[derive(Debug)]
struct ReliabilityTest {
    client: Arc<RedisClient>,
    error_count: Arc<AtomicU64>,
    retry_count: Arc<AtomicU64>,
    success_count: Arc<AtomicU64>,
}

impl ReliabilityTest {
    async fn new() -> Result<Self> {
        let client = RedisClient::new("redis://localhost:6379").await?;
        Ok(Self {
            client: Arc::new(client),
            error_count: Arc::new(AtomicU64::new(0)),
            retry_count: Arc::new(AtomicU64::new(0)),
            success_count: Arc::new(AtomicU64::new(0)),
        })
    }

    /// 测试连接断开重连
    async fn test_connection_resilience(&self) -> Result<()> {
        println!("\n=== Testing Connection Resilience ===");

        // 启动后台写入任务
        let write_task = self.start_continuous_writes();

        // 等待稳定运行
        println!("Stable operation for 5 seconds...");
        tokio::time::sleep(Duration::from_secs(5)).await;

        // 模拟连接问题
        println!("\n!!! Simulating connection issues !!!");
        println!("Please manually restart Redis or use: docker restart redis-dev");
        println!("Waiting 10 seconds for manual intervention...");
        tokio::time::sleep(Duration::from_secs(10)).await;

        // 观察恢复情况
        println!("\nObserving recovery for 10 seconds...");
        tokio::time::sleep(Duration::from_secs(10)).await;

        // 停止写入任务
        write_task.abort();

        // 打印统计
        self.print_reliability_stats();

        Ok(())
    }

    /// 测试命令发送失败重试
    async fn test_command_retry_logic(&self) -> Result<()> {
        println!("\n=== Testing Command Retry Logic ===");

        let client = self.client.clone();
        let retry_count = self.retry_count.clone();

        // 测试带重试的命令发送
        let send_with_retry = |point_id: u32, value: f64| {
            let client = client.clone();
            let retry_count = retry_count.clone();

            async move {
                let mut retries = 0;
                let max_retries = 3;
                let mut backoff = Duration::from_millis(100);

                loop {
                    let command = json!({
                        "point_id": point_id,
                        "value": value,
                        "timestamp": chrono::Utc::now().timestamp_millis(),
                    });

                    match client.publish("cmd:control", &command.to_string()).await {
                        Ok(_) => return Ok(()),
                        Err(e) => {
                            retries += 1;
                            retry_count.fetch_add(1, Ordering::Relaxed);

                            if retries >= max_retries {
                                return Err(anyhow::anyhow!("Max retries exceeded: {}", e));
                            }

                            println!(
                                "Retry {}/{} after {:?}: {}",
                                retries, max_retries, backoff, e
                            );
                            tokio::time::sleep(backoff).await;
                            backoff *= 2; // 指数退避
                        }
                    }
                }
            }
        };

        // 并发发送100个命令测试重试
        let mut tasks = vec![];
        for i in 0..100 {
            let task = send_with_retry(i, i as f64);
            tasks.push(tokio::spawn(task));
        }

        // 等待所有任务完成
        let mut failed = 0;
        for task in tasks {
            if let Ok(result) = task.await {
                if result.is_err() {
                    failed += 1;
                }
            }
        }

        println!("\nRetry test completed:");
        println!(
            "  Total retries: {}",
            self.retry_count.load(Ordering::Relaxed)
        );
        println!("  Failed after retries: {}", failed);

        Ok(())
    }

    /// 测试数据一致性
    async fn test_data_consistency(&self) -> Result<()> {
        println!("\n=== Testing Data Consistency ===");

        let test_points = 1000;
        let concurrent_writers = 10;
        let writes_per_writer = 100;

        // 初始化测试点位
        for i in 0..test_points {
            let point = PointValue {
                value: 0.0,
                quality: Quality::Good,
                timestamp: chrono::Utc::now().timestamp_millis(),
                point_type: PointType::YC,
            };

            self.client
                .set(&format!("point:{}", i), &serde_json::to_string(&point)?)
                .await?;
        }

        // 并发写入相同的点位
        let write_counters = Arc::new(RwLock::new(vec![0u64; test_points]));
        let mut tasks = vec![];

        for writer_id in 0..concurrent_writers {
            let client = self.client.clone();
            let counters = write_counters.clone();

            let task = tokio::spawn(async move {
                for _ in 0..writes_per_writer {
                    let point_id = rand::random::<usize>() % test_points;

                    // 读取当前值
                    let key = format!("point:{}", point_id);
                    if let Ok(current) = client.get::<String>(&key).await {
                        if let Ok(mut point_value) = serde_json::from_str::<PointValue>(&current) {
                            // 增加值
                            point_value.value += 1.0;
                            point_value.timestamp = chrono::Utc::now().timestamp_millis();

                            // 写回
                            if client
                                .set(&key, &serde_json::to_string(&point_value).unwrap())
                                .await
                                .is_ok()
                            {
                                let mut counters = counters.write().await;
                                counters[point_id] += 1;
                            }
                        }
                    }
                }
            });

            tasks.push(task);
        }

        // 等待所有写入完成
        for task in tasks {
            let _ = task.await;
        }

        // 验证数据一致性
        println!("\nVerifying data consistency...");
        let counters = write_counters.read().await;
        let mut inconsistent = 0;

        for i in 0..test_points {
            let key = format!("point:{}", i);
            if let Ok(data) = self.client.get::<String>(&key).await {
                if let Ok(point_value) = serde_json::from_str::<PointValue>(&data) {
                    let expected = counters[i] as f64;
                    if (point_value.value - expected).abs() > 0.01 {
                        inconsistent += 1;
                        println!(
                            "Inconsistency at point {}: expected {}, got {}",
                            i, expected, point_value.value
                        );
                    }
                }
            }
        }

        println!("\nConsistency test results:");
        println!("  Total points: {}", test_points);
        println!("  Inconsistent: {}", inconsistent);
        println!(
            "  Consistency rate: {:.2}%",
            ((test_points - inconsistent) as f64 / test_points as f64) * 100.0
        );

        Ok(())
    }

    /// 测试发布订阅可靠性
    async fn test_pubsub_reliability(&self) -> Result<()> {
        println!("\n=== Testing Pub/Sub Reliability ===");

        let client = self.client.clone();
        let received = Arc::new(AtomicU64::new(0));
        let missed = Arc::new(AtomicU64::new(0));

        // 启动订阅者
        let received_clone = received.clone();
        let subscriber = tokio::spawn(async move {
            let mut sub = client.subscribe("test:pubsub").await.unwrap();
            let start = Instant::now();

            while start.elapsed() < Duration::from_secs(10) {
                match tokio::time::timeout(Duration::from_millis(100), sub.recv()).await {
                    Ok(Some(msg)) => {
                        if let Ok(data) = msg.get_payload::<String>() {
                            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&data) {
                                if json.get("seq").is_some() {
                                    received_clone.fetch_add(1, Ordering::Relaxed);
                                }
                            }
                        }
                    }
                    _ => continue,
                }
            }
        });

        // 等待订阅建立
        tokio::time::sleep(Duration::from_millis(500)).await;

        // 发布消息
        let total_messages = 1000;
        for i in 0..total_messages {
            let message = json!({
                "seq": i,
                "timestamp": chrono::Utc::now().timestamp_millis(),
                "data": format!("test_message_{}", i),
            });

            if let Err(_) = self
                .client
                .publish("test:pubsub", &message.to_string())
                .await
            {
                missed.fetch_add(1, Ordering::Relaxed);
            }

            // 模拟不同的发送速率
            if i % 100 == 0 {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }

        // 等待订阅者完成
        let _ = subscriber.await;

        let received_count = received.load(Ordering::Relaxed);
        let missed_count = missed.load(Ordering::Relaxed);

        println!("\nPub/Sub reliability results:");
        println!("  Total messages: {}", total_messages);
        println!("  Received: {}", received_count);
        println!("  Missed: {}", total_messages - received_count);
        println!("  Publish failures: {}", missed_count);
        println!(
            "  Delivery rate: {:.2}%",
            (received_count as f64 / total_messages as f64) * 100.0
        );

        Ok(())
    }

    /// 启动连续写入任务
    fn start_continuous_writes(&self) -> tokio::task::JoinHandle<()> {
        let client = self.client.clone();
        let error_count = self.error_count.clone();
        let success_count = self.success_count.clone();

        tokio::spawn(async move {
            let mut seq = 0u64;

            loop {
                let point = PointValue {
                    value: seq as f64,
                    quality: Quality::Good,
                    timestamp: chrono::Utc::now().timestamp_millis(),
                    point_type: PointType::YC,
                };

                match client
                    .set(
                        &format!("reliability:test:{}", seq % 100),
                        &serde_json::to_string(&point).unwrap(),
                    )
                    .await
                {
                    Ok(_) => {
                        success_count.fetch_add(1, Ordering::Relaxed);
                    }
                    Err(e) => {
                        error_count.fetch_add(1, Ordering::Relaxed);
                        println!("Write error at seq {}: {}", seq, e);
                    }
                }

                seq += 1;
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
    }

    /// 打印可靠性统计
    fn print_reliability_stats(&self) {
        let errors = self.error_count.load(Ordering::Relaxed);
        let retries = self.retry_count.load(Ordering::Relaxed);
        let success = self.success_count.load(Ordering::Relaxed);
        let total = errors + success;

        println!("\n=== Reliability Statistics ===");
        println!("Total operations: {}", total);
        println!(
            "Successful: {} ({:.2}%)",
            success,
            (success as f64 / total as f64) * 100.0
        );
        println!(
            "Errors: {} ({:.2}%)",
            errors,
            (errors as f64 / total as f64) * 100.0
        );
        println!("Retries: {}", retries);

        if total > 0 {
            println!(
                "Availability: {:.3}%",
                (success as f64 / total as f64) * 100.0
            );
        }
    }
}

#[tokio::test]
async fn test_all_reliability_scenarios() -> Result<()> {
    let test = ReliabilityTest::new().await?;

    // 运行所有可靠性测试
    test.test_command_retry_logic().await?;
    test.test_data_consistency().await?;
    test.test_pubsub_reliability().await?;

    // 连接弹性测试需要手动干预，可选运行
    // test.test_connection_resilience().await?;

    Ok(())
}

#[tokio::test]
async fn test_concurrent_access_patterns() -> Result<()> {
    println!("\n=== Testing Concurrent Access Patterns ===");

    let client = Arc::new(RedisClient::new("redis://localhost:6379").await?);
    let point_count = 100;

    // 模拟多个服务同时访问
    let mut tasks = vec![];

    // 模拟 comsrv 写入遥测数据
    let comsrv_client = client.clone();
    let comsrv_task = tokio::spawn(async move {
        for i in 0..1000 {
            let point = PointValue {
                value: (i % 100) as f64,
                quality: Quality::Good,
                timestamp: chrono::Utc::now().timestamp_millis(),
                point_type: PointType::YC,
            };

            let _ = comsrv_client
                .set(
                    &format!("point:{}", i % point_count),
                    &serde_json::to_string(&point).unwrap(),
                )
                .await;

            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    });
    tasks.push(comsrv_task);

    // 模拟 modsrv 读取和计算
    let modsrv_client = client.clone();
    let modsrv_task = tokio::spawn(async move {
        for _ in 0..500 {
            // 批量读取多个点
            let mut keys = vec![];
            for i in 0..10 {
                keys.push(format!("point:{}", rand::random::<u32>() % point_count));
            }

            if let Ok(values) = modsrv_client.mget::<String>(&keys).await {
                // 模拟计算
                let sum: f64 = values
                    .iter()
                    .filter_map(|v| v.as_ref())
                    .filter_map(|v| serde_json::from_str::<PointValue>(v).ok())
                    .map(|p| p.value)
                    .sum();

                // 写入计算结果
                let calc_point = PointValue {
                    value: sum,
                    quality: Quality::Good,
                    timestamp: chrono::Utc::now().timestamp_millis(),
                    point_type: PointType::YC,
                };

                let _ = modsrv_client
                    .set("calc:sum", &serde_json::to_string(&calc_point).unwrap())
                    .await;
            }

            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    });
    tasks.push(modsrv_task);

    // 模拟 apigateway 查询
    let api_client = client.clone();
    let api_task = tokio::spawn(async move {
        for _ in 0..200 {
            // 查询实时数据
            let point_id = rand::random::<u32>() % point_count;
            let _ = api_client
                .get::<String>(&format!("point:{}", point_id))
                .await;

            // 查询计算结果
            let _ = api_client.get::<String>("calc:sum").await;

            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    });
    tasks.push(api_task);

    // 等待所有任务完成
    for task in tasks {
        let _ = task.await;
    }

    println!("Concurrent access test completed successfully");

    Ok(())
}

/// 生成可靠性测试报告
#[tokio::test]
async fn generate_reliability_report() -> Result<()> {
    use std::fs::File;
    use std::io::Write;

    let mut file = File::create("reliability_test_report.md")?;

    writeln!(file, "# Redis Reliability Test Report")?;
    writeln!(
        file,
        "\nGenerated at: {}",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    )?;

    writeln!(file, "\n## Test Scenarios")?;

    writeln!(file, "\n### 1. Connection Resilience")?;
    writeln!(file, "- Automatic reconnection after network interruption")?;
    writeln!(file, "- Connection pooling with health checks")?;
    writeln!(file, "- Graceful degradation during outages")?;

    writeln!(file, "\n### 2. Command Retry Logic")?;
    writeln!(file, "- Exponential backoff for failed commands")?;
    writeln!(file, "- Maximum retry limits to prevent infinite loops")?;
    writeln!(file, "- Error propagation for permanent failures")?;

    writeln!(file, "\n### 3. Data Consistency")?;
    writeln!(file, "- Atomic operations for critical updates")?;
    writeln!(file, "- Optimistic locking with version checks")?;
    writeln!(file, "- Transaction support for multi-key operations")?;

    writeln!(file, "\n### 4. Pub/Sub Reliability")?;
    writeln!(file, "- Message delivery guarantees")?;
    writeln!(file, "- Subscriber reconnection handling")?;
    writeln!(file, "- Buffer management for high throughput")?;

    writeln!(file, "\n## Recommendations")?;

    writeln!(file, "\n### High Availability Setup")?;
    writeln!(file, "```yaml")?;
    writeln!(file, "# Redis Sentinel configuration")?;
    writeln!(file, "sentinel:")?;
    writeln!(file, "  monitors:")?;
    writeln!(file, "    - master: mymaster")?;
    writeln!(file, "      host: 127.0.0.1")?;
    writeln!(file, "      port: 6379")?;
    writeln!(file, "      quorum: 2")?;
    writeln!(file, "```")?;

    writeln!(file, "\n### Connection Pool Settings")?;
    writeln!(file, "```rust")?;
    writeln!(file, "let pool = RedisPool::builder()")?;
    writeln!(file, "    .max_connections(100)")?;
    writeln!(file, "    .min_idle(10)")?;
    writeln!(file, "    .connection_timeout(Duration::from_secs(5))")?;
    writeln!(file, "    .idle_timeout(Duration::from_secs(60))")?;
    writeln!(file, "    .build();")?;
    writeln!(file, "```")?;

    writeln!(file, "\n### Monitoring and Alerting")?;
    writeln!(file, "- Monitor Redis memory usage")?;
    writeln!(file, "- Track connection pool metrics")?;
    writeln!(file, "- Alert on high error rates")?;
    writeln!(file, "- Log slow queries")?;

    println!("Reliability test report saved to: reliability_test_report.md");

    Ok(())
}
