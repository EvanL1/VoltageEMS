//! 压力测试运行器
//! 
//! 提供简单的入口来运行各种压力测试

mod stress_tests;

use std::env;
use stress_tests::{run_300k_comsrv_pressure_test, run_modbus_protocol_test, run_comsrv_integration_test};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 设置日志
    env_logger::init();
    
    println!("🚀 启动comsrv压力测试系统");
    
    // 检查命令行参数
    let args: Vec<String> = env::args().collect();
    
    match args.get(1).map(|s| s.as_str()) {
        Some("comsrv") | None => {
            println!("运行comsrv 300K点位压力测试...");
            run_300k_comsrv_pressure_test().await?;
        },
        Some("modbus") => {
            println!("运行Modbus协议报文测试...");
            run_modbus_protocol_test().await?;
        },
        Some("protocol") => {
            println!("运行Modbus协议报文测试...");
            run_modbus_protocol_test().await?;
        },
        Some("integration") => {
            println!("运行comsrv集成测试...");
            run_comsrv_integration_test().await?;
        },
        Some(test_type) => {
            eprintln!("未知的测试类型: {}", test_type);
            eprintln!("可用的测试类型:");
            eprintln!("  comsrv      - 运行comsrv多通道压力测试 (默认)");
            eprintln!("  modbus      - 运行Modbus协议报文测试");
            eprintln!("  protocol    - 运行Modbus协议报文测试");
            eprintln!("  integration - 运行comsrv集成测试");
            std::process::exit(1);
        }
    }
    
    println!("✅ 测试完成");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_simple_pressure_test() {
        let config = TestConfig {
            channels: 2,
            points_per_channel: 10,
            duration_secs: 30,
            base_port: 5030,
            redis_batch_size: 5,
        };
        
        let mut test = SimplePressureTest::new(config);
        
        // 运行诊断
        let diagnosis = test.diagnose();
        diagnosis.print_summary();
        
        // 如果Redis可用，尝试运行简短测试
        if diagnosis.redis_connected {
            println!("🧪 运行简短测试...");
            
            if let Err(e) = test.start_simulators() {
                println!("⚠️  模拟器启动失败: {}", e);
                return;
            }
            
            // 运行10秒测试
            let short_config = TestConfig {
                duration_secs: 10,
                ..test.config
            };
            test.config = short_config;
            
            if let Err(e) = test.run_pressure_test().await {
                println!("❌ 测试失败: {}", e);
            } else {
                println!("✅ 测试完成");
            }
        }
    }
    
    #[test]
    fn test_redis_connection() {
        match check_redis_connection() {
            Ok(_) => println!("✅ Redis连接测试通过"),
            Err(e) => println!("❌ Redis连接测试失败: {}", e),
        }
    }
    
    #[test]
    fn test_port_availability() {
        let port = 5040;
        if check_port_available(port) {
            println!("✅ 端口 {} 可用", port);
        } else {
            println!("❌ 端口 {} 被占用", port);
        }
    }
} 