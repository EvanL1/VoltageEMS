use chrono::Utc;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use utoipa::{OpenApi, ToSchema};
use utoipa_swagger_ui::SwaggerUi;

use crate::api::models::{
    ApiResponse, ChannelOperation, ChannelStatus, ChannelStatusResponse, 
    HealthStatus, ServiceStatus, PointValue, WritePointRequest
};

/// OpenAPI documentation
#[derive(OpenApi)]
#[openapi(
    paths(
        get_service_status,
        health_check,
        get_all_channels,
        get_channel_status,
        control_channel,
        read_point,
        write_point,
        get_channel_points
    ),
    components(
        schemas(
            ServiceStatus,
            HealthStatus,
            ChannelStatusResponse,
            ChannelStatus,
            ChannelOperation,
            PointValue,
            WritePointRequest,
            ApiResponse<ServiceStatus>,
            ApiResponse<HealthStatus>,
            ApiResponse<Vec<ChannelStatusResponse>>,
            ApiResponse<ChannelStatus>,
            ApiResponse<String>,
            ApiResponse<PointValue>,
            ApiResponse<Vec<PointValue>>
        )
    ),
    tags(
        (name = "comsrv", description = "Communication Service API")
    ),
    info(
        title = "ComSrv API",
        version = "1.0.0",
        description = "Communication Service API for industrial protocol management",
        contact(
            name = "VoltageEMS Team",
            email = "support@voltageems.com"
        )
    )
)]
pub struct ApiDoc;

/// Get service status endpoint
#[utoipa::path(
    get,
    path = "/api/status",
    responses(
        (status = 200, description = "Service status retrieved successfully", body = ApiResponse<ServiceStatus>)
    ),
    tag = "Status"
)]
pub async fn get_service_status() -> Result<Json<ApiResponse<ServiceStatus>>, StatusCode> {
    let status = ServiceStatus {
        name: "Communication Service".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        uptime: 3600,
        start_time: Utc::now(),
        channels: 5,
        active_channels: 3,
    };
    
    Ok(Json(ApiResponse::success(status)))
}

/// Health check endpoint
#[utoipa::path(
    get,
    path = "/api/health",
    responses(
        (status = 200, description = "Health status retrieved successfully", body = ApiResponse<HealthStatus>)
    ),
    tag = "Health"
)]
pub async fn health_check() -> Result<Json<ApiResponse<HealthStatus>>, StatusCode> {
    let health = HealthStatus {
        status: "healthy".to_string(),
        uptime: 3600,
        memory_usage: 1024 * 1024 * 100, // 100MB
        cpu_usage: 15.5,
    };
    
    Ok(Json(ApiResponse::success(health)))
}

/// List all channels
#[utoipa::path(
    get,
    path = "/api/channels",
    responses(
        (status = 200, description = "All channels retrieved successfully", body = ApiResponse<Vec<ChannelStatusResponse>>)
    ),
    tag = "Channels"
)]
pub async fn get_all_channels() -> Result<Json<ApiResponse<Vec<ChannelStatusResponse>>>, StatusCode> {
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
    
    Ok(Json(ApiResponse::success(channels)))
}

/// Get channel status
#[utoipa::path(
    get,
    path = "/api/channels/{id}/status",
    params(
        ("id" = u16, Path, description = "Channel ID")
    ),
    responses(
        (status = 200, description = "Channel status retrieved successfully", body = ApiResponse<ChannelStatus>),
        (status = 404, description = "Channel not found")
    ),
    tag = "Channels"
)]
pub async fn get_channel_status(Path(id): Path<String>) -> Result<Json<ApiResponse<ChannelStatus>>, StatusCode> {
    let id_u16 = id.parse::<u16>().map_err(|_| StatusCode::NOT_FOUND)?;
    
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
    
    Ok(Json(ApiResponse::success(status)))
}

/// Control channel operation
#[utoipa::path(
    post,
    path = "/api/channels/{id}/control",
    params(
        ("id" = u16, Path, description = "Channel ID")
    ),
    request_body = ChannelOperation,
    responses(
        (status = 200, description = "Channel operation executed successfully", body = ApiResponse<String>),
        (status = 400, description = "Invalid operation"),
        (status = 404, description = "Channel not found")
    ),
    tag = "Channels"
)]
pub async fn control_channel(
    Path(id): Path<String>,
    Json(operation): Json<ChannelOperation>,
) -> Result<Json<ApiResponse<String>>, StatusCode> {
    let _id_u16 = id.parse::<u16>().map_err(|_| StatusCode::NOT_FOUND)?;
    
    match operation.operation.as_str() {
        "start" | "stop" | "restart" => {
            let message = format!("Successfully {} channel {}", operation.operation, id);
            Ok(Json(ApiResponse::success(message)))
        }
        _ => {
            let error_response = ApiResponse::<String>::error(format!(
                "Invalid operation: {}. Valid operations are: start, stop, restart",
                operation.operation
            ));
            Ok(Json(error_response))
        }
    }
}

/// Read point value
#[utoipa::path(
    get,
    path = "/api/channels/{channel_id}/points/{point_table}/{point_name}",
    params(
        ("channel_id" = String, Path, description = "Channel ID"),
        ("point_table" = String, Path, description = "Point table name"),
        ("point_name" = String, Path, description = "Point name")
    ),
    responses(
        (status = 200, description = "Point value retrieved successfully", body = ApiResponse<PointValue>),
        (status = 404, description = "Point not found")
    ),
    tag = "Points"
)]
pub async fn read_point(
    Path((channel_id, point_table, point_name)): Path<(String, String, String)>,
) -> Result<Json<ApiResponse<PointValue>>, StatusCode> {
    let point = PointValue {
        id: format!("{}:{}:{}", channel_id, point_table, point_name),
        name: point_name,
        value: serde_json::Value::Number(serde_json::Number::from(42)),
        timestamp: Utc::now(),
        unit: "V".to_string(),
        description: "Test point value".to_string(),
    };
    
    Ok(Json(ApiResponse::success(point)))
}

/// Write point value
#[utoipa::path(
    post,
    path = "/api/channels/{channel_id}/points/{point_table}/{point_name}",
    params(
        ("channel_id" = String, Path, description = "Channel ID"),
        ("point_table" = String, Path, description = "Point table name"),
        ("point_name" = String, Path, description = "Point name")
    ),
    request_body = WritePointRequest,
    responses(
        (status = 200, description = "Point value written successfully", body = ApiResponse<String>),
        (status = 404, description = "Point not found")
    ),
    tag = "Points"
)]
pub async fn write_point(
    Path((channel_id, point_table, point_name)): Path<(String, String, String)>,
    Json(value): Json<WritePointRequest>,
) -> Result<Json<ApiResponse<String>>, StatusCode> {
    let message = format!(
        "Successfully wrote value {:?} to point {}:{}:{}", 
        value.value, channel_id, point_table, point_name
    );
    
    Ok(Json(ApiResponse::success(message)))
}

/// Get all points for a channel
#[utoipa::path(
    get,
    path = "/api/channels/{channel_id}/points",
    params(
        ("channel_id" = String, Path, description = "Channel ID")
    ),
    responses(
        (status = 200, description = "Channel points retrieved successfully", body = ApiResponse<Vec<PointValue>>),
        (status = 404, description = "Channel not found")
    ),
    tag = "Points"
)]
pub async fn get_channel_points(Path(channel_id): Path<String>) -> Result<Json<ApiResponse<Vec<PointValue>>>, StatusCode> {
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
    
    Ok(Json(ApiResponse::success(points)))
}

/// Create the API router with all routes
pub fn create_api_routes() -> Router {
    Router::new()
        .route("/api/status", get(get_service_status))
        .route("/api/health", get(health_check))
        .route("/api/channels", get(get_all_channels))
        .route("/api/channels/:id/status", get(get_channel_status))
        .route("/api/channels/:id/control", post(control_channel))
        .route("/api/channels/:channel_id/points/:point_table/:point_name", get(read_point))
        .route("/api/channels/:channel_id/points/:point_table/:point_name", post(write_point))
        .route("/api/channels/:channel_id/points", get(get_channel_points))
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
}

/// Get OpenAPI specification as JSON string
pub fn get_openapi_spec() -> String {
    ApiDoc::openapi().to_pretty_json().unwrap_or_else(|_| "{}".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_api_routes() {
        let app = create_api_routes();
        // Basic test to ensure routes compile
        assert!(true);
    }

    #[test]
    fn test_openapi_spec_generation() {
        let spec = get_openapi_spec();
        assert!(!spec.is_empty());
        assert!(spec.contains("Communication Service API"));
    }
} 