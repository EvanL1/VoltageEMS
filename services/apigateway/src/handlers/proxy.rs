use crate::auth::Claims;
use crate::error::ApiGatewayError;
use axum::{
    extract::{Request, State},
    http::header,
    response::Response,
};

/// 统一的服务代理处理器
async fn proxy_handler(
    service_name: &str,
    State(app_state): State<crate::AppState>,
    _claims: Claims,
    req: Request,
) -> Result<Response, ApiGatewayError> {
    // 获取服务配置
    let service_url = match service_name {
        "comsrv" => &app_state.config.services.comsrv.url,
        "modsrv" => &app_state.config.services.modsrv.url,
        "hissrv" => &app_state.config.services.hissrv.url,
        "netsrv" => &app_state.config.services.netsrv.url,
        "alarmsrv" => &app_state.config.services.alarmsrv.url,
        "rulesrv" => &app_state.config.services.rulesrv.url,
        _ => {
            return Err(ApiGatewayError::BadRequest(format!(
                "Unknown service: {}",
                service_name
            )))
        },
    };

    // 提取请求信息
    let (parts, body) = req.into_parts();
    let body_bytes = axum::body::to_bytes(body, usize::MAX)
        .await
        .map_err(|e| ApiGatewayError::InternalError(format!("Failed to read body: {}", e)))?;

    // 构建目标URL
    let path = parts.uri.path();
    let service_prefix = format!("/api/{}/", service_name);
    let target_path = path.strip_prefix(&service_prefix).unwrap_or("");
    let query = parts
        .uri
        .query()
        .map(|q| format!("?{}", q))
        .unwrap_or_default();
    let target_url = format!("{}/{}{}", service_url, target_path, query);

    // 构建代理请求
    let mut proxy_req = app_state
        .http_client
        .request(parts.method, &target_url)
        .body(body_bytes);

    // 复制请求头（除了Host和Authorization）
    for (name, value) in &parts.headers {
        if name != header::HOST && name != header::AUTHORIZATION {
            proxy_req = proxy_req.header(name, value);
        }
    }

    // 发送请求
    let resp = proxy_req.send().await.map_err(|e| {
        ApiGatewayError::ServiceUnavailable(format!("{} error: {}", service_name, e))
    })?;

    // 构建响应
    let status = resp.status();
    let mut response = Response::builder().status(status);

    // 复制响应头
    for (name, value) in resp.headers() {
        if name != header::CONNECTION && name != header::TRANSFER_ENCODING {
            response = response.header(name, value);
        }
    }

    // 返回响应体
    let body = resp
        .bytes()
        .await
        .map_err(|e| ApiGatewayError::InternalError(format!("Failed to read response: {}", e)))?;

    response
        .body(body.into())
        .map_err(|_| ApiGatewayError::InternalError("Failed to build response body".to_string()))
}

/// 为每个服务创建代理handler
pub async fn comsrv_proxy(
    State(app_state): State<crate::AppState>,
    req: Request,
) -> Result<Response, ApiGatewayError> {
    // Claims将通过middleware注入到request extensions中
    let claims = req
        .extensions()
        .get::<Claims>()
        .cloned()
        .ok_or(ApiGatewayError::Unauthorized)?;
    proxy_handler("comsrv", State(app_state), claims, req).await
}

pub async fn modsrv_proxy(
    State(app_state): State<crate::AppState>,
    req: Request,
) -> Result<Response, ApiGatewayError> {
    let claims = req
        .extensions()
        .get::<Claims>()
        .cloned()
        .ok_or(ApiGatewayError::Unauthorized)?;
    proxy_handler("modsrv", State(app_state), claims, req).await
}

pub async fn hissrv_proxy(
    State(app_state): State<crate::AppState>,
    req: Request,
) -> Result<Response, ApiGatewayError> {
    let claims = req
        .extensions()
        .get::<Claims>()
        .cloned()
        .ok_or(ApiGatewayError::Unauthorized)?;
    proxy_handler("hissrv", State(app_state), claims, req).await
}

pub async fn netsrv_proxy(
    State(app_state): State<crate::AppState>,
    req: Request,
) -> Result<Response, ApiGatewayError> {
    let claims = req
        .extensions()
        .get::<Claims>()
        .cloned()
        .ok_or(ApiGatewayError::Unauthorized)?;
    proxy_handler("netsrv", State(app_state), claims, req).await
}

pub async fn alarmsrv_proxy(
    State(app_state): State<crate::AppState>,
    req: Request,
) -> Result<Response, ApiGatewayError> {
    let claims = req
        .extensions()
        .get::<Claims>()
        .cloned()
        .ok_or(ApiGatewayError::Unauthorized)?;
    proxy_handler("alarmsrv", State(app_state), claims, req).await
}

pub async fn rulesrv_proxy(
    State(app_state): State<crate::AppState>,
    req: Request,
) -> Result<Response, ApiGatewayError> {
    let claims = req
        .extensions()
        .get::<Claims>()
        .cloned()
        .ok_or(ApiGatewayError::Unauthorized)?;
    proxy_handler("rulesrv", State(app_state), claims, req).await
}
