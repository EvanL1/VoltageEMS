use actix::Message;
use serde_json;

use super::messages::WsMessage;
use crate::error::{ApiError, ApiResult};

/// 编码WebSocket消息为JSON字符串
pub fn encode_message(msg: &WsMessage) -> ApiResult<String> {
    serde_json::to_string(msg)
        .map_err(|e| ApiError::InternalError(format!("Failed to encode message: {}", e)))
}

/// 解码JSON字符串为WebSocket消息
pub fn decode_message(text: &str) -> ApiResult<WsMessage> {
    serde_json::from_str(text)
        .map_err(|e| ApiError::BadRequest(format!("Failed to decode message: {}", e)))
}

/// Actor消息包装器
#[derive(Clone, Message)]
#[rtype(result = "()")]
pub struct WsMessageWrapper(pub WsMessage);

/// 文本消息
#[derive(Message)]
#[rtype(result = "()")]
pub struct TextMessage(pub String);