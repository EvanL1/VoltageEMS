//! Time Series Processor
//!
//! Temporal data analysis: moving average, rate of change.

/// Time series processor for temporal data analysis
pub struct TimeSeriesProcessor;

impl TimeSeriesProcessor {
    /// Create new time series processor
    pub fn new() -> Self {
        Self
    }

    /// Calculate moving average over a sliding window
    ///
    /// # Arguments
    /// * `series` - Time series data as (timestamp, value) pairs
    /// * `window` - Window size (number of points)
    ///
    /// # Returns
    /// Vector of moving average values
    pub fn calculate_moving_average(&self, series: &[(f64, f64)], window: usize) -> Vec<f64> {
        if series.len() < window {
            return vec![];
        }

        let mut results = Vec::new();
        for i in window..=series.len() {
            let sum: f64 = series[i - window..i].iter().map(|(_, v)| v).sum();
            results.push(sum / window as f64);
        }
        results
    }

    /// Calculate rate of change between consecutive points
    ///
    /// # Arguments
    /// * `series` - Time series data as (timestamp, value) pairs
    ///
    /// # Returns
    /// Vector of rate of change values (dv/dt)
    pub fn calculate_rate_of_change(&self, series: &[(f64, f64)]) -> Vec<f64> {
        if series.len() < 2 {
            return vec![];
        }

        let mut results = Vec::new();
        for i in 1..series.len() {
            let dt = series[i].0 - series[i - 1].0;
            let dv = series[i].1 - series[i - 1].1;
            if dt > 0.0 {
                results.push(dv / dt);
            }
        }
        results
    }
}

impl Default for TimeSeriesProcessor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_moving_average() {
        let processor = TimeSeriesProcessor::new();
        let series = vec![
            (1.0, 10.0),
            (2.0, 20.0),
            (3.0, 30.0),
            (4.0, 40.0),
            (5.0, 50.0),
        ];

        let result = processor.calculate_moving_average(&series, 3);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0], 20.0); // (10+20+30)/3
        assert_eq!(result[1], 30.0); // (20+30+40)/3
        assert_eq!(result[2], 40.0); // (30+40+50)/3
    }

    #[test]
    fn test_moving_average_insufficient_data() {
        let processor = TimeSeriesProcessor::new();
        let series = vec![(1.0, 10.0), (2.0, 20.0)];

        let result = processor.calculate_moving_average(&series, 5);
        assert!(result.is_empty());
    }

    #[test]
    fn test_moving_average_window_one() {
        let processor = TimeSeriesProcessor::new();
        let series = vec![(1.0, 10.0), (2.0, 20.0), (3.0, 30.0)];

        let result = processor.calculate_moving_average(&series, 1);
        assert_eq!(result, vec![10.0, 20.0, 30.0]);
    }

    #[test]
    fn test_rate_of_change() {
        let processor = TimeSeriesProcessor::new();
        let series = vec![(0.0, 0.0), (1.0, 10.0), (2.0, 20.0), (3.0, 30.0)];

        let result = processor.calculate_rate_of_change(&series);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0], 10.0); // (10-0)/(1-0)
        assert_eq!(result[1], 10.0); // (20-10)/(2-1)
        assert_eq!(result[2], 10.0); // (30-20)/(3-2)
    }

    #[test]
    fn test_rate_of_change_varying() {
        let processor = TimeSeriesProcessor::new();
        let series = vec![
            (0.0, 0.0),
            (2.0, 10.0), // +5/s
            (4.0, 30.0), // +10/s
            (5.0, 35.0), // +5/s
        ];

        let result = processor.calculate_rate_of_change(&series);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0], 5.0);
        assert_eq!(result[1], 10.0);
        assert_eq!(result[2], 5.0);
    }

    #[test]
    fn test_rate_of_change_negative() {
        let processor = TimeSeriesProcessor::new();
        let series = vec![(0.0, 100.0), (1.0, 80.0), (2.0, 60.0)];

        let result = processor.calculate_rate_of_change(&series);
        assert_eq!(result[0], -20.0);
        assert_eq!(result[1], -20.0);
    }

    #[test]
    fn test_rate_of_change_insufficient_data() {
        let processor = TimeSeriesProcessor::new();
        let series = vec![(1.0, 10.0)];

        let result = processor.calculate_rate_of_change(&series);
        assert!(result.is_empty());
    }

    #[test]
    fn test_rate_of_change_zero_dt_skipped() {
        let processor = TimeSeriesProcessor::new();
        let series = vec![
            (1.0, 10.0),
            (1.0, 20.0), // Same timestamp - invalid
            (2.0, 30.0),
        ];

        let result = processor.calculate_rate_of_change(&series);
        assert_eq!(result.len(), 1); // Only one valid rate
        assert_eq!(result[0], 10.0);
    }
}
