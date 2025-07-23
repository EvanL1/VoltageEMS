use axum::{
    extract::{Extension, Path, Query, State},
    response::IntoResponse,
    Json,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::auth::Claims;
use crate::error::{ApiError, ApiResult};
use crate::response::success_response;
use crate::AppState;

// 有效通道ID列表
const VALID_CHANNEL_IDS: &[u32] = &[1001, 1002, 1003, 1004, 1005];

/// 验证通道ID是否有效
fn validate_channel_id(channel_id: u32) -> Result<(), ApiError> {
    if VALID_CHANNEL_IDS.contains(&channel_id) {
        Ok(())
    } else {
        Err(ApiError::NotFound(format!("Channel {} not found", channel_id)))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryData {
    pub point_id: u32,
    pub value: f64,
    pub timestamp: i64,
    pub name: Option<String>,
    pub unit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalData {
    pub point_id: u32,
    pub value: bool,
    pub timestamp: i64,
    pub name: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlCommand {
    pub point_id: u32,
    pub value: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResponse {
    pub success: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alarm {
    pub id: String,
    pub channel_id: u32,
    pub point_id: u32,
    pub level: String,
    pub message: String,
    pub timestamp: i64,
    pub acknowledged: bool,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acknowledged_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acknowledged_at: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct TelemetryQuery {
    pub point_ids: Option<String>, // comma-separated IDs
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct AlarmQuery {
    pub active_only: Option<bool>,
    pub status: Option<String>,
    pub level: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct HistoricalQuery {
    pub channel_id: u32,
    pub point_ids: String, // comma-separated IDs
    pub start_time: i64,
    pub end_time: i64,
    pub interval: Option<String>, // e.g., "1m", "5m", "1h"
}

/// 获取遥测数据
pub async fn get_telemetry(
    Path(channel_id): Path<u32>,
    Query(query): Query<TelemetryQuery>,
    State(state): State<AppState>,
) -> ApiResult<impl IntoResponse> {
    // 验证通道ID
    validate_channel_id(channel_id)?;
    
    let limit = query.limit.unwrap_or(100);
    let mut telemetry_data = Vec::new();

    if let Some(point_ids_str) = query.point_ids {
        // Get specific points
        let point_ids: Vec<u32> = point_ids_str
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();

        for point_id in point_ids.into_iter().take(limit) {
            let key = format!("{}:m:{}", channel_id, point_id);
            if let Ok(Some(data)) = state.redis_client.get(&key).await {
                if let Ok(value) = serde_json::from_str::<TelemetryData>(&data) {
                    telemetry_data.push(value);
                }
            }
        }
    } else {
        // Get all telemetry points for the channel
        let pattern = format!("{}:m:*", channel_id);
        if let Ok(keys) = state.redis_client.keys(&pattern).await {
            for key in keys.into_iter().take(limit) {
                if let Ok(Some(data)) = state.redis_client.get(&key).await {
                    if let Ok(value) = serde_json::from_str::<TelemetryData>(&data) {
                        telemetry_data.push(value);
                    }
                }
            }
        }
    }

    Ok(success_response(telemetry_data))
}

/// 获取信号数据
pub async fn get_signals(
    Path(channel_id): Path<u32>,
    Query(query): Query<TelemetryQuery>,
    State(state): State<AppState>,
) -> ApiResult<impl IntoResponse> {
    // 验证通道ID
    validate_channel_id(channel_id)?;
    
    let limit = query.limit.unwrap_or(100);
    let mut signal_data = Vec::new();

    if let Some(point_ids_str) = query.point_ids {
        // Get specific points
        let point_ids: Vec<u32> = point_ids_str
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();

        for point_id in point_ids.into_iter().take(limit) {
            let key = format!("{}:s:{}", channel_id, point_id);
            if let Ok(Some(data)) = state.redis_client.get(&key).await {
                if let Ok(value) = serde_json::from_str::<SignalData>(&data) {
                    signal_data.push(value);
                }
            }
        }
    } else {
        // Get all signal points for the channel
        let pattern = format!("{}:s:*", channel_id);
        if let Ok(keys) = state.redis_client.keys(&pattern).await {
            for key in keys.into_iter().take(limit) {
                if let Ok(Some(data)) = state.redis_client.get(&key).await {
                    if let Ok(value) = serde_json::from_str::<SignalData>(&data) {
                        signal_data.push(value);
                    }
                }
            }
        }
    }

    Ok(success_response(signal_data))
}

/// 发送控制命令
pub async fn send_control(
    Path(channel_id): Path<u32>,
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(command): Json<ControlCommand>,
) -> ApiResult<impl IntoResponse> {
    // 验证通道ID
    validate_channel_id(channel_id)?;
    
    // Check permission
    if !claims.has_permission("channel:write") {
        return Err(ApiError::Forbidden);
    }

    // Generate command ID
    let command_id = uuid::Uuid::new_v4().to_string();

    // Create command with metadata
    let redis_command = serde_json::json!({
        "id": command_id,
        "channel_id": channel_id,
        "point_id": command.point_id,
        "value": command.value,
        "params": command.params,
        "user": claims.username,
        "timestamp": Utc::now().timestamp(),
    });

    // Publish to Redis channel
    let channel_name = format!("cmd:{}:control", channel_id);
    let json_str = serde_json::to_string(&redis_command)?;
    state.redis_client
        .publish(&channel_name, &json_str)
        .await?;

    Ok(success_response(CommandResponse {
        success: true,
        message: format!("Control command sent to channel {}", channel_id),
        command_id: Some(command_id),
    }))
}

/// 发送调节命令
pub async fn send_adjustment(
    Path(channel_id): Path<u32>,
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(command): Json<ControlCommand>,
) -> ApiResult<impl IntoResponse> {
    // 验证通道ID
    validate_channel_id(channel_id)?;
    
    // Check permission
    if !claims.has_permission("channel:write") {
        return Err(ApiError::Forbidden);
    }

    // Generate command ID
    let command_id = uuid::Uuid::new_v4().to_string();

    // Create command with metadata
    let redis_command = serde_json::json!({
        "id": command_id,
        "channel_id": channel_id,
        "point_id": command.point_id,
        "value": command.value,
        "params": command.params,
        "user": claims.username,
        "timestamp": Utc::now().timestamp(),
    });

    // Publish to Redis channel
    let channel_name = format!("cmd:{}:adjustment", channel_id);
    let json_str = serde_json::to_string(&redis_command)?;
    state.redis_client
        .publish(&channel_name, &json_str)
        .await?;

    Ok(success_response(CommandResponse {
        success: true,
        message: format!("Adjustment command sent to channel {}", channel_id),
        command_id: Some(command_id),
    }))
}

/// 获取告警列表
pub async fn get_alarms(
    Query(query): Query<AlarmQuery>,
    State(state): State<AppState>,
) -> ApiResult<impl IntoResponse> {
    let mut alarms = Vec::new();
    let limit = query.limit.unwrap_or(100);

    // Get alarms from Redis
    let pattern = "alarm:*";
    if let Ok(keys) = state.redis_client.keys(pattern).await {
        for key in keys.into_iter().take(limit) {
            if let Ok(Some(data)) = state.redis_client.get(&key).await {
                if let Ok(alarm) = serde_json::from_str::<Alarm>(&data) {
                    // Apply filters
                    if query.active_only.unwrap_or(false) && alarm.acknowledged {
                        continue;
                    }
                    
                    if let Some(ref status_filter) = query.status {
                        if alarm.status != *status_filter {
                            continue;
                        }
                    }
                    
                    if let Some(ref level_filter) = query.level {
                        if alarm.level != *level_filter {
                            continue;
                        }
                    }

                    alarms.push(alarm);
                }
            }
        }
    }

    // Sort by timestamp (newest first)
    alarms.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

    Ok(success_response(alarms))
}

/// 获取活动告警
pub async fn get_active_alarms(
    State(state): State<AppState>,
) -> ApiResult<impl IntoResponse> {
    get_alarms(
        Query(AlarmQuery {
            active_only: Some(true),
            status: Some("active".to_string()),
            level: None,
            limit: Some(100),
        }),
        State(state),
    )
    .await
}

/// 确认告警
pub async fn acknowledge_alarm(
    Path(alarm_id): Path<String>,
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> ApiResult<impl IntoResponse> {
    // Check permission
    if !claims.has_permission("alarm:write") {
        return Err(ApiError::Forbidden);
    }

    let key = format!("alarm:{}", alarm_id);
    
    // Get the alarm
    let alarm_data: String = state.redis_client
        .get(&key)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Alarm {} not found", alarm_id)))?;

    let mut alarm: Alarm = serde_json::from_str(&alarm_data)?;

    // Update alarm
    alarm.acknowledged = true;
    alarm.acknowledged_by = Some(claims.username.clone());
    alarm.acknowledged_at = Some(Utc::now().timestamp());
    alarm.status = "acknowledged".to_string();

    // Save back to Redis
    state.redis_client
        .set(&key, &serde_json::to_string(&alarm)?)
        .await?;

    Ok(success_response(serde_json::json!({
        "success": true,
        "message": format!("Alarm {} acknowledged", alarm_id),
        "alarm": alarm,
    })))
}

/// 获取历史数据
pub async fn get_historical(
    Query(query): Query<HistoricalQuery>,
    State(_state): State<AppState>,
) -> ApiResult<impl IntoResponse> {
    // 验证通道ID
    validate_channel_id(query.channel_id)?;
    
    // In a real implementation, this would query InfluxDB
    // For now, return mock data
    let mock_data = serde_json::json!({
        "channel_id": query.channel_id,
        "point_ids": query.point_ids,
        "start_time": query.start_time,
        "end_time": query.end_time,
        "interval": query.interval,
        "data": []
    });

    Ok(success_response(mock_data))
}

/// 获取点位历史数据
pub async fn get_point_history(
    Path((channel_id, point_id)): Path<(u32, u32)>,
    Query(query): Query<HistoricalQuery>,
    State(_state): State<AppState>,
) -> ApiResult<impl IntoResponse> {
    // 验证通道ID
    validate_channel_id(channel_id)?;
    
    // In a real implementation, this would query InfluxDB
    // For now, return mock data
    let mock_data = serde_json::json!({
        "channel_id": channel_id,
        "point_id": point_id,
        "start_time": query.start_time,
        "end_time": query.end_time,
        "interval": query.interval,
        "data": []
    });

    Ok(success_response(mock_data))
}