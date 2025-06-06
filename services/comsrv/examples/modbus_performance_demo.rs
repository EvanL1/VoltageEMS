/// Modbus 性能测试演示
/// 
/// 这个示例展示如何使用基本的性能测试功能
/// 包含多种测试场景和详细的性能分析

use std::time::{Duration, Instant};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::time::sleep;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    
    println!("🔥 Modbus性能测试演示");
    println!("{}", "=".repeat(50));
    
    // 运行不同的测试场景
    run_all_scenarios().await?;
    
    println!("\n🎉 所有测试场景完成!");
    println!("{}", "=".repeat(50));
    
    Ok(())
}

/// 运行所有测试场景
async fn run_all_scenarios() -> anyhow::Result<()> {
    // 场景1: 基本性能测试
    println!("\n🎯 场景1: 基本性能测试");
    println!("{}", "-".repeat(30));
    
    let start_time = Instant::now();
    let total_requests = Arc::new(AtomicUsize::new(0));
    let successful_requests = Arc::new(AtomicUsize::new(0));
    
    // 模拟基本性能测试
    let concurrent_clients = 5;
    let requests_per_client = 50;
    
    let mut tasks = Vec::new();
    for client_id in 0..concurrent_clients {
        let total_clone = total_requests.clone();
        let success_clone = successful_requests.clone();
        
        let task = tokio::spawn(async move {
            for i in 0..requests_per_client {
                total_clone.fetch_add(1, Ordering::Relaxed);
                
                // 模拟请求处理时间
                sleep(Duration::from_millis(10)).await;
                
                // 模拟90%成功率 (简单的模拟逻辑)
                if (client_id * requests_per_client + i) % 10 != 0 {
                    success_clone.fetch_add(1, Ordering::Relaxed);
                }
            }
        });
        tasks.push(task);
    }
    
    // 等待所有任务完成
    for task in tasks {
        let _ = task.await;
    }
    
    let elapsed = start_time.elapsed();
    let total = total_requests.load(Ordering::Relaxed);
    let successful = successful_requests.load(Ordering::Relaxed);
    
    print_test_results("基本性能测试", total, successful, elapsed);
    
    // 短暂休息
    sleep(Duration::from_secs(2)).await;
    
    // 场景2: 高并发测试
    println!("\n🚀 场景2: 高并发测试");
    println!("{}", "-".repeat(30));
    
    let start_time = Instant::now();
    let total_requests = Arc::new(AtomicUsize::new(0));
    let successful_requests = Arc::new(AtomicUsize::new(0));
    
    let concurrent_clients = 50;
    let requests_per_client = 20;
    
    let mut tasks = Vec::new();
    for client_id in 0..concurrent_clients {
        let total_clone = total_requests.clone();
        let success_clone = successful_requests.clone();
        
        let task = tokio::spawn(async move {
            for i in 0..requests_per_client {
                total_clone.fetch_add(1, Ordering::Relaxed);
                
                // 模拟更短的请求处理时间
                sleep(Duration::from_millis(5)).await;
                
                // 模拟85%成功率（高并发下略低）
                if (client_id * requests_per_client + i) % 7 != 0 {
                    success_clone.fetch_add(1, Ordering::Relaxed);
                }
            }
        });
        tasks.push(task);
    }
    
    // 等待所有任务完成
    for task in tasks {
        let _ = task.await;
    }
    
    let elapsed = start_time.elapsed();
    let total = total_requests.load(Ordering::Relaxed);
    let successful = successful_requests.load(Ordering::Relaxed);
    
    print_test_results("高并发测试", total, successful, elapsed);
    
    // 短暂休息
    sleep(Duration::from_secs(3)).await;
    
    // 场景3: 多功能码测试
    println!("\n🔧 场景3: 多功能码测试");
    println!("{}", "-".repeat(30));
    
    let function_codes = vec!["读取保持寄存器(0x03)", "读取输入寄存器(0x04)", "读取线圈(0x01)", "读取离散输入(0x02)"];
    
    for (func_idx, func_name) in function_codes.iter().enumerate() {
        println!("  测试 {}...", func_name);
        
        let start_time = Instant::now();
        let total_requests = Arc::new(AtomicUsize::new(0));
        let successful_requests = Arc::new(AtomicUsize::new(0));
        
        let concurrent_clients = 10;
        let requests_per_client = 20;
        
        let mut tasks = Vec::new();
        for client_id in 0..concurrent_clients {
            let total_clone = total_requests.clone();
            let success_clone = successful_requests.clone();
            
            let task = tokio::spawn(async move {
                for i in 0..requests_per_client {
                    total_clone.fetch_add(1, Ordering::Relaxed);
                    
                    // 模拟请求处理时间
                    sleep(Duration::from_millis(8)).await;
                    
                    // 模拟88%成功率 (基于func_idx和其他参数的简单模拟)
                    if (func_idx + client_id * requests_per_client + i) % 8 != 0 {
                        success_clone.fetch_add(1, Ordering::Relaxed);
                    }
                }
            });
            tasks.push(task);
        }
        
        // 等待所有任务完成
        for task in tasks {
            let _ = task.await;
        }
        
        let elapsed = start_time.elapsed();
        let total = total_requests.load(Ordering::Relaxed);
        let successful = successful_requests.load(Ordering::Relaxed);
        
        println!("    完成: {}/{} 请求, 成功率: {:.1}%, 耗时: {:.2}s", 
               successful, total, 
               (successful as f64 / total as f64) * 100.0,
               elapsed.as_secs_f64());
    }
    
    Ok(())
}

/// 打印测试环境信息
fn print_system_info() {
    println!("🖥️  测试环境信息:");
    println!("  操作系统: {}", std::env::consts::OS);
    println!("  架构: {}", std::env::consts::ARCH);
    println!("  Rust版本: {}", env!("CARGO_PKG_RUST_VERSION", "unknown"));
    
    // 获取系统负载信息（如果可用）
    if let Ok(load_avg) = std::fs::read_to_string("/proc/loadavg") {
        let load_parts: Vec<&str> = load_avg.split_whitespace().collect();
        if load_parts.len() >= 3 {
            println!("  系统负载: {} {} {}", load_parts[0], load_parts[1], load_parts[2]);
        }
    }
}

/// 运行基准性能测试
async fn run_benchmark_suite() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n🏁 Modbus性能基准测试套件");
    println!("{}", "=".repeat(50));
    
    print_system_info();
    
    // 预热测试
    println!("\n🔥 预热测试...");
    sleep(Duration::from_secs(1)).await;
    println!("预热完成，开始正式基准测试...\n");
    
    // 不同并发级别的基准测试
    let concurrency_levels = vec![1, 5, 10, 20, 50];
    
    for &concurrency in &concurrency_levels {
        println!("📈 测试并发级别: {} 客户端", concurrency);
        
        let start_time = Instant::now();
        let total_requests = Arc::new(AtomicUsize::new(0));
        let successful_requests = Arc::new(AtomicUsize::new(0));
        
        let requests_per_client = 100;
        
        let mut tasks = Vec::new();
        for client_id in 0..concurrency {
            let total_clone = total_requests.clone();
            let success_clone = successful_requests.clone();
            
            let task = tokio::spawn(async move {
                for i in 0..requests_per_client {
                    total_clone.fetch_add(1, Ordering::Relaxed);
                    
                    // 无间隔，最大化吞吐量
                    // 模拟请求处理
                    
                    // 模拟高成功率 (95% 成功率的简单模拟)
                    if (client_id * requests_per_client + i) % 20 != 0 {
                        success_clone.fetch_add(1, Ordering::Relaxed);
                    }
                }
            });
            tasks.push(task);
        }
        
        // 等待所有任务完成
        for task in tasks {
            let _ = task.await;
        }
        
        let elapsed = start_time.elapsed();
        let total = total_requests.load(Ordering::Relaxed);
        let successful = successful_requests.load(Ordering::Relaxed);
        
        let throughput = total as f64 / elapsed.as_secs_f64();
        let success_rate = (successful as f64 / total as f64) * 100.0;
        
        println!("  并发{}: 吞吐量={:.1} RPS, 成功率={:.1}%",
                concurrency, throughput, success_rate);
        
        // 短暂休息以避免资源竞争
        sleep(Duration::from_secs(2)).await;
    }
    
    println!("\n✅ 基准测试套件完成");
    Ok(())
}

/// 打印测试结果
fn print_test_results(scenario_name: &str, total: usize, successful: usize, elapsed: Duration) {
    println!("\n📈 {} 结果:", scenario_name);
    println!("   总请求数: {}", total);
    println!("   成功请求: {}", successful);
    println!("   失败请求: {}", total - successful);
    println!("   成功率: {:.2}%", (successful as f64 / total as f64) * 100.0);
    println!("   总耗时: {:.2}秒", elapsed.as_secs_f64());
    println!("   吞吐量: {:.2} RPS", total as f64 / elapsed.as_secs_f64());
} 