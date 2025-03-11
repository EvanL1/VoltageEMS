//! Modbus command batching for optimized communications
//!
//! This module provides batching functionality to group multiple Modbus commands
//! for more efficient communication with devices.

use crate::core::combase::RedisValue;
use std::collections::HashMap;

// Batch processing constants
pub const BATCH_WINDOW_MS: u64 = 20; // 20ms batch window
pub const MAX_BATCH_SIZE: usize = 100; // Maximum batch size

/// Command batch entry
#[derive(Debug, Clone)]

pub struct BatchCommand {
    pub point_id: u32,
    pub value: RedisValue,
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
