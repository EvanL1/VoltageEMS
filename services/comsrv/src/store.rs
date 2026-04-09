//! Store module - DataStore implementations
//!
//! This module provides the bridge between protocol adapters and VoltageEMS storage.
//!
//! # Architecture
//!
//! ```text
//! Protocol Layer (with TransformConfig)
//!       ↓ poll_once() returns already-transformed DataBatch
//! RedisDataStore
//!       ↓
//! ┌─────────────────────────────────┐
//! │  voltage-routing (C2M/C2C)      │
//! │  voltage-rtdb (Redis access)    │
//! └─────────────────────────────────┘
//! ```
//!
//! Note: Data transformation (scale/offset/reverse) is handled by the protocol layer's
//! TransformConfig in poll_once(), so RedisDataStore receives pre-transformed values.

mod redis_store;
pub mod shm_redis_sync;

pub use redis_store::RedisDataStore;
pub use shm_redis_sync::ShmRedisSync;
