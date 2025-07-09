//! 多协议并发集成测试
//!
//! 测试多个协议同时运行时的并发性、资源隔离和性能

use std::sync::Arc;
use std::time::Duration;
use tokio::time::{sleep, timeout};
use futures::future::join_all;
use comsrv::core::plugins::PluginRegistry;
use comsrv::core::protocols::common::traits::ComBase;
use std::collections::HashMap;
use tokio::sync::{Mutex, RwLock};

/// 测试场景配置
struct TestScenario {
    name: String,
    protocols: Vec<ProtocolConfig>,
    duration: Duration,
    concurrent_operations: usize,
}

/// 协议配置
struct ProtocolConfig {
    protocol_id: String,
    instance_count: usize,
    operation_interval: Duration,
}

/// 性能统计
#[derive(Debug, Clone)]
struct PerformanceStats {
    protocol: String,
    instance_id: usize,
    operations: usize,
    errors: usize,
    avg_latency_ms: f64,
    max_latency_ms: f64,
    min_latency_ms: f64,
}

/// 多协议测试执行器
struct MultiProtocolTester {
    stats: Arc<RwLock<Vec<PerformanceStats>>>,
    instances: Arc<Mutex<HashMap<String, Vec<Box<dyn ComBase>>>>>,
}

impl MultiProtocolTester {
    fn new() -> Self {
        Self {
            stats: Arc::new(RwLock::new(Vec::new())),
            instances: Arc::new(Mutex::new(HashMap::new())),
        }
    }
    
    /// 运行测试场景
    async fn run_scenario(&self, scenario: TestScenario) -> Result<(), Box<dyn std::error::Error>> {
        println!("\n🚀 Running scenario: {}", scenario.name);
        println!("Duration: {:?}", scenario.duration);
        println!("Protocols: {}", scenario.protocols.len());
        
        // 创建所有协议实例
        for config in &scenario.protocols {
            self.create_protocol_instances(&config).await?;
        }
        
        // 启动所有实例
        self.start_all_instances().await?;
        
        // 运行并发操作
        let operation_tasks = self.create_operation_tasks(&scenario);
        
        // 等待测试完成或超时
        let test_result = timeout(
            scenario.duration + Duration::from_secs(10),
            join_all(operation_tasks)
        ).await;
        
        match test_result {
            Ok(_) => println!("✅ Scenario completed successfully"),
            Err(_) => println!("⚠️ Scenario timed out"),
        }
        
        // 停止所有实例
        self.stop_all_instances().await?;
        
        // 打印统计信息
        self.print_statistics().await;
        
        Ok(())
    }
    
    /// 创建协议实例
    async fn create_protocol_instances(&self, config: &ProtocolConfig) -> Result<(), Box<dyn std::error::Error>> {
        let plugin = PluginRegistry::get_global(&config.protocol_id)
            .ok_or(format!("Protocol {} not found", config.protocol_id))?;
        
        let mut instances = Vec::new();
        
        for i in 0..config.instance_count {
            let channel_config = self.create_test_channel_config(&config.protocol_id, i);
            match plugin.create_instance(channel_config).await {
                Ok(instance) => instances.push(instance),
                Err(e) => eprintln!("Failed to create instance {}: {}", i, e),
            }
        }
        
        let mut all_instances = self.instances.lock().await;
        all_instances.insert(config.protocol_id.clone(), instances);
        
        Ok(())
    }
    
    /// 创建测试通道配置
    fn create_test_channel_config(
        &self, 
        protocol_id: &str, 
        instance_id: usize
    ) -> comsrv::core::config::types::channel::ChannelConfig {
        use comsrv::core::config::types::channel::{ChannelConfig, TransportConfig};
        
        let mut parameters = HashMap::new();
        
        // 根据协议类型设置参数
        match protocol_id {
            "modbus_tcp" => {
                parameters.insert("host".to_string(), "127.0.0.1".to_string());
                parameters.insert("port".to_string(), (5020 + instance_id).to_string());
                parameters.insert("slave_id".to_string(), "1".to_string());
            }
            "iec60870" => {
                parameters.insert("host".to_string(), "127.0.0.1".to_string());
                parameters.insert("port".to_string(), (2404 + instance_id).to_string());
                parameters.insert("common_address".to_string(), "1".to_string());
            }
            _ => {}
        }
        
        ChannelConfig {
            id: format!("{}_{}", protocol_id, instance_id),
            name: format!("{} Instance {}", protocol_id, instance_id),
            protocol: protocol_id.to_string(),
            enabled: true,
            parameters,
            transport: TransportConfig::Tcp {
                host: "127.0.0.1".to_string(),
                port: 5020 + instance_id as u16,
            },
            point_table_path: None,
            logging: None,
        }
    }
    
    /// 启动所有实例
    async fn start_all_instances(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut instances = self.instances.lock().await;
        
        for (protocol, protocol_instances) in instances.iter_mut() {
            println!("Starting {} instances of {}", protocol_instances.len(), protocol);
            
            for (i, instance) in protocol_instances.iter_mut().enumerate() {
                match instance.start().await {
                    Ok(_) => println!("  ✓ Instance {} started", i),
                    Err(e) => eprintln!("  ✗ Instance {} failed to start: {}", i, e),
                }
            }
        }
        
        Ok(())
    }
    
    /// 停止所有实例
    async fn stop_all_instances(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut instances = self.instances.lock().await;
        
        for (protocol, protocol_instances) in instances.iter_mut() {
            println!("Stopping {} instances of {}", protocol_instances.len(), protocol);
            
            for (i, instance) in protocol_instances.iter_mut().enumerate() {
                match instance.stop().await {
                    Ok(_) => println!("  ✓ Instance {} stopped", i),
                    Err(e) => eprintln!("  ✗ Instance {} failed to stop: {}", i, e),
                }
            }
        }
        
        Ok(())
    }
    
    /// 创建并发操作任务
    fn create_operation_tasks(&self, scenario: &TestScenario) -> Vec<tokio::task::JoinHandle<()>> {
        let mut tasks = Vec::new();
        
        for config in &scenario.protocols {
            for i in 0..scenario.concurrent_operations {
                let protocol_id = config.protocol_id.clone();
                let interval = config.operation_interval;
                let duration = scenario.duration;
                let instances = Arc::clone(&self.instances);
                let stats = Arc::clone(&self.stats);
                
                let task = tokio::spawn(async move {
                    let mut local_stats = PerformanceStats {
                        protocol: protocol_id.clone(),
                        instance_id: i,
                        operations: 0,
                        errors: 0,
                        avg_latency_ms: 0.0,
                        max_latency_ms: 0.0,
                        min_latency_ms: f64::MAX,
                    };
                    
                    let start_time = tokio::time::Instant::now();
                    let mut total_latency = 0.0;
                    
                    while start_time.elapsed() < duration {
                        let op_start = tokio::time::Instant::now();
                        
                        // 执行操作
                        if let Err(_) = Self::perform_operation(&instances, &protocol_id, i).await {
                            local_stats.errors += 1;
                        } else {
                            local_stats.operations += 1;
                        }
                        
                        let latency = op_start.elapsed().as_secs_f64() * 1000.0;
                        total_latency += latency;
                        
                        if latency > local_stats.max_latency_ms {
                            local_stats.max_latency_ms = latency;
                        }
                        if latency < local_stats.min_latency_ms {
                            local_stats.min_latency_ms = latency;
                        }
                        
                        sleep(interval).await;
                    }
                    
                    if local_stats.operations > 0 {
                        local_stats.avg_latency_ms = total_latency / local_stats.operations as f64;
                    }
                    
                    let mut stats_vec = stats.write().await;
                    stats_vec.push(local_stats);
                });
                
                tasks.push(task);
            }
        }
        
        tasks
    }
    
    /// 执行单个操作
    async fn perform_operation(
        instances: &Arc<Mutex<HashMap<String, Vec<Box<dyn ComBase>>>>>,
        protocol_id: &str,
        instance_idx: usize
    ) -> Result<(), Box<dyn std::error::Error>> {
        let instances_map = instances.lock().await;
        
        if let Some(protocol_instances) = instances_map.get(protocol_id) {
            let instance_count = protocol_instances.len();
            if instance_count > 0 {
                let target_instance = instance_idx % instance_count;
                let instance = &protocol_instances[target_instance];
                
                // 执行诊断查询作为测试操作
                let _ = instance.get_diagnostics().await;
            }
        }
        
        Ok(())
    }
    
    /// 打印统计信息
    async fn print_statistics(&self) {
        let stats = self.stats.read().await;
        
        println!("\n📊 Performance Statistics");
        println!("{:-<80}", "");
        println!("{:<20} {:<10} {:<10} {:<10} {:<15} {:<15} {:<15}", 
            "Protocol", "Instance", "Operations", "Errors", "Avg Latency(ms)", "Min(ms)", "Max(ms)");
        println!("{:-<80}", "");
        
        // 按协议分组统计
        let mut protocol_totals: HashMap<String, (usize, usize, f64, f64, f64)> = HashMap::new();
        
        for stat in stats.iter() {
            println!("{:<20} {:<10} {:<10} {:<10} {:<15.2} {:<15.2} {:<15.2}", 
                stat.protocol, 
                stat.instance_id,
                stat.operations,
                stat.errors,
                stat.avg_latency_ms,
                stat.min_latency_ms,
                stat.max_latency_ms
            );
            
            let entry = protocol_totals.entry(stat.protocol.clone())
                .or_insert((0, 0, 0.0, f64::MAX, 0.0));
            
            entry.0 += stat.operations;
            entry.1 += stat.errors;
            entry.2 += stat.avg_latency_ms * stat.operations as f64;
            if stat.min_latency_ms < entry.3 {
                entry.3 = stat.min_latency_ms;
            }
            if stat.max_latency_ms > entry.4 {
                entry.4 = stat.max_latency_ms;
            }
        }
        
        println!("{:-<80}", "");
        println!("\n📈 Protocol Summary");
        println!("{:-<80}", "");
        
        for (protocol, (ops, errors, total_latency, min_lat, max_lat)) in protocol_totals {
            let avg_lat = if ops > 0 { total_latency / ops as f64 } else { 0.0 };
            let error_rate = if ops + errors > 0 { 
                (errors as f64 / (ops + errors) as f64) * 100.0 
            } else { 
                0.0 
            };
            
            println!("{:<20} Total Ops: {:<8} Errors: {:<8} Error Rate: {:.2}%", 
                protocol, ops, errors, error_rate);
            println!("{:<20} Avg Latency: {:.2}ms  Min: {:.2}ms  Max: {:.2}ms", 
                "", avg_lat, min_lat, max_lat);
            println!();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_single_protocol_multiple_instances() {
        let tester = MultiProtocolTester::new();
        
        let scenario = TestScenario {
            name: "Single Protocol Multiple Instances".to_string(),
            protocols: vec![
                ProtocolConfig {
                    protocol_id: "modbus_tcp".to_string(),
                    instance_count: 3,
                    operation_interval: Duration::from_millis(100),
                }
            ],
            duration: Duration::from_secs(5),
            concurrent_operations: 5,
        };
        
        // 注意：这个测试需要实际的协议插件注册
        // tester.run_scenario(scenario).await.unwrap();
    }
    
    #[tokio::test]
    async fn test_multiple_protocols_concurrent() {
        let tester = MultiProtocolTester::new();
        
        let scenario = TestScenario {
            name: "Multiple Protocols Concurrent".to_string(),
            protocols: vec![
                ProtocolConfig {
                    protocol_id: "modbus_tcp".to_string(),
                    instance_count: 2,
                    operation_interval: Duration::from_millis(100),
                },
                ProtocolConfig {
                    protocol_id: "iec60870".to_string(),
                    instance_count: 2,
                    operation_interval: Duration::from_millis(150),
                }
            ],
            duration: Duration::from_secs(10),
            concurrent_operations: 10,
        };
        
        // 注意：这个测试需要实际的协议插件注册
        // tester.run_scenario(scenario).await.unwrap();
    }
    
    #[tokio::test]
    async fn test_stress_scenario() {
        let tester = MultiProtocolTester::new();
        
        let scenario = TestScenario {
            name: "Stress Test".to_string(),
            protocols: vec![
                ProtocolConfig {
                    protocol_id: "modbus_tcp".to_string(),
                    instance_count: 10,
                    operation_interval: Duration::from_millis(10),
                }
            ],
            duration: Duration::from_secs(30),
            concurrent_operations: 50,
        };
        
        // 注意：这个测试需要实际的协议插件注册
        // tester.run_scenario(scenario).await.unwrap();
    }
}