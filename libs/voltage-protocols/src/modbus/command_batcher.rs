//! Modbus command batching for optimized communications
//!
//! This module provides batching functionality to group multiple Modbus commands
//! for more efficient communication with devices.

use std::collections::HashMap;
use voltage_comlink::ProtocolValue;

// Batch processing constants
pub const BATCH_WINDOW_MS: u64 = 20; // 20ms batch window
pub const MAX_BATCH_SIZE: usize = 100; // Maximum batch size

/// Command batch entry
#[derive(Debug, Clone)]

pub struct BatchCommand {
    pub point_id: u32,
    pub value: ProtocolValue,
    pub slave_id: u8,
    pub function_code: u8,
    pub register_address: u16,
    pub data_type: String,
    pub byte_order: Option<String>,
}

/// Command batcher for optimizing Modbus communications
#[derive(Debug)]
pub struct CommandBatcher {
    /// Pending commands grouped by (slave_id, function_code)
    pending_commands: HashMap<(u8, u8), Vec<BatchCommand>>,
    /// Last batch execution time
    last_batch_time: tokio::time::Instant,
    /// Total pending commands count
    total_pending: usize,
}

impl CommandBatcher {
    pub fn new() -> Self {
        Self {
            pending_commands: HashMap::new(),
            last_batch_time: tokio::time::Instant::now(),
            total_pending: 0,
        }
    }

    /// Get the number of pending commands
    pub fn pending_count(&self) -> usize {
        self.total_pending
    }

    /// Get time elapsed since last batch
    pub fn elapsed_since_last_batch(&self) -> tokio::time::Duration {
        self.last_batch_time.elapsed()
    }

    /// Check if batch should be executed
    pub fn should_execute(&self) -> bool {
        // Execute if time window exceeded or batch size limit reached
        self.last_batch_time.elapsed().as_millis() >= BATCH_WINDOW_MS as u128
            || self.total_pending >= MAX_BATCH_SIZE
    }

    /// Take all pending commands and reset
    pub fn take_commands(&mut self) -> HashMap<(u8, u8), Vec<BatchCommand>> {
        self.last_batch_time = tokio::time::Instant::now();
        self.total_pending = 0;
        std::mem::take(&mut self.pending_commands)
    }

    /// Add a command to the pending batch
    pub fn add_command(&mut self, command: BatchCommand) {
        let key = (command.slave_id, command.function_code);
        self.pending_commands.entry(key).or_default().push(command);
        self.total_pending += 1;
    }

    /// Check if registers are strictly consecutive (for FC16 batch write)
    pub fn are_strictly_consecutive(commands: &[BatchCommand]) -> bool {
        if commands.len() < 2 {
            return false;
        }

        let mut sorted = commands.to_vec();
        sorted.sort_by_key(|c| c.register_address);

        let mut expected_addr = sorted[0].register_address;

        for cmd in &sorted {
            if cmd.register_address != expected_addr {
                return false; // Not consecutive
            }
            // Calculate registers used by this data type
            expected_addr += Self::get_register_count(&cmd.data_type);
        }
        true
    }

    /// Get number of 16-bit registers used by a data type
    pub fn get_register_count(data_type: &str) -> u16 {
        match data_type {
            "uint16" | "int16" | "bool" => 1,
            "uint32" | "int32" | "float32" => 2,
            "uint64" | "int64" | "float64" => 4,
            _ => 1,
        }
    }
}

impl Default for CommandBatcher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;

    /// Helper function to create a test BatchCommand
    fn create_test_command(
        point_id: u32,
        slave_id: u8,
        function_code: u8,
        register_address: u16,
        data_type: &str,
    ) -> BatchCommand {
        BatchCommand {
            point_id,
            value: ProtocolValue::Float(100.0),
            slave_id,
            function_code,
            register_address,
            data_type: data_type.to_string(),
            byte_order: None,
        }
    }

    // ========== new() tests ==========

    #[test]
    fn test_new_creates_empty_batcher() {
        let batcher = CommandBatcher::new();

        assert_eq!(batcher.pending_count(), 0);
        assert!(batcher.pending_commands.is_empty());
    }

    #[test]
    fn test_default_is_equivalent_to_new() {
        let batcher1 = CommandBatcher::new();
        let batcher2 = CommandBatcher::default();

        assert_eq!(batcher1.pending_count(), batcher2.pending_count());
    }

    // ========== pending_count() tests ==========

    #[test]
    fn test_pending_count_after_add() {
        let mut batcher = CommandBatcher::new();

        batcher.add_command(create_test_command(1, 1, 6, 100, "uint16"));
        assert_eq!(batcher.pending_count(), 1);

        batcher.add_command(create_test_command(2, 1, 6, 101, "uint16"));
        assert_eq!(batcher.pending_count(), 2);
    }

    #[test]
    fn test_pending_count_resets_after_take() {
        let mut batcher = CommandBatcher::new();

        batcher.add_command(create_test_command(1, 1, 6, 100, "uint16"));
        batcher.add_command(create_test_command(2, 1, 6, 101, "uint16"));
        assert_eq!(batcher.pending_count(), 2);

        let _ = batcher.take_commands();
        assert_eq!(batcher.pending_count(), 0);
    }

    // ========== should_execute() tests ==========

    #[test]
    fn test_should_execute_false_when_empty_and_recent() {
        let batcher = CommandBatcher::new();

        // Just created, should not execute immediately
        assert!(!batcher.should_execute());
    }

    #[test]
    fn test_should_execute_true_at_max_batch_size() {
        let mut batcher = CommandBatcher::new();

        // Add MAX_BATCH_SIZE commands
        for i in 0..MAX_BATCH_SIZE {
            batcher.add_command(create_test_command(i as u32, 1, 6, i as u16, "uint16"));
        }

        assert!(batcher.should_execute());
    }

    #[tokio::test]
    async fn test_should_execute_true_after_time_window() {
        let mut batcher = CommandBatcher::new();
        batcher.add_command(create_test_command(1, 1, 6, 100, "uint16"));

        // Wait for batch window to expire
        tokio::time::sleep(tokio::time::Duration::from_millis(BATCH_WINDOW_MS + 5)).await;

        assert!(batcher.should_execute());
    }

    // ========== take_commands() tests ==========

    #[test]
    fn test_take_commands_returns_all_pending() {
        let mut batcher = CommandBatcher::new();

        batcher.add_command(create_test_command(1, 1, 6, 100, "uint16"));
        batcher.add_command(create_test_command(2, 1, 6, 101, "uint16"));
        batcher.add_command(create_test_command(3, 2, 6, 200, "uint16"));

        let commands = batcher.take_commands();

        // Commands should be grouped by (slave_id, function_code)
        assert_eq!(commands.len(), 2); // Two groups: (1, 6) and (2, 6)
        assert_eq!(commands.get(&(1, 6)).map(|v| v.len()), Some(2));
        assert_eq!(commands.get(&(2, 6)).map(|v| v.len()), Some(1));
    }

    #[test]
    fn test_take_commands_empties_batcher() {
        let mut batcher = CommandBatcher::new();

        batcher.add_command(create_test_command(1, 1, 6, 100, "uint16"));
        let _ = batcher.take_commands();

        assert!(batcher.pending_commands.is_empty());
        assert_eq!(batcher.pending_count(), 0);
    }

    // ========== add_command() tests ==========

    #[test]
    fn test_add_command_groups_by_slave_and_function() {
        let mut batcher = CommandBatcher::new();

        // Same slave, same function code
        batcher.add_command(create_test_command(1, 1, 6, 100, "uint16"));
        batcher.add_command(create_test_command(2, 1, 6, 101, "uint16"));

        // Different slave
        batcher.add_command(create_test_command(3, 2, 6, 100, "uint16"));

        // Different function code
        batcher.add_command(create_test_command(4, 1, 16, 100, "uint16"));

        let commands = batcher.take_commands();

        assert_eq!(commands.len(), 3); // (1,6), (2,6), (1,16)
        assert_eq!(commands.get(&(1, 6)).map(|v| v.len()), Some(2));
        assert_eq!(commands.get(&(2, 6)).map(|v| v.len()), Some(1));
        assert_eq!(commands.get(&(1, 16)).map(|v| v.len()), Some(1));
    }

    #[test]
    fn test_add_command_preserves_all_fields() {
        let mut batcher = CommandBatcher::new();

        let cmd = BatchCommand {
            point_id: 42,
            value: ProtocolValue::Float(123.456),
            slave_id: 5,
            function_code: 16,
            register_address: 999,
            data_type: "float32".to_string(),
            byte_order: Some("big".to_string()),
        };

        batcher.add_command(cmd.clone());
        let commands = batcher.take_commands();

        let stored = &commands.get(&(5, 16)).unwrap()[0];
        assert_eq!(stored.point_id, 42);
        assert_eq!(stored.slave_id, 5);
        assert_eq!(stored.function_code, 16);
        assert_eq!(stored.register_address, 999);
        assert_eq!(stored.data_type, "float32");
        assert_eq!(stored.byte_order, Some("big".to_string()));
    }

    // ========== are_strictly_consecutive() tests ==========

    #[test]
    fn test_consecutive_single_command_returns_false() {
        let commands = vec![create_test_command(1, 1, 6, 100, "uint16")];
        assert!(!CommandBatcher::are_strictly_consecutive(&commands));
    }

    #[test]
    fn test_consecutive_empty_returns_false() {
        let commands: Vec<BatchCommand> = vec![];
        assert!(!CommandBatcher::are_strictly_consecutive(&commands));
    }

    #[test]
    fn test_consecutive_uint16_sequence() {
        // uint16 uses 1 register each
        let commands = vec![
            create_test_command(1, 1, 6, 100, "uint16"),
            create_test_command(2, 1, 6, 101, "uint16"),
            create_test_command(3, 1, 6, 102, "uint16"),
        ];
        assert!(CommandBatcher::are_strictly_consecutive(&commands));
    }

    #[test]
    fn test_consecutive_float32_sequence() {
        // float32 uses 2 registers each
        let commands = vec![
            create_test_command(1, 1, 6, 100, "float32"),
            create_test_command(2, 1, 6, 102, "float32"),
            create_test_command(3, 1, 6, 104, "float32"),
        ];
        assert!(CommandBatcher::are_strictly_consecutive(&commands));
    }

    #[test]
    fn test_consecutive_mixed_types() {
        // Mixed: uint16(1 reg) + float32(2 regs) + uint16(1 reg)
        let commands = vec![
            create_test_command(1, 1, 6, 100, "uint16"),
            create_test_command(2, 1, 6, 101, "float32"),
            create_test_command(3, 1, 6, 103, "uint16"),
        ];
        assert!(CommandBatcher::are_strictly_consecutive(&commands));
    }

    #[test]
    fn test_non_consecutive_gap() {
        let commands = vec![
            create_test_command(1, 1, 6, 100, "uint16"),
            create_test_command(2, 1, 6, 105, "uint16"), // Gap at 101-104
        ];
        assert!(!CommandBatcher::are_strictly_consecutive(&commands));
    }

    #[test]
    fn test_consecutive_out_of_order_input() {
        // Registers should be sorted internally
        let commands = vec![
            create_test_command(3, 1, 6, 102, "uint16"),
            create_test_command(1, 1, 6, 100, "uint16"),
            create_test_command(2, 1, 6, 101, "uint16"),
        ];
        assert!(CommandBatcher::are_strictly_consecutive(&commands));
    }

    // ========== get_register_count() tests ==========

    #[test]
    fn test_register_count_16bit_types() {
        assert_eq!(CommandBatcher::get_register_count("uint16"), 1);
        assert_eq!(CommandBatcher::get_register_count("int16"), 1);
        assert_eq!(CommandBatcher::get_register_count("bool"), 1);
    }

    #[test]
    fn test_register_count_32bit_types() {
        assert_eq!(CommandBatcher::get_register_count("uint32"), 2);
        assert_eq!(CommandBatcher::get_register_count("int32"), 2);
        assert_eq!(CommandBatcher::get_register_count("float32"), 2);
    }

    #[test]
    fn test_register_count_64bit_types() {
        assert_eq!(CommandBatcher::get_register_count("uint64"), 4);
        assert_eq!(CommandBatcher::get_register_count("int64"), 4);
        assert_eq!(CommandBatcher::get_register_count("float64"), 4);
    }

    #[test]
    fn test_register_count_unknown_defaults_to_one() {
        assert_eq!(CommandBatcher::get_register_count("unknown"), 1);
        assert_eq!(CommandBatcher::get_register_count(""), 1);
    }

    // ========== elapsed_since_last_batch() tests ==========

    #[tokio::test]
    async fn test_elapsed_increases_over_time() {
        let batcher = CommandBatcher::new();
        let initial = batcher.elapsed_since_last_batch();

        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let later = batcher.elapsed_since_last_batch();
        assert!(later > initial);
    }

    #[tokio::test]
    async fn test_elapsed_resets_after_take() {
        let mut batcher = CommandBatcher::new();

        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        let before_take = batcher.elapsed_since_last_batch();

        let _ = batcher.take_commands();
        let after_take = batcher.elapsed_since_last_batch();

        assert!(after_take < before_take);
    }

    // ========== Integration-style tests ==========

    #[test]
    fn test_batch_workflow() {
        let mut batcher = CommandBatcher::new();

        // Initially empty
        assert_eq!(batcher.pending_count(), 0);

        // Add commands
        for i in 0..5 {
            batcher.add_command(create_test_command(i, 1, 6, 100 + i as u16, "uint16"));
        }
        assert_eq!(batcher.pending_count(), 5);

        // Take and verify
        let batch = batcher.take_commands();
        assert_eq!(batch.get(&(1, 6)).unwrap().len(), 5);

        // Should be empty after take
        assert_eq!(batcher.pending_count(), 0);
        assert!(batcher.pending_commands.is_empty());
    }
}
