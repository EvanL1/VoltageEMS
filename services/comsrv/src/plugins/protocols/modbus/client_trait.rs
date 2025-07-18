//! Unified Modbus Client Trait
//!
//! This module defines a unified trait for Modbus clients that abstracts away
//! the differences between TCP and RTU protocols, providing a common interface
//! for all Modbus operations.

use async_trait::async_trait;
use std::time::Duration;

use crate::utils::error::Result;

/// Unified Modbus client trait that abstracts TCP/RTU protocol differences
///
/// This trait provides a common interface for all standard Modbus operations,
/// allowing upper layers to use Modbus functionality without caring about
/// the underlying transport mechanism (TCP vs RTU).
#[async_trait]
pub trait ModbusClient: Send + Sync {
    /// Read holding registers (Function Code 0x03)
    ///
    /// # Arguments
    /// * `slave_id` - Target slave/unit identifier
    /// * `start_address` - Starting register address
    /// * `count` - Number of registers to read
    ///
    /// # Returns
    /// * `Result<Vec<u16>>` - Vector of register values
    async fn read_holding_registers(
        &mut self,
        slave_id: u8,
        start_address: u16,
        count: u16,
    ) -> Result<Vec<u16>>;

    /// Read input registers (Function Code 0x04)
    ///
    /// # Arguments
    /// * `slave_id` - Target slave/unit identifier
    /// * `start_address` - Starting register address
    /// * `count` - Number of registers to read
    ///
    /// # Returns
    /// * `Result<Vec<u16>>` - Vector of register values
    async fn read_input_registers(
        &mut self,
        slave_id: u8,
        start_address: u16,
        count: u16,
    ) -> Result<Vec<u16>>;

    /// Read coils (Function Code 0x01)
    ///
    /// # Arguments
    /// * `slave_id` - Target slave/unit identifier
    /// * `start_address` - Starting coil address
    /// * `count` - Number of coils to read
    ///
    /// # Returns
    /// * `Result<Vec<bool>>` - Vector of coil states
    async fn read_coils(
        &mut self,
        slave_id: u8,
        start_address: u16,
        count: u16,
    ) -> Result<Vec<bool>>;

    /// Read discrete inputs (Function Code 0x02)
    ///
    /// # Arguments
    /// * `slave_id` - Target slave/unit identifier
    /// * `start_address` - Starting input address
    /// * `count` - Number of inputs to read
    ///
    /// # Returns
    /// * `Result<Vec<bool>>` - Vector of input states
    async fn read_discrete_inputs(
        &mut self,
        slave_id: u8,
        start_address: u16,
        count: u16,
    ) -> Result<Vec<bool>>;

    /// Write single coil (Function Code 0x05)
    ///
    /// # Arguments
    /// * `slave_id` - Target slave/unit identifier
    /// * `address` - Coil address
    /// * `value` - Coil state to write
    ///
    /// # Returns
    /// * `Result<()>` - Success or error
    async fn write_single_coil(&mut self, slave_id: u8, address: u16, value: bool) -> Result<()>;

    /// Write single register (Function Code 0x06)
    ///
    /// # Arguments
    /// * `slave_id` - Target slave/unit identifier
    /// * `address` - Register address
    /// * `value` - Register value to write
    ///
    /// # Returns
    /// * `Result<()>` - Success or error
    async fn write_single_register(&mut self, slave_id: u8, address: u16, value: u16)
        -> Result<()>;

    /// Write multiple coils (Function Code 0x0F)
    ///
    /// # Arguments
    /// * `slave_id` - Target slave/unit identifier
    /// * `start_address` - Starting coil address
    /// * `values` - Coil states to write
    ///
    /// # Returns
    /// * `Result<()>` - Success or error
    async fn write_multiple_coils(
        &mut self,
        slave_id: u8,
        start_address: u16,
        values: &[bool],
    ) -> Result<()>;

    /// Write multiple registers (Function Code 0x10)
    ///
    /// # Arguments
    /// * `slave_id` - Target slave/unit identifier
    /// * `start_address` - Starting register address
    /// * `values` - Register values to write
    ///
    /// # Returns
    /// * `Result<()>` - Success or error
    async fn write_multiple_registers(
        &mut self,
        slave_id: u8,
        start_address: u16,
        values: &[u16],
    ) -> Result<()>;

    /// Get connection status
    ///
    /// # Returns
    /// * `bool` - True if connected, false otherwise
    async fn is_connected(&self) -> bool;

    /// Connect to the Modbus device/server
    ///
    /// # Returns
    /// * `Result<()>` - Success or error
    async fn connect(&mut self) -> Result<()>;

    /// Disconnect from the Modbus device/server
    ///
    /// # Returns
    /// * `Result<()>` - Success or error
    async fn disconnect(&mut self) -> Result<()>;

    /// Set request timeout
    ///
    /// # Arguments
    /// * `timeout` - New timeout duration
    async fn set_timeout(&mut self, timeout: Duration);

    /// Get current timeout setting
    ///
    /// # Returns
    /// * `Duration` - Current timeout duration
    fn get_timeout(&self) -> Duration;

    /// Get protocol-specific diagnostics
    ///
    /// # Returns
    /// * `std::collections::HashMap<String, String>` - Diagnostic information
    async fn get_diagnostics(&self) -> std::collections::HashMap<String, String>;

    // Batch operations for performance optimization

    /// Read multiple register ranges in a single optimized operation
    ///
    /// # Arguments
    /// * `slave_id` - Target slave/unit identifier
    /// * `ranges` - Vector of (start_address, count) tuples
    ///
    /// # Returns
    /// * `Result<Vec<Vec<u16>>>` - Vector of register value vectors
    async fn read_multiple_register_ranges(
        &mut self,
        slave_id: u8,
        ranges: &[(u16, u16)],
    ) -> Result<Vec<Vec<u16>>>;

    /// Read multiple coil ranges in a single optimized operation
    ///
    /// # Arguments
    /// * `slave_id` - Target slave/unit identifier
    /// * `ranges` - Vector of (start_address, count) tuples
    ///
    /// # Returns
    /// * `Result<Vec<Vec<bool>>>` - Vector of coil state vectors
    async fn read_multiple_coil_ranges(
        &mut self,
        slave_id: u8,
        ranges: &[(u16, u16)],
    ) -> Result<Vec<Vec<bool>>>;
}

/// Extended Modbus client trait for advanced operations
///
/// This trait provides additional functionality that may not be supported
/// by all implementations but offers enhanced capabilities when available.
#[async_trait]
pub trait ExtendedModbusClient: ModbusClient {
    /// Read device identification (Function Code 0x2B/0x0E)
    ///
    /// # Arguments
    /// * `slave_id` - Target slave/unit identifier
    /// * `object_id` - Device identification object ID
    ///
    /// # Returns
    /// * `Result<String>` - Device identification string
    async fn read_device_identification(&mut self, slave_id: u8, object_id: u8) -> Result<String>;

    /// Read exception status (Function Code 0x07)
    ///
    /// # Arguments
    /// * `slave_id` - Target slave/unit identifier
    ///
    /// # Returns
    /// * `Result<u8>` - Exception status byte
    async fn read_exception_status(&mut self, slave_id: u8) -> Result<u8>;

    /// Diagnostics function (Function Code 0x08)
    ///
    /// # Arguments
    /// * `slave_id` - Target slave/unit identifier
    /// * `sub_function` - Diagnostic sub-function code
    /// * `data` - Sub-function specific data
    ///
    /// # Returns
    /// * `Result<Vec<u8>>` - Diagnostic response data
    async fn diagnostics(
        &mut self,
        slave_id: u8,
        sub_function: u16,
        data: &[u8],
    ) -> Result<Vec<u8>>;

    /// Get communication event counter (Function Code 0x0B)
    ///
    /// # Arguments
    /// * `slave_id` - Target slave/unit identifier
    ///
    /// # Returns
    /// * `Result<u16>` - Communication event counter
    async fn get_comm_event_counter(&mut self, slave_id: u8) -> Result<u16>;

    /// Get communication event log (Function Code 0x0C)
    ///
    /// # Arguments
    /// * `slave_id` - Target slave/unit identifier
    ///
    /// # Returns
    /// * `Result<Vec<u8>>` - Communication event log data
    async fn get_comm_event_log(&mut self, slave_id: u8) -> Result<Vec<u8>>;
}

/// High-level data type operations
///
/// This trait provides convenient methods for reading/writing common data types
/// without needing to handle register-level operations.
#[async_trait]
pub trait ModbusDataOperations: ModbusClient {
    /// Read a 32-bit float value from holding registers
    ///
    /// # Arguments
    /// * `slave_id` - Target slave/unit identifier
    /// * `address` - Starting register address
    /// * `byte_order` - Byte order for multi-register values
    ///
    /// # Returns
    /// * `Result<f32>` - Float value
    async fn read_float32(
        &mut self,
        slave_id: u8,
        address: u16,
        byte_order: super::common::ByteOrder,
    ) -> Result<f32>;

    /// Write a 32-bit float value to holding registers
    ///
    /// # Arguments
    /// * `slave_id` - Target slave/unit identifier
    /// * `address` - Starting register address
    /// * `value` - Float value to write
    /// * `byte_order` - Byte order for multi-register values
    ///
    /// # Returns
    /// * `Result<()>` - Success or error
    async fn write_float32(
        &mut self,
        slave_id: u8,
        address: u16,
        value: f32,
        byte_order: super::common::ByteOrder,
    ) -> Result<()>;

    /// Read a 64-bit float value from holding registers
    ///
    /// # Arguments
    /// * `slave_id` - Target slave/unit identifier
    /// * `address` - Starting register address
    /// * `byte_order` - Byte order for multi-register values
    ///
    /// # Returns
    /// * `Result<f64>` - Double value
    async fn read_float64(
        &mut self,
        slave_id: u8,
        address: u16,
        byte_order: super::common::ByteOrder,
    ) -> Result<f64>;

    /// Write a 64-bit float value to holding registers
    ///
    /// # Arguments
    /// * `slave_id` - Target slave/unit identifier
    /// * `address` - Starting register address
    /// * `value` - Double value to write
    /// * `byte_order` - Byte order for multi-register values
    ///
    /// # Returns
    /// * `Result<()>` - Success or error
    async fn write_float64(
        &mut self,
        slave_id: u8,
        address: u16,
        value: f64,
        byte_order: super::common::ByteOrder,
    ) -> Result<()>;

    /// Read a 32-bit integer value from holding registers
    ///
    /// # Arguments
    /// * `slave_id` - Target slave/unit identifier
    /// * `address` - Starting register address
    /// * `signed` - Whether to interpret as signed integer
    /// * `byte_order` - Byte order for multi-register values
    ///
    /// # Returns
    /// * `Result<i32>` or `Result<u32>` - Integer value
    async fn read_int32(
        &mut self,
        slave_id: u8,
        address: u16,
        signed: bool,
        byte_order: super::common::ByteOrder,
    ) -> Result<i32>;

    /// Write a 32-bit integer value to holding registers
    ///
    /// # Arguments
    /// * `slave_id` - Target slave/unit identifier
    /// * `address` - Starting register address
    /// * `value` - Integer value to write
    /// * `byte_order` - Byte order for multi-register values
    ///
    /// # Returns
    /// * `Result<()>` - Success or error
    async fn write_int32(
        &mut self,
        slave_id: u8,
        address: u16,
        value: i32,
        byte_order: super::common::ByteOrder,
    ) -> Result<()>;

    /// Read a string value from holding registers
    ///
    /// # Arguments
    /// * `slave_id` - Target slave/unit identifier
    /// * `address` - Starting register address
    /// * `length` - Number of characters to read
    /// * `encoding` - String encoding (ASCII, UTF-8, etc.)
    ///
    /// # Returns
    /// * `Result<String>` - String value
    async fn read_string(
        &mut self,
        slave_id: u8,
        address: u16,
        length: u16,
        encoding: StringEncoding,
    ) -> Result<String>;

    /// Write a string value to holding registers
    ///
    /// # Arguments
    /// * `slave_id` - Target slave/unit identifier
    /// * `address` - Starting register address
    /// * `value` - String value to write
    /// * `encoding` - String encoding (ASCII, UTF-8, etc.)
    ///
    /// # Returns
    /// * `Result<()>` - Success or error
    async fn write_string(
        &mut self,
        slave_id: u8,
        address: u16,
        value: &str,
        encoding: StringEncoding,
    ) -> Result<()>;
}

/// String encoding options for string operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StringEncoding {
    /// ASCII encoding (7-bit)
    Ascii,
    /// UTF-8 encoding
    Utf8,
    /// Latin-1 (ISO 8859-1) encoding
    Latin1,
    /// Custom encoding with specific charset
    Custom(&'static str),
}

impl Default for StringEncoding {
    fn default() -> Self {
        StringEncoding::Ascii
    }
}

/// Result type for batch operations
#[derive(Debug, Clone)]
pub struct BatchOperationResult<T> {
    /// Operation results (one per request)
    pub results: Vec<Result<T>>,
    /// Overall success status
    pub success: bool,
    /// Execution time in milliseconds
    pub execution_time_ms: u64,
    /// Number of successful operations
    pub successful_operations: usize,
    /// Number of failed operations
    pub failed_operations: usize,
}

impl<T> BatchOperationResult<T> {
    /// Create a new batch operation result
    pub fn new(results: Vec<Result<T>>, execution_time_ms: u64) -> Self {
        let successful_operations = results.iter().filter(|r| r.is_ok()).count();
        let failed_operations = results.len() - successful_operations;
        let success = failed_operations == 0;

        Self {
            results,
            success,
            execution_time_ms,
            successful_operations,
            failed_operations,
        }
    }

    /// Get success rate as percentage
    pub fn success_rate(&self) -> f64 {
        if self.results.is_empty() {
            0.0
        } else {
            (self.successful_operations as f64 / self.results.len() as f64) * 100.0
        }
    }

    /// Check if all operations succeeded
    pub fn all_succeeded(&self) -> bool {
        self.success
    }

    /// Get only successful results
    pub fn successful_results(&self) -> Vec<&T> {
        self.results
            .iter()
            .filter_map(|r| r.as_ref().ok())
            .collect()
    }
}
