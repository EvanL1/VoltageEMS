pub mod alarmsrv;
pub mod auth;
pub mod comsrv;
pub mod health;
pub mod hissrv;
pub mod modsrv;
pub mod netsrv;

use actix_web::{web, HttpRequest, HttpResponse};
use log::{debug, error};
use reqwest::Client;
use std::sync::Arc;
use std::time::Duration;

use crate::config::Config;
use crate::error::{ApiError, ApiResult};

/// Common proxy function for forwarding requests to backend services
pub async fn proxy_request(
    service_name: &str,
    path: &str,
    method: &str,
    req: &HttpRequest,
    body: Option<web::Bytes>,
    config: &Config,
    client: &Arc<Client>,
) -> ApiResult<HttpResponse> {
    let service_url = config
        .get_service_url(service_name)
        .ok_or_else(|| ApiError::ServiceNotFound(service_name.to_string()))?;

    let timeout_seconds = config
        .get_service_timeout(service_name)
        .unwrap_or(30);

    let full_url = format!("{}{}", service_url, path);
    
    debug!("Proxying {} request to {}", method, full_url);

    let mut request = match method.to_uppercase().as_str() {
        "GET" => client.get(&full_url),
        "POST" => client.post(&full_url),
        "PUT" => client.put(&full_url),
        "DELETE" => client.delete(&full_url),
        "PATCH" => client.patch(&full_url),
        _ => return Err(ApiError::BadRequest(format!("Unsupported method: {}", method))),
    };

    // Copy headers from original request
    for (name, value) in req.headers() {
        if name != "host" && name != "connection" {
            if let Ok(value_str) = value.to_str() {
                request = request.header(name.as_str(), value_str);
            }
        }
    }

    // Add body if present
    if let Some(body_bytes) = body {
        request = request.body(body_bytes.to_vec());
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
                ApiError::BadGateway(format!("Failed to read response from {}", service_name))
            })?;

            // Build response
            let mut res = HttpResponse::build(status);
            
            // Copy response headers
            for (name, value) in headers {
                if let Some(name) = name {
                    if name != "connection" && name != "transfer-encoding" {
                        res.insert_header((name.as_str(), value.as_bytes()));
                    }
                }
            }

            Ok(res.body(body))
        }
        Err(e) => {
            error!("Failed to proxy request to {}: {}", service_name, e);
            if e.is_timeout() {
                Err(ApiError::Timeout(format!("Request to {} timed out", service_name)))
            } else if e.is_connect() {
                Err(ApiError::ServiceUnavailable(format!("{} is unavailable", service_name)))
            } else {
                Err(ApiError::BadGateway(format!("Failed to communicate with {}", service_name)))
            }
        }
    }
}