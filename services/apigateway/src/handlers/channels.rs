use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
};
use log::{debug, warn};
use serde::{Deserialize, Serialize};

use crate::data_access::{AccessOptions, DataAccessError};
use crate::error::{ApiError, ApiResult};
use crate::response::success_response;
use crate::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Channel {
    pub id: u32,
    pub name: String,
    pub protocol: String,
    pub status: String,
    pub description: Option<String>,
    pub config: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct ChannelQuery {
    pub status: Option<String>,
    pub protocol: Option<String>,
}

/// 获取通道列表
pub async fn list_channels(
    Query(query): Query<ChannelQuery>,
    State(state): State<AppState>,
) -> ApiResult<impl IntoResponse> {
    debug!("Getting channel list with query: {:?}", query);

    // 1. 从Redis获取通道ID列表
    let channel_ids = match state
        .data_access_layer
        .get_data("meta:channels:list", AccessOptions::config_cached(300))
        .await
    {
        Ok(value) => {
            match serde_json::from_value::<Vec<u32>>(value) {
                Ok(ids) => ids,
                Err(e) => {
                    warn!("Failed to parse channel IDs from Redis: {}", e);
                    // 如果Redis中没有数据，返回默认的通道ID列表
                    vec![1001, 1002, 1003, 1004, 1005]
                }
            }
        }
        Err(DataAccessError::NotFound(_)) => {
            debug!("Channel list not found in Redis, using default IDs");
            // 如果Redis中没有数据，返回默认的通道ID列表
            vec![1001, 1002, 1003, 1004, 1005]
        }
        Err(e) => {
            warn!("Error getting channel list from Redis: {}", e);
            // 降级到默认值
            vec![1001, 1002, 1003, 1004, 1005]
        }
    };

    // 2. 批量获取通道配置
    let config_keys: Vec<String> = channel_ids
        .iter()
        .map(|id| format!("cfg:channel:{}", id))
        .collect();

    let configs = state
        .data_access_layer
        .batch_get(config_keys, AccessOptions::cache_with_fallback())
        .await
        .map_err(|e| {
            warn!("Failed to get channel configs: {}", e);
            ApiError::ServiceError("Failed to get channel configurations".to_string())
        })?;

    // 3. 组装通道数据
    let mut channels = Vec::new();
    for (i, config_opt) in configs.into_iter().enumerate() {
        let channel_id = channel_ids[i];
        
        let channel = if let Some(config_value) = config_opt {
            // 从Redis配置解析通道信息
            match serde_json::from_value::<Channel>(config_value) {
                Ok(ch) => ch,
                Err(e) => {
                    warn!("Failed to parse channel {} config: {}", channel_id, e);
                    // 使用默认配置
                    create_default_channel(channel_id)
                }
            }
        } else {
            // 如果没有配置，使用默认配置
            debug!("No config found for channel {}, using default", channel_id);
            create_default_channel(channel_id)
        };

        channels.push(channel);
    }

    // 4. 根据查询参数过滤
    if let Some(status) = &query.status {
        channels.retain(|c| c.status == *status);
    }
    if let Some(protocol) = &query.protocol {
        channels.retain(|c| c.protocol == *protocol);
    }

    debug!("Returning {} channels", channels.len());
    Ok(success_response(channels))
}

/// 创建默认的通道配置
fn create_default_channel(channel_id: u32) -> Channel {
    match channel_id {
        1001 => Channel {
            id: 1001,
            name: "主站通道1".to_string(),
            protocol: "modbus_tcp".to_string(),
            status: "online".to_string(),
            description: Some("主站Modbus TCP通道".to_string()),
            config: Some(serde_json::json!({
                "host": "192.168.1.100",
                "port": 502,
                "timeout": 5000,
                "retry": 3
            })),
        },
        1002 => Channel {
            id: 1002,
            name: "主站通道2".to_string(),
            protocol: "iec60870".to_string(),
            status: "online".to_string(),
            description: Some("IEC 60870-5-104通道".to_string()),
            config: Some(serde_json::json!({
                "host": "192.168.1.101",
                "port": 2404,
                "commonAddress": 1
            })),
        },
        1003 => Channel {
            id: 1003,
            name: "备用通道".to_string(),
            protocol: "modbus_rtu".to_string(),
            status: "offline".to_string(),
            description: Some("Modbus RTU备用通道".to_string()),
            config: Some(serde_json::json!({
                "port": "/dev/ttyUSB0",
                "baudRate": 9600
            })),
        },
        1004 => Channel {
            id: 1004,
            name: "CAN总线通道".to_string(),
            protocol: "can".to_string(),
            status: "online".to_string(),
            description: Some("CAN总线数据采集通道".to_string()),
            config: Some(serde_json::json!({
                "interface": "can0",
                "bitrate": 250000
            })),
        },
        1005 => Channel {
            id: 1005,
            name: "测试通道".to_string(),
            protocol: "modbus_tcp".to_string(),
            status: "online".to_string(),
            description: Some("测试用Modbus TCP通道".to_string()),
            config: Some(serde_json::json!({
                "host": "127.0.0.1",
                "port": 5020
            })),
        },
        _ => Channel {
            id: channel_id,
            name: format!("通道{}", channel_id),
            protocol: "unknown".to_string(),
            status: "unknown".to_string(),
            description: Some(format!("未知通道{}", channel_id)),
            config: None,
        },
    }
}

/// 获取单个通道详情
pub async fn get_channel(
    Path(channel_id): Path<u32>,
    State(state): State<AppState>,
) -> ApiResult<impl IntoResponse> {
    debug!("Getting channel details for ID: {}", channel_id);

    // 1. 验证通道ID是否有效
    let valid_channel_ids = vec![1001, 1002, 1003, 1004, 1005];
    if !valid_channel_ids.contains(&channel_id) {
        return Err(ApiError::NotFound(format!("Channel {} not found", channel_id)));
    }

    // 2. 从数据访问层获取通道配置
    let config_key = format!("cfg:channel:{}", channel_id);
    
    let channel = match state
        .data_access_layer
        .get_data(&config_key, AccessOptions::cache_with_fallback())
        .await
    {
        Ok(config_value) => {
            // 尝试从Redis配置解析
            match serde_json::from_value::<Channel>(config_value) {
                Ok(ch) => ch,
                Err(e) => {
                    warn!("Failed to parse channel {} config from Redis: {}", channel_id, e);
                    // 使用默认配置
                    create_default_channel_with_details(channel_id)
                }
            }
        }
        Err(DataAccessError::NotFound(_)) => {
            debug!("Channel {} config not found in Redis, using default", channel_id);
            // 使用默认配置
            create_default_channel_with_details(channel_id)
        }
        Err(e) => {
            warn!("Error getting channel {} config: {}", channel_id, e);
            // 降级到默认配置
            create_default_channel_with_details(channel_id)
        }
    };

    debug!("Returning channel details for ID: {}", channel_id);
    Ok(success_response(channel))
}

/// 创建包含详细配置的默认通道
fn create_default_channel_with_details(channel_id: u32) -> Channel {
    match channel_id {
        1001 => Channel {
            id: 1001,
            name: "主站通道1".to_string(),
            protocol: "modbus_tcp".to_string(),
            status: "online".to_string(),
            description: Some("主站Modbus TCP通道".to_string()),
            config: Some(serde_json::json!({
                "host": "192.168.1.100",
                "port": 502,
                "timeout": 5000,
                "retry": 3,
                "points": {
                    "telemetry": 150,
                    "signal": 50,
                    "control": 20,
                    "adjustment": 10
                }
            })),
        },
        1002 => Channel {
            id: 1002,
            name: "主站通道2".to_string(),
            protocol: "iec60870".to_string(),
            status: "online".to_string(),
            description: Some("IEC 60870-5-104通道".to_string()),
            config: Some(serde_json::json!({
                "host": "192.168.1.101",
                "port": 2404,
                "commonAddress": 1,
                "timeout": 15000,
                "k": 12,
                "w": 8,
                "t1": 15,
                "t2": 10,
                "t3": 20
            })),
        },
        1003 => Channel {
            id: 1003,
            name: "备用通道".to_string(),
            protocol: "modbus_rtu".to_string(),
            status: "offline".to_string(),
            description: Some("Modbus RTU备用通道".to_string()),
            config: Some(serde_json::json!({
                "port": "/dev/ttyUSB0",
                "baudRate": 9600,
                "dataBits": 8,
                "stopBits": 1,
                "parity": "N",
                "timeout": 1000
            })),
        },
        1004 => Channel {
            id: 1004,
            name: "CAN总线通道".to_string(),
            protocol: "can".to_string(),
            status: "online".to_string(),
            description: Some("CAN总线数据采集通道".to_string()),
            config: Some(serde_json::json!({
                "interface": "can0",
                "bitrate": 250000,
                "filters": [
                    {"id": "0x100", "mask": "0x7FF"},
                    {"id": "0x200", "mask": "0x7FF"}
                ]
            })),
        },
        1005 => Channel {
            id: 1005,
            name: "测试通道".to_string(),
            protocol: "modbus_tcp".to_string(),
            status: "online".to_string(),
            description: Some("测试用Modbus TCP通道".to_string()),
            config: Some(serde_json::json!({
                "host": "127.0.0.1",
                "port": 5020,
                "timeout": 3000,
                "retry": 2
            })),
        },
        _ => Channel {
            id: channel_id,
            name: format!("通道{}", channel_id),
            protocol: "unknown".to_string(),
            status: "unknown".to_string(),
            description: Some(format!("未知通道{}", channel_id)),
            config: None,
        },
    }
}