use crate::{error::ApiGatewayError, response::ApiResponse, AppState};
use axum::{
    extract::{Path, Query, State},
    response::Json,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, info};

/// Query parameters for data endpoints
#[derive(Debug, Deserialize)]
pub struct DataQuery {
    /// Data type filter (T=Telemetry, S=Signal, C=Control, A=Adjustment)
    #[serde(default)]
    pub data_type: Option<String>,
    /// Point ID filter
    #[serde(default)]
    pub point_id: Option<u32>,
    /// Limit number of results
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    100
}

/// Real-time data response
#[derive(Debug, Serialize)]
pub struct RealtimeData {
    pub channel_id: u32,
    pub data_type: String,
    pub timestamp: i64,
    pub values: HashMap<String, serde_json::Value>,
}

/// Channel status response
#[derive(Debug, Serialize)]
pub struct ChannelStatus {
    pub channel_id: u32,
    pub status: String,
    pub last_update: Option<i64>,
    pub active_points: u32,
}

/// Get real-time data for a specific channel
pub async fn get_realtime_data(
    State(_state): State<AppState>,
    Path(channel_id): Path<u32>,
    Query(query): Query<DataQuery>,
) -> Result<Json<ApiResponse<Vec<RealtimeData>>>, ApiGatewayError> {
    info!(
        "Getting realtime data for channel {} with query: {:?}",
        channel_id, query
    );

    let mut data = Vec::new();

    // Determine which data types to query
    let data_types = match &query.data_type {
        Some(dt) => vec![dt.clone()],
        None => vec![
            "T".to_string(),
            "S".to_string(),
            "C".to_string(),
            "A".to_string(),
        ],
    };

    for data_type in data_types {
        let key = format!("comsrv:{}:{}", channel_id, data_type);
        debug!("Querying Redis key: {}", key);

        // In a real implementation, we would:
        // 1. Query Redis for the latest data
        // 2. Parse the data structure
        // 3. Filter by point_id if specified
        // 4. Apply limit

        // Check if point_id filter is specified (to avoid dead code warning)
        if let Some(pid) = query.point_id {
            debug!("Filtering by point_id: {}", pid);
        }

        // For now, create mock data
        let mock_data = RealtimeData {
            channel_id,
            data_type: data_type.clone(),
            timestamp: chrono::Utc::now().timestamp(),
            values: {
                let mut values = HashMap::new();
                values.insert("mock_point_1".to_string(), serde_json::json!(42.5));
                values.insert("mock_point_2".to_string(), serde_json::json!(true));
                values
            },
        };

        data.push(mock_data);

        if data.len() >= query.limit {
            break;
        }
    }

    Ok(Json(ApiResponse::success(data)))
}

/// Get channel status
pub async fn get_channel_status(
    State(_state): State<AppState>,
    Path(channel_id): Path<u32>,
) -> Result<Json<ApiResponse<ChannelStatus>>, ApiGatewayError> {
    info!("Getting status for channel {}", channel_id);

    // Mock channel status - in real implementation, query Redis for channel health
    let status = ChannelStatus {
        channel_id,
        status: "active".to_string(),
        last_update: Some(chrono::Utc::now().timestamp()),
        active_points: 5,
    };

    Ok(Json(ApiResponse::success(status)))
}

/// List all available channels
pub async fn list_channels(
    State(_state): State<AppState>,
) -> Result<Json<ApiResponse<Vec<ChannelStatus>>>, ApiGatewayError> {
    info!("Listing all channels");

    // Mock channel list - in real implementation, scan Redis for comsrv:*:* patterns
    let channels = vec![
        ChannelStatus {
            channel_id: 1,
            status: "active".to_string(),
            last_update: Some(chrono::Utc::now().timestamp()),
            active_points: 8,
        },
        ChannelStatus {
            channel_id: 1001,
            status: "active".to_string(),
            last_update: Some(chrono::Utc::now().timestamp()),
            active_points: 12,
        },
        ChannelStatus {
            channel_id: 1002,
            status: "active".to_string(),
            last_update: Some(chrono::Utc::now().timestamp()),
            active_points: 6,
        },
        ChannelStatus {
            channel_id: 1003,
            status: "active".to_string(),
            last_update: Some(chrono::Utc::now().timestamp()),
            active_points: 3,
        },
    ];

    Ok(Json(ApiResponse::success(channels)))
}

/// Get historical data (placeholder for future InfluxDB integration)
pub async fn get_historical_data(
    State(_state): State<AppState>,
    Path(channel_id): Path<u32>,
    Query(query): Query<DataQuery>,
) -> Result<Json<ApiResponse<Vec<RealtimeData>>>, ApiGatewayError> {
    info!(
        "Getting historical data for channel {} with query: {:?}",
        channel_id, query
    );

    // This would integrate with hissrv or directly with InfluxDB
    // For now, return empty data
    let data = Vec::new();

    Ok(Json(ApiResponse::success(data)))
}
