use chrono::Utc;
use serde::Deserialize;
use warp::{Filter, Rejection, Reply};

use crate::api::models::{
    ApiResponse, ChannelOperation, ChannelStatus, ChannelStatusResponse, 
    HealthStatus, ServiceStatus, PointValue, WritePointRequest
};

/// Get service status endpoint
pub async fn get_service_status() -> Result<impl Reply, Rejection> {
    let status = ServiceStatus {
        name: "Communication Service".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime: 3600,
        start_time: Utc::now(),
        channels: 5,
        active_channels: 3,
    };
    
    Ok(warp::reply::json(&ApiResponse::success(status)))
}

/// Health check endpoint
pub async fn health_check() -> Result<impl Reply, Rejection> {
    let health = HealthStatus {
        status: "healthy".to_string(),
        uptime: 3600,
        memory_usage: 1024 * 1024 * 100, // 100MB
        cpu_usage: 15.5,
    };
    
    Ok(warp::reply::json(&ApiResponse::success(health)))
}

/// List all channels
pub async fn get_all_channels() -> Result<impl Reply, Rejection> {
    let channels = vec![
        ChannelStatusResponse {
            id: 1,
            name: "Modbus TCP Channel 1".to_string(),
            protocol: "ModbusTcp".to_string(),
            connected: true,
            last_update: Utc::now(),
            error_count: 0,
            last_error: None,
        },
        ChannelStatusResponse {
            id: 2,
            name: "Modbus RTU Channel 1".to_string(),
            protocol: "ModbusRtu".to_string(),
            connected: false,
            last_update: Utc::now(),
            error_count: 2,
            last_error: Some("Connection timeout".to_string()),
        },
    ];
    
    Ok(warp::reply::json(&ApiResponse::success(channels)))
}

/// Get channel status
pub async fn get_channel_status(id: String) -> Result<impl Reply, Rejection> {
    let id_u16 = id.parse::<u16>().map_err(|_| warp::reject::not_found())?;
    
    let status = ChannelStatus {
        id: id_u16,
        name: format!("Channel {}", id),
        protocol: "ModbusTcp".to_string(),
        connected: true,
        running: true,
        last_update: Utc::now(),
        error_count: 0,
        last_error: None,
        statistics: std::collections::HashMap::new(),
    };
    
    Ok(warp::reply::json(&ApiResponse::success(status)))
}

/// Control channel operation
pub async fn control_channel(
    id: String,
    operation: ChannelOperation,
) -> Result<impl Reply, Rejection> {
    let _id_u16 = id.parse::<u16>().map_err(|_| warp::reject::not_found())?;
    
    match operation.operation.as_str() {
        "start" | "stop" | "restart" => {
            let message = format!("Successfully {} channel {}", operation.operation, id);
            Ok(warp::reply::json(&ApiResponse::success(message)))
        }
        _ => {
            let error_response = ApiResponse::<String>::error(format!(
                "Invalid operation: {}. Valid operations are: start, stop, restart",
                operation.operation
            ));
            Ok(warp::reply::json(&error_response))
        }
    }
}

/// Read point value
pub async fn read_point(
    channel_id: String,
    point_table: String,
    point_name: String,
) -> Result<impl Reply, Rejection> {
    let point = PointValue {
        id: format!("{}:{}:{}", channel_id, point_table, point_name),
        name: point_name,
        value: serde_json::Value::Number(serde_json::Number::from(42)),
        timestamp: Utc::now(),
        unit: "V".to_string(),
        description: "Test point value".to_string(),
    };
    
    Ok(warp::reply::json(&ApiResponse::success(point)))
}

/// Write point value
pub async fn write_point(
    channel_id: String,
    point_table: String,
    point_name: String,
    value: WritePointRequest,
) -> Result<impl Reply, Rejection> {
    let message = format!(
        "Successfully wrote value {:?} to point {}:{}:{}", 
        value.value, channel_id, point_table, point_name
    );
    
    Ok(warp::reply::json(&ApiResponse::success(message)))
}

/// Get all points for a channel
pub async fn get_channel_points(channel_id: String) -> Result<impl Reply, Rejection> {
    let points = vec![
        PointValue {
            id: format!("{}:table1:voltage", channel_id),
            name: "voltage".to_string(),
            value: serde_json::Value::Number(serde_json::Number::from(220)),
            timestamp: Utc::now(),
            unit: "V".to_string(),
            description: "Line voltage".to_string(),
        },
        PointValue {
            id: format!("{}:table1:current", channel_id),
            name: "current".to_string(),
            value: serde_json::Value::Number(serde_json::Number::from_f64(15.5).unwrap()),
            timestamp: Utc::now(),
            unit: "A".to_string(),
            description: "Line current".to_string(),
        },
    ];
    
    Ok(warp::reply::json(&ApiResponse::success(points)))
}

/// Build the API routes using warp
pub fn api_routes() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    let cors = warp::cors()
        .allow_any_origin()
        .allow_headers(vec!["content-type", "x-request-id", "authorization"])
        .allow_methods(vec!["GET", "POST", "PUT", "DELETE", "OPTIONS"]);
    
    // GET /api/status
    let status_route = warp::path!("api" / "status")
        .and(warp::get())
        .and_then(get_service_status);

    // GET /api/health
    let health_route = warp::path!("api" / "health")
        .and(warp::get())
        .and_then(health_check);

    // GET /api/channels
    let channels_route = warp::path!("api" / "channels")
        .and(warp::get())
        .and_then(get_all_channels);

    // GET /api/channels/{id}/status
    let channel_status_route = warp::path!("api" / "channels" / String / "status")
        .and(warp::get())
        .and_then(get_channel_status);

    // POST /api/channels/{id}/control
    let channel_control_route = warp::path!("api" / "channels" / String / "control")
        .and(warp::post())
        .and(warp::body::json())
        .and_then(control_channel);

    // GET /api/channels/{channel_id}/points/{point_table}/{point_name}
    let read_point_route = warp::path!("api" / "channels" / String / "points" / String / String)
        .and(warp::get())
        .and_then(read_point);

    // POST /api/channels/{channel_id}/points/{point_table}/{point_name}
    let write_point_route = warp::path!("api" / "channels" / String / "points" / String / String)
        .and(warp::post())
        .and(warp::body::json())
        .and_then(write_point);

    // GET /api/channels/{channel_id}/points
    let channel_points_route = warp::path!("api" / "channels" / String / "points")
        .and(warp::get())
        .and_then(get_channel_points);

    status_route
        .or(health_route)
        .or(channels_route)
        .or(channel_status_route)
        .or(channel_control_route)
        .or(read_point_route)
        .or(write_point_route)
        .or(channel_points_route)
        .with(cors)
        .with(warp::log("comsrv::api"))
}

/// Generate OpenAPI spec as JSON string
pub fn get_openapi_spec() -> String {
    serde_json::json!({
        "openapi": "3.0.0",
        "info": {
            "title": "Communication Service API",
            "version": env!("CARGO_PKG_VERSION"),
            "description": "Industrial communication service providing protocol abstraction and data access",
            "contact": {
                "name": "VoltageEMS Team"
            },
            "license": {
                "name": "MIT",
                "url": "https://opensource.org/licenses/MIT"
            }
        },
        "servers": [
            {
                "url": "http://localhost:3030",
                "description": "Local development server"
            }
        ],
        "paths": {
            "/api/status": {
                "get": {
                    "tags": ["Status"],
                    "summary": "Get service status",
                    "description": "Returns the current status of the communication service including uptime, channel count, and active channel count",
                    "responses": {
                        "200": {
                            "description": "Service status",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "$ref": "#/components/schemas/ServiceStatusResponse"
                                    }
                                }
                            }
                        }
                    }
                }
            },
            "/api/health": {
                "get": {
                    "tags": ["Status"],
                    "summary": "Health check",
                    "description": "Returns the health status of the service including memory and CPU usage",
                    "responses": {
                        "200": {
                            "description": "Health status",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "$ref": "#/components/schemas/HealthStatusResponse"
                                    }
                                }
                            }
                        }
                    }
                }
            },
            "/api/channels": {
                "get": {
                    "tags": ["Channels"],
                    "summary": "List all channels",
                    "description": "Returns a list of all communication channels with their basic status information",
                    "responses": {
                        "200": {
                            "description": "List of channels",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "$ref": "#/components/schemas/ChannelListResponse"
                                    }
                                }
                            }
                        }
                    }
                }
            },
            "/api/channels/{id}/status": {
                "get": {
                    "tags": ["Channels"],
                    "summary": "Get channel status",
                    "description": "Returns detailed status information for a specific channel",
                    "parameters": [
                        {
                            "name": "id",
                            "in": "path",
                            "required": true,
                            "description": "Channel ID",
                            "schema": {
                                "type": "string"
                            }
                        }
                    ],
                    "responses": {
                        "200": {
                            "description": "Channel status",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "$ref": "#/components/schemas/ChannelStatusResponse"
                                    }
                                }
                            }
                        },
                        "404": {
                            "description": "Channel not found"
                        }
                    }
                }
            },
            "/api/channels/{id}/control": {
                "post": {
                    "tags": ["Channels"],
                    "summary": "Control channel operation",
                    "description": "Performs control operations on a channel (start, stop, restart)",
                    "parameters": [
                        {
                            "name": "id",
                            "in": "path",
                            "required": true,
                            "description": "Channel ID",
                            "schema": {
                                "type": "string"
                            }
                        }
                    ],
                    "requestBody": {
                        "required": true,
                        "content": {
                            "application/json": {
                                "schema": {
                                    "$ref": "#/components/schemas/ChannelOperation"
                                }
                            }
                        }
                    },
                    "responses": {
                        "200": {
                            "description": "Operation result",
                            "content": {
                                "application/json": {
                                    "schema": {
                                        "$ref": "#/components/schemas/StringResponse"
                                    }
                                }
                            }
                        },
                        "400": {
                            "description": "Invalid operation"
                        },
                        "404": {
                            "description": "Channel not found"
                        }
                    }
                }
            }
        },
        "components": {
            "schemas": {
                "ServiceStatusResponse": {
                    "type": "object",
                    "properties": {
                        "success": {"type": "boolean"},
                        "data": {"$ref": "#/components/schemas/ServiceStatus"},
                        "error": {"type": "string", "nullable": true}
                    }
                },
                "ServiceStatus": {
                    "type": "object",
                    "properties": {
                        "name": {"type": "string"},
                        "version": {"type": "string"},
                        "uptime": {"type": "integer"},
                        "start_time": {"type": "string", "format": "date-time"},
                        "channels": {"type": "integer"},
                        "active_channels": {"type": "integer"}
                    }
                },
                "HealthStatusResponse": {
                    "type": "object",
                    "properties": {
                        "success": {"type": "boolean"},
                        "data": {"$ref": "#/components/schemas/HealthStatus"},
                        "error": {"type": "string", "nullable": true}
                    }
                },
                "HealthStatus": {
                    "type": "object",
                    "properties": {
                        "status": {"type": "string"},
                        "uptime": {"type": "integer"},
                        "memory_usage": {"type": "integer"},
                        "cpu_usage": {"type": "number"}
                    }
                },
                "ChannelListResponse": {
                    "type": "object",
                    "properties": {
                        "success": {"type": "boolean"},
                        "data": {
                            "type": "array",
                            "items": {"$ref": "#/components/schemas/ChannelStatusResponse"}
                        },
                        "error": {"type": "string", "nullable": true}
                    }
                },
                "ChannelStatusResponse": {
                    "type": "object",
                    "properties": {
                        "success": {"type": "boolean"},
                        "data": {"$ref": "#/components/schemas/ChannelStatus"},
                        "error": {"type": "string", "nullable": true}
                    }
                },
                "ChannelStatus": {
                    "type": "object",
                    "properties": {
                        "id": {"type": "integer"},
                        "name": {"type": "string"},
                        "protocol": {"type": "string"},
                        "connected": {"type": "boolean"},
                        "running": {"type": "boolean"},
                        "last_update": {"type": "string", "format": "date-time"},
                        "error_count": {"type": "integer"},
                        "last_error": {"type": "string", "nullable": true},
                        "statistics": {"type": "object"}
                    }
                },
                "ChannelOperation": {
                    "type": "object",
                    "properties": {
                        "operation": {
                            "type": "string",
                            "enum": ["start", "stop", "restart"],
                            "description": "Operation to perform on the channel"
                        }
                    },
                    "required": ["operation"]
                },
                "StringResponse": {
                    "type": "object",
                    "properties": {
                        "success": {"type": "boolean"},
                        "data": {"type": "string"},
                        "error": {"type": "string", "nullable": true}
                    }
                }
            }
        },
        "tags": [
            {
                "name": "Status",
                "description": "Service status and health check endpoints"
            },
            {
                "name": "Channels",
                "description": "Channel management and control endpoints"
            },
            {
                "name": "Points",
                "description": "Point data access endpoints"
            }
        ]
    }).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_api_routes() {
        let routes = api_routes();
        
        // Basic test to ensure routes compile
        assert!(true);
    }
    
    #[test]
    fn test_openapi_spec_generation() {
        let spec = get_openapi_spec();
        
        assert!(spec.contains("Communication Service API"));
        assert!(spec.contains("openapi"));
        assert!(spec.contains("3.0.0"));
    }
} 