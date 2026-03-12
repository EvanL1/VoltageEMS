//! State storage for stateful functions
//!
//! Functions like `integrate()` and `moving_avg()` need to persist state
//! between evaluations (last timestamp, window values, etc.).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::future::Future;
use tokio::sync::RwLock;

use crate::error::Result;

/// State storage trait for stateful functions
///
/// Implementations can use Redis, in-memory storage, or other backends.
pub trait StateStore: Send + Sync {
    /// Get state for a key
    fn get(&self, key: &str) -> impl Future<Output = Result<Option<Vec<u8>>>> + Send;

    /// Set state for a key
    fn set(&self, key: &str, value: &[u8]) -> impl Future<Output = Result<()>> + Send;

    /// Delete state for a key
    fn delete(&self, key: &str) -> impl Future<Output = Result<()>> + Send;
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

impl StateStore for MemoryStateStore {
    fn get(&self, key: &str) -> impl Future<Output = Result<Option<Vec<u8>>>> + Send {
        let key = key.to_string();
        async move {
            let data = self.data.read().await;
            Ok(data.get(&key).cloned())
        }
    }

    fn set(&self, key: &str, value: &[u8]) -> impl Future<Output = Result<()>> + Send {
        let key = key.to_string();
        let value = value.to_vec();
        async move {
            let mut data = self.data.write().await;
            data.insert(key, value);
            Ok(())
        }
    }

    fn delete(&self, key: &str) -> impl Future<Output = Result<()>> + Send {
        let key = key.to_string();
        async move {
            let mut data = self.data.write().await;
            data.remove(&key);
            Ok(())
        }
    }
}

// === Redis-backed state store for production ===

use bytes::Bytes;
use std::sync::Arc;
use voltage_rtdb::Rtdb;

use crate::error::CalcError;

/// Redis-backed state store using Rtdb trait
///
/// This provides persistent storage for stateful functions like `integrate()`,
/// `moving_avg()`, `rate_of_change()`, and `period_delta()`.
///
/// # Performance
/// - Typical latency: ~1ms per operation
/// - For high-frequency calls, consider using MemoryStateStore with periodic sync
///
/// # Example
/// ```ignore
/// use voltage_rtdb::RedisRtdb;
/// use voltage_calc::state::RtdbStateStore;
///
/// let rtdb = Arc::new(RedisRtdb::new(redis_pool).await?);
/// let state_store = RtdbStateStore::new(rtdb);
/// ```
pub struct RtdbStateStore<R: Rtdb> {
    rtdb: Arc<R>,
}

impl<R: Rtdb> RtdbStateStore<R> {
    /// Create a new Redis-backed state store
    pub fn new(rtdb: Arc<R>) -> Self {
        Self { rtdb }
    }
}

impl<R: Rtdb> StateStore for RtdbStateStore<R> {
    fn get(&self, key: &str) -> impl Future<Output = Result<Option<Vec<u8>>>> + Send {
        let rtdb = self.rtdb.clone();
        let key = key.to_string();
        async move {
            match rtdb.get(&key).await {
                Ok(Some(bytes)) => Ok(Some(bytes.to_vec())),
                Ok(None) => Ok(None),
                Err(e) => Err(CalcError::state(format!("Redis get failed: {}", e))),
            }
        }
    }

    fn set(&self, key: &str, value: &[u8]) -> impl Future<Output = Result<()>> + Send {
        let rtdb = self.rtdb.clone();
        let key = key.to_string();
        let value = Bytes::copy_from_slice(value);
        async move {
            rtdb.set(&key, value)
                .await
                .map_err(|e| CalcError::state(format!("Redis set failed: {}", e)))
        }
    }

    fn delete(&self, key: &str) -> impl Future<Output = Result<()>> + Send {
        let rtdb = self.rtdb.clone();
        let key = key.to_string();
        async move {
            rtdb.del(&key)
                .await
                .map(|_| ()) // Ignore the bool return value
                .map_err(|e| CalcError::state(format!("Redis del failed: {}", e)))
        }
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

/// Period delta function state
///
/// Tracks the snapshot value at the start of each period (daily, weekly, monthly, quarterly)
/// to calculate the delta (change) within the current period.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeriodDeltaState {
    /// Snapshot value at period start (cumulative counter reading)
    pub snapshot: f64,
    /// Period start timestamp (Unix seconds)
    pub period_start_ts: i64,
}

/// Helper function to create state key
///
/// Format: `calc:state:{context}:{func}:{var}`
pub fn state_key(context: &str, func: &str, var: &str) -> String {
    format!("calc:state:{}:{}:{}", context, func, var)
}
