//! Store module - IGW DataStore implementations
//!
//! This module provides the bridge between IGW protocols and VoltageEMS storage.
//!
//! # Architecture
//!
//! ```text
//! IGW Protocol Layer
//!       ↓ igw::DataStore trait
//! RedisDataStore
//!       ↓
//! ┌─────────────────────────────────┐
//! │  voltage-routing (C2M/C2C)      │
//! │  voltage-rtdb (Redis access)    │
//! │  RuntimeConfigProvider (xform)  │
//! └─────────────────────────────────┘
//! ```

mod redis_store;

pub use redis_store::RedisDataStore;
