//! 压力测试工具函数

use std::time::{Duration, Instant};
use redis::Client as RedisClient;
use serde_json::json;
use std::collections::HashMap;

/// 测试配置
#[derive(Debug, Clone)]
pub struct TestConfig {
    pub channels: usize,
    pub points_per_channel: usize,
    pub duration_secs: u64,
    pub base_port: u16,
    pub redis_batch_size: usize,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            channels: 5,
            points_per_channel: 100,
            duration_secs: 60,
            base_port: 5020,
            redis_batch_size: 10,
        }
    }
}

/// 性能统计
#[derive(Debug, Default)]
pub struct PerformanceStats {
    pub total_reads: u64,
    pub successful_reads: u64,
    pub failed_reads: u64,
    pub total_points: u64,
    pub redis_writes: u64,
    pub redis_errors: u64,
    pub start_time: Option<Instant>,
    pub last_update: Option<Instant>,
}

impl PerformanceStats {
    pub fn new() -> Self {
        Self {
            start_time: Some(Instant::now()),
            ..Default::default()
        }
    }

    pub fn success_rate(&self) -> f64 {
        if self.total_reads == 0 {
            0.0
        } else {
            self.successful_reads as f64 / self.total_reads as f64
        }
    }

    pub fn operations_per_second(&self) -> f64 {
        if let Some(start) = self.start_time {
            let elapsed = start.elapsed().as_secs_f64();
            if elapsed > 0.0 {
                self.total_reads as f64 / elapsed
            } else {
                0.0
            }
        } else {
            0.0
        }
    }
}

/// 检查Redis连接
pub fn check_redis_connection() -> Result<RedisClient, Box<dyn std::error::Error>> {
    let redis_url = std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
    let client = RedisClient::open(redis_url)?;
    
    // 测试连接
    let mut conn = client.get_connection()?;
    let _: String = redis::cmd("PING").query(&mut conn)?;
    
    Ok(client)
}

/// 检查端口是否可用
pub fn check_port_available(port: u16) -> bool {
    std::net::TcpListener::bind(format!("127.0.0.1:{}", port)).is_ok()
}

/// 等待端口开放
pub fn wait_for_port(port: u16, timeout: Duration) -> bool {
    let start = Instant::now();
    
    while start.elapsed() < timeout {
        if std::net::TcpStream::connect_timeout(
            &format!("127.0.0.1:{}", port).parse().unwrap(),
            Duration::from_millis(100)
        ).is_ok() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    
    false
}

/// 生成模拟数据点
pub fn generate_test_points(count: usize, channel_id: usize) -> Vec<TestDataPoint> {
    (0..count).map(|i| TestDataPoint {
        name: format!("point_{}_{}", channel_id, i),
        address: i as u16,
        data_type: if i % 2 == 0 { "uint16" } else { "float32" }.to_string(),
        unit: if i % 3 == 0 { Some("°C".to_string()) } else { None },
        description: Some(format!("Test point {} for channel {}", i, channel_id)),
    }).collect()
}

/// 测试数据点
#[derive(Debug, Clone)]
pub struct TestDataPoint {
    pub name: String,
    pub address: u16,
    pub data_type: String,
    pub unit: Option<String>,
    pub description: Option<String>,
}

/// 创建Modbus模拟器脚本
pub fn create_simple_modbus_simulator(output_path: &str, port: u16) -> std::io::Result<()> {
    let script_content = format!(r#"#!/usr/bin/env python3
import socket
import struct
import threading
import time
import random
import math

class SimpleModbusSimulator:
    def __init__(self, registers=1000):
        self.registers = [0] * registers
        self.running = True
        self.request_count = 0
        
        # 启动数据生成线程
        threading.Thread(target=self._generate_data, daemon=True).start()
    
    def _generate_data(self):
        counter = 0
        while self.running:
            for i in range(min(50, len(self.registers))):
                addr = (counter + i) % len(self.registers)
                value = addr * 10 + int(50 * math.sin(counter * 0.1 + addr * 0.01))
                self.registers[addr] = max(0, min(65535, value + random.randint(-10, 10)))
            counter += 1
            time.sleep(0.2)
    
    def stop(self):
        self.running = False

class ModbusHandler:
    def __init__(self, connection, simulator):
        self.connection = connection
        self.simulator = simulator
    
    def handle(self):
        try:
            while True:
                data = self.connection.recv(1024)
                if not data:
                    break
                
                if len(data) < 8:
                    continue
                
                # 解析Modbus TCP请求
                transaction_id = struct.unpack('>H', data[0:2])[0]
                function_code = data[7]
                
                self.simulator.request_count += 1
                
                if function_code == 0x03:  # 读保持寄存器
                    start_addr = struct.unpack('>H', data[8:10])[0]
                    count = struct.unpack('>H', data[10:12])[0]
                    count = min(count, 125)
                    
                    if start_addr + count <= len(self.simulator.registers):
                        response_data = bytearray([data[6], function_code, count * 2])
                        for i in range(start_addr, start_addr + count):
                            response_data.extend(struct.pack('>H', self.simulator.registers[i]))
                        
                        response = struct.pack('>HHH', transaction_id, 0, len(response_data)) + response_data
                        self.connection.send(response)
        except:
            pass
        finally:
            self.connection.close()

def main():
    port = {port}
    print(f"启动Modbus模拟器，端口: {{port}}")
    
    simulator = SimpleModbusSimulator()
    server = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    server.bind(('0.0.0.0', port))
    server.listen(10)
    
    try:
        while True:
            conn, addr = server.accept()
            threading.Thread(target=ModbusHandler(conn, simulator).handle).start()
    except KeyboardInterrupt:
        print("停止模拟器")
    finally:
        simulator.stop()
        server.close()

if __name__ == "__main__":
    main()
"#, port = port);

    std::fs::write(output_path, script_content)?;
    
    // 设置执行权限
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = std::fs::metadata(output_path)?;
        let mut permissions = metadata.permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(output_path, permissions)?;
    }
    
    Ok(())
}

/// 清理测试环境
pub fn cleanup_test_environment() {
    // 清理Redis数据
    if let Ok(client) = check_redis_connection() {
        if let Ok(mut conn) = client.get_connection() {
            let _: Result<(), redis::RedisError> = redis::cmd("FLUSHDB").query(&mut conn);
        }
    }
    
    // 清理临时文件
    let _ = std::fs::remove_dir_all("/tmp/comsrv_test");
}

/// 设置测试环境
pub fn setup_test_environment() -> Result<(), Box<dyn std::error::Error>> {
    // 创建临时目录
    std::fs::create_dir_all("/tmp/comsrv_test")?;
    
    // 检查Redis连接
    check_redis_connection()?;
    
    println!("✅ 测试环境准备完成");
    
    Ok(())
}

/// 创建大规模测试配置
pub fn create_large_scale_config(point_count: usize, port: u16) -> HashMap<u16, serde_json::Value> {
    let mut data_points = HashMap::new();
    
    for i in 0..point_count {
        let address = i as u16;
        let data = json!({
            "address": address,
            "value": rand::random::<u16>(),
            "type": match i % 4 {
                0 => "holding_register",
                1 => "input_register", 
                2 => "coil",
                _ => "discrete_input"
            },
            "data_type": match i % 3 {
                0 => "uint16",
                1 => "int16",
                _ => "float32"
            },
            "port": port,
            "timestamp": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
        });
        data_points.insert(address, data);
    }
    
    data_points
} 