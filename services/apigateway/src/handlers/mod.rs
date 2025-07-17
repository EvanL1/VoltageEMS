pub mod alarmsrv;
pub mod auth;
pub mod channels;
pub mod comsrv;
pub mod data;
pub mod health;
pub mod hissrv;
pub mod modsrv;
pub mod netsrv;
pub mod rulesrv;
pub mod system;

use axum::{
    body::Body,
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use bytes::Bytes;
use log::{debug, error};
use std::time::Duration;

use crate::error::{ApiError, ApiResult};
use crate::AppState;

/// Common proxy function for forwarding requests to backend services
pub async fn proxy_request(
    service_name: &str,
    path: &str,
    method: &str,
    headers: &HeaderMap,
    body: Option<Bytes>,
    state: &AppState,
) -> ApiResult<Response> {
    let service_url = state
        .config
        .get_service_url(service_name)
        .ok_or_else(|| ApiError::NotFound(format!("Service not found: {}", service_name)))?;

    let timeout_seconds = state.config.get_service_timeout(service_name).unwrap_or(30);

    let full_url = format!("{}{}", service_url, path);

    debug!("Proxying {} request to {}", method, full_url);

    let mut request = match method.to_uppercase().as_str() {
        "GET" => state.http_client.get(&full_url),
        "POST" => state.http_client.post(&full_url),
        "PUT" => state.http_client.put(&full_url),
        "DELETE" => state.http_client.delete(&full_url),
        "PATCH" => state.http_client.patch(&full_url),
        _ => {
            return Err(ApiError::BadRequest(format!(
                "Unsupported method: {}",
                method
            )))
        }
    };

    // Copy headers from original request
    for (name, value) in headers {
        if name != "host" && name != "connection" {
            if let Ok(value_str) = value.to_str() {
                request = request.header(name.as_str(), value_str);
            }
        }
    }

    // Add body if present
    if let Some(body_bytes) = body {
        request = request.body(body_bytes);
    }

    // Set timeout
    request = request.timeout(Duration::from_secs(timeout_seconds));

    // Send request
    match request.send().await {
        Ok(response) => {
            let status = response.status();
            let headers = response.headers().clone();
            let body = response.bytes().await.map_err(|e| {
                error!("Failed to read response body: {}", e);
                ApiError::ServiceError(format!("Failed to read response from {}", service_name))
            })?;

            // Build response
            let mut res = Response::builder()
                .status(StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR));

            // Copy response headers
            for (name, value) in headers {
                if let Some(name) = name {
                    if name != "connection" && name != "transfer-encoding" {
                        res = res.header(name.as_str(), value.as_bytes());
                    }
                }
            }

            Ok(res.body(Body::from(body)).unwrap())
        }
        Err(e) => {
            error!("Failed to proxy request to {}: {}", service_name, e);
            if e.is_timeout() {
                Err(ApiError::ServiceTimeout(format!(
                    "Request to {} timed out",
                    service_name
                )))
            } else if e.is_connect() {
                Err(ApiError::ServiceUnavailable(format!(
                    "{} is unavailable",
                    service_name
                )))
            } else {
                Err(ApiError::ServiceError(format!(
                    "Failed to communicate with {}",
                    service_name
                )))
            }
        }
    }
}

/// Create a generic proxy handler for a service
pub async fn handle_proxy(
    service_name: &'static str,
    req: Request,
    State(state): State<AppState>,
) -> Response {
    let path = req.uri().path().to_string();
    let method = req.method().as_str().to_string();
    let headers = req.headers().clone();
    
    let body = match axum::body::to_bytes(req.into_body(), usize::MAX).await {
        Ok(bytes) => Some(bytes),
        Err(_) => None,
    };

    match proxy_request(service_name, &path, &method, &headers, body, &state).await {
        Ok(response) => response,
        Err(error) => error.into_response(),
    }
}