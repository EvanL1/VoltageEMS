//! 性能基准测试
//!
//! 测试协议插件系统的性能指标，包括吞吐量、延迟、内存使用等

use std::time::{Duration, Instant};
use std::sync::Arc;
use tokio::sync::{Mutex, Semaphore};
use futures::future::join_all;
use sysinfo::{System, SystemExt, ProcessExt};
use std::process;

/// 性能测试配置
#[derive(Clone)]
struct BenchmarkConfig {
    /// 测试名称
    name: String,
    /// 并发连接数
    concurrent_connections: usize,
    /// 每个连接的操作数
    operations_per_connection: usize,
    /// 操作间隔
    operation_interval: Duration,
    /// 数据包大小
    payload_size: usize,
    /// 预热时间
    warmup_duration: Duration,
    /// 测试持续时间
    test_duration: Duration,
}

/// 性能指标
#[derive(Debug, Clone)]
struct PerformanceMetrics {
    /// 总操作数
    total_operations: usize,
    /// 成功操作数
    successful_operations: usize,
    /// 失败操作数
    failed_operations: usize,
    /// 平均延迟（毫秒）
    avg_latency_ms: f64,
    /// 最小延迟（毫秒）
    min_latency_ms: f64,
    /// 最大延迟（毫秒）
    max_latency_ms: f64,
    /// P50延迟（毫秒）
    p50_latency_ms: f64,
    /// P95延迟（毫秒）
    p95_latency_ms: f64,
    /// P99延迟（毫秒）
    p99_latency_ms: f64,
    /// 吞吐量（操作/秒）
    throughput_ops_per_sec: f64,
    /// 内存使用（MB）
    memory_usage_mb: f64,
    /// CPU使用率（%）
    cpu_usage_percent: f64,
}

/// 性能测试器
struct PerformanceBenchmark {
    config: BenchmarkConfig,
    metrics: Arc<Mutex<Vec<f64>>>, // 延迟数据
    operations: Arc<Mutex<usize>>, // 操作计数
    errors: Arc<Mutex<usize>>,     // 错误计数
}

impl PerformanceBenchmark {
    fn new(config: BenchmarkConfig) -> Self {
        Self {
            config,
            metrics: Arc::new(Mutex::new(Vec::new())),
            operations: Arc::new(Mutex::new(0)),
            errors: Arc::new(Mutex::new(0)),
        }
    }
    
    /// 运行基准测试
    async fn run(&self) -> PerformanceMetrics {
        println!("\n🚀 Running benchmark: {}", self.config.name);
        println!("Concurrent connections: {}", self.config.concurrent_connections);
        println!("Operations per connection: {}", self.config.operations_per_connection);
        println!("Payload size: {} bytes", self.config.payload_size);
        
        // 预热阶段
        if self.config.warmup_duration > Duration::ZERO {
            println!("Warming up for {:?}...", self.config.warmup_duration);
            self.warmup().await;
        }
        
        // 重置计数器
        *self.operations.lock().await = 0;
        *self.errors.lock().await = 0;
        self.metrics.lock().await.clear();
        
        // 开始测试
        println!("Starting benchmark...");
        let start_time = Instant::now();
        let initial_memory = self.get_memory_usage();
        let initial_cpu = self.get_cpu_usage().await;
        
        // 创建并发任务
        let tasks = self.create_benchmark_tasks();
        
        // 等待所有任务完成或超时
        let _ = tokio::time::timeout(
            self.config.test_duration,
            join_all(tasks)
        ).await;
        
        let elapsed = start_time.elapsed();
        let final_memory = self.get_memory_usage();
        let final_cpu = self.get_cpu_usage().await;
        
        // 计算性能指标
        self.calculate_metrics(
            elapsed,
            initial_memory,
            final_memory,
            initial_cpu,
            final_cpu
        ).await
    }
    
    /// 预热阶段
    async fn warmup(&self) {
        let warmup_tasks = self.create_benchmark_tasks();
        let _ = tokio::time::timeout(
            self.config.warmup_duration,
            join_all(warmup_tasks)
        ).await;
    }
    
    /// 创建基准测试任务
    fn create_benchmark_tasks(&self) -> Vec<tokio::task::JoinHandle<()>> {
        let semaphore = Arc::new(Semaphore::new(self.config.concurrent_connections));
        let mut tasks = Vec::new();
        
        for conn_id in 0..self.config.concurrent_connections {
            let sem = Arc::clone(&semaphore);
            let metrics = Arc::clone(&self.metrics);
            let operations = Arc::clone(&self.operations);
            let errors = Arc::clone(&self.errors);
            let config = self.config.clone();
            
            let task = tokio::spawn(async move {
                let _permit = sem.acquire().await.unwrap();
                
                for op_id in 0..config.operations_per_connection {
                    let start = Instant::now();
                    
                    // 模拟协议操作
                    match Self::simulate_protocol_operation(&config, conn_id, op_id).await {
                        Ok(_) => {
                            let latency = start.elapsed().as_secs_f64() * 1000.0;
                            metrics.lock().await.push(latency);
                            *operations.lock().await += 1;
                        }
                        Err(_) => {
                            *errors.lock().await += 1;
                        }
                    }
                    
                    // 操作间隔
                    if config.operation_interval > Duration::ZERO {
                        tokio::time::sleep(config.operation_interval).await;
                    }
                }
            });
            
            tasks.push(task);
        }
        
        tasks
    }
    
    /// 模拟协议操作
    async fn simulate_protocol_operation(
        config: &BenchmarkConfig,
        _conn_id: usize,
        _op_id: usize
    ) -> Result<(), Box<dyn std::error::Error>> {
        // 模拟数据处理延迟
        let processing_time = Duration::from_micros(
            (config.payload_size as u64 / 100).max(1)
        );
        tokio::time::sleep(processing_time).await;
        
        // 模拟随机错误（1%概率）
        if rand::random::<f64>() < 0.01 {
            return Err("Simulated error".into());
        }
        
        Ok(())
    }
    
    /// 计算性能指标
    async fn calculate_metrics(
        &self,
        elapsed: Duration,
        initial_memory: f64,
        final_memory: f64,
        initial_cpu: f64,
        final_cpu: f64
    ) -> PerformanceMetrics {
        let mut latencies = self.metrics.lock().await;
        let total_ops = *self.operations.lock().await;
        let total_errors = *self.errors.lock().await;
        
        // 排序延迟数据
        latencies.sort_by(|a, b| a.partial_cmp(b).unwrap());
        
        let metrics = if !latencies.is_empty() {
            let sum: f64 = latencies.iter().sum();
            let avg = sum / latencies.len() as f64;
            let min = *latencies.first().unwrap();
            let max = *latencies.last().unwrap();
            
            let p50_idx = (latencies.len() as f64 * 0.50) as usize;
            let p95_idx = (latencies.len() as f64 * 0.95) as usize;
            let p99_idx = (latencies.len() as f64 * 0.99) as usize;
            
            let p50 = latencies.get(p50_idx).copied().unwrap_or(0.0);
            let p95 = latencies.get(p95_idx).copied().unwrap_or(0.0);
            let p99 = latencies.get(p99_idx).copied().unwrap_or(0.0);
            
            PerformanceMetrics {
                total_operations: total_ops + total_errors,
                successful_operations: total_ops,
                failed_operations: total_errors,
                avg_latency_ms: avg,
                min_latency_ms: min,
                max_latency_ms: max,
                p50_latency_ms: p50,
                p95_latency_ms: p95,
                p99_latency_ms: p99,
                throughput_ops_per_sec: total_ops as f64 / elapsed.as_secs_f64(),
                memory_usage_mb: final_memory - initial_memory,
                cpu_usage_percent: (final_cpu - initial_cpu).max(0.0),
            }
        } else {
            PerformanceMetrics {
                total_operations: 0,
                successful_operations: 0,
                failed_operations: total_errors,
                avg_latency_ms: 0.0,
                min_latency_ms: 0.0,
                max_latency_ms: 0.0,
                p50_latency_ms: 0.0,
                p95_latency_ms: 0.0,
                p99_latency_ms: 0.0,
                throughput_ops_per_sec: 0.0,
                memory_usage_mb: 0.0,
                cpu_usage_percent: 0.0,
            }
        };
        
        self.print_metrics(&metrics);
        metrics
    }
    
    /// 获取内存使用
    fn get_memory_usage(&self) -> f64 {
        let mut system = System::new_all();
        system.refresh_all();
        
        let pid = process::id();
        if let Some(process) = system.process(pid as i32) {
            process.memory() as f64 / 1024.0 / 1024.0 // 转换为MB
        } else {
            0.0
        }
    }
    
    /// 获取CPU使用率
    async fn get_cpu_usage(&self) -> f64 {
        let mut system = System::new_all();
        system.refresh_all();
        
        // 等待一段时间以获取准确的CPU使用率
        tokio::time::sleep(Duration::from_millis(100)).await;
        system.refresh_all();
        
        let pid = process::id();
        if let Some(process) = system.process(pid as i32) {
            process.cpu_usage() as f64
        } else {
            0.0
        }
    }
    
    /// 打印性能指标
    fn print_metrics(&self, metrics: &PerformanceMetrics) {
        println!("\n📊 Performance Metrics");
        println!("{:-<60}", "");
        println!("Total Operations: {}", metrics.total_operations);
        println!("Successful: {} ({:.1}%)", 
            metrics.successful_operations,
            (metrics.successful_operations as f64 / metrics.total_operations as f64) * 100.0
        );
        println!("Failed: {}", metrics.failed_operations);
        println!();
        println!("Throughput: {:.2} ops/sec", metrics.throughput_ops_per_sec);
        println!();
        println!("Latency (ms):");
        println!("  Average: {:.2}", metrics.avg_latency_ms);
        println!("  Min: {:.2}", metrics.min_latency_ms);
        println!("  Max: {:.2}", metrics.max_latency_ms);
        println!("  P50: {:.2}", metrics.p50_latency_ms);
        println!("  P95: {:.2}", metrics.p95_latency_ms);
        println!("  P99: {:.2}", metrics.p99_latency_ms);
        println!();
        println!("Resource Usage:");
        println!("  Memory: {:.2} MB", metrics.memory_usage_mb);
        println!("  CPU: {:.1}%", metrics.cpu_usage_percent);
        println!("{:-<60}", "");
    }
}

/// 基准测试套件
pub struct BenchmarkSuite;

impl BenchmarkSuite {
    /// 运行所有基准测试
    pub async fn run_all() -> Vec<PerformanceMetrics> {
        let mut results = Vec::new();
        
        // 基础性能测试
        let basic_config = BenchmarkConfig {
            name: "Basic Performance".to_string(),
            concurrent_connections: 10,
            operations_per_connection: 100,
            operation_interval: Duration::from_millis(10),
            payload_size: 1024,
            warmup_duration: Duration::from_secs(2),
            test_duration: Duration::from_secs(10),
        };
        let benchmark = PerformanceBenchmark::new(basic_config);
        results.push(benchmark.run().await);
        
        // 高并发测试
        let high_concurrency_config = BenchmarkConfig {
            name: "High Concurrency".to_string(),
            concurrent_connections: 100,
            operations_per_connection: 50,
            operation_interval: Duration::from_millis(5),
            payload_size: 512,
            warmup_duration: Duration::from_secs(3),
            test_duration: Duration::from_secs(15),
        };
        let benchmark = PerformanceBenchmark::new(high_concurrency_config);
        results.push(benchmark.run().await);
        
        // 大数据包测试
        let large_payload_config = BenchmarkConfig {
            name: "Large Payload".to_string(),
            concurrent_connections: 20,
            operations_per_connection: 50,
            operation_interval: Duration::from_millis(20),
            payload_size: 65536, // 64KB
            warmup_duration: Duration::from_secs(2),
            test_duration: Duration::from_secs(10),
        };
        let benchmark = PerformanceBenchmark::new(large_payload_config);
        results.push(benchmark.run().await);
        
        // 持续负载测试
        let sustained_load_config = BenchmarkConfig {
            name: "Sustained Load".to_string(),
            concurrent_connections: 50,
            operations_per_connection: 1000,
            operation_interval: Duration::from_millis(2),
            payload_size: 2048,
            warmup_duration: Duration::from_secs(5),
            test_duration: Duration::from_secs(60),
        };
        let benchmark = PerformanceBenchmark::new(sustained_load_config);
        results.push(benchmark.run().await);
        
        Self::print_summary(&results);
        results
    }
    
    /// 打印测试总结
    fn print_summary(results: &[PerformanceMetrics]) {
        println!("\n🏁 Benchmark Summary");
        println!("{:=<80}", "");
        println!("{:<20} {:>15} {:>15} {:>15} {:>15}", 
            "Test", "Throughput", "Avg Latency", "P95 Latency", "Memory");
        println!("{:-<80}", "");
        
        for (i, metrics) in results.iter().enumerate() {
            let test_name = match i {
                0 => "Basic",
                1 => "High Concurrency",
                2 => "Large Payload",
                3 => "Sustained Load",
                _ => "Unknown",
            };
            
            println!("{:<20} {:>15.2} {:>15.2} {:>15.2} {:>15.2}",
                test_name,
                metrics.throughput_ops_per_sec,
                metrics.avg_latency_ms,
                metrics.p95_latency_ms,
                metrics.memory_usage_mb
            );
        }
        println!("{:=<80}", "");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_basic_benchmark() {
        let config = BenchmarkConfig {
            name: "Test Benchmark".to_string(),
            concurrent_connections: 5,
            operations_per_connection: 10,
            operation_interval: Duration::from_millis(1),
            payload_size: 256,
            warmup_duration: Duration::ZERO,
            test_duration: Duration::from_secs(2),
        };
        
        let benchmark = PerformanceBenchmark::new(config);
        let metrics = benchmark.run().await;
        
        assert!(metrics.successful_operations > 0);
        assert!(metrics.throughput_ops_per_sec > 0.0);
        assert!(metrics.avg_latency_ms >= 0.0);
    }
    
    #[tokio::test]
    #[ignore] // 忽略长时间运行的测试
    async fn test_full_benchmark_suite() {
        let results = BenchmarkSuite::run_all().await;
        assert_eq!(results.len(), 4);
        
        for metrics in results {
            assert!(metrics.throughput_ops_per_sec > 0.0);
        }
    }
}