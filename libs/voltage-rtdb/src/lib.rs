//! VoltageEMS Realtime Database Abstraction
//!
//! Provides a unified interface for realtime data storage,
//! supporting multiple backends (Redis, in-memory, etc.)

pub mod traits;

#[cfg(feature = "redis-backend")]
pub mod redis_impl;

pub mod memory_impl;

pub mod error;

pub mod cleanup;

// Re-exports
pub use traits::Rtdb;

#[cfg(feature = "redis-backend")]
pub use redis_impl::RedisRtdb;

pub use memory_impl::{MemoryRtdb, MemoryStats};

pub use cleanup::{cleanup_invalid_keys, CleanupProvider};
