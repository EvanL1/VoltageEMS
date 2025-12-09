//! Built-in functions for expression evaluation
//!
//! Provides stateful functions: integrate, moving_avg, rate_of_change
//! And stateless functions: scale, clamp, abs, min, max

use crate::error::{CalcError, Result};
use crate::state::{
    state_key, IntegrateState, MovingAvgState, RateOfChangeState, SharedStateStore,
};
use chrono::Utc;
use tracing::debug;

/// Built-in function executor
///
/// Handles execution of stateful and stateless built-in functions.
pub struct BuiltinFunctions {
    /// State store for stateful functions
    state_store: SharedStateStore,
    /// Context identifier (e.g., rule_id, instance_id)
    context: String,
}

impl BuiltinFunctions {
    pub fn new(state_store: SharedStateStore, context: impl Into<String>) -> Self {
        Self {
            state_store,
            context: context.into(),
        }
    }

    /// Execute integrate function
    ///
    /// Calculates time integral: accumulated += value * dt
    /// Returns accumulated value (e.g., kWh from W)
    ///
    /// # Arguments
    /// * `var_name` - Variable name for state tracking
    /// * `value` - Current value to integrate
    /// * `unit_factor` - Conversion factor (default 1.0, use 1/3600 for W→Wh)
    pub async fn integrate(&self, var_name: &str, value: f64, unit_factor: f64) -> Result<f64> {
        let key = state_key(&self.context, "integrate", var_name);
        let now = Utc::now().timestamp() as f64;

        // Load existing state
        let state = if let Some(data) = self.state_store.get(&key).await? {
            serde_json::from_slice::<IntegrateState>(&data)
                .map_err(|e| CalcError::state(format!("Failed to deserialize state: {}", e)))?
        } else {
            // First call - initialize with current time, no accumulation yet
            let initial = IntegrateState {
                last_ts: now,
                accumulated: 0.0,
            };
            let data = serde_json::to_vec(&initial)
                .map_err(|e| CalcError::state(format!("Failed to serialize state: {}", e)))?;
            self.state_store.set(&key, &data).await?;
            return Ok(0.0); // First call returns 0
        };

        // Calculate dt (time delta in seconds)
        let dt = now - state.last_ts;
        if dt <= 0.0 {
            return Ok(state.accumulated);
        }

        // Integrate: accumulated += value * dt * unit_factor
        let delta = value * dt * unit_factor;
        let new_accumulated = state.accumulated + delta;

        debug!(
            var = var_name,
            value = value,
            dt = dt,
            delta = delta,
            accumulated = new_accumulated,
            "integrate"
        );

        // Save new state
        let new_state = IntegrateState {
            last_ts: now,
            accumulated: new_accumulated,
        };
        let data = serde_json::to_vec(&new_state)
            .map_err(|e| CalcError::state(format!("Failed to serialize state: {}", e)))?;
        self.state_store.set(&key, &data).await?;

        Ok(new_accumulated)
    }

    /// Execute moving average function
    ///
    /// Calculates moving average over a sliding window
    ///
    /// # Arguments
    /// * `var_name` - Variable name for state tracking
    /// * `value` - Current value to add
    /// * `window` - Window size (number of samples)
    pub async fn moving_avg(&self, var_name: &str, value: f64, window: usize) -> Result<f64> {
        let key = state_key(&self.context, "moving_avg", var_name);

        // Load or create state
        let mut state = if let Some(data) = self.state_store.get(&key).await? {
            let s: MovingAvgState = serde_json::from_slice(&data)
                .map_err(|e| CalcError::state(format!("Failed to deserialize state: {}", e)))?;
            // Handle window size change
            if s.values.len() != window {
                MovingAvgState::new(window)
            } else {
                s
            }
        } else {
            MovingAvgState::new(window)
        };

        // Add value and calculate average
        let avg = state.add(value);

        debug!(
            var = var_name,
            value = value,
            window = window,
            avg = avg,
            "moving_avg"
        );

        // Save state
        let data = serde_json::to_vec(&state)
            .map_err(|e| CalcError::state(format!("Failed to serialize state: {}", e)))?;
        self.state_store.set(&key, &data).await?;

        Ok(avg)
    }

    /// Execute rate of change function
    ///
    /// Calculates dv/dt (change rate per second)
    ///
    /// # Arguments
    /// * `var_name` - Variable name for state tracking
    /// * `value` - Current value
    pub async fn rate_of_change(&self, var_name: &str, value: f64) -> Result<f64> {
        let key = state_key(&self.context, "rate", var_name);
        let now = Utc::now().timestamp() as f64;

        // Load existing state
        let state = if let Some(data) = self.state_store.get(&key).await? {
            serde_json::from_slice::<RateOfChangeState>(&data)
                .map_err(|e| CalcError::state(format!("Failed to deserialize state: {}", e)))?
        } else {
            // First call - store current and return 0
            let initial = RateOfChangeState {
                last_ts: now,
                last_value: value,
            };
            let data = serde_json::to_vec(&initial)
                .map_err(|e| CalcError::state(format!("Failed to serialize state: {}", e)))?;
            self.state_store.set(&key, &data).await?;
            return Ok(0.0);
        };

        // Calculate rate
        let dt = now - state.last_ts;
        let rate = if dt > 0.0 {
            (value - state.last_value) / dt
        } else {
            0.0
        };

        debug!(
            var = var_name,
            value = value,
            last_value = state.last_value,
            dt = dt,
            rate = rate,
            "rate_of_change"
        );

        // Save new state
        let new_state = RateOfChangeState {
            last_ts: now,
            last_value: value,
        };
        let data = serde_json::to_vec(&new_state)
            .map_err(|e| CalcError::state(format!("Failed to serialize state: {}", e)))?;
        self.state_store.set(&key, &data).await?;

        Ok(rate)
    }

    /// Reset all states for this context
    pub async fn reset_states(&self) -> Result<()> {
        // This is a simplified implementation
        // In production, you'd want to iterate and delete all keys with the context prefix
        Ok(())
    }
}

// === Stateless functions (pure, no state needed) ===

/// Scale a value by a factor
pub fn scale(value: f64, factor: f64) -> f64 {
    value * factor
}

/// Clamp a value to a range
pub fn clamp(value: f64, min: f64, max: f64) -> f64 {
    value.clamp(min, max)
}

/// Absolute value
pub fn abs(value: f64) -> f64 {
    value.abs()
}

/// Minimum of two values
pub fn min(a: f64, b: f64) -> f64 {
    a.min(b)
}

/// Maximum of two values
pub fn max(a: f64, b: f64) -> f64 {
    a.max(b)
}

/// Round to specified decimal places
pub fn round(value: f64, decimals: i32) -> f64 {
    let factor = 10_f64.powi(decimals);
    (value * factor).round() / factor
}

/// Sign function: returns -1, 0, or 1
pub fn sign(value: f64) -> f64 {
    if value > 0.0 {
        1.0
    } else if value < 0.0 {
        -1.0
    } else {
        0.0
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
#[allow(clippy::approx_constant)]
mod tests {
    use super::*;
    use crate::state::MemoryStateStore;
    use std::sync::Arc;

    #[test]
    fn test_scale() {
        assert_eq!(scale(100.0, 0.5), 50.0);
        assert_eq!(scale(100.0, 2.0), 200.0);
    }

    #[test]
    fn test_clamp() {
        assert_eq!(clamp(50.0, 0.0, 100.0), 50.0);
        assert_eq!(clamp(-10.0, 0.0, 100.0), 0.0);
        assert_eq!(clamp(150.0, 0.0, 100.0), 100.0);
    }

    #[test]
    fn test_round() {
        assert_eq!(round(3.14159, 2), 3.14);
        assert_eq!(round(3.145, 2), 3.15);
        assert_eq!(round(3.14159, 0), 3.0);
    }

    #[test]
    fn test_moving_avg_state() {
        let mut state = MovingAvgState::new(3);
        assert_eq!(state.add(10.0), 10.0); // [10], avg=10
        assert_eq!(state.add(20.0), 15.0); // [10,20], avg=15
        assert_eq!(state.add(30.0), 20.0); // [10,20,30], avg=20
        assert_eq!(state.add(40.0), 30.0); // [40,20,30], avg=30 (overwrites 10)
    }

    #[tokio::test]
    async fn test_integrate_basic() {
        let store = Arc::new(MemoryStateStore::new());
        let funcs = BuiltinFunctions::new(store, "test");

        // First call initializes, returns 0
        let result = funcs.integrate("power", 1000.0, 1.0).await.unwrap();
        assert_eq!(result, 0.0);
    }

    #[tokio::test]
    async fn test_moving_avg_async() {
        let store = Arc::new(MemoryStateStore::new());
        let funcs = BuiltinFunctions::new(store, "test");

        let _ = funcs.moving_avg("temp", 10.0, 3).await.unwrap();
        let _ = funcs.moving_avg("temp", 20.0, 3).await.unwrap();
        let avg = funcs.moving_avg("temp", 30.0, 3).await.unwrap();
        assert_eq!(avg, 20.0); // (10+20+30)/3
    }

    #[tokio::test]
    async fn test_rate_of_change_basic() {
        let store = Arc::new(MemoryStateStore::new());
        let funcs = BuiltinFunctions::new(store, "test");

        // First call returns 0
        let rate = funcs.rate_of_change("voltage", 100.0).await.unwrap();
        assert_eq!(rate, 0.0);
    }
}
