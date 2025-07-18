//! 综合测试套件执行器
//!
//! 这个模块提供了一个统一的测试执行框架，用于运行各种类型的测试

use std::time::{Duration, Instant};
use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use tokio::time::sleep;
use anyhow::Result;
use chrono::{DateTime, Utc};

/// 测试结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestResult {
    pub test_name: String,
    pub test_type: TestType,
    pub status: TestStatus,
    pub duration: Duration,
    pub error_message: Option<String>,
    pub metrics: HashMap<String, f64>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TestType {
    Unit,
    Integration,
    Performance,
    EndToEnd,
    Security,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TestStatus {
    Passed,
    Failed,
    Skipped,
    Timeout,
}

/// 测试套件配置
#[derive(Debug, Clone)]
pub struct TestSuiteConfig {
    pub enable_unit_tests: bool,
    pub enable_integration_tests: bool,
    pub enable_performance_tests: bool,
    pub enable_e2e_tests: bool,
    pub enable_security_tests: bool,
    pub parallel_execution: bool,
    pub max_test_duration: Duration,
    pub test_environment_url: String,
    pub redis_url: String,
}

impl Default for TestSuiteConfig {
    fn default() -> Self {
        Self {
            enable_unit_tests: true,
            enable_integration_tests: true,
            enable_performance_tests: false,
            enable_e2e_tests: false,
            enable_security_tests: false,
            parallel_execution: true,
            max_test_duration: Duration::from_secs(300),
            test_environment_url: "http://localhost:8080".to_string(),
            redis_url: "redis://localhost:6379".to_string(),
        }
    }
}

/// 测试套件执行器
pub struct TestSuiteRunner {
    config: TestSuiteConfig,
    results: Vec<TestResult>,
}

impl TestSuiteRunner {
    pub fn new(config: TestSuiteConfig) -> Self {
        Self {
            config,
            results: Vec::new(),
        }
    }

    /// 运行所有测试
    pub async fn run_all_tests(&mut self) -> Result<TestSuiteReport> {
        println!("🚀 启动综合测试套件");
        println!("配置: {:#?}", self.config);

        let start_time = Instant::now();
        
        // 按顺序运行不同类型的测试
        if self.config.enable_unit_tests {
            self.run_unit_tests().await?;
        }

        if self.config.enable_integration_tests {
            self.run_integration_tests().await?;
        }

        if self.config.enable_performance_tests {
            self.run_performance_tests().await?;
        }

        if self.config.enable_e2e_tests {
            self.run_e2e_tests().await?;
        }

        if self.config.enable_security_tests {
            self.run_security_tests().await?;
        }

        let total_duration = start_time.elapsed();
        let report = self.generate_report(total_duration);

        println!("✅ 测试套件执行完成");
        self.print_summary(&report);

        Ok(report)
    }

    /// 运行单元测试
    async fn run_unit_tests(&mut self) -> Result<()> {
        println!("\n📝 运行单元测试...");

        let unit_tests = vec![
            ("test_modbus_tcp_plugin", Box::new(|| Box::pin(test_modbus_tcp_plugin())) as Box<dyn Fn() -> _>),
            ("test_modbus_rtu_plugin", Box::new(|| Box::pin(test_modbus_rtu_plugin()))),
            ("test_iec60870_plugin", Box::new(|| Box::pin(test_iec60870_plugin()))),
            ("test_redis_storage", Box::new(|| Box::pin(test_redis_storage()))),
            ("test_config_parsing", Box::new(|| Box::pin(test_config_parsing()))),
            ("test_point_mapping", Box::new(|| Box::pin(test_point_mapping()))),
            ("test_error_handling", Box::new(|| Box::pin(test_error_handling()))),
        ];

        for (test_name, test_fn) in unit_tests {
            let result = self.run_single_test(test_name, TestType::Unit, test_fn()).await;
            self.results.push(result);
        }

        Ok(())
    }

    /// 运行集成测试
    async fn run_integration_tests(&mut self) -> Result<()> {
        println!("\n🔗 运行集成测试...");

        let integration_tests = vec![
            ("test_modbus_tcp_integration", Box::new(|| Box::pin(test_modbus_tcp_integration())) as Box<dyn Fn() -> _>),
            ("test_modbus_rtu_integration", Box::new(|| Box::pin(test_modbus_rtu_integration()))),
            ("test_iec60870_integration", Box::new(|| Box::pin(test_iec60870_integration()))),
            ("test_redis_pubsub_integration", Box::new(|| Box::pin(test_redis_pubsub_integration()))),
            ("test_multi_protocol_integration", Box::new(|| Box::pin(test_multi_protocol_integration()))),
            ("test_command_subscription", Box::new(|| Box::pin(test_command_subscription()))),
            ("test_data_flow_integration", Box::new(|| Box::pin(test_data_flow_integration()))),
        ];

        for (test_name, test_fn) in integration_tests {
            let result = self.run_single_test(test_name, TestType::Integration, test_fn()).await;
            self.results.push(result);
        }

        Ok(())
    }

    /// 运行性能测试
    async fn run_performance_tests(&mut self) -> Result<()> {
        println!("\n⚡ 运行性能测试...");

        let performance_tests = vec![
            ("test_concurrent_connections", Box::new(|| Box::pin(test_concurrent_connections())) as Box<dyn Fn() -> _>),
            ("test_data_throughput", Box::new(|| Box::pin(test_data_throughput()))),
            ("test_memory_usage", Box::new(|| Box::pin(test_memory_usage()))),
            ("test_cpu_usage", Box::new(|| Box::pin(test_cpu_usage()))),
            ("test_latency_measurement", Box::new(|| Box::pin(test_latency_measurement()))),
            ("test_stress_testing", Box::new(|| Box::pin(test_stress_testing()))),
        ];

        for (test_name, test_fn) in performance_tests {
            let result = self.run_single_test(test_name, TestType::Performance, test_fn()).await;
            self.results.push(result);
        }

        Ok(())
    }

    /// 运行端到端测试
    async fn run_e2e_tests(&mut self) -> Result<()> {
        println!("\n🎯 运行端到端测试...");

        let e2e_tests = vec![
            ("test_complete_data_flow", Box::new(|| Box::pin(test_complete_data_flow())) as Box<dyn Fn() -> _>),
            ("test_fault_recovery", Box::new(|| Box::pin(test_fault_recovery()))),
            ("test_long_term_stability", Box::new(|| Box::pin(test_long_term_stability()))),
            ("test_system_restart", Box::new(|| Box::pin(test_system_restart()))),
            ("test_network_interruption", Box::new(|| Box::pin(test_network_interruption()))),
        ];

        for (test_name, test_fn) in e2e_tests {
            let result = self.run_single_test(test_name, TestType::EndToEnd, test_fn()).await;
            self.results.push(result);
        }

        Ok(())
    }

    /// 运行安全测试
    async fn run_security_tests(&mut self) -> Result<()> {
        println!("\n🔒 运行安全测试...");

        let security_tests = vec![
            ("test_authentication", Box::new(|| Box::pin(test_authentication())) as Box<dyn Fn() -> _>),
            ("test_authorization", Box::new(|| Box::pin(test_authorization()))),
            ("test_data_encryption", Box::new(|| Box::pin(test_data_encryption()))),
            ("test_input_validation", Box::new(|| Box::pin(test_input_validation()))),
            ("test_sql_injection", Box::new(|| Box::pin(test_sql_injection()))),
        ];

        for (test_name, test_fn) in security_tests {
            let result = self.run_single_test(test_name, TestType::Security, test_fn()).await;
            self.results.push(result);
        }

        Ok(())
    }

    /// 运行单个测试
    async fn run_single_test<F, Fut>(&self, test_name: &str, test_type: TestType, test_fn: F) -> TestResult
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = Result<HashMap<String, f64>>>,
    {
        println!("  执行测试: {}", test_name);
        
        let start_time = Instant::now();
        
        let result = tokio::time::timeout(self.config.max_test_duration, test_fn()).await;

        let duration = start_time.elapsed();

        match result {
            Ok(Ok(metrics)) => {
                println!("    ✅ 通过 ({:.2}s)", duration.as_secs_f64());
                TestResult {
                    test_name: test_name.to_string(),
                    test_type,
                    status: TestStatus::Passed,
                    duration,
                    error_message: None,
                    metrics,
                    timestamp: Utc::now(),
                }
            }
            Ok(Err(e)) => {
                println!("    ❌ 失败: {}", e);
                TestResult {
                    test_name: test_name.to_string(),
                    test_type,
                    status: TestStatus::Failed,
                    duration,
                    error_message: Some(e.to_string()),
                    metrics: HashMap::new(),
                    timestamp: Utc::now(),
                }
            }
            Err(_) => {
                println!("    ⏰ 超时");
                TestResult {
                    test_name: test_name.to_string(),
                    test_type,
                    status: TestStatus::Timeout,
                    duration,
                    error_message: Some("Test timeout".to_string()),
                    metrics: HashMap::new(),
                    timestamp: Utc::now(),
                }
            }
        }
    }

    /// 生成测试报告
    fn generate_report(&self, total_duration: Duration) -> TestSuiteReport {
        let total_tests = self.results.len();
        let passed_tests = self.results.iter().filter(|r| matches!(r.status, TestStatus::Passed)).count();
        let failed_tests = self.results.iter().filter(|r| matches!(r.status, TestStatus::Failed)).count();
        let timeout_tests = self.results.iter().filter(|r| matches!(r.status, TestStatus::Timeout)).count();
        let skipped_tests = self.results.iter().filter(|r| matches!(r.status, TestStatus::Skipped)).count();

        let success_rate = if total_tests > 0 {
            passed_tests as f64 / total_tests as f64
        } else {
            0.0
        };

        let by_type = self.group_results_by_type();

        TestSuiteReport {
            total_tests,
            passed_tests,
            failed_tests,
            timeout_tests,
            skipped_tests,
            success_rate,
            total_duration,
            by_type,
            detailed_results: self.results.clone(),
            timestamp: Utc::now(),
        }
    }

    /// 按类型分组结果
    fn group_results_by_type(&self) -> HashMap<TestType, TestTypeReport> {
        let mut by_type = HashMap::new();

        for test_type in [TestType::Unit, TestType::Integration, TestType::Performance, TestType::EndToEnd, TestType::Security] {
            let type_results: Vec<_> = self.results.iter().filter(|r| matches!(r.test_type, test_type)).collect();
            
            if !type_results.is_empty() {
                let total = type_results.len();
                let passed = type_results.iter().filter(|r| matches!(r.status, TestStatus::Passed)).count();
                let failed = type_results.iter().filter(|r| matches!(r.status, TestStatus::Failed)).count();
                let timeout = type_results.iter().filter(|r| matches!(r.status, TestStatus::Timeout)).count();
                let skipped = type_results.iter().filter(|r| matches!(r.status, TestStatus::Skipped)).count();
                
                let success_rate = if total > 0 { passed as f64 / total as f64 } else { 0.0 };
                
                by_type.insert(test_type.clone(), TestTypeReport {
                    total,
                    passed,
                    failed,
                    timeout,
                    skipped,
                    success_rate,
                });
            }
        }

        by_type
    }

    /// 打印测试总结
    fn print_summary(&self, report: &TestSuiteReport) {
        println!("\n📊 测试总结");
        println!("={}", "=".repeat(60));
        println!("总测试数: {}", report.total_tests);
        println!("通过: {} ({:.1}%)", report.passed_tests, report.success_rate * 100.0);
        println!("失败: {}", report.failed_tests);
        println!("超时: {}", report.timeout_tests);
        println!("跳过: {}", report.skipped_tests);
        println!("总耗时: {:.2}s", report.total_duration.as_secs_f64());
        
        if !report.by_type.is_empty() {
            println!("\n按类型统计:");
            for (test_type, type_report) in &report.by_type {
                println!("  {:?}: {}/{} ({:.1}%)", 
                    test_type, 
                    type_report.passed, 
                    type_report.total, 
                    type_report.success_rate * 100.0
                );
            }
        }
        
        if report.failed_tests > 0 {
            println!("\n❌ 失败的测试:");
            for result in &report.detailed_results {
                if matches!(result.status, TestStatus::Failed) {
                    println!("  - {}: {}", result.test_name, result.error_message.as_deref().unwrap_or("Unknown error"));
                }
            }
        }
        
        println!("={}", "=".repeat(60));
    }
}

/// 测试套件报告
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestSuiteReport {
    pub total_tests: usize,
    pub passed_tests: usize,
    pub failed_tests: usize,
    pub timeout_tests: usize,
    pub skipped_tests: usize,
    pub success_rate: f64,
    pub total_duration: Duration,
    pub by_type: HashMap<TestType, TestTypeReport>,
    pub detailed_results: Vec<TestResult>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestTypeReport {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub timeout: usize,
    pub skipped: usize,
    pub success_rate: f64,
}

// 测试函数实现（示例）
async fn test_modbus_tcp_plugin() -> Result<HashMap<String, f64>> {
    // 模拟Modbus TCP插件测试
    sleep(Duration::from_millis(100)).await;
    
    let mut metrics = HashMap::new();
    metrics.insert("response_time_ms".to_string(), 50.0);
    metrics.insert("success_rate".to_string(), 1.0);
    
    Ok(metrics)
}

async fn test_modbus_rtu_plugin() -> Result<HashMap<String, f64>> {
    // 模拟Modbus RTU插件测试
    sleep(Duration::from_millis(150)).await;
    
    let mut metrics = HashMap::new();
    metrics.insert("response_time_ms".to_string(), 75.0);
    metrics.insert("success_rate".to_string(), 0.98);
    
    Ok(metrics)
}

async fn test_iec60870_plugin() -> Result<HashMap<String, f64>> {
    // 模拟IEC60870插件测试
    sleep(Duration::from_millis(200)).await;
    
    let mut metrics = HashMap::new();
    metrics.insert("response_time_ms".to_string(), 100.0);
    metrics.insert("success_rate".to_string(), 0.95);
    
    Ok(metrics)
}

async fn test_redis_storage() -> Result<HashMap<String, f64>> {
    // 模拟Redis存储测试
    sleep(Duration::from_millis(50)).await;
    
    let mut metrics = HashMap::new();
    metrics.insert("write_latency_ms".to_string(), 5.0);
    metrics.insert("read_latency_ms".to_string(), 3.0);
    metrics.insert("throughput_ops_per_sec".to_string(), 10000.0);
    
    Ok(metrics)
}

async fn test_config_parsing() -> Result<HashMap<String, f64>> {
    // 模拟配置解析测试
    sleep(Duration::from_millis(10)).await;
    
    let mut metrics = HashMap::new();
    metrics.insert("parse_time_ms".to_string(), 1.0);
    
    Ok(metrics)
}

async fn test_point_mapping() -> Result<HashMap<String, f64>> {
    // 模拟点位映射测试
    sleep(Duration::from_millis(20)).await;
    
    let mut metrics = HashMap::new();
    metrics.insert("mapping_accuracy".to_string(), 1.0);
    
    Ok(metrics)
}

async fn test_error_handling() -> Result<HashMap<String, f64>> {
    // 模拟错误处理测试
    sleep(Duration::from_millis(30)).await;
    
    let mut metrics = HashMap::new();
    metrics.insert("recovery_time_ms".to_string(), 500.0);
    
    Ok(metrics)
}

async fn test_modbus_tcp_integration() -> Result<HashMap<String, f64>> {
    // 模拟Modbus TCP集成测试
    sleep(Duration::from_millis(500)).await;
    
    let mut metrics = HashMap::new();
    metrics.insert("end_to_end_latency_ms".to_string(), 200.0);
    metrics.insert("data_accuracy".to_string(), 0.999);
    
    Ok(metrics)
}

async fn test_modbus_rtu_integration() -> Result<HashMap<String, f64>> {
    // 模拟Modbus RTU集成测试
    sleep(Duration::from_millis(600)).await;
    
    let mut metrics = HashMap::new();
    metrics.insert("end_to_end_latency_ms".to_string(), 250.0);
    metrics.insert("data_accuracy".to_string(), 0.997);
    
    Ok(metrics)
}

async fn test_iec60870_integration() -> Result<HashMap<String, f64>> {
    // 模拟IEC60870集成测试
    sleep(Duration::from_millis(700)).await;
    
    let mut metrics = HashMap::new();
    metrics.insert("end_to_end_latency_ms".to_string(), 300.0);
    metrics.insert("data_accuracy".to_string(), 0.995);
    
    Ok(metrics)
}

async fn test_redis_pubsub_integration() -> Result<HashMap<String, f64>> {
    // 模拟Redis发布订阅集成测试
    sleep(Duration::from_millis(300)).await;
    
    let mut metrics = HashMap::new();
    metrics.insert("message_delivery_rate".to_string(), 0.999);
    metrics.insert("latency_ms".to_string(), 10.0);
    
    Ok(metrics)
}

async fn test_multi_protocol_integration() -> Result<HashMap<String, f64>> {
    // 模拟多协议集成测试
    sleep(Duration::from_millis(1000)).await;
    
    let mut metrics = HashMap::new();
    metrics.insert("concurrent_protocols".to_string(), 3.0);
    metrics.insert("overall_success_rate".to_string(), 0.98);
    
    Ok(metrics)
}

async fn test_command_subscription() -> Result<HashMap<String, f64>> {
    // 模拟命令订阅测试
    sleep(Duration::from_millis(400)).await;
    
    let mut metrics = HashMap::new();
    metrics.insert("command_response_time_ms".to_string(), 100.0);
    metrics.insert("command_success_rate".to_string(), 0.99);
    
    Ok(metrics)
}

async fn test_data_flow_integration() -> Result<HashMap<String, f64>> {
    // 模拟数据流集成测试
    sleep(Duration::from_millis(800)).await;
    
    let mut metrics = HashMap::new();
    metrics.insert("data_throughput_points_per_sec".to_string(), 1000.0);
    metrics.insert("data_loss_rate".to_string(), 0.001);
    
    Ok(metrics)
}

async fn test_concurrent_connections() -> Result<HashMap<String, f64>> {
    // 模拟并发连接测试
    sleep(Duration::from_secs(2)).await;
    
    let mut metrics = HashMap::new();
    metrics.insert("max_concurrent_connections".to_string(), 100.0);
    metrics.insert("connection_success_rate".to_string(), 0.95);
    
    Ok(metrics)
}

async fn test_data_throughput() -> Result<HashMap<String, f64>> {
    // 模拟数据吞吐量测试
    sleep(Duration::from_secs(3)).await;
    
    let mut metrics = HashMap::new();
    metrics.insert("throughput_points_per_sec".to_string(), 5000.0);
    metrics.insert("peak_throughput".to_string(), 8000.0);
    
    Ok(metrics)
}

async fn test_memory_usage() -> Result<HashMap<String, f64>> {
    // 模拟内存使用测试
    sleep(Duration::from_secs(2)).await;
    
    let mut metrics = HashMap::new();
    metrics.insert("memory_usage_mb".to_string(), 128.0);
    metrics.insert("memory_leak_rate".to_string(), 0.0);
    
    Ok(metrics)
}

async fn test_cpu_usage() -> Result<HashMap<String, f64>> {
    // 模拟CPU使用测试
    sleep(Duration::from_secs(2)).await;
    
    let mut metrics = HashMap::new();
    metrics.insert("cpu_usage_percent".to_string(), 45.0);
    metrics.insert("cpu_efficiency".to_string(), 0.85);
    
    Ok(metrics)
}

async fn test_latency_measurement() -> Result<HashMap<String, f64>> {
    // 模拟延迟测量测试
    sleep(Duration::from_secs(1)).await;
    
    let mut metrics = HashMap::new();
    metrics.insert("p50_latency_ms".to_string(), 50.0);
    metrics.insert("p95_latency_ms".to_string(), 100.0);
    metrics.insert("p99_latency_ms".to_string(), 200.0);
    
    Ok(metrics)
}

async fn test_stress_testing() -> Result<HashMap<String, f64>> {
    // 模拟压力测试
    sleep(Duration::from_secs(5)).await;
    
    let mut metrics = HashMap::new();
    metrics.insert("stress_test_duration_sec".to_string(), 300.0);
    metrics.insert("error_rate_under_stress".to_string(), 0.02);
    
    Ok(metrics)
}

async fn test_complete_data_flow() -> Result<HashMap<String, f64>> {
    // 模拟完整数据流测试
    sleep(Duration::from_secs(3)).await;
    
    let mut metrics = HashMap::new();
    metrics.insert("end_to_end_success_rate".to_string(), 0.998);
    metrics.insert("total_data_points".to_string(), 10000.0);
    
    Ok(metrics)
}

async fn test_fault_recovery() -> Result<HashMap<String, f64>> {
    // 模拟故障恢复测试
    sleep(Duration::from_secs(4)).await;
    
    let mut metrics = HashMap::new();
    metrics.insert("recovery_time_sec".to_string(), 30.0);
    metrics.insert("data_loss_during_recovery".to_string(), 0.005);
    
    Ok(metrics)
}

async fn test_long_term_stability() -> Result<HashMap<String, f64>> {
    // 模拟长期稳定性测试（缩短时间）
    sleep(Duration::from_secs(10)).await;
    
    let mut metrics = HashMap::new();
    metrics.insert("uptime_hours".to_string(), 24.0);
    metrics.insert("stability_score".to_string(), 0.999);
    
    Ok(metrics)
}

async fn test_system_restart() -> Result<HashMap<String, f64>> {
    // 模拟系统重启测试
    sleep(Duration::from_secs(2)).await;
    
    let mut metrics = HashMap::new();
    metrics.insert("restart_time_sec".to_string(), 15.0);
    metrics.insert("data_persistence_rate".to_string(), 1.0);
    
    Ok(metrics)
}

async fn test_network_interruption() -> Result<HashMap<String, f64>> {
    // 模拟网络中断测试
    sleep(Duration::from_secs(3)).await;
    
    let mut metrics = HashMap::new();
    metrics.insert("reconnection_time_sec".to_string(), 10.0);
    metrics.insert("data_buffering_capacity".to_string(), 1000.0);
    
    Ok(metrics)
}

async fn test_authentication() -> Result<HashMap<String, f64>> {
    // 模拟认证测试
    sleep(Duration::from_millis(100)).await;
    
    let mut metrics = HashMap::new();
    metrics.insert("auth_success_rate".to_string(), 1.0);
    metrics.insert("auth_time_ms".to_string(), 50.0);
    
    Ok(metrics)
}

async fn test_authorization() -> Result<HashMap<String, f64>> {
    // 模拟授权测试
    sleep(Duration::from_millis(80)).await;
    
    let mut metrics = HashMap::new();
    metrics.insert("authz_success_rate".to_string(), 1.0);
    metrics.insert("authz_time_ms".to_string(), 20.0);
    
    Ok(metrics)
}

async fn test_data_encryption() -> Result<HashMap<String, f64>> {
    // 模拟数据加密测试
    sleep(Duration::from_millis(200)).await;
    
    let mut metrics = HashMap::new();
    metrics.insert("encryption_overhead_ms".to_string(), 5.0);
    metrics.insert("encryption_success_rate".to_string(), 1.0);
    
    Ok(metrics)
}

async fn test_input_validation() -> Result<HashMap<String, f64>> {
    // 模拟输入验证测试
    sleep(Duration::from_millis(50)).await;
    
    let mut metrics = HashMap::new();
    metrics.insert("validation_accuracy".to_string(), 1.0);
    metrics.insert("validation_time_ms".to_string(), 1.0);
    
    Ok(metrics)
}

async fn test_sql_injection() -> Result<HashMap<String, f64>> {
    // 模拟SQL注入测试
    sleep(Duration::from_millis(100)).await;
    
    let mut metrics = HashMap::new();
    metrics.insert("injection_prevention_rate".to_string(), 1.0);
    
    Ok(metrics)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_suite_runner_basic() {
        let config = TestSuiteConfig {
            enable_performance_tests: false,
            enable_e2e_tests: false,
            enable_security_tests: false,
            max_test_duration: Duration::from_secs(10),
            ..Default::default()
        };

        let mut runner = TestSuiteRunner::new(config);
        let report = runner.run_all_tests().await.unwrap();

        assert!(report.total_tests > 0);
        assert!(report.success_rate > 0.0);
    }

    #[tokio::test]
    async fn test_suite_runner_with_performance() {
        let config = TestSuiteConfig {
            enable_unit_tests: false,
            enable_integration_tests: false,
            enable_performance_tests: true,
            enable_e2e_tests: false,
            enable_security_tests: false,
            max_test_duration: Duration::from_secs(30),
            ..Default::default()
        };

        let mut runner = TestSuiteRunner::new(config);
        let report = runner.run_all_tests().await.unwrap();

        assert!(report.total_tests > 0);
        assert!(report.by_type.contains_key(&TestType::Performance));
    }
}