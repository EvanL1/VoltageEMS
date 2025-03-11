//! Time Series Calculation Module
//!
//! Provides time-series calculations for virtual points including:
//! - Delta (difference calculations for daily/monthly/yearly usage)
//! - Moving Average (data smoothing)
//! - Peak/Valley Detection (max/min values in periods)
//! - Integration (accumulation like power to energy)

use anyhow::{Context, Result};
use bytes::Bytes;
use chrono::{DateTime, Local};
use cron::Schedule;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use tracing::{debug, trace};
use voltage_rtdb::Rtdb;

/// Time series function types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimeSeriesFunction {
    /// Delta calculation - compute difference from baseline
    Delta {
        schedule: String, // Cron expression for reset timing
    },
    /// Moving average - smooth data over a window
    MovingAverage {
        window_minutes: u32, // Window size in minutes
    },
    /// Peak detection - track maximum value in period
    Peak {
        schedule: String, // Cron expression for reset timing
    },
    /// Valley detection - track minimum value in period  
    Valley {
        schedule: String, // Cron expression for reset timing
    },
    /// Integration - accumulate values over time
    Integration {
        reset_schedule: Option<String>, // Optional reset schedule
    },
}

/// Data interval representation
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct DataInterval {
    pub start: DateTime<Local>,
    pub end: DateTime<Local>,
    pub is_complete: bool,
}

/// Manages data intervals and boundary detection using cron expressions
#[derive(Default)]
#[allow(dead_code)]
pub struct DataIntervalManager {
    schedules: HashMap<String, Schedule>,
    last_intervals: HashMap<String, DataInterval>,
}

#[allow(dead_code)]
impl DataIntervalManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get or create a cron schedule
    fn get_schedule(&mut self, schedule_expr: &str) -> Result<&Schedule> {
        if !self.schedules.contains_key(schedule_expr) {
            let schedule = Schedule::from_str(schedule_expr)
                .context(format!("Invalid cron expression: {}", schedule_expr))?;
            self.schedules.insert(schedule_expr.to_string(), schedule);
        }

        match self.schedules.get(schedule_expr) {
            Some(schedule) => Ok(schedule),
            None => {
                tracing::error!(
                    "Failed to retrieve schedule after insertion: {}",
                    schedule_expr
                );
                Err(anyhow::anyhow!(
                    "Internal error: schedule not found after insertion"
                ))
            },
        }
    }

    /// Get current data interval based on cron schedule
    pub fn get_current_interval(
        &mut self,
        _point_id: &str,
        schedule_expr: &str,
        now: DateTime<Local>,
    ) -> Result<DataInterval> {
        let schedule = self.get_schedule(schedule_expr)?;

        // Find previous and next schedule points
        let prev = schedule
            .upcoming(Local)
            .take_while(|&dt| dt < now)
            .last()
            .unwrap_or_else(|| now - chrono::Duration::days(1));

        let next = schedule
            .upcoming(Local)
            .find(|&dt| dt >= now)
            .unwrap_or_else(|| now + chrono::Duration::days(1));

        Ok(DataInterval {
            start: prev,
            end: next,
            is_complete: now >= next,
        })
    }

    /// Check if boundary has been crossed
    pub fn has_crossed_boundary(
        &mut self,
        point_id: &str,
        schedule_expr: &str,
        now: DateTime<Local>,
    ) -> Result<bool> {
        let current = self.get_current_interval(point_id, schedule_expr, now)?;

        if let Some(last) = self.last_intervals.get(point_id) {
            if current.start != last.start {
                debug!(
                    "Boundary crossed for {}: {:?} -> {:?}",
                    point_id, last.start, current.start
                );
                self.last_intervals.insert(point_id.to_string(), current);
                return Ok(true);
            }
        } else {
            self.last_intervals.insert(point_id.to_string(), current);
        }

        Ok(false)
    }
}

/// Time series calculator for various calculations
///
/// Provides advanced time-series calculations including:
/// - Delta calculations (difference from baseline)
/// - Moving averages over configurable windows
/// - Peak/valley detection with schedule-based resets
/// - Integration/accumulation with automatic resets
///
/// NOTE: Will be used by VirtualCalculator when real-time processing is re-enabled
#[allow(dead_code)]
pub struct TimeSeriesCalculator<R: Rtdb> {
    interval_manager: DataIntervalManager,
    rtdb: Arc<R>,
}

#[allow(dead_code)]
impl<R: Rtdb> TimeSeriesCalculator<R> {
    pub fn new(rtdb: Arc<R>) -> Self {
        Self {
            interval_manager: DataIntervalManager::new(),
            rtdb,
        }
    }

    /// Main calculation entry point
    pub async fn calculate(
        &mut self,
        model_id: &str,
        point_id: &str,
        source_value: f64,
        function: &TimeSeriesFunction,
    ) -> Result<f64> {
        match function {
            TimeSeriesFunction::Delta { schedule } => {
                self.calculate_delta(model_id, point_id, source_value, schedule)
                    .await
            },
            TimeSeriesFunction::MovingAverage { window_minutes } => {
                self.calculate_moving_avg(model_id, point_id, source_value, *window_minutes)
                    .await
            },
            TimeSeriesFunction::Peak { schedule } => {
                self.calculate_peak(model_id, point_id, source_value, schedule)
                    .await
            },
            TimeSeriesFunction::Valley { schedule } => {
                self.calculate_valley(model_id, point_id, source_value, schedule)
                    .await
            },
            TimeSeriesFunction::Integration { reset_schedule } => {
                self.calculate_integration(
                    model_id,
                    point_id,
                    source_value,
                    reset_schedule.as_deref(),
                )
                .await
            },
        }
    }

    /// Calculate delta (difference from baseline)
    async fn calculate_delta(
        &mut self,
        model_id: &str,
        point_id: &str,
        source_value: f64,
        schedule: &str,
    ) -> Result<f64> {
        let now = Local::now();

        // Check if boundary crossed
        if self
            .interval_manager
            .has_crossed_boundary(point_id, schedule, now)?
        {
            // Save snapshot and reset
            let snapshot_key = format!("modsrv:ts:snapshot:{}:{}", model_id, point_id);

            // Get previous snapshot value before resetting
            let prev_value: Option<f64> = match self.rtdb.get(&snapshot_key).await? {
                Some(bytes) => String::from_utf8_lossy(&bytes).parse().ok(),
                None => None,
            };

            // Save new snapshot
            self.rtdb
                .set(&snapshot_key, Bytes::from(source_value.to_string()))
                .await?;

            // Calculate delta from previous interval
            if let Some(prev) = prev_value {
                trace!("Delta reset for {}: {} -> {}", point_id, prev, source_value);
                return Ok(source_value - prev);
            }

            return Ok(0.0);
        }

        // Within same interval, calculate from snapshot
        let snapshot_key = format!("modsrv:ts:snapshot:{}:{}", model_id, point_id);
        let snapshot_value: f64 = match self.rtdb.get(&snapshot_key).await? {
            Some(bytes) => String::from_utf8_lossy(&bytes)
                .parse()
                .unwrap_or(source_value),
            None => source_value,
        };

        Ok(source_value - snapshot_value)
    }

    /// Calculate moving average
    async fn calculate_moving_avg(
        &mut self,
        model_id: &str,
        point_id: &str,
        value: f64,
        window_minutes: u32,
    ) -> Result<f64> {
        let buffer_key = format!("modsrv:ts:buffer:{}:{}", model_id, point_id);

        // Add new value to buffer (left push)
        self.rtdb
            .list_lpush(&buffer_key, Bytes::from(value.to_string()))
            .await?;

        // Trim buffer to window size
        let max_size = window_minutes as isize;
        self.rtdb.list_trim(&buffer_key, 0, max_size - 1).await?;

        // Get all values and calculate average
        let values_bytes = self.rtdb.list_range(&buffer_key, 0, -1).await?;
        let values: Vec<f64> = values_bytes
            .iter()
            .filter_map(|b| String::from_utf8_lossy(b).parse().ok())
            .collect();

        if values.is_empty() {
            return Ok(value);
        }

        let sum: f64 = values.iter().sum();
        Ok(sum / values.len() as f64)
    }

    /// Calculate peak (maximum value)
    async fn calculate_peak(
        &mut self,
        model_id: &str,
        point_id: &str,
        value: f64,
        schedule: &str,
    ) -> Result<f64> {
        let now = Local::now();
        let peak_key = format!("modsrv:ts:peak:{}:{}", model_id, point_id);

        // Check if boundary crossed (reset needed)
        if self
            .interval_manager
            .has_crossed_boundary(point_id, schedule, now)?
        {
            // Reset peak to current value
            self.rtdb
                .set(&peak_key, Bytes::from(value.to_string()))
                .await?;
            debug!("Peak reset for {}: {}", point_id, value);
            return Ok(value);
        }

        // Get current peak
        let current_peak: Option<f64> = match self.rtdb.get(&peak_key).await? {
            Some(bytes) => String::from_utf8_lossy(&bytes).parse().ok(),
            None => None,
        };

        let new_peak = match current_peak {
            Some(peak) => value.max(peak),
            None => value,
        };

        // Update if new peak found
        if current_peak.is_none() || current_peak.is_some_and(|peak| new_peak > peak) {
            self.rtdb
                .set(&peak_key, Bytes::from(new_peak.to_string()))
                .await?;
            trace!("New peak for {}: {}", point_id, new_peak);
        }

        Ok(new_peak)
    }

    /// Calculate valley (minimum value)
    async fn calculate_valley(
        &mut self,
        model_id: &str,
        point_id: &str,
        value: f64,
        schedule: &str,
    ) -> Result<f64> {
        let now = Local::now();
        let valley_key = format!("modsrv:ts:valley:{}:{}", model_id, point_id);

        // Check if boundary crossed (reset needed)
        if self
            .interval_manager
            .has_crossed_boundary(point_id, schedule, now)?
        {
            // Reset valley to current value
            self.rtdb
                .set(&valley_key, Bytes::from(value.to_string()))
                .await?;
            debug!("Valley reset for {}: {}", point_id, value);
            return Ok(value);
        }

        // Get current valley
        let current_valley: Option<f64> = match self.rtdb.get(&valley_key).await? {
            Some(bytes) => String::from_utf8_lossy(&bytes).parse().ok(),
            None => None,
        };

        let new_valley = match current_valley {
            Some(valley) => value.min(valley),
            None => value,
        };

        // Update if new valley found
        if current_valley.is_none() || current_valley.is_some_and(|valley| new_valley < valley) {
            self.rtdb
                .set(&valley_key, Bytes::from(new_valley.to_string()))
                .await?;
            trace!("New valley for {}: {}", point_id, new_valley);
        }

        Ok(new_valley)
    }

    /// Calculate integration (accumulation)
    async fn calculate_integration(
        &mut self,
        model_id: &str,
        point_id: &str,
        value: f64,
        reset_schedule: Option<&str>,
    ) -> Result<f64> {
        let now = Local::now();
        let integral_key = format!("modsrv:ts:integral:{}:{}", model_id, point_id);
        let timestamp_key = format!("modsrv:ts:integral:{}:{}:ts", model_id, point_id);

        // Check for reset if schedule provided
        if let Some(schedule) = reset_schedule {
            if self
                .interval_manager
                .has_crossed_boundary(point_id, schedule, now)?
            {
                // Reset integral
                self.rtdb.del(&integral_key).await?;
                self.rtdb.del(&timestamp_key).await?;
                debug!("Integration reset for {}", point_id);
            }
        }

        // Get last timestamp and integral value
        let last_timestamp: Option<i64> = match self.rtdb.get(&timestamp_key).await? {
            Some(bytes) => String::from_utf8_lossy(&bytes).parse().ok(),
            None => None,
        };
        let current_integral: f64 = match self.rtdb.get(&integral_key).await? {
            Some(bytes) => String::from_utf8_lossy(&bytes).parse().unwrap_or(0.0),
            None => 0.0,
        };

        let new_integral = if let Some(last_ts) = last_timestamp {
            // Calculate time delta in seconds
            let last_time = DateTime::from_timestamp(last_ts, 0)
                .map(|dt| dt.with_timezone(&Local))
                .unwrap_or(now);
            let time_delta = now.signed_duration_since(last_time).num_seconds() as f64;

            if time_delta > 0.0 {
                // Use trapezoidal rule for better accuracy
                // Assuming linear change between samples
                current_integral + (value * time_delta / 3600.0) // Convert to hours for kWh
            } else {
                current_integral
            }
        } else {
            // First value, start integration
            0.0
        };

        // Save new integral and timestamp
        self.rtdb
            .set(&integral_key, Bytes::from(new_integral.to_string()))
            .await?;
        self.rtdb
            .set(&timestamp_key, Bytes::from(now.timestamp().to_string()))
            .await?;

        Ok(new_integral)
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;

    #[test]
    fn test_cron_parsing() {
        // Cron format: second minute hour day month weekday year
        let daily = "0 0 0 * * * *"; // Daily at 00:00:00
        let schedule = Schedule::from_str(daily).unwrap();
        assert!(schedule.upcoming(Local).next().is_some());
    }

    #[test]
    fn test_time_series_function_serialization() {
        let func = TimeSeriesFunction::Delta {
            schedule: "0 0 * * *".to_string(),
        };
        let json = serde_json::to_string(&func).unwrap();
        assert!(json.contains("delta"));
        assert!(json.contains("schedule"));
    }
}
