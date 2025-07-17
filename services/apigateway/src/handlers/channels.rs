use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};

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
    State(_state): State<AppState>,
) -> ApiResult<impl IntoResponse> {
    // 模拟数据 - 在实际应用中，这些数据应该从Redis或配置中读取
    let mut channels = vec![
        Channel {
            id: 1001,
            name: "主站通道1".to_string(),
            protocol: "modbus_tcp".to_string(),
            status: "online".to_string(),
            description: Some("主站Modbus TCP通道".to_string()),
            config: None,
        },
        Channel {
            id: 1002,
            name: "主站通道2".to_string(),
            protocol: "iec60870".to_string(),
            status: "online".to_string(),
            description: Some("IEC 60870-5-104通道".to_string()),
            config: None,
        },
        Channel {
            id: 1003,
            name: "备用通道".to_string(),
            protocol: "modbus_rtu".to_string(),
            status: "offline".to_string(),
            description: Some("Modbus RTU备用通道".to_string()),
            config: None,
        },
        Channel {
            id: 1004,
            name: "CAN总线通道".to_string(),
            protocol: "can".to_string(),
            status: "online".to_string(),
            description: Some("CAN总线数据采集通道".to_string()),
            config: None,
        },
        Channel {
            id: 1005,
            name: "测试通道".to_string(),
            protocol: "modbus_tcp".to_string(),
            status: "online".to_string(),
            description: Some("测试用Modbus TCP通道".to_string()),
            config: None,
        },
    ];

    // 根据查询参数过滤
    if let Some(status) = &query.status {
        channels.retain(|c| c.status == *status);
    }
    if let Some(protocol) = &query.protocol {
        channels.retain(|c| c.protocol == *protocol);
    }

    Ok(success_response(channels))
}

/// 获取单个通道详情
pub async fn get_channel(
    Path(channel_id): Path<u32>,
    State(_state): State<AppState>,
) -> ApiResult<impl IntoResponse> {
    // 模拟数据
    let channel = match channel_id {
        1001 => Some(Channel {
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
        }),
        1002 => Some(Channel {
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
        }),
        1003 => Some(Channel {
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
        }),
        1004 => Some(Channel {
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
        }),
        1005 => Some(Channel {
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
        }),
        _ => None,
    };

    match channel {
        Some(ch) => Ok(success_response(ch)),
        None => Err(ApiError::NotFound(format!(
            "Channel {} not found",
            channel_id
        ))),
    }
}