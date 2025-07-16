use serde::{Deserialize, Serialize};
use serde_json::Value;

/// WebSocket消息的主要类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsMessage {
    // === 请求/响应模式 ===
    /// JSON-RPC 2.0请求
    Request {
        id: u64,
        method: String,
        params: Option<Value>,
    },
    
    /// JSON-RPC 2.0响应
    Response {
        id: u64,
        #[serde(skip_serializing_if = "Option::is_none")]
        result: Option<Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<RpcError>,
    },
    
    // === 订阅管理 ===
    /// 订阅实时数据
    Subscribe {
        /// 要订阅的通道ID列表
        channels: Vec<u32>,
        /// 数据类型 ["telemetry", "signal", "alarm", "all"]
        types: Vec<String>,
    },
    
    /// 取消订阅
    Unsubscribe {
        /// 要取消订阅的通道ID列表
        channels: Vec<u32>,
        /// 数据类型，如果为空则取消所有类型
        types: Option<Vec<String>>,
    },
    
    // === 实时数据推送 ===
    /// 遥测数据更新
    TelemetryData {
        channel_id: u32,
        point_id: u32,
        value: f64,
        quality: u8,
        timestamp: i64,
    },
    
    /// 信号数据更新
    SignalData {
        channel_id: u32,
        point_id: u32,
        value: bool,
        quality: u8,
        timestamp: i64,
    },
    
    /// 告警事件
    AlarmEvent {
        alarm_id: String,
        channel_id: u32,
        point_id: u32,
        level: String,
        message: String,
        timestamp: i64,
    },
    
    // === 控制命令 ===
    /// 发送控制命令
    Control {
        channel_id: u32,
        point_id: u32,
        value: Value,
        #[serde(skip_serializing_if = "Option::is_none")]
        params: Option<Value>,
    },
    
    /// 控制命令反馈
    ControlFeedback {
        channel_id: u32,
        point_id: u32,
        success: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        message: Option<String>,
        timestamp: i64,
    },
    
    // === 系统消息 ===
    /// 心跳请求
    Ping {
        #[serde(skip_serializing_if = "Option::is_none")]
        timestamp: Option<i64>,
    },
    
    /// 心跳响应
    Pong {
        timestamp: i64,
    },
    
    /// 错误通知
    Error {
        code: String,
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        data: Option<Value>,
    },
    
    /// 连接成功确认
    Connected {
        session_id: String,
        version: String,
        features: Vec<String>,
    },
}

/// JSON-RPC 2.0错误对象
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl RpcError {
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }
    
    pub fn with_data(mut self, data: Value) -> Self {
        self.data = Some(data);
        self
    }
    
    // 标准JSON-RPC 2.0错误码
    pub fn parse_error() -> Self {
        Self::new(-32700, "Parse error")
    }
    
    pub fn invalid_request() -> Self {
        Self::new(-32600, "Invalid Request")
    }
    
    pub fn method_not_found() -> Self {
        Self::new(-32601, "Method not found")
    }
    
    pub fn invalid_params() -> Self {
        Self::new(-32602, "Invalid params")
    }
    
    pub fn internal_error() -> Self {
        Self::new(-32603, "Internal error")
    }
    
    // 自定义错误码 (-32000 to -32099)
    pub fn unauthorized() -> Self {
        Self::new(-32001, "Unauthorized")
    }
    
    pub fn forbidden() -> Self {
        Self::new(-32002, "Forbidden")
    }
    
    pub fn not_found() -> Self {
        Self::new(-32003, "Not found")
    }
    
    pub fn timeout() -> Self {
        Self::new(-32004, "Request timeout")
    }
}

/// 订阅过滤器
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscriptionFilter {
    pub channels: Vec<u32>,
    pub types: Vec<DataType>,
}

/// 数据类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataType {
    Telemetry,  // 遥测 (YC)
    Signal,     // 信号 (YX)
    Control,    // 控制 (YK)
    Adjustment, // 调节 (YT)
    Alarm,      // 告警
    All,        // 所有类型
}

impl DataType {
    pub fn as_str(&self) -> &'static str {
        match self {
            DataType::Telemetry => "m",
            DataType::Signal => "s",
            DataType::Control => "c",
            DataType::Adjustment => "a",
            DataType::Alarm => "alarm",
            DataType::All => "*",
        }
    }
    
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "m" | "telemetry" => Some(DataType::Telemetry),
            "s" | "signal" => Some(DataType::Signal),
            "c" | "control" => Some(DataType::Control),
            "a" | "adjustment" => Some(DataType::Adjustment),
            "alarm" => Some(DataType::Alarm),
            "*" | "all" => Some(DataType::All),
            _ => None,
        }
    }
}

/// WebSocket内部消息（用于Actor通信）
#[derive(Debug, Clone, actix::Message)]
#[rtype(result = "()")]
pub enum InternalMessage {
    /// 新客户端连接
    Connect {
        session_id: String,
        addr: actix::Addr<crate::websocket::session::WsSession>,
    },
    /// 客户端断开
    Disconnect {
        session_id: String,
    },
    /// 订阅请求
    Subscribe {
        session_id: String,
        filter: SubscriptionFilter,
    },
    /// 取消订阅
    Unsubscribe {
        session_id: String,
        filter: SubscriptionFilter,
    },
    /// 广播消息给订阅者
    Broadcast {
        channel_id: u32,
        data_type: DataType,
        message: WsMessage,
    },
}