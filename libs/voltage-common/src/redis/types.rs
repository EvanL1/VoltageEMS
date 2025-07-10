//! Common Redis types and enums

use serde::{Deserialize, Serialize};

/// Redis data type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RedisType {
    String,
    List,
    Set,
    Hash,
    ZSet,
    Stream,
    None,
}

impl RedisType {
    /// Convert from Redis type string
    pub fn from_redis_string(s: &str) -> Self {
        match s {
            "string" => RedisType::String,
            "list" => RedisType::List,
            "set" => RedisType::Set,
            "hash" | "zset" => RedisType::Hash,
            "stream" => RedisType::Stream,
            _ => RedisType::None,
        }
    }
}

/// Connection state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Error,
}

/// Redis operation statistics
#[derive(Debug, Clone, Default)]
pub struct RedisStats {
    pub total_commands: u64,
    pub failed_commands: u64,
    pub total_connections: u64,
    pub failed_connections: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
}

impl RedisStats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_command(&mut self, success: bool) {
        self.total_commands += 1;
        if !success {
            self.failed_commands += 1;
        }
    }

    pub fn record_connection(&mut self, success: bool) {
        self.total_connections += 1;
        if !success {
            self.failed_connections += 1;
        }
    }
}
