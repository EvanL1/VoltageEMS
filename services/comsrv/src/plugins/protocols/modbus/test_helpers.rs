//! Modbus 测试助手模块
//!
//! 提供用于测试的模拟设备和辅助函数

#[cfg(test)]
pub mod test_helpers {
    use crate::utils::error::{ComSrvError, Result};
    use std::collections::HashMap;

    /// Mock Modbus设备，用于测试
    pub struct MockModbusDevice {
        holding_registers: HashMap<u16, u16>,
        input_registers: HashMap<u16, u16>,
        coils: HashMap<u16, bool>,
        discrete_inputs: HashMap<u16, bool>,
    }

    impl MockModbusDevice {
        pub fn new() -> Self {
            Self {
                holding_registers: HashMap::new(),
                input_registers: HashMap::new(),
                coils: HashMap::new(),
                discrete_inputs: HashMap::new(),
            }
        }

        /// 设置32位浮点数（占用2个寄存器）
        pub fn set_holding_register_f32(&mut self, addr: u16, value: f32) {
            let bytes = value.to_be_bytes();
            let high = u16::from_be_bytes([bytes[0], bytes[1]]);
            let low = u16::from_be_bytes([bytes[2], bytes[3]]);
            self.holding_registers.insert(addr, high);
            self.holding_registers.insert(addr + 1, low);
        }

        /// 设置16位整数
        pub fn set_holding_register_u16(&mut self, addr: u16, value: u16) {
            self.holding_registers.insert(addr, value);
        }

        /// 设置线圈状态
        pub fn set_coil(&mut self, addr: u16, value: bool) {
            self.coils.insert(addr, value);
        }

        /// 获取保持寄存器
        pub fn get_holding_registers(&self, addr: u16, count: u16) -> Vec<u16> {
            (addr..addr + count)
                .map(|a| self.holding_registers.get(&a).copied().unwrap_or(0))
                .collect()
        }

        /// 获取输入寄存器
        pub fn get_input_registers(&self, addr: u16, count: u16) -> Vec<u16> {
            (addr..addr + count)
                .map(|a| self.input_registers.get(&a).copied().unwrap_or(0))
                .collect()
        }

        /// 获取线圈状态
        pub fn get_coils(&self, addr: u16, count: u16) -> Vec<bool> {
            (addr..addr + count)
                .map(|a| self.coils.get(&a).copied().unwrap_or(false))
                .collect()
        }

        /// 获取离散输入
        pub fn get_discrete_inputs(&self, addr: u16, count: u16) -> Vec<bool> {
            (addr..addr + count)
                .map(|a| self.discrete_inputs.get(&a).copied().unwrap_or(false))
                .collect()
        }
    }

    /// 创建测试用的Modbus响应数据
    pub fn create_modbus_response(function_code: u8, data: Vec<u8>) -> Vec<u8> {
        let mut response = vec![1, function_code]; // slave_id + function_code
        response.push(data.len() as u8); // byte count
        response.extend(data);
        response
    }

    /// 验证Modbus请求格式
    pub fn validate_modbus_request(request: &[u8]) -> Result<(u8, u8, u16, u16)> {
        if request.len() < 6 {
            return Err(ComSrvError::protocol("Invalid Modbus request length"));
        }

        let slave_id = request[0];
        let function_code = request[1];
        let start_address = u16::from_be_bytes([request[2], request[3]]);
        let quantity = u16::from_be_bytes([request[4], request[5]]);

        Ok((slave_id, function_code, start_address, quantity))
    }

    /// 创建批量测试点位
    pub fn create_test_points(count: u32) -> Vec<crate::core::config::types::UnifiedPointMapping> {
        use crate::core::config::types::UnifiedPointMapping;

        (1..=count)
            .map(|i| UnifiedPointMapping {
                point_id: i,
                signal_name: format!("Point_{}", i),
                telemetry_type: "YC".to_string(),
                data_type: "float32".to_string(),
                protocol_params: {
                    let mut params = HashMap::new();
                    params.insert("address".to_string(), format!("1:3:{}", (i - 1) * 2));
                    params.insert("data_format".to_string(), "float32_be".to_string());
                    params.insert("register_count".to_string(), "2".to_string());
                    params
                },
                scaling: None,
            })
            .collect()
    }

    /// 创建混合类型的测试点位
    pub fn create_mixed_test_points() -> Vec<crate::core::config::types::UnifiedPointMapping> {
        use crate::core::config::types::UnifiedPointMapping;

        vec![
            // YC - 遥测
            UnifiedPointMapping {
                point_id: 1,
                signal_name: "Temperature".to_string(),
                telemetry_type: "YC".to_string(),
                data_type: "float32".to_string(),
                protocol_params: {
                    let mut params = HashMap::new();
                    params.insert("address".to_string(), "1:3:0".to_string());
                    params.insert("data_format".to_string(), "float32_be".to_string());
                    params.insert("register_count".to_string(), "2".to_string());
                    params
                },
                scaling: None,
            },
            // YX - 信号
            UnifiedPointMapping {
                point_id: 2,
                signal_name: "Switch_Status".to_string(),
                telemetry_type: "YX".to_string(),
                data_type: "bool".to_string(),
                protocol_params: {
                    let mut params = HashMap::new();
                    params.insert("address".to_string(), "1:1:0".to_string());
                    params.insert("data_format".to_string(), "bool".to_string());
                    params
                },
                scaling: None,
            },
            // YK - 控制
            UnifiedPointMapping {
                point_id: 3,
                signal_name: "Switch_Control".to_string(),
                telemetry_type: "YK".to_string(),
                data_type: "bool".to_string(),
                protocol_params: {
                    let mut params = HashMap::new();
                    params.insert("address".to_string(), "1:5:0".to_string());
                    params.insert("data_format".to_string(), "bool".to_string());
                    params
                },
                scaling: None,
            },
            // YT - 调节
            UnifiedPointMapping {
                point_id: 4,
                signal_name: "Setpoint".to_string(),
                telemetry_type: "YT".to_string(),
                data_type: "float32".to_string(),
                protocol_params: {
                    let mut params = HashMap::new();
                    params.insert("address".to_string(), "1:6:10".to_string());
                    params.insert("data_format".to_string(), "float32_be".to_string());
                    params.insert("register_count".to_string(), "2".to_string());
                    params
                },
                scaling: None,
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::test_helpers::*;

    #[test]
    fn test_mock_device_f32() {
        let mut device = MockModbusDevice::new();
        device.set_holding_register_f32(0, 123.45);

        let registers = device.get_holding_registers(0, 2);
        assert_eq!(registers.len(), 2);

        // 重新组合为f32
        let bytes = [
            (registers[0] >> 8) as u8,
            registers[0] as u8,
            (registers[1] >> 8) as u8,
            registers[1] as u8,
        ];
        let value = f32::from_be_bytes(bytes);
        assert!((value - 123.45).abs() < 0.001);
    }

    #[test]
    fn test_create_modbus_response() {
        let data = vec![0x00, 0x01, 0x00, 0x02];
        let response = create_modbus_response(3, data.clone());

        assert_eq!(response[0], 1); // slave_id
        assert_eq!(response[1], 3); // function_code
        assert_eq!(response[2], 4); // byte count
        assert_eq!(&response[3..], &data[..]);
    }

    #[test]
    fn test_validate_modbus_request() {
        let request = vec![1, 3, 0, 0, 0, 10];
        let result = validate_modbus_request(&request).unwrap();

        assert_eq!(result.0, 1); // slave_id
        assert_eq!(result.1, 3); // function_code
        assert_eq!(result.2, 0); // start_address
        assert_eq!(result.3, 10); // quantity
    }
}
