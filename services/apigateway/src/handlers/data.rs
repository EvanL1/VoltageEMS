use actix_web::{web, HttpResponse};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::error::{ApiError, ApiResult};
use crate::redis_client::RedisClient;
use crate::response::success_response;

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
    pub quality: u8,
    pub timestamp: i64,
    pub name: Option<String>,
    pub unit: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalData {
    pub point_id: u32,
    pub value: bool,
    pub quality: u8,
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
    path: web::Path<u32>,
    query: web::Query<TelemetryQuery>,
    redis: web::Data<Arc<RedisClient>>,
) -> ApiResult<HttpResponse> {
    let channel_id = path.into_inner();
    
    // 验证通道ID有效性
    validate_channel_id(channel_id)?;
    
    // 解析点位ID
    let point_ids: Vec<u32> = if let Some(ids) = &query.point_ids {
        ids.split(',')
            .filter_map(|id| id.trim().parse().ok())
            .collect()
    } else {
        // 默认返回前10个点位
        (1..=10).map(|i| 10000 + i).collect()
    };

    let mut telemetry_data = Vec::new();
    let timestamp = Utc::now().timestamp_millis();

    // 尝试从Redis获取数据
    for point_id in point_ids.iter().take(query.limit.unwrap_or(100)) {
        // 构造Redis键
        let key = format!("{}:m:{}", channel_id, point_id);
        
        // 尝试从Redis获取
        match redis.get(&key).await {
            Ok(Some(data)) => {
                // 解析Redis中的数据
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(&data) {
                    telemetry_data.push(TelemetryData {
                        point_id: *point_id,
                        value: value["value"].as_f64().unwrap_or(0.0),
                        quality: value["quality"].as_u64().unwrap_or(192) as u8,
                        timestamp: value["timestamp"].as_i64().unwrap_or(timestamp),
                        name: Some(format!("测量点{}", point_id)),
                        unit: Some("kW".to_string()),
                    });
                }
            }
            _ => {
                // 如果Redis中没有数据，生成模拟数据
                telemetry_data.push(TelemetryData {
                    point_id: *point_id,
                    value: 100.0 + (point_id % 50) as f64 + rand::random::<f64>() * 10.0,
                    quality: 192,
                    timestamp,
                    name: Some(format!("测量点{}", point_id)),
                    unit: Some("kW".to_string()),
                });
            }
        }
    }

    Ok(success_response(telemetry_data))
}

/// 获取信号数据
pub async fn get_signals(
    path: web::Path<u32>,
    redis: web::Data<Arc<RedisClient>>,
) -> ApiResult<HttpResponse> {
    let channel_id = path.into_inner();
    
    // 验证通道ID有效性
    validate_channel_id(channel_id)?;
    let mut signal_data = Vec::new();
    let timestamp = Utc::now().timestamp_millis();

    // 生成一些信号数据
    for i in 1..=10 {
        let point_id = 20000 + i;
        let key = format!("{}:s:{}", channel_id, point_id);
        
        let value = match redis.get(&key).await {
            Ok(Some(data)) => {
                serde_json::from_str::<serde_json::Value>(&data)
                    .ok()
                    .and_then(|v| v["value"].as_bool())
                    .unwrap_or(i % 2 == 0)
            }
            _ => i % 2 == 0,
        };

        signal_data.push(SignalData {
            point_id,
            value,
            quality: 192,
            timestamp,
            name: Some(format!("信号{}", i)),
            description: Some(if value { "开启" } else { "关闭" }.to_string()),
        });
    }

    Ok(success_response(signal_data))
}

/// 发送控制命令
pub async fn send_control(
    path: web::Path<u32>,
    command: web::Json<ControlCommand>,
    redis: web::Data<Arc<RedisClient>>,
) -> ApiResult<HttpResponse> {
    let channel_id = path.into_inner();
    
    // 验证通道ID有效性
    validate_channel_id(channel_id)?;
    
    // 构造命令消息
    let command_msg = serde_json::json!({
        "type": "control",
        "channel_id": channel_id,
        "point_id": command.point_id,
        "value": command.value,
        "params": command.params,
        "timestamp": Utc::now().timestamp_millis(),
    });

    // 发布到Redis
    let channel = format!("cmd:{}:control", channel_id);
    redis.publish(&channel, &command_msg.to_string()).await?;

    let response = CommandResponse {
        success: true,
        message: "Control command sent successfully".to_string(),
        command_id: Some(format!("cmd_{}", Utc::now().timestamp_millis())),
    };

    Ok(HttpResponse::Accepted().json(&response))
}

/// 发送调节命令
pub async fn send_adjustment(
    path: web::Path<u32>,
    command: web::Json<ControlCommand>,
    redis: web::Data<Arc<RedisClient>>,
) -> ApiResult<HttpResponse> {
    let channel_id = path.into_inner();
    
    // 验证通道ID有效性
    validate_channel_id(channel_id)?;
    
    // 构造命令消息
    let command_msg = serde_json::json!({
        "type": "adjustment",
        "channel_id": channel_id,
        "point_id": command.point_id,
        "value": command.value,
        "params": command.params,
        "timestamp": Utc::now().timestamp_millis(),
    });

    // 发布到Redis
    let channel = format!("cmd:{}:adjustment", channel_id);
    redis.publish(&channel, &command_msg.to_string()).await?;

    let response = CommandResponse {
        success: true,
        message: "Adjustment command sent successfully".to_string(),
        command_id: Some(format!("adj_{}", Utc::now().timestamp_millis())),
    };

    Ok(HttpResponse::Accepted().json(&response))
}

/// 获取告警列表
pub async fn get_alarms(
    query: web::Query<AlarmQuery>,
    _redis: web::Data<Arc<RedisClient>>,
) -> ApiResult<HttpResponse> {
    let mut alarms = vec![
        Alarm {
            id: "alarm_001".to_string(),
            channel_id: 1001,
            point_id: 10001,
            level: "warning".to_string(),
            message: "功率超限".to_string(),
            timestamp: Utc::now().timestamp_millis() - 3600000,
            acknowledged: true,
            status: "acknowledged".to_string(),
            acknowledged_by: Some("admin".to_string()),
            acknowledged_at: Some(Utc::now().timestamp_millis() - 1800000),
        },
        Alarm {
            id: "alarm_002".to_string(),
            channel_id: 1001,
            point_id: 10002,
            level: "critical".to_string(),
            message: "设备离线".to_string(),
            timestamp: Utc::now().timestamp_millis() - 7200000,
            acknowledged: false,
            status: "active".to_string(),
            acknowledged_by: None,
            acknowledged_at: None,
        },
        Alarm {
            id: "alarm_003".to_string(),
            channel_id: 1002,
            point_id: 10003,
            level: "info".to_string(),
            message: "维护提醒".to_string(),
            timestamp: Utc::now().timestamp_millis() - 86400000,
            acknowledged: true,
            status: "acknowledged".to_string(),
            acknowledged_by: Some("operator".to_string()),
            acknowledged_at: Some(Utc::now().timestamp_millis() - 43200000),
        },
    ];

    // 根据查询参数过滤
    if query.active_only.unwrap_or(false) {
        alarms.retain(|a| a.status == "active");
    }
    if let Some(status) = &query.status {
        alarms.retain(|a| a.status == *status);
    }
    if let Some(level) = &query.level {
        alarms.retain(|a| a.level == *level);
    }

    // 限制返回数量
    let limit = query.limit.unwrap_or(100);
    alarms.truncate(limit);

    Ok(success_response(alarms))
}

/// 获取活动告警
pub async fn get_active_alarms(
    _redis: web::Data<Arc<RedisClient>>,
) -> ApiResult<HttpResponse> {
    let query = AlarmQuery {
        active_only: Some(true),
        status: None,
        level: None,
        limit: Some(50),
    };
    get_alarms(web::Query(query), _redis).await
}

/// 确认告警
pub async fn acknowledge_alarm(
    path: web::Path<String>,
) -> ApiResult<HttpResponse> {
    let alarm_id = path.into_inner();
    
    // 在实际应用中，这里应该更新Redis中的告警状态
    let response = serde_json::json!({
        "success": true,
        "message": format!("Alarm {} acknowledged", alarm_id),
        "acknowledged_at": Utc::now().timestamp_millis(),
    });

    Ok(success_response(response))
}

/// 获取历史数据
pub async fn get_historical(
    query: web::Query<HistoricalQuery>,
) -> ApiResult<HttpResponse> {
    // 解析点位ID
    let point_ids: Vec<u32> = query
        .point_ids
        .split(',')
        .filter_map(|id| id.trim().parse().ok())
        .collect();

    if point_ids.is_empty() {
        return Err(ApiError::BadRequest("No valid point IDs provided".to_string()));
    }

    // 模拟历史数据
    let mut data = Vec::new();
    let interval_ms = match query.interval.as_deref() {
        Some("1m") => 60000,
        Some("5m") => 300000,
        Some("1h") => 3600000,
        _ => 300000, // 默认5分钟
    };

    for point_id in point_ids {
        let mut point_data = Vec::new();
        let mut timestamp = query.start_time;
        
        while timestamp <= query.end_time {
            point_data.push(serde_json::json!({
                "timestamp": timestamp,
                "value": 100.0 + (point_id % 50) as f64 + rand::random::<f64>() * 20.0 - 10.0,
                "quality": 192,
            }));
            timestamp += interval_ms;
        }

        data.push(serde_json::json!({
            "point_id": point_id,
            "data": point_data,
        }));
    }

    Ok(success_response(serde_json::json!({
        "channel_id": query.channel_id,
        "start_time": query.start_time,
        "end_time": query.end_time,
        "interval": query.interval.as_deref().unwrap_or("5m"),
        "points": data,
    })))
}

#[derive(Debug, Deserialize)]
pub struct PointHistoryQuery {
    pub start_time: i64,
    pub end_time: i64,
    pub interval: Option<String>, // e.g., "1m", "5m", "1h"
}

/// 获取单个点位的历史数据
pub async fn get_point_history(
    path: web::Path<(u32, u32)>,
    query: web::Query<PointHistoryQuery>,
) -> ApiResult<HttpResponse> {
    let (_channel_id, point_id) = path.into_inner();
    
    // 验证时间范围
    if query.start_time >= query.end_time {
        return Err(ApiError::BadRequest("Invalid time range".to_string()));
    }

    // 模拟历史数据
    let interval_ms = match query.interval.as_deref() {
        Some("1m") => 60000,
        Some("5m") => 300000,
        Some("1h") => 3600000,
        _ => 300000, // 默认5分钟
    };

    let mut data = Vec::new();
    let mut timestamp = query.start_time;
    
    while timestamp <= query.end_time {
        data.push(serde_json::json!({
            "timestamp": timestamp,
            "value": 100.0 + (point_id % 50) as f64 + rand::random::<f64>() * 20.0 - 10.0,
            "quality": 192,
        }));
        timestamp += interval_ms;
    }

    // 返回数组格式的数据
    Ok(success_response(data))
}