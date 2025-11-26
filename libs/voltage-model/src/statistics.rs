//! Statistics Processor
//!
//! Aggregation operations on numeric data: sum, average, min, max,
//! standard deviation, median, percentile, weighted average.

use crate::error::{ModelError, Result};
use voltage_config::calculations::AggregationType;

/// Statistics processor for aggregations
pub struct StatisticsProcessor;

impl StatisticsProcessor {
    /// Create new statistics processor
    pub fn new() -> Self {
        Self
    }

    /// Aggregate values using the specified operation
    ///
    /// # Arguments
    /// * `operation` - Aggregation type (Sum, Average, Min, Max, etc.)
    /// * `values` - Slice of f64 values to aggregate
    ///
    /// # Returns
    /// Aggregation result as f64
    pub fn aggregate(&self, operation: &AggregationType, values: &[f64]) -> Result<f64> {
        if values.is_empty() {
            return Err(ModelError::statistics("Cannot aggregate empty dataset"));
        }

        match operation {
            AggregationType::Sum => Ok(values.iter().sum()),

            AggregationType::Average => Ok(values.iter().sum::<f64>() / values.len() as f64),

            AggregationType::Min => Ok(values.iter().cloned().fold(f64::INFINITY, f64::min)),

            AggregationType::Max => Ok(values.iter().cloned().fold(f64::NEG_INFINITY, f64::max)),

            AggregationType::Count => Ok(values.len() as f64),

            AggregationType::StandardDeviation => {
                let mean = values.iter().sum::<f64>() / values.len() as f64;
                let variance =
                    values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64;
                Ok(variance.sqrt())
            },

            AggregationType::Median => {
                let mut sorted = values.to_vec();
                sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                let mid = sorted.len() / 2;
                if sorted.len().is_multiple_of(2) {
                    Ok((sorted[mid - 1] + sorted[mid]) / 2.0)
                } else {
                    Ok(sorted[mid])
                }
            },

            AggregationType::Percentile { value } => {
                if *value < 0.0 || *value > 100.0 {
                    return Err(ModelError::statistics(
                        "Percentile must be between 0 and 100",
                    ));
                }
                let mut sorted = values.to_vec();
                sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                let index = (value / 100.0 * (sorted.len() - 1) as f64).round() as usize;
                Ok(sorted[index])
            },

            AggregationType::WeightedAverage { weights } => {
                if weights.len() != values.len() {
                    return Err(ModelError::statistics(
                        "Weights and values must have same length",
                    ));
                }
                let weighted_sum: f64 = values.iter().zip(weights.iter()).map(|(v, w)| v * w).sum();
                let weight_sum: f64 = weights.iter().sum();
                if weight_sum == 0.0 {
                    return Err(ModelError::statistics("Sum of weights cannot be zero"));
                }
                Ok(weighted_sum / weight_sum)
            },

            // Handle new aggregation types not yet implemented
            #[allow(unreachable_patterns)]
            _ => Err(ModelError::statistics(format!(
                "Aggregation type not yet implemented: {:?}",
                operation
            ))),
        }
    }
}

impl Default for StatisticsProcessor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_aggregations() {
        let processor = StatisticsProcessor::new();
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];

        assert_eq!(
            processor.aggregate(&AggregationType::Sum, &values).unwrap(),
            15.0
        );
        assert_eq!(
            processor
                .aggregate(&AggregationType::Average, &values)
                .unwrap(),
            3.0
        );
        assert_eq!(
            processor.aggregate(&AggregationType::Min, &values).unwrap(),
            1.0
        );
        assert_eq!(
            processor.aggregate(&AggregationType::Max, &values).unwrap(),
            5.0
        );
        assert_eq!(
            processor
                .aggregate(&AggregationType::Count, &values)
                .unwrap(),
            5.0
        );
    }

    #[test]
    fn test_median_odd() {
        let processor = StatisticsProcessor::new();
        let values = vec![1.0, 3.0, 5.0, 7.0, 9.0];
        assert_eq!(
            processor
                .aggregate(&AggregationType::Median, &values)
                .unwrap(),
            5.0
        );
    }

    #[test]
    fn test_median_even() {
        let processor = StatisticsProcessor::new();
        let values = vec![1.0, 2.0, 3.0, 4.0];
        assert_eq!(
            processor
                .aggregate(&AggregationType::Median, &values)
                .unwrap(),
            2.5
        );
    }

    #[test]
    fn test_standard_deviation() {
        let processor = StatisticsProcessor::new();
        let values = vec![2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0];
        let result = processor
            .aggregate(&AggregationType::StandardDeviation, &values)
            .unwrap();
        assert!((result - 2.0).abs() < 0.01);
    }

    #[test]
    fn test_percentile() {
        let processor = StatisticsProcessor::new();
        let values = vec![1.0, 2.0, 3.0, 4.0, 5.0];

        // 0th percentile (min)
        assert_eq!(
            processor
                .aggregate(&AggregationType::Percentile { value: 0.0 }, &values)
                .unwrap(),
            1.0
        );
        // 100th percentile (max)
        assert_eq!(
            processor
                .aggregate(&AggregationType::Percentile { value: 100.0 }, &values)
                .unwrap(),
            5.0
        );
    }

    #[test]
    fn test_weighted_average() {
        let processor = StatisticsProcessor::new();
        let values = vec![10.0, 20.0, 30.0];
        let weights = vec![1.0, 2.0, 3.0];

        let result = processor
            .aggregate(&AggregationType::WeightedAverage { weights }, &values)
            .unwrap();
        // (10*1 + 20*2 + 30*3) / (1+2+3) = 140/6 = 23.333...
        assert!((result - 23.333).abs() < 0.01);
    }

    #[test]
    fn test_empty_dataset() {
        let processor = StatisticsProcessor::new();
        let empty: Vec<f64> = vec![];
        assert!(processor.aggregate(&AggregationType::Sum, &empty).is_err());
    }

    #[test]
    fn test_negative_values() {
        let processor = StatisticsProcessor::new();
        let values = vec![-10.0, -5.0, 0.0, 5.0, 10.0];

        assert_eq!(
            processor.aggregate(&AggregationType::Sum, &values).unwrap(),
            0.0
        );
        assert_eq!(
            processor.aggregate(&AggregationType::Min, &values).unwrap(),
            -10.0
        );
        assert_eq!(
            processor.aggregate(&AggregationType::Max, &values).unwrap(),
            10.0
        );
    }
}
