use axum::{extract::State, response::IntoResponse};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::error::ApiResult;
use crate::response::success_response;
use crate::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub version: String,
    pub name: String,
    pub description: String,
    pub uptime: u64,
    pub services: HashMap<String, ServiceStatus>,
    pub metrics: SystemMetrics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceStatus {
    pub name: String,
    pub status: String,
    pub url: String,
    pub last_check: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemMetrics {
    pub total_channels: u32,
    pub online_channels: u32,
    pub total_points: u32,
    pub active_alarms: u32,
    pub websocket_connections: u32,
    pub redis_connected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceModel {
    pub id: String,
    pub name: String,
    pub manufacturer: String,
    pub model: String,
    pub version: String,
    pub properties: Vec<PropertyDefinition>,
    pub telemetry: Vec<TelemetryDefinition>,
    pub commands: Vec<CommandDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropertyDefinition {
    pub name: String,
    pub key: String,
    pub data_type: String,
    pub unit: Option<String>,
    pub writable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryDefinition {
    pub name: String,
    pub key: String,
    pub data_type: String,
    pub unit: Option<String>,
    pub min_value: Option<f64>,
    pub max_value: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandDefinition {
    pub name: String,
    pub key: String,
    pub input_params: Vec<ParamDefinition>,
    pub output_params: Vec<ParamDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamDefinition {
    pub name: String,
    pub key: String,
    pub data_type: String,
    pub required: bool,
}

/// 获取系统信息
pub async fn get_info(
    State(state): State<AppState>,
) -> ApiResult<impl IntoResponse> {
    // 检查Redis连接
    let redis_connected = state.redis_client.ping().await.is_ok();
    
    // 获取系统启动时间（模拟）
    let uptime = 3600 * 24 * 7; // 7天
    
    // 获取WebSocket连接数
    let ws_connections = {
        let hub = state.ws_hub.read().await;
        hub.session_count()
    };
    
    let system_info = SystemInfo {
        version: "2.0.0".to_string(),
        name: "VoltageEMS API Gateway".to_string(),
        description: "工业物联网能源管理系统API网关".to_string(),
        uptime,
        services: {
            let mut services = HashMap::new();
            services.insert("comsrv".to_string(), ServiceStatus {
                name: "comsrv".to_string(),
                status: "online".to_string(),
                url: "http://comsrv:8001".to_string(),
                last_check: chrono::Utc::now().timestamp_millis(),
            });
            services.insert("modsrv".to_string(), ServiceStatus {
                name: "modsrv".to_string(),
                status: "online".to_string(),
                url: "http://modsrv:8002".to_string(),
                last_check: chrono::Utc::now().timestamp_millis(),
            });
            services.insert("hissrv".to_string(), ServiceStatus {
                name: "hissrv".to_string(),
                status: "online".to_string(),
                url: "http://hissrv:8003".to_string(),
                last_check: chrono::Utc::now().timestamp_millis(),
            });
            services.insert("netsrv".to_string(), ServiceStatus {
                name: "netsrv".to_string(),
                status: "online".to_string(),
                url: "http://netsrv:8004".to_string(),
                last_check: chrono::Utc::now().timestamp_millis(),
            });
            services.insert("alarmsrv".to_string(), ServiceStatus {
                name: "alarmsrv".to_string(),
                status: "online".to_string(),
                url: "http://alarmsrv:8005".to_string(),
                last_check: chrono::Utc::now().timestamp_millis(),
            });
            services
        },
        metrics: SystemMetrics {
            total_channels: 5,
            online_channels: 4,
            total_points: 230,
            active_alarms: 1,
            websocket_connections: ws_connections as u32,
            redis_connected,
        },
    };

    Ok(success_response(system_info))
}

/// 获取设备模型列表
pub async fn get_device_models() -> ApiResult<impl IntoResponse> {
    let device_models = vec![
        DeviceModel {
            id: "power_meter_v1".to_string(),
            name: "智能电表".to_string(),
            manufacturer: "VoltageEMS".to_string(),
            model: "PM-2000".to_string(),
            version: "1.0.0".to_string(),
            properties: vec![
                PropertyDefinition {
                    name: "设备名称".to_string(),
                    key: "device_name".to_string(),
                    data_type: "string".to_string(),
                    unit: None,
                    writable: true,
                },
                PropertyDefinition {
                    name: "额定功率".to_string(),
                    key: "rated_power".to_string(),
                    data_type: "number".to_string(),
                    unit: Some("kW".to_string()),
                    writable: false,
                },
            ],
            telemetry: vec![
                TelemetryDefinition {
                    name: "有功功率".to_string(),
                    key: "active_power".to_string(),
                    data_type: "number".to_string(),
                    unit: Some("kW".to_string()),
                    min_value: Some(0.0),
                    max_value: Some(1000.0),
                },
                TelemetryDefinition {
                    name: "无功功率".to_string(),
                    key: "reactive_power".to_string(),
                    data_type: "number".to_string(),
                    unit: Some("kVar".to_string()),
                    min_value: Some(-500.0),
                    max_value: Some(500.0),
                },
                TelemetryDefinition {
                    name: "电压".to_string(),
                    key: "voltage".to_string(),
                    data_type: "number".to_string(),
                    unit: Some("V".to_string()),
                    min_value: Some(0.0),
                    max_value: Some(500.0),
                },
                TelemetryDefinition {
                    name: "电流".to_string(),
                    key: "current".to_string(),
                    data_type: "number".to_string(),
                    unit: Some("A".to_string()),
                    min_value: Some(0.0),
                    max_value: Some(2000.0),
                },
            ],
            commands: vec![
                CommandDefinition {
                    name: "复位".to_string(),
                    key: "reset".to_string(),
                    input_params: vec![],
                    output_params: vec![
                        ParamDefinition {
                            name: "结果".to_string(),
                            key: "result".to_string(),
                            data_type: "boolean".to_string(),
                            required: true,
                        },
                    ],
                },
                CommandDefinition {
                    name: "设置限值".to_string(),
                    key: "set_limit".to_string(),
                    input_params: vec![
                        ParamDefinition {
                            name: "功率限值".to_string(),
                            key: "power_limit".to_string(),
                            data_type: "number".to_string(),
                            required: true,
                        },
                    ],
                    output_params: vec![
                        ParamDefinition {
                            name: "结果".to_string(),
                            key: "result".to_string(),
                            data_type: "boolean".to_string(),
                            required: true,
                        },
                    ],
                },
            ],
        },
        DeviceModel {
            id: "circuit_breaker_v1".to_string(),
            name: "智能断路器".to_string(),
            manufacturer: "VoltageEMS".to_string(),
            model: "CB-1000".to_string(),
            version: "1.0.0".to_string(),
            properties: vec![
                PropertyDefinition {
                    name: "设备名称".to_string(),
                    key: "device_name".to_string(),
                    data_type: "string".to_string(),
                    unit: None,
                    writable: true,
                },
                PropertyDefinition {
                    name: "额定电流".to_string(),
                    key: "rated_current".to_string(),
                    data_type: "number".to_string(),
                    unit: Some("A".to_string()),
                    writable: false,
                },
            ],
            telemetry: vec![
                TelemetryDefinition {
                    name: "开关状态".to_string(),
                    key: "switch_status".to_string(),
                    data_type: "boolean".to_string(),
                    unit: None,
                    min_value: None,
                    max_value: None,
                },
                TelemetryDefinition {
                    name: "电流".to_string(),
                    key: "current".to_string(),
                    data_type: "number".to_string(),
                    unit: Some("A".to_string()),
                    min_value: Some(0.0),
                    max_value: Some(1000.0),
                },
            ],
            commands: vec![
                CommandDefinition {
                    name: "分闸".to_string(),
                    key: "open".to_string(),
                    input_params: vec![],
                    output_params: vec![
                        ParamDefinition {
                            name: "结果".to_string(),
                            key: "result".to_string(),
                            data_type: "boolean".to_string(),
                            required: true,
                        },
                    ],
                },
                CommandDefinition {
                    name: "合闸".to_string(),
                    key: "close".to_string(),
                    input_params: vec![],
                    output_params: vec![
                        ParamDefinition {
                            name: "结果".to_string(),
                            key: "result".to_string(),
                            data_type: "boolean".to_string(),
                            required: true,
                        },
                    ],
                },
            ],
        },
    ];

    Ok(success_response(device_models))
}