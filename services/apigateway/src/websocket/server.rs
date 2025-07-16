use actix::{Actor, Addr};
use actix_web::{web, HttpRequest, HttpResponse};
use actix_web_actors::ws;
use log::{debug, error, info};
use std::sync::Arc;

use crate::auth::jwt::JwtManager;
use crate::error::{ApiError, ApiResult};
use crate::redis_client::RedisClient;
use crate::websocket::{hub::WsHub, session::WsSession};

/// WebSocket连接处理器
pub async fn ws_handler(
    req: HttpRequest,
    stream: web::Payload,
    hub: web::Data<Addr<WsHub>>,
    redis: web::Data<Arc<RedisClient>>,
) -> ApiResult<HttpResponse> {
    info!("WebSocket connection request from: {:?}", req.peer_addr());
    
    // 提取认证令牌（可选）
    let auth_token = extract_auth_token(&req);
    
    // 如果提供了令牌，验证它
    let user_info = if let Some(token) = &auth_token {
        match JwtManager::verify_token(token) {
            Ok(claims) => {
                info!("WebSocket authenticated user: {}", claims.sub);
                Some((claims.sub, token.clone()))
            }
            Err(e) => {
                // 认证失败，但我们允许未认证的连接（可以限制功能）
                debug!("WebSocket auth failed: {}, allowing anonymous connection", e);
                None
            }
        }
    } else {
        debug!("WebSocket connection without auth token");
        None
    };
    
    // 创建WebSocket会话
    let mut session = WsSession::new(hub.get_ref().clone(), redis.get_ref().clone());
    
    // 设置认证信息
    if let Some((user_id, token)) = user_info {
        session.user_id = Some(user_id);
        session.auth_token = Some(token);
    }
    
    // 开始WebSocket握手
    let resp = ws::start(session, &req, stream)
        .map_err(|e| {
            error!("WebSocket handshake failed: {}", e);
            ApiError::InternalError(format!("WebSocket handshake failed: {}", e))
        })?;
    
    Ok(resp)
}

/// 从请求中提取认证令牌
fn extract_auth_token(req: &HttpRequest) -> Option<String> {
    // 1. 尝试从Authorization头获取
    if let Some(auth_header) = req.headers().get("Authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            if auth_str.starts_with("Bearer ") {
                return Some(auth_str[7..].to_string());
            }
        }
    }
    
    // 2. 尝试从查询参数获取
    if let Ok(query) = web::Query::<TokenQuery>::from_query(req.query_string()) {
        if let Some(token) = query.token.clone() {
            return Some(token);
        }
    }
    
    // 3. 尝试从Cookie获取
    if let Some(cookie_header) = req.headers().get("Cookie") {
        if let Ok(cookie_str) = cookie_header.to_str() {
            for cookie in cookie_str.split(';') {
                let cookie = cookie.trim();
                if cookie.starts_with("token=") {
                    return Some(cookie[6..].to_string());
                }
            }
        }
    }
    
    None
}

/// 查询参数中的令牌
#[derive(serde::Deserialize)]
struct TokenQuery {
    token: Option<String>,
}

/// 创建WebSocket Hub Actor
pub fn create_hub(redis: Arc<RedisClient>) -> Addr<WsHub> {
    WsHub::new(redis).start()
}