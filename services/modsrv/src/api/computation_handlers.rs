//! Direct Computation API Handlers
//!
//! This module provides endpoints for real-time mathematical computations without persistence.
//! Supports expression evaluation, aggregation operations, energy calculations, and time series analysis.

#![allow(clippy::disallowed_methods)] // json! macro used in multiple functions

use axum::{extract::State, response::Json};
use serde_json::json;
use std::sync::Arc;
use voltage_config::{
    api::SuccessResponse,
    calculations::{AggregationType, EnergyCalculation},
};

use crate::app_state::AppState;
use crate::dto::{AggregationRequest, EnergyRequest, ExpressionRequest, TimeSeriesRequest};
use crate::error::ModSrvError;

/// Compute mathematical expression with variables
///
/// @route POST /api/compute/expression
/// @input request: Json<ExpressionRequest> - Formula and variables
/// @output Json - Computation result
/// @example {"formula": "a + b * 2", "variables": {"a": 10, "b": 5}}
pub async fn compute_expression(
    State(state): State<Arc<AppState>>,
    Json(request): Json<ExpressionRequest>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ModSrvError> {
    match state
        .calculation_engine
        .execute_expression_values(&request.formula, &request.variables)
        .await
    {
        Ok(value) => Ok(Json(SuccessResponse::new(json!({
            "formula": request.formula,
            "result": value,
            "type": "expression"
        })))),
        Err(e) => Err(ModSrvError::InvalidData(e.to_string())),
    }
}

/// Compute aggregation over values
///
/// @route POST /api/compute/aggregate
/// @input request: Json<AggregationRequest> - Operation and values
/// @output Json - Aggregation result
/// @example {"operation": "avg", "values": [10, 20, 30]}
pub async fn compute_aggregation(
    State(state): State<Arc<AppState>>,
    Json(request): Json<AggregationRequest>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ModSrvError> {
    let op = match request.operation.as_str() {
        "sum" => AggregationType::Sum,
        "avg" | "average" => AggregationType::Average,
        "min" => AggregationType::Min,
        "max" => AggregationType::Max,
        "count" => AggregationType::Count,
        _ => AggregationType::Sum,
    };

    match state
        .calculation_engine
        .execute_aggregation_values(&op, &request.values)
    {
        Ok(value) => Ok(Json(SuccessResponse::new(json!({
            "operation": request.operation,
            "result": value,
            "count": request.values.len()
        })))),
        Err(e) => Err(ModSrvError::InvalidData(e.to_string())),
    }
}

/// Compute energy calculations
///
/// @route POST /api/compute/energy
/// @input request: Json<EnergyRequest> - Calculation type and inputs
/// @output Json - Energy calculation result
/// @example {"calculation_type": "efficiency", "inputs": {"input_power": 100, "output_power": 85}}
pub async fn compute_energy(
    State(state): State<Arc<AppState>>,
    Json(request): Json<EnergyRequest>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ModSrvError> {
    let op = match request.calculation_type.as_str() {
        "power_balance" => EnergyCalculation::PowerBalance,
        "efficiency" => EnergyCalculation::EnergyEfficiency,
        _ => {
            return Err(ModSrvError::InvalidData(
                "Unknown energy calculation type".to_string(),
            ));
        },
    };

    match state
        .calculation_engine
        .execute_energy_values(&op, &request.inputs)
    {
        Ok(value) => Ok(Json(SuccessResponse::new(value))),
        Err(e) => Err(ModSrvError::InternalError(e.to_string())),
    }
}

/// Compute time series operations
///
/// @route POST /api/compute/timeseries
/// @input request: Json<TimeSeriesRequest> - Operation and data
/// @output Json - Time series computation result
/// @example {"operation": "moving_average", "data": [1,2,3,4,5], "window_size": 3}
pub async fn compute_timeseries(
    State(_state): State<Arc<AppState>>,
    Json(request): Json<TimeSeriesRequest>,
) -> Result<Json<SuccessResponse<serde_json::Value>>, ModSrvError> {
    match request.operation.as_str() {
        "moving_average" => {
            let window = request.window_size.unwrap_or(5);
            if request.data.len() < window {
                return Err(ModSrvError::InvalidData(
                    "Insufficient data for moving average".to_string(),
                ));
            }

            let mut results = Vec::new();
            for i in window..=request.data.len() {
                let sum: f64 = request.data[i - window..i].iter().sum();
                results.push(sum / window as f64);
            }

            Ok(Json(SuccessResponse::new(json!({
                "type": "moving_average",
                "window_size": window,
                "results": results
            }))))
        },
        "rate_of_change" => {
            if request.data.len() < 2 {
                return Err(ModSrvError::InvalidData(
                    "Insufficient data for rate of change".to_string(),
                ));
            }

            let mut rates = Vec::new();
            for i in 1..request.data.len() {
                rates.push(request.data[i] - request.data[i - 1]);
            }

            Ok(Json(SuccessResponse::new(json!({
                "type": "rate_of_change",
                "rates": rates
            }))))
        },
        _ => Err(ModSrvError::InvalidData(
            "Unknown time series operation".to_string(),
        )),
    }
}
