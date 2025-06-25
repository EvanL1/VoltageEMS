use crate::utils::error::{ComSrvError, Result};
/// Modbus Bit Operations Module
///
/// This module provides utilities for bit-level operations on Modbus registers,
/// allowing extraction and manipulation of individual bits within 16-bit registers.
use serde::{Deserialize, Serialize};

/// Bit position within a 16-bit register (0-15)
pub type BitPosition = u8;

/// Bit operation configuration for extracting boolean values from registers
/// 位操作配置，用于从寄存器中提取布尔值
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitExtractConfig {
    /// Source register address
    pub register_address: u16,
    /// Bit position (0-15, where 0 is LSB)
    pub bit_position: BitPosition,
    /// Point name for this bit
    pub point_name: String,
    /// Chinese name
    pub chinese_name: String,
    /// Description
    pub description: Option<String>,
    /// Group identifier
    pub group: Option<String>,
}

impl BitExtractConfig {
    /// Create a new bit extract configuration
    pub fn new(
        register_address: u16,
        bit_position: BitPosition,
        point_name: String,
        chinese_name: String,
    ) -> Result<Self> {
        if bit_position > 15 {
            return Err(ComSrvError::ConfigError(format!(
                "Bit position {} is out of range (0-15)",
                bit_position
            )));
        }

        Ok(Self {
            register_address,
            bit_position,
            point_name,
            chinese_name,
            description: None,
            group: None,
        })
    }

    /// Set optional description
    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }

    /// Set optional group
    pub fn with_group(mut self, group: String) -> Self {
        self.group = Some(group);
        self
    }
}

/// Bit operations utility for Modbus registers
/// Modbus寄存器位操作工具
pub struct ModbusBitOperations;

impl ModbusBitOperations {
    /// Extract a specific bit from a 16-bit register value
    /// 从16位寄存器值中提取指定位
    ///
    /// # Arguments
    /// * `register_value` - The 16-bit register value
    /// * `bit_position` - Bit position (0-15, where 0 is LSB)
    ///
    /// # Returns
    /// * `Ok(bool)` - The bit value (true/false)
    /// * `Err(ComSrvError)` - If bit position is out of range
    ///
    /// # Example
    /// ```rust
    /// use comsrv::core::protocols::modbus::bit_operations::ModbusBitOperations;
    ///
    /// let register_value = 0b1010110100110101; // 0xAD35
    /// let bit_0 = ModbusBitOperations::extract_bit(register_value, 0).unwrap(); // true (LSB)
    /// let bit_1 = ModbusBitOperations::extract_bit(register_value, 1).unwrap(); // false
    /// let bit_15 = ModbusBitOperations::extract_bit(register_value, 15).unwrap(); // true (MSB)
    /// ```
    pub fn extract_bit(register_value: u16, bit_position: BitPosition) -> Result<bool> {
        if bit_position > 15 {
            return Err(ComSrvError::ConfigError(format!(
                "Bit position {} is out of range (0-15)",
                bit_position
            )));
        }

        let mask = 1u16 << bit_position;
        Ok((register_value & mask) != 0)
    }

    /// Extract multiple bits from a register value using configurations
    /// 使用配置从寄存器值中提取多个位
    ///
    /// # Arguments
    /// * `register_value` - The 16-bit register value
    /// * `bit_configs` - Vector of bit extraction configurations
    ///
    /// # Returns
    /// * `Vec<(String, bool)>` - Vector of (point_name, bit_value) pairs
    pub fn extract_bits_by_config(
        register_value: u16,
        bit_configs: &[BitExtractConfig],
    ) -> Result<Vec<(String, bool)>> {
        let mut results = Vec::new();

        for config in bit_configs {
            let bit_value = Self::extract_bit(register_value, config.bit_position)?;
            results.push((config.point_name.clone(), bit_value));
        }

        Ok(results)
    }

    /// Set a specific bit in a 16-bit register value
    /// 在16位寄存器值中设置指定位
    ///
    /// # Arguments
    /// * `register_value` - The current 16-bit register value
    /// * `bit_position` - Bit position (0-15, where 0 is LSB)
    /// * `bit_value` - The new bit value (true/false)
    ///
    /// # Returns
    /// * `Ok(u16)` - The new register value with the bit set
    /// * `Err(ComSrvError)` - If bit position is out of range
    ///
    /// # Example
    /// ```rust
    /// use comsrv::core::protocols::modbus::bit_operations::ModbusBitOperations;
    ///
    /// let mut register_value = 0b0000000000000000; // 0x0000
    /// register_value = ModbusBitOperations::set_bit(register_value, 0, true).unwrap();  // Set bit 0
    /// register_value = ModbusBitOperations::set_bit(register_value, 15, true).unwrap(); // Set bit 15
    /// // Result: 0b1000000000000001 (0x8001)
    /// ```
    pub fn set_bit(register_value: u16, bit_position: BitPosition, bit_value: bool) -> Result<u16> {
        if bit_position > 15 {
            return Err(ComSrvError::ConfigError(format!(
                "Bit position {} is out of range (0-15)",
                bit_position
            )));
        }

        let mask = 1u16 << bit_position;

        if bit_value {
            // Set bit to 1
            Ok(register_value | mask)
        } else {
            // Set bit to 0
            Ok(register_value & !mask)
        }
    }

    /// Clear a specific bit in a 16-bit register value (set to 0)
    /// 清除16位寄存器值中的指定位（设置为0）
    pub fn clear_bit(register_value: u16, bit_position: BitPosition) -> Result<u16> {
        Self::set_bit(register_value, bit_position, false)
    }

    /// Toggle a specific bit in a 16-bit register value
    /// 切换16位寄存器值中的指定位
    pub fn toggle_bit(register_value: u16, bit_position: BitPosition) -> Result<u16> {
        if bit_position > 15 {
            return Err(ComSrvError::ConfigError(format!(
                "Bit position {} is out of range (0-15)",
                bit_position
            )));
        }

        let mask = 1u16 << bit_position;
        Ok(register_value ^ mask)
    }

    /// Extract all 16 bits from a register value as a vector of booleans
    /// 从寄存器值中提取所有16位作为布尔值向量
    ///
    /// # Returns
    /// Vector of 16 boolean values, where index 0 is LSB and index 15 is MSB
    pub fn extract_all_bits(register_value: u16) -> Vec<bool> {
        (0..16)
            .map(|bit_pos| (register_value & (1u16 << bit_pos)) != 0)
            .collect()
    }

    /// Create a register value from a vector of bit values
    /// 从位值向量创建寄存器值
    ///
    /// # Arguments
    /// * `bit_values` - Vector of up to 16 boolean values (LSB first)
    ///
    /// # Returns
    /// 16-bit register value constructed from the bit values
    pub fn create_register_from_bits(bit_values: &[bool]) -> u16 {
        let mut register_value = 0u16;

        for (bit_pos, &bit_value) in bit_values.iter().enumerate().take(16) {
            if bit_value {
                register_value |= 1u16 << bit_pos;
            }
        }

        register_value
    }

    /// Get bit mask for a specific bit position
    /// 获取指定位位置的位掩码
    pub fn get_bit_mask(bit_position: BitPosition) -> Result<u16> {
        if bit_position > 15 {
            return Err(ComSrvError::ConfigError(format!(
                "Bit position {} is out of range (0-15)",
                bit_position
            )));
        }

        Ok(1u16 << bit_position)
    }

    /// Check if multiple bits are set in a register
    /// 检查寄存器中是否设置了多个位
    ///
    /// # Arguments
    /// * `register_value` - The 16-bit register value
    /// * `bit_positions` - Vector of bit positions to check
    ///
    /// # Returns
    /// * `true` if ALL specified bits are set
    /// * `false` if ANY specified bit is not set
    pub fn are_bits_set(register_value: u16, bit_positions: &[BitPosition]) -> Result<bool> {
        for &bit_pos in bit_positions {
            if bit_pos > 15 {
                return Err(ComSrvError::ConfigError(format!(
                    "Bit position {} is out of range (0-15)",
                    bit_pos
                )));
            }

            if !Self::extract_bit(register_value, bit_pos)? {
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// Count the number of set bits in a register
    /// 计算寄存器中设置位的数量
    pub fn count_set_bits(register_value: u16) -> u8 {
        register_value.count_ones() as u8
    }

    /// Find the position of the first set bit (LSB)
    /// 查找第一个设置位的位置（LSB）
    ///
    /// # Returns
    /// * `Some(BitPosition)` - Position of the first set bit
    /// * `None` - If no bits are set
    pub fn find_first_set_bit(register_value: u16) -> Option<BitPosition> {
        if register_value == 0 {
            return None;
        }

        Some(register_value.trailing_zeros() as BitPosition)
    }

    /// Find the position of the last set bit (MSB)
    /// 查找最后一个设置位的位置（MSB）
    ///
    /// # Returns
    /// * `Some(BitPosition)` - Position of the last set bit
    /// * `None` - If no bits are set
    pub fn find_last_set_bit(register_value: u16) -> Option<BitPosition> {
        if register_value == 0 {
            return None;
        }

        Some(15 - register_value.leading_zeros() as BitPosition)
    }
}

/// Bit mapping configuration for a complete register
/// 完整寄存器的位映射配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterBitMapping {
    /// Register address
    pub register_address: u16,
    /// Bit configurations for each position used
    pub bit_configs: Vec<BitExtractConfig>,
}

impl RegisterBitMapping {
    /// Create a new register bit mapping
    pub fn new(register_address: u16) -> Self {
        Self {
            register_address,
            bit_configs: Vec::new(),
        }
    }

    /// Add a bit configuration
    pub fn add_bit_config(mut self, config: BitExtractConfig) -> Result<Self> {
        // Validate that the register address matches
        if config.register_address != self.register_address {
            return Err(ComSrvError::ConfigError(format!(
                "Bit config register address {} does not match mapping address {}",
                config.register_address, self.register_address
            )));
        }

        // Check for duplicate bit positions
        if self
            .bit_configs
            .iter()
            .any(|c| c.bit_position == config.bit_position)
        {
            return Err(ComSrvError::ConfigError(format!(
                "Bit position {} is already configured for register {}",
                config.bit_position, self.register_address
            )));
        }

        self.bit_configs.push(config);
        Ok(self)
    }

    /// Extract all configured bits from a register value
    pub fn extract_all_configured_bits(&self, register_value: u16) -> Result<Vec<(String, bool)>> {
        ModbusBitOperations::extract_bits_by_config(register_value, &self.bit_configs)
    }

    /// Get the number of configured bits
    pub fn bit_count(&self) -> usize {
        self.bit_configs.len()
    }

    /// Check if a specific bit position is configured
    pub fn has_bit_position(&self, bit_position: BitPosition) -> bool {
        self.bit_configs
            .iter()
            .any(|c| c.bit_position == bit_position)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_bit() {
        let register_value = 0b1010110100110101; // 0xAD35

        assert_eq!(
            ModbusBitOperations::extract_bit(register_value, 0).unwrap(),
            true
        ); // LSB
        assert_eq!(
            ModbusBitOperations::extract_bit(register_value, 1).unwrap(),
            false
        );
        assert_eq!(
            ModbusBitOperations::extract_bit(register_value, 2).unwrap(),
            true
        );
        assert_eq!(
            ModbusBitOperations::extract_bit(register_value, 15).unwrap(),
            true
        ); // MSB

        // Test error case
        assert!(ModbusBitOperations::extract_bit(register_value, 16).is_err());
    }

    #[test]
    fn test_set_bit() {
        let mut register_value = 0b0000000000000000;

        register_value = ModbusBitOperations::set_bit(register_value, 0, true).unwrap();
        assert_eq!(register_value, 0b0000000000000001);

        register_value = ModbusBitOperations::set_bit(register_value, 15, true).unwrap();
        assert_eq!(register_value, 0b1000000000000001);

        register_value = ModbusBitOperations::set_bit(register_value, 0, false).unwrap();
        assert_eq!(register_value, 0b1000000000000000);

        // Test error case
        assert!(ModbusBitOperations::set_bit(register_value, 16, true).is_err());
    }

    #[test]
    fn test_toggle_bit() {
        let mut register_value = 0b0000000000000001;

        register_value = ModbusBitOperations::toggle_bit(register_value, 0).unwrap();
        assert_eq!(register_value, 0b0000000000000000);

        register_value = ModbusBitOperations::toggle_bit(register_value, 1).unwrap();
        assert_eq!(register_value, 0b0000000000000010);
    }

    #[test]
    fn test_extract_all_bits() {
        let register_value = 0b1010110100110101; // 0xAD35
        let bits = ModbusBitOperations::extract_all_bits(register_value);

        assert_eq!(bits.len(), 16);
        assert_eq!(bits[0], true); // LSB
        assert_eq!(bits[1], false);
        assert_eq!(bits[2], true);
        assert_eq!(bits[15], true); // MSB
    }

    #[test]
    fn test_create_register_from_bits() {
        let bit_values = vec![
            true, false, true, false, // bits 0-3
            false, true, true, false, // bits 4-7
            true, false, true, true, // bits 8-11
            false, true, false, true, // bits 12-15
        ];

        let register_value = ModbusBitOperations::create_register_from_bits(&bit_values);
        let expected = 0b1010110101100101; // 0xAD65 - 正确的计算值
        assert_eq!(register_value, expected);
    }

    #[test]
    fn test_count_set_bits() {
        let register_value = 0b1010110100110101; // 0xAD35
        let count = ModbusBitOperations::count_set_bits(register_value);
        assert_eq!(count, 9); // 正确的计数是9，不是10
    }

    #[test]
    fn test_find_first_and_last_set_bit() {
        let register_value = 0b1010110100110100; // 0xAD34 (bit 0 is 0)

        assert_eq!(
            ModbusBitOperations::find_first_set_bit(register_value),
            Some(2)
        );
        assert_eq!(
            ModbusBitOperations::find_last_set_bit(register_value),
            Some(15)
        );

        assert_eq!(ModbusBitOperations::find_first_set_bit(0), None);
        assert_eq!(ModbusBitOperations::find_last_set_bit(0), None);
    }

    #[test]
    fn test_are_bits_set() {
        let register_value = 0b1010110100110101; // 0xAD35

        assert!(ModbusBitOperations::are_bits_set(register_value, &[0, 2, 4]).unwrap());
        assert!(!ModbusBitOperations::are_bits_set(register_value, &[0, 1, 2]).unwrap());
        // bit 1 is not set
    }

    #[test]
    fn test_bit_extract_config() {
        let config =
            BitExtractConfig::new(1000, 5, "pump_status".to_string(), "泵状态".to_string())
                .unwrap()
                .with_description("Main pump running status".to_string())
                .with_group("pumps".to_string());

        assert_eq!(config.register_address, 1000);
        assert_eq!(config.bit_position, 5);
        assert_eq!(config.point_name, "pump_status");
        assert_eq!(config.chinese_name, "泵状态");
        assert_eq!(
            config.description,
            Some("Main pump running status".to_string())
        );
        assert_eq!(config.group, Some("pumps".to_string()));

        // Test error case
        assert!(BitExtractConfig::new(1000, 16, "test".to_string(), "测试".to_string()).is_err());
    }

    #[test]
    fn test_register_bit_mapping() {
        let mut mapping = RegisterBitMapping::new(1000);

        let config1 =
            BitExtractConfig::new(1000, 0, "bit0".to_string(), "位0".to_string()).unwrap();
        let config2 =
            BitExtractConfig::new(1000, 5, "bit5".to_string(), "位5".to_string()).unwrap();

        mapping = mapping.add_bit_config(config1).unwrap();
        mapping = mapping.add_bit_config(config2).unwrap();

        assert_eq!(mapping.bit_count(), 2);
        assert!(mapping.has_bit_position(0));
        assert!(mapping.has_bit_position(5));
        assert!(!mapping.has_bit_position(1));

        let register_value = 0b0000000000100001; // bits 0 and 5 set
        let results = mapping.extract_all_configured_bits(register_value).unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0], ("bit0".to_string(), true));
        assert_eq!(results[1], ("bit5".to_string(), true));
    }

    #[test]
    fn test_register_bit_mapping_errors() {
        let mapping = RegisterBitMapping::new(1000);

        // Test wrong register address
        let wrong_config =
            BitExtractConfig::new(1001, 0, "test".to_string(), "测试".to_string()).unwrap();
        assert!(mapping.clone().add_bit_config(wrong_config).is_err());

        // Test duplicate bit position
        let config1 =
            BitExtractConfig::new(1000, 0, "bit0_1".to_string(), "位0_1".to_string()).unwrap();
        let config2 =
            BitExtractConfig::new(1000, 0, "bit0_2".to_string(), "位0_2".to_string()).unwrap();

        let mapping = mapping.add_bit_config(config1).unwrap();
        assert!(mapping.add_bit_config(config2).is_err());
    }
}
