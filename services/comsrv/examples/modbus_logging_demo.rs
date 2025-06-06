/// Modbus 报文日志演示程序
/// 展示我们新增的详细报文解析和日志记录功能

use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, Level};

// 模拟我们的 RawModbusTcpClient 来展示报文日志功能
async fn demo_modbus_logging() {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .with_target(false)
        .init();

    info!("🚀 开始 Modbus 报文日志演示");
    
    // 模拟各种Modbus请求和响应的报文
    let demo_requests = vec![
        (
            "读取线圈 (FC=0x01)",
            vec![0x00, 0x01, 0x00, 0x00, 0x00, 0x06, 0x01, 0x01, 0x00, 0x00, 0x00, 0x10],
            vec![0x00, 0x01, 0x00, 0x00, 0x00, 0x05, 0x01, 0x01, 0x02, 0x20, 0x04]
        ),
        (
            "读取保持寄存器 (FC=0x03)",
            vec![0x00, 0x02, 0x00, 0x00, 0x00, 0x06, 0x01, 0x03, 0x00, 0x00, 0x00, 0x05],
            vec![0x00, 0x02, 0x00, 0x00, 0x00, 0x0D, 0x01, 0x03, 0x0A, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]
        ),
        (
            "写单个线圈 (FC=0x05)",
            vec![0x00, 0x03, 0x00, 0x00, 0x00, 0x06, 0x01, 0x05, 0x00, 0x0A, 0xFF, 0x00],
            vec![0x00, 0x03, 0x00, 0x00, 0x00, 0x06, 0x01, 0x05, 0x00, 0x0A, 0xFF, 0x00]
        ),
        (
            "写单个寄存器 (FC=0x06)",
            vec![0x00, 0x04, 0x00, 0x00, 0x00, 0x06, 0x01, 0x06, 0x00, 0x14, 0x03, 0xE8],
            vec![0x00, 0x04, 0x00, 0x00, 0x00, 0x06, 0x01, 0x06, 0x00, 0x14, 0x03, 0xE8]
        ),
        (
            "读取输入寄存器 (FC=0x04)",
            vec![0x00, 0x05, 0x00, 0x00, 0x00, 0x06, 0x01, 0x04, 0x00, 0x64, 0x00, 0x03],
            vec![0x00, 0x05, 0x00, 0x00, 0x00, 0x09, 0x01, 0x04, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]
        ),
        (
            "异常响应 - 非法地址 (FC=0x83)",
            vec![0x00, 0x06, 0x00, 0x00, 0x00, 0x06, 0x01, 0x03, 0xFF, 0xFF, 0x00, 0x01],
            vec![0x00, 0x06, 0x00, 0x00, 0x00, 0x03, 0x01, 0x83, 0x02]
        ),
    ];

    for (i, (description, request, response)) in demo_requests.iter().enumerate() {
        info!("\n📋 演示 {}: {}", i + 1, description);
        
        // 演示请求解析
        simulate_request_logging("通道1", request).await;
        
        sleep(Duration::from_millis(100)).await;
        
        // 演示响应解析
        simulate_response_logging("通道1", response).await;
        
        info!("✅ 演示 {} 完成\n{}", i + 1, "=".repeat(60));
        sleep(Duration::from_millis(500)).await;
    }
    
    info!("🎉 Modbus 报文日志演示完成！");
}

async fn simulate_request_logging(channel_id: &str, request: &[u8]) {
    // 记录发送的报文
    info!("📤 Channel {} - 发送 Modbus 请求: {} bytes", channel_id, request.len());
    info!("📤 Channel {} - 发送报文: {:02X?}", channel_id, request);
    
    // 解析并记录请求详情
    if request.len() >= 8 {
        let transaction_id = u16::from_be_bytes([request[0], request[1]]);
        let protocol_id = u16::from_be_bytes([request[2], request[3]]);
        let length = u16::from_be_bytes([request[4], request[5]]);
        let unit_id = request[6];
        let function_code = request[7];
        
        info!("📋 Channel {} - 请求详情: TID={}, PID={}, Len={}, Unit={}, FC=0x{:02X}", 
            channel_id, transaction_id, protocol_id, length, unit_id, function_code);
            
        // 解析具体的功能码含义和参数
        let function_description = match function_code {
            0x01 => {
                if request.len() >= 12 {
                    let start_addr = u16::from_be_bytes([request[8], request[9]]);
                    let quantity = u16::from_be_bytes([request[10], request[11]]);
                    format!("读取线圈(Read Coils) - 起始地址:{}, 数量:{}", start_addr, quantity)
                } else {
                    "读取线圈(Read Coils) - 数据不完整".to_string()
                }
            },
            0x02 => {
                if request.len() >= 12 {
                    let start_addr = u16::from_be_bytes([request[8], request[9]]);
                    let quantity = u16::from_be_bytes([request[10], request[11]]);
                    format!("读取离散输入(Read Discrete Inputs) - 起始地址:{}, 数量:{}", start_addr, quantity)
                } else {
                    "读取离散输入(Read Discrete Inputs) - 数据不完整".to_string()
                }
            },
            0x03 => {
                if request.len() >= 12 {
                    let start_addr = u16::from_be_bytes([request[8], request[9]]);
                    let quantity = u16::from_be_bytes([request[10], request[11]]);
                    format!("读取保持寄存器(Read Holding Registers) - 起始地址:{}, 数量:{}", start_addr, quantity)
                } else {
                    "读取保持寄存器(Read Holding Registers) - 数据不完整".to_string()
                }
            },
            0x04 => {
                if request.len() >= 12 {
                    let start_addr = u16::from_be_bytes([request[8], request[9]]);
                    let quantity = u16::from_be_bytes([request[10], request[11]]);
                    format!("读取输入寄存器(Read Input Registers) - 起始地址:{}, 数量:{}", start_addr, quantity)
                } else {
                    "读取输入寄存器(Read Input Registers) - 数据不完整".to_string()
                }
            },
            0x05 => {
                if request.len() >= 12 {
                    let start_addr = u16::from_be_bytes([request[8], request[9]]);
                    let value = u16::from_be_bytes([request[10], request[11]]);
                    let coil_state = if value == 0xFF00 { "ON" } else { "OFF" };
                    format!("写单个线圈(Write Single Coil) - 地址:{}, 值:{} ({})", start_addr, value, coil_state)
                } else {
                    "写单个线圈(Write Single Coil) - 数据不完整".to_string()
                }
            },
            0x06 => {
                if request.len() >= 12 {
                    let start_addr = u16::from_be_bytes([request[8], request[9]]);
                    let value = u16::from_be_bytes([request[10], request[11]]);
                    format!("写单个寄存器(Write Single Register) - 地址:{}, 值:{}", start_addr, value)
                } else {
                    "写单个寄存器(Write Single Register) - 数据不完整".to_string()
                }
            },
            _ => format!("未知功能码 0x{:02X}", function_code),
        };
        info!("🔧 Channel {} - {}", channel_id, function_description);
    }
}

async fn simulate_response_logging(channel_id: &str, response: &[u8]) {
    // 记录接收的报文
    info!("📥 Channel {} - 接收 Modbus 响应: {} bytes", channel_id, response.len());
    info!("📥 Channel {} - 接收报文: {:02X?}", channel_id, response);
    
    // 解析响应详情
    if response.len() >= 7 {
        let response_length = u16::from_be_bytes([response[4], response[5]]) as usize;
        let response_unit_id = response[6];
        
        info!("📋 Channel {} - 响应长度: {} bytes, Unit ID: {}", 
            channel_id, response_length, response_unit_id);

        // 解析PDU数据
        let pdu_length = response_length.saturating_sub(1);
        if response.len() >= 7 + pdu_length && pdu_length > 0 {
            let pdu_data = &response[7..7 + pdu_length];
            let function_code = pdu_data[0];
            
            if (function_code & 0x80) != 0 {
                // 错误响应
                let original_function = function_code & 0x7F;
                let exception_code = if pdu_data.len() > 1 { pdu_data[1] } else { 0 };
                let exception_description = match exception_code {
                    0x01 => "非法功能码 - 从站不支持此功能码",
                    0x02 => "非法数据地址 - 地址超出范围或无效", 
                    0x03 => "非法数据值 - 请求的数据值无效",
                    0x04 => "从站设备故障 - 从站无法执行请求",
                    0x05 => "确认 - 从站接受请求但需要长时间处理",
                    0x06 => "从站设备忙 - 从站正在处理其他命令",
                    0x08 => "存储器奇偶性错误 - 从站内存校验失败",
                    0x0A => "不可用网关路径 - 网关配置错误",
                    0x0B => "网关目标设备响应失败 - 目标设备无响应",
                    _ => "未知异常",
                };
                info!("❌ Channel {} - Modbus异常响应: 原功能码=0x{:02X}, 异常码=0x{:02X} ({})", 
                    channel_id, original_function, exception_code, exception_description);
            } else {
                // 正常响应 - 详细分析各种功能码的响应数据
                let response_description = match function_code {
                    0x01 | 0x02 => {
                        // 读取线圈/离散输入响应
                        if pdu_data.len() > 1 {
                            let byte_count = pdu_data[1];
                            let coil_count = (byte_count * 8) as u16;
                            if pdu_data.len() >= 2 + byte_count as usize {
                                let mut coil_states = Vec::new();
                                for i in 0..byte_count {
                                    let byte_val = pdu_data[2 + i as usize];
                                    for bit in 0..8 {
                                        if (coil_count as usize) > coil_states.len() {
                                            coil_states.push((byte_val >> bit) & 1 == 1);
                                        }
                                    }
                                }
                                let on_count = coil_states.iter().filter(|&&x| x).count();
                                format!("读取{}响应 - 字节数:{}, 线圈总数:{}, ON:{}, OFF:{}", 
                                    if function_code == 0x01 { "线圈" } else { "离散输入" },
                                    byte_count, coil_states.len(), on_count, coil_states.len() - on_count)
                            } else {
                                format!("读取{}响应 - 数据不完整", 
                                    if function_code == 0x01 { "线圈" } else { "离散输入" })
                            }
                        } else {
                            format!("读取{}响应 - 数据不完整", 
                                if function_code == 0x01 { "线圈" } else { "离散输入" })
                        }
                    },
                    0x03 | 0x04 => {
                        // 读取寄存器响应
                        if pdu_data.len() > 1 {
                            let byte_count = pdu_data[1];
                            let register_count = byte_count / 2;
                            if pdu_data.len() >= 2 + byte_count as usize {
                                let mut register_values = Vec::new();
                                for i in 0..register_count {
                                    let idx = 2 + (i * 2) as usize;
                                    let value = u16::from_be_bytes([pdu_data[idx], pdu_data[idx + 1]]);
                                    register_values.push(value);
                                }
                                let values_str = if register_values.len() <= 10 {
                                    format!("{:?}", register_values)
                                } else {
                                    format!("[{}...] (共{}个)", 
                                        register_values.iter().take(5).map(|v| format!("{}", v)).collect::<Vec<_>>().join(","),
                                        register_values.len())
                                };
                                format!("读取{}响应 - 字节数:{}, 寄存器数:{}, 值:{}", 
                                    if function_code == 0x03 { "保持寄存器" } else { "输入寄存器" },
                                    byte_count, register_count, values_str)
                            } else {
                                format!("读取{}响应 - 数据不完整", 
                                    if function_code == 0x03 { "保持寄存器" } else { "输入寄存器" })
                            }
                        } else {
                            format!("读取{}响应 - 数据不完整", 
                                if function_code == 0x03 { "保持寄存器" } else { "输入寄存器" })
                        }
                    },
                    0x05 => {
                        // 写单个线圈响应
                        if pdu_data.len() >= 5 {
                            let address = u16::from_be_bytes([pdu_data[1], pdu_data[2]]);
                            let value = u16::from_be_bytes([pdu_data[3], pdu_data[4]]);
                            let state = if value == 0xFF00 { "ON" } else { "OFF" };
                            format!("写单个线圈响应 - 地址:{}, 值:{} ({})", address, value, state)
                        } else {
                            "写单个线圈响应 - 数据不完整".to_string()
                        }
                    },
                    0x06 => {
                        // 写单个寄存器响应
                        if pdu_data.len() >= 5 {
                            let address = u16::from_be_bytes([pdu_data[1], pdu_data[2]]);
                            let value = u16::from_be_bytes([pdu_data[3], pdu_data[4]]);
                            format!("写单个寄存器响应 - 地址:{}, 值:{}", address, value)
                        } else {
                            "写单个寄存器响应 - 数据不完整".to_string()
                        }
                    },
                    _ => format!("未知功能码响应 0x{:02X}", function_code),
                };
                info!("✅ Channel {} - {}", channel_id, response_description);
            }
        }
    }
}

#[tokio::main]
async fn main() {
    demo_modbus_logging().await;
} 