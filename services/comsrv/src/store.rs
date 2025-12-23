//! Store module - IGW DataStore implementations
//!
//! This module provides the bridge between IGW protocols and VoltageEMS storage.
//!
//! # Architecture
//!
//! ```text
//! IGW Protocol Layer (with TransformConfig)
//!       ↓ poll_once() returns already-transformed DataBatch
//! RedisDataStore
//!       ↓
//! ┌─────────────────────────────────┐
//! │  voltage-routing (C2M/C2C)      │
//! │  voltage-rtdb (Redis access)    │
//! └─────────────────────────────────┘
//! ```
//!
//! Note: Data transformation (scale/offset/reverse) is now handled by IGW's
//! TransformConfig in poll_once(), so RedisDataStore receives pre-transformed values.

mod redis_store;

pub use redis_store::RedisDataStore;
