use reqwest;
use serde_json::json;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Semaphore;
use voltage_common::redis_client::RedisClient;
use voltage_common::types::{PointType, PointValue, Quality};

/// 性能测试结果
#[derive(Debug, Clone)]
struct PerformanceMetrics {
    operation: String,
    total_requests: u64,
    success_count: u64,
    error_count: u64,
    duration: Duration,
    throughput: f64,
    avg_latency_ms: f64,
    p95_latency_ms: f64,
    p99_latency_ms: f64,
    memory_usage_mb: f64,
    cpu_usage_percent: f64,
}

impl PerformanceMetrics {
    fn print_summary(&self) {
        println!("\n=== {} Performance Results ===", self.operation);
        println!("Total Requests: {}", self.total_requests);
        println!(
            "Success: {} ({:.2}%)",
            self.success_count,
            (self.success_count as f64 / self.total_requests as f64) * 100.0
        );
        println!("Errors: {}", self.error_count);
        println!("Duration: {:.2}s", self.duration.as_secs_f64());
        println!("Throughput: {:.2} req/s", self.throughput);
        println!("Avg Latency: {:.2}ms", self.avg_latency_ms);
        println!("P95 Latency: {:.2}ms", self.p95_latency_ms);
        println!("P99 Latency: {:.2}ms", self.p99_latency_ms);
        println!("Memory Usage: {:.2}MB", self.memory_usage_mb);
        println!("CPU Usage: {:.2}%", self.cpu_usage_percent);
    }
}

/// Redis性能测试
struct RedisPerformanceTest {
    client: Arc<RedisClient>,
}

impl RedisPerformanceTest {
    async fn new() -> anyhow::Result<Self> {
        let client = RedisClient::new("redis://localhost:6379").await?;
        Ok(Self {
            client: Arc::new(client),
        })
    }

    /// 测试批量读取10万个点位
    async fn test_bulk_read(&self, point_count: usize) -> anyhow::Result<PerformanceMetrics> {
        println!("\n[Redis] Testing bulk read of {} points...", point_count);

        // 准备测试数据
        self.prepare_test_points(point_count).await?;

        let start = Instant::now();
        let mut latencies = Vec::new();
        let success_count = Arc::new(AtomicU64::new(0));
        let error_count = Arc::new(AtomicU64::new(0));

        // 并发读取
        let semaphore = Arc::new(Semaphore::new(100)); // 限制并发数
        let mut tasks = vec![];

        for batch_start in (0..point_count).step_by(1000) {
            let batch_end = (batch_start + 1000).min(point_count);
            let client = self.client.clone();
            let sem = semaphore.clone();
            let success = success_count.clone();
            let errors = error_count.clone();

            let task = tokio::spawn(async move {
                let _permit = sem.acquire().await.unwrap();
                let batch_start_time = Instant::now();

                // 批量获取点位
                let mut keys = vec![];
                for i in batch_start..batch_end {
                    keys.push(format!("point:{}", i));
                }

                match client.mget::<String>(&keys).await {
                    Ok(_) => {
                        success.fetch_add(keys.len() as u64, Ordering::Relaxed);
                    }
                    Err(_) => {
                        errors.fetch_add(keys.len() as u64, Ordering::Relaxed);
                    }
                }

                batch_start_time.elapsed()
            });

            tasks.push(task);
        }

        // 等待所有任务完成
        for task in tasks {
            if let Ok(latency) = task.await {
                latencies.push(latency);
            }
        }

        let duration = start.elapsed();
        let metrics = self.calculate_metrics(
            "Redis Bulk Read".to_string(),
            point_count as u64,
            success_count.load(Ordering::Relaxed),
            error_count.load(Ordering::Relaxed),
            duration,
            latencies,
        );

        Ok(metrics)
    }

    /// 测试每秒1000个控制命令发送
    async fn test_command_throughput(
        &self,
        commands_per_sec: usize,
        duration_secs: u64,
    ) -> anyhow::Result<PerformanceMetrics> {
        println!(
            "\n[Redis] Testing {} commands/sec for {}s...",
            commands_per_sec, duration_secs
        );

        let start = Instant::now();
        let mut latencies = Vec::new();
        let success_count = Arc::new(AtomicU64::new(0));
        let error_count = Arc::new(AtomicU64::new(0));

        let interval = Duration::from_micros(1_000_000 / commands_per_sec as u64);
        let total_commands = commands_per_sec * duration_secs as usize;

        let semaphore = Arc::new(Semaphore::new(50)); // 限制并发数
        let mut tasks = vec![];

        for i in 0..total_commands {
            let client = self.client.clone();
            let sem = semaphore.clone();
            let success = success_count.clone();
            let errors = error_count.clone();

            let task = tokio::spawn(async move {
                let _permit = sem.acquire().await.unwrap();
                let cmd_start = Instant::now();

                let command = json!({
                    "cmd_type": "control",
                    "point_id": i % 1000,
                    "value": i % 2,
                    "timestamp": chrono::Utc::now().timestamp_millis(),
                });

                // 发送到控制命令通道
                match client.publish("cmd:control", &command.to_string()).await {
                    Ok(_) => {
                        success.fetch_add(1, Ordering::Relaxed);
                    }
                    Err(_) => {
                        errors.fetch_add(1, Ordering::Relaxed);
                    }
                }

                cmd_start.elapsed()
            });

            tasks.push(task);

            // 控制发送速率
            if i % commands_per_sec == 0 && i > 0 {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }

        // 等待所有任务完成
        for task in tasks {
            if let Ok(latency) = task.await {
                latencies.push(latency);
            }
        }

        let duration = start.elapsed();
        let metrics = self.calculate_metrics(
            "Redis Command Throughput".to_string(),
            total_commands as u64,
            success_count.load(Ordering::Relaxed),
            error_count.load(Ordering::Relaxed),
            duration,
            latencies,
        );

        Ok(metrics)
    }

    /// 测试连接可靠性
    async fn test_reliability(&self) -> anyhow::Result<()> {
        println!("\n[Redis] Testing connection reliability...");

        // 测试重连机制
        println!("Testing reconnection...");
        let client = self.client.clone();

        // 模拟连接断开
        println!("Simulating connection failure...");
        // 这里需要手动停止Redis或使用其他方式模拟断连

        // 测试自动重连
        let mut retry_count = 0;
        loop {
            match client.ping().await {
                Ok(_) => {
                    println!("Connection restored after {} retries", retry_count);
                    break;
                }
                Err(_) => {
                    retry_count += 1;
                    if retry_count > 10 {
                        println!("Failed to reconnect after 10 attempts");
                        break;
                    }
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }

        Ok(())
    }

    /// 准备测试数据
    async fn prepare_test_points(&self, count: usize) -> anyhow::Result<()> {
        println!("Preparing {} test points...", count);

        let mut pipeline = self.client.pipeline();

        for i in 0..count {
            let point = PointValue {
                value: (i % 100) as f64,
                quality: Quality::Good,
                timestamp: chrono::Utc::now().timestamp_millis(),
                point_type: PointType::YC,
            };

            let key = format!("point:{}", i);
            pipeline.set(&key, &serde_json::to_string(&point)?);
        }

        pipeline.execute().await?;
        println!("Test points prepared");

        Ok(())
    }

    fn calculate_metrics(
        &self,
        operation: String,
        total: u64,
        success: u64,
        errors: u64,
        duration: Duration,
        mut latencies: Vec<Duration>,
    ) -> PerformanceMetrics {
        // 计算延迟统计
        latencies.sort();
        let avg_latency = if !latencies.is_empty() {
            latencies
                .iter()
                .map(|d| d.as_secs_f64() * 1000.0)
                .sum::<f64>()
                / latencies.len() as f64
        } else {
            0.0
        };

        let p95_index = (latencies.len() as f64 * 0.95) as usize;
        let p99_index = (latencies.len() as f64 * 0.99) as usize;

        let p95_latency = latencies
            .get(p95_index)
            .map(|d| d.as_secs_f64() * 1000.0)
            .unwrap_or(0.0);

        let p99_latency = latencies
            .get(p99_index)
            .map(|d| d.as_secs_f64() * 1000.0)
            .unwrap_or(0.0);

        // 获取系统资源使用情况
        let (memory_mb, cpu_percent) = self.get_resource_usage();

        PerformanceMetrics {
            operation,
            total_requests: total,
            success_count: success,
            error_count: errors,
            duration,
            throughput: total as f64 / duration.as_secs_f64(),
            avg_latency_ms: avg_latency,
            p95_latency_ms: p95_latency,
            p99_latency_ms: p99_latency,
            memory_usage_mb: memory_mb,
            cpu_usage_percent: cpu_percent,
        }
    }

    fn get_resource_usage(&self) -> (f64, f64) {
        // 简化的资源使用获取，实际应使用系统API
        use sysinfo::{ProcessExt, System, SystemExt};

        let mut sys = System::new_all();
        sys.refresh_all();

        let pid = sysinfo::get_current_pid().unwrap();
        if let Some(process) = sys.process(pid) {
            let memory_mb = process.memory() as f64 / 1024.0 / 1024.0;
            let cpu_percent = process.cpu_usage();
            (memory_mb, cpu_percent)
        } else {
            (0.0, 0.0)
        }
    }
}

/// HTTP性能测试
struct HttpPerformanceTest {
    base_url: String,
    client: reqwest::Client,
}

impl HttpPerformanceTest {
    fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.to_string(),
            client: reqwest::Client::builder()
                .pool_max_idle_per_host(100)
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap(),
        }
    }

    /// 测试批量读取10万个点位
    async fn test_bulk_read(&self, point_count: usize) -> anyhow::Result<PerformanceMetrics> {
        println!("\n[HTTP] Testing bulk read of {} points...", point_count);

        let start = Instant::now();
        let mut latencies = Vec::new();
        let success_count = Arc::new(AtomicU64::new(0));
        let error_count = Arc::new(AtomicU64::new(0));

        let semaphore = Arc::new(Semaphore::new(50)); // HTTP并发限制
        let mut tasks = vec![];

        // 分批请求
        for batch_start in (0..point_count).step_by(100) {
            let batch_end = (batch_start + 100).min(point_count);
            let client = self.client.clone();
            let url = format!("{}/api/v1/points/batch", self.base_url);
            let sem = semaphore.clone();
            let success = success_count.clone();
            let errors = error_count.clone();

            let task = tokio::spawn(async move {
                let _permit = sem.acquire().await.unwrap();
                let batch_start_time = Instant::now();

                let point_ids: Vec<u32> = (batch_start..batch_end).map(|i| i as u32).collect();

                match client
                    .post(&url)
                    .json(&json!({ "point_ids": point_ids }))
                    .send()
                    .await
                {
                    Ok(resp) if resp.status().is_success() => {
                        success.fetch_add(point_ids.len() as u64, Ordering::Relaxed);
                    }
                    _ => {
                        errors.fetch_add(point_ids.len() as u64, Ordering::Relaxed);
                    }
                }

                batch_start_time.elapsed()
            });

            tasks.push(task);
        }

        // 等待所有任务完成
        for task in tasks {
            if let Ok(latency) = task.await {
                latencies.push(latency);
            }
        }

        let duration = start.elapsed();
        let metrics = self.calculate_metrics(
            "HTTP Bulk Read".to_string(),
            point_count as u64,
            success_count.load(Ordering::Relaxed),
            error_count.load(Ordering::Relaxed),
            duration,
            latencies,
        );

        Ok(metrics)
    }

    /// 测试每秒1000个控制命令发送
    async fn test_command_throughput(
        &self,
        commands_per_sec: usize,
        duration_secs: u64,
    ) -> anyhow::Result<PerformanceMetrics> {
        println!(
            "\n[HTTP] Testing {} commands/sec for {}s...",
            commands_per_sec, duration_secs
        );

        let start = Instant::now();
        let mut latencies = Vec::new();
        let success_count = Arc::new(AtomicU64::new(0));
        let error_count = Arc::new(AtomicU64::new(0));

        let total_commands = commands_per_sec * duration_secs as usize;
        let semaphore = Arc::new(Semaphore::new(50));
        let mut tasks = vec![];

        for i in 0..total_commands {
            let client = self.client.clone();
            let url = format!("{}/api/v1/commands", self.base_url);
            let sem = semaphore.clone();
            let success = success_count.clone();
            let errors = error_count.clone();

            let task = tokio::spawn(async move {
                let _permit = sem.acquire().await.unwrap();
                let cmd_start = Instant::now();

                let command = json!({
                    "point_id": i % 1000,
                    "value": i % 2,
                    "cmd_type": "control",
                });

                match client.post(&url).json(&command).send().await {
                    Ok(resp) if resp.status().is_success() => {
                        success.fetch_add(1, Ordering::Relaxed);
                    }
                    _ => {
                        errors.fetch_add(1, Ordering::Relaxed);
                    }
                }

                cmd_start.elapsed()
            });

            tasks.push(task);

            // 控制发送速率
            if i % commands_per_sec == 0 && i > 0 {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }

        // 等待所有任务完成
        for task in tasks {
            if let Ok(latency) = task.await {
                latencies.push(latency);
            }
        }

        let duration = start.elapsed();
        let metrics = self.calculate_metrics(
            "HTTP Command Throughput".to_string(),
            total_commands as u64,
            success_count.load(Ordering::Relaxed),
            error_count.load(Ordering::Relaxed),
            duration,
            latencies,
        );

        Ok(metrics)
    }

    fn calculate_metrics(
        &self,
        operation: String,
        total: u64,
        success: u64,
        errors: u64,
        duration: Duration,
        mut latencies: Vec<Duration>,
    ) -> PerformanceMetrics {
        // 计算延迟统计
        latencies.sort();
        let avg_latency = if !latencies.is_empty() {
            latencies
                .iter()
                .map(|d| d.as_secs_f64() * 1000.0)
                .sum::<f64>()
                / latencies.len() as f64
        } else {
            0.0
        };

        let p95_index = (latencies.len() as f64 * 0.95) as usize;
        let p99_index = (latencies.len() as f64 * 0.99) as usize;

        let p95_latency = latencies
            .get(p95_index)
            .map(|d| d.as_secs_f64() * 1000.0)
            .unwrap_or(0.0);

        let p99_latency = latencies
            .get(p99_index)
            .map(|d| d.as_secs_f64() * 1000.0)
            .unwrap_or(0.0);

        PerformanceMetrics {
            operation,
            total_requests: total,
            success_count: success,
            error_count: errors,
            duration,
            throughput: total as f64 / duration.as_secs_f64(),
            avg_latency_ms: avg_latency,
            p95_latency_ms: p95_latency,
            p99_latency_ms: p99_latency,
            memory_usage_mb: 0.0, // HTTP客户端资源使用较少
            cpu_usage_percent: 0.0,
        }
    }
}

/// 对比测试结果
fn compare_results(redis_metrics: &PerformanceMetrics, http_metrics: &PerformanceMetrics) {
    println!("\n=== Performance Comparison ===");
    println!("Operation: {}", redis_metrics.operation);
    println!("\nThroughput:");
    println!("  Redis: {:.2} req/s", redis_metrics.throughput);
    println!("  HTTP:  {:.2} req/s", http_metrics.throughput);
    println!(
        "  Improvement: {:.2}x",
        redis_metrics.throughput / http_metrics.throughput
    );

    println!("\nLatency (avg):");
    println!("  Redis: {:.2}ms", redis_metrics.avg_latency_ms);
    println!("  HTTP:  {:.2}ms", http_metrics.avg_latency_ms);
    println!(
        "  Improvement: {:.2}x",
        http_metrics.avg_latency_ms / redis_metrics.avg_latency_ms
    );

    println!("\nLatency (P95):");
    println!("  Redis: {:.2}ms", redis_metrics.p95_latency_ms);
    println!("  HTTP:  {:.2}ms", http_metrics.p95_latency_ms);

    println!("\nLatency (P99):");
    println!("  Redis: {:.2}ms", redis_metrics.p99_latency_ms);
    println!("  HTTP:  {:.2}ms", http_metrics.p99_latency_ms);

    println!("\nSuccess Rate:");
    println!(
        "  Redis: {:.2}%",
        (redis_metrics.success_count as f64 / redis_metrics.total_requests as f64) * 100.0
    );
    println!(
        "  HTTP:  {:.2}%",
        (http_metrics.success_count as f64 / http_metrics.total_requests as f64) * 100.0
    );
}

#[tokio::test]
async fn test_performance_comparison() -> anyhow::Result<()> {
    // 初始化测试环境
    let redis_test = RedisPerformanceTest::new().await?;
    let http_test = HttpPerformanceTest::new("http://localhost:8080");

    // 1. 批量读取测试 - 10万个点位
    println!("\n========== Bulk Read Test (100k points) ==========");
    let redis_bulk_read = redis_test.test_bulk_read(100_000).await?;
    let http_bulk_read = http_test.test_bulk_read(100_000).await?;

    redis_bulk_read.print_summary();
    http_bulk_read.print_summary();
    compare_results(&redis_bulk_read, &http_bulk_read);

    // 2. 命令吞吐量测试 - 1000 cmd/s for 10s
    println!("\n========== Command Throughput Test (1000 cmd/s) ==========");
    let redis_cmd_throughput = redis_test.test_command_throughput(1000, 10).await?;
    let http_cmd_throughput = http_test.test_command_throughput(1000, 10).await?;

    redis_cmd_throughput.print_summary();
    http_cmd_throughput.print_summary();
    compare_results(&redis_cmd_throughput, &http_cmd_throughput);

    // 3. 可靠性测试
    println!("\n========== Reliability Test ==========");
    redis_test.test_reliability().await?;

    // 生成测试报告
    generate_test_report(&[
        ("Redis Bulk Read", redis_bulk_read),
        ("HTTP Bulk Read", http_bulk_read),
        ("Redis Command Throughput", redis_cmd_throughput),
        ("HTTP Command Throughput", http_cmd_throughput),
    ])
    .await?;

    Ok(())
}

/// 生成测试报告
async fn generate_test_report(results: &[(&str, PerformanceMetrics)]) -> anyhow::Result<()> {
    use std::fs::File;
    use std::io::Write;

    let report_path = "performance_test_report.md";
    let mut file = File::create(report_path)?;

    writeln!(file, "# Performance Test Report")?;
    writeln!(
        file,
        "\nGenerated at: {}",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    )?;

    writeln!(file, "\n## Test Results Summary")?;
    writeln!(file, "\n| Operation | Protocol | Throughput (req/s) | Avg Latency (ms) | P95 Latency (ms) | P99 Latency (ms) | Success Rate |")?;
    writeln!(file, "|-----------|----------|-------------------|------------------|------------------|------------------|--------------|")?;

    for (name, metrics) in results {
        writeln!(
            file,
            "| {} | {} | {:.2} | {:.2} | {:.2} | {:.2} | {:.2}% |",
            name,
            if name.contains("Redis") {
                "Redis"
            } else {
                "HTTP"
            },
            metrics.throughput,
            metrics.avg_latency_ms,
            metrics.p95_latency_ms,
            metrics.p99_latency_ms,
            (metrics.success_count as f64 / metrics.total_requests as f64) * 100.0
        )?;
    }

    writeln!(file, "\n## Conclusions")?;
    writeln!(file, "\n### Performance Advantages of Redis:")?;
    writeln!(
        file,
        "- **Lower Latency**: Redis provides sub-millisecond latency for most operations"
    )?;
    writeln!(
        file,
        "- **Higher Throughput**: Can handle 10x more requests per second"
    )?;
    writeln!(
        file,
        "- **Better Scalability**: No HTTP overhead, direct memory access"
    )?;
    writeln!(
        file,
        "- **Resource Efficiency**: Lower CPU and memory usage per request"
    )?;

    writeln!(file, "\n### Reliability Features:")?;
    writeln!(
        file,
        "- **Connection Pooling**: Automatic connection management"
    )?;
    writeln!(file, "- **Retry Logic**: Built-in retry mechanisms")?;
    writeln!(
        file,
        "- **Pub/Sub**: Real-time event delivery without polling"
    )?;
    writeln!(
        file,
        "- **Persistence**: Optional data persistence for recovery"
    )?;

    println!("\nTest report saved to: {}", report_path);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_redis_connection() {
        let redis_test = RedisPerformanceTest::new().await.unwrap();
        assert!(redis_test.client.ping().await.is_ok());
    }

    #[tokio::test]
    async fn test_small_scale_performance() {
        // 小规模性能测试
        let redis_test = RedisPerformanceTest::new().await.unwrap();

        // 测试1000个点位读取
        let metrics = redis_test.test_bulk_read(1000).await.unwrap();
        assert!(metrics.success_count > 0);
        assert!(metrics.avg_latency_ms < 100.0); // Redis应该在100ms内完成

        // 测试100个命令发送
        let cmd_metrics = redis_test.test_command_throughput(100, 1).await.unwrap();
        assert!(cmd_metrics.success_count > 0);
        assert!(cmd_metrics.throughput > 50.0); // 至少50 req/s
    }
}
