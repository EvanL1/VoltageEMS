//! State storage for stateful functions
//!
//! Functions like `integrate()` and `moving_avg()` need to persist state
//! between evaluations (last timestamp, window values, etc.).

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::error::Result;

/// State storage trait for stateful functions
///
/// Implementations can use Redis, in-memory storage, or other backends.
#[async_trait]
pub trait StateStore: Send + Sync {
    /// Get state for a key
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>>;

    /// Set state for a key
    async fn set(&self, key: &str, value: &[u8]) -> Result<()>;

    /// Delete state for a key
    async fn delete(&self, key: &str) -> Result<()>;
}

/// In-memory state store for testing and simple use cases
#[derive(Default)]
pub struct MemoryStateStore {
    data: RwLock<HashMap<String, Vec<u8>>>,
}

impl MemoryStateStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl StateStore for MemoryStateStore {
    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let data = self.data.read().await;
        Ok(data.get(key).cloned())
    }

    async fn set(&self, key: &str, value: &[u8]) -> Result<()> {
        let mut data = self.data.write().await;
        data.insert(key.to_string(), value.to_vec());
        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<()> {
        let mut data = self.data.write().await;
        data.remove(key);
        Ok(())
    }
}

/// Null state store - no persistence (stateful functions will fail)
pub struct NullStateStore;

#[async_trait]
impl StateStore for NullStateStore {
    async fn get(&self, _key: &str) -> Result<Option<Vec<u8>>> {
        Ok(None)
    }

    async fn set(&self, _key: &str, _value: &[u8]) -> Result<()> {
        Ok(())
    }

    async fn delete(&self, _key: &str) -> Result<()> {
        Ok(())
    }
}

// === State data structures for built-in functions ===

/// Integrate function state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrateState {
    /// Last timestamp (Unix seconds, f64 for precision)
    pub last_ts: f64,
    /// Accumulated value
    pub accumulated: f64,
}

/// Moving average function state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MovingAvgState {
    /// Circular buffer of recent values
    pub values: Vec<f64>,
    /// Next write position in buffer
    pub position: usize,
    /// Number of values stored (may be less than buffer size initially)
    pub count: usize,
}

impl MovingAvgState {
    pub fn new(window_size: usize) -> Self {
        Self {
            values: vec![0.0; window_size],
            position: 0,
            count: 0,
        }
    }

    /// Add a value and return the new moving average
    pub fn add(&mut self, value: f64) -> f64 {
        self.values[self.position] = value;
        self.position = (self.position + 1) % self.values.len();
        if self.count < self.values.len() {
            self.count += 1;
        }
        self.average()
    }

    /// Get current average
    pub fn average(&self) -> f64 {
        if self.count == 0 {
            return 0.0;
        }
        let sum: f64 = self.values.iter().take(self.count).sum();
        sum / self.count as f64
    }
}

/// Rate of change function state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateOfChangeState {
    /// Last timestamp (Unix seconds)
    pub last_ts: f64,
    /// Last value
    pub last_value: f64,
}

/// Helper function to create state key
///
/// Format: `calc:state:{context}:{func}:{var}`
pub fn state_key(context: &str, func: &str, var: &str) -> String {
    format!("calc:state:{}:{}:{}", context, func, var)
}

/// Shared state store reference
pub type SharedStateStore = Arc<dyn StateStore>;
