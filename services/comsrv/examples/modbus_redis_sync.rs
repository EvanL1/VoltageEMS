/// ModbusClient with Redis Integration Example
/// 
/// 本示例展示如何使用ModbusClient的Redis集成功能，
/// 实现内存数据与Redis的实时同步
use std::time::Duration;
use tokio::time::sleep;
use comsrv::core::protocols::modbus::client::{ModbusClient, ModbusClientConfig, ModbusCommunicationMode};
use comsrv::core::protocols::modbus::common::{ModbusRegisterMapping, ModbusRegisterType, ModbusDataType};
use comsrv::core::config::config_manager::{RedisConfig, RedisConnectionType};
use comsrv::utils::logger::ChannelLogger;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logger
    env_logger::init();
    
    println!("=== ModbusClient Redis Integration Example ===\n");

    // 1. Configure Modbus client
    let mut modbus_config = ModbusClientConfig {
        mode: ModbusCommunicationMode::Tcp,
        slave_id: 1,
        timeout: Duration::from_secs(5),
        max_retries: 3,
        poll_interval: Duration::from_secs(2),
        host: Some("127.0.0.1".to_string()),
        tcp_port: Some(502),
        point_mappings: vec![
            ModbusRegisterMapping {
                name: "temperature".to_string(),
                display_name: Some("室温".to_string()),
                register_type: ModbusRegisterType::HoldingRegister,
                address: 40001,
                data_type: ModbusDataType::Float32,
                scale: 0.1,
                offset: 0.0,
                unit: Some("°C".to_string()),
                description: Some("环境温度".to_string()),
                ..Default::default()
            },
            ModbusRegisterMapping {
                name: "pressure".to_string(),
                display_name: Some("压力".to_string()),
                register_type: ModbusRegisterType::HoldingRegister,
                address: 40003,
                data_type: ModbusDataType::Float32,
                scale: 0.01,
                offset: 0.0,
                unit: Some("Pa".to_string()),
                description: Some("系统压力".to_string()),
                ..Default::default()
            },
            ModbusRegisterMapping {
                name: "status".to_string(),
                display_name: Some("运行状态".to_string()),
                register_type: ModbusRegisterType::Coil,
                address: 1,
                data_type: ModbusDataType::Bool,
                scale: 1.0,
                offset: 0.0,
                unit: None,
                description: Some("设备运行状态".to_string()),
                ..Default::default()
            },
        ],
        ..Default::default()
    };

    // 2. Configure Redis connection
    let redis_config = RedisConfig {
        enabled: true,
        connection_type: RedisConnectionType::Tcp,
        address: "redis://127.0.0.1:6379".to_string(),
        db: Some(0),
    };

    // 3. Create ModbusClient with Redis integration
    println!("📡 创建带Redis集成的ModbusClient...");
    let mut client = match ModbusClient::new_with_redis(
        modbus_config,
        ModbusCommunicationMode::Tcp,
        Some(&redis_config),
    ).await {
        Ok(client) => {
            println!("✅ ModbusClient创建成功，Redis集成已启用");
            client
        }
        Err(e) => {
            println!("❌ ModbusClient创建失败: {}", e);
            println!("💡 提示：请确保Redis服务器正在运行 (redis-server)");
            return Ok(());
        }
    };

    // 4. Setup channel logger
    let logger = ChannelLogger::new("modbus_tcp_001".to_string());
    client.set_channel_logger(logger);

    // 5. Start the client and begin data polling
    println!("🚀 启动ModbusClient...");
    match client.start().await {
        Ok(_) => {
            println!("✅ ModbusClient启动成功，开始数据采集与Redis同步");
        }
        Err(e) => {
            println!("❌ ModbusClient启动失败: {}", e);
            println!("💡 提示：请确保Modbus服务器正在运行在 127.0.0.1:502");
            return Ok(());
        }
    }

    // 6. Monitor the system for a while
    println!("\n📊 监控系统运行状态...");
    println!("   - 内存中的数据会自动同步到Redis");
    println!("   - Redis键格式: modbus:modbus_tcp_1:{{point_name}}");
    println!("   - 数据过期时间: 1小时\n");

    for i in 1..=10 {
        sleep(Duration::from_secs(3)).await;
        
        // Get current statistics
        let stats = client.get_stats().await;
        let is_connected = client.is_connected().await;
        let connection_state = client.connection_state().await;
        
        println!("📈 状态报告 #{}", i);
        println!("   连接状态: {:?}", connection_state);
        println!("   总请求数: {}", stats.total_requests());
        println!("   成功请求: {}", stats.successful_requests());
        println!("   通信质量: {:.1}%", stats.communication_quality());
        println!("   平均响应时间: {:.1}ms", stats.avg_response_time_ms());
        
        if is_connected {
            println!("   🔄 数据正在实时同步到Redis...");
        } else {
            println!("   ⚠️  连接断开，正在尝试重连...");
        }
        
        // Get all points from memory cache
        let points = client.get_all_points().await;
        if !points.is_empty() {
            println!("   📋 内存中的数据点:");
            for point in &points {
                println!("      {} = {} {} (质量: {})", 
                    point.name, point.value, point.unit, point.quality);
            }
        }
        
        println!();
    }

    // 7. Stop the client
    println!("🛑 停止ModbusClient...");
    if let Err(e) = client.stop().await {
        println!("❌ 停止失败: {}", e);
    } else {
        println!("✅ ModbusClient已停止");
    }

    // 8. Final statistics
    let final_stats = client.get_stats().await;
    println!("\n📊 最终统计:");
    println!("   总请求数: {}", final_stats.total_requests());
    println!("   成功请求: {}", final_stats.successful_requests());
    println!("   失败请求: {}", final_stats.failed_requests());
    println!("   通信质量: {:.1}%", final_stats.communication_quality());
    println!("   重连次数: {}", final_stats.reconnect_attempts());

    println!("\n💡 Redis数据查看提示:");
    println!("   redis-cli");
    println!("   > KEYS modbus:*");
    println!("   > GET modbus:modbus_tcp_1:temperature");
    println!("   > GET modbus:modbus_tcp_1:pressure");
    println!("   > GET modbus:modbus_tcp_1:status");

    Ok(())
}

/// Helper function to demonstrate Redis data structure
#[allow(dead_code)]
async fn demonstrate_redis_data_structure() {
    println!("\n=== Redis数据结构说明 ===");
    println!("键格式: modbus:{{channel_id}}:{{point_name}}");
    println!("值格式: JSON格式的RealtimeValue");
    println!("示例:");
    println!("  键: modbus:modbus_tcp_1:temperature");
    println!("  值: {{");
    println!("    \"raw\": 234.5,");
    println!("    \"processed\": 23.45,");
    println!("    \"timestamp\": \"2023-12-01T10:30:15.123Z\"");
    println!("  }}");
    println!();
    println!("过期时间: 3600秒 (1小时)");
    println!("更新频率: 根据poll_interval配置 (示例中为2秒)");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_modbus_redis_config() {
        let redis_config = RedisConfig {
            enabled: true,
            connection_type: RedisConnectionType::Tcp,
            address: "redis://localhost:6379".to_string(),
            db: Some(0),
        };
        
        assert!(redis_config.enabled);
        assert!(redis_config.address.starts_with("redis://"));
    }

    #[test]
    fn test_modbus_mapping_config() {
        let mapping = ModbusRegisterMapping {
            name: "test_point".to_string(),
            register_type: ModbusRegisterType::HoldingRegister,
            address: 40001,
            data_type: ModbusDataType::Float32,
            scale: 0.1,
            offset: 0.0,
            ..Default::default()
        };
        
        assert_eq!(mapping.name, "test_point");
        assert_eq!(mapping.address, 40001);
        assert_eq!(mapping.scale, 0.1);
    }
} 