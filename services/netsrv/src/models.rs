use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use utoipa::ToSchema;

// ── MQTT publish payloads ─────────────────────────────────────────────────────

/// Uploaded to `property/{productSN}/{deviceSN}`.
#[derive(Serialize)]
pub struct PropertyPayload {
    pub timestamp: i64,
    pub property: Vec<PropertyEntry>,
}

#[derive(Clone, Serialize)]
pub struct PropertyEntry {
    pub source: String,
    pub device: String,
    pub data_type: String,
    /// Field → value mapping from Redis hash (values may be f64 or String).
    pub value: HashMap<String, serde_json::Value>,
}

/// Uploaded to `status/{productSN}/{deviceSN}`.
#[derive(Serialize)]
pub struct StatusPayload {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub gateway: String,
    pub timestamp: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

// ── MQTT command payloads (incoming) ─────────────────────────────────────────

/// Incoming single-point read request on `read/{productSN}/{deviceSN}`.
/// Field name in JSON is `key` (matching Python netsrv protocol); `msgId` is the
/// correlation ID echoed back in the reply.
#[derive(Debug, Deserialize)]
pub struct ReadRequest {
    pub source: String,
    pub device: String,
    pub data_type: String,
    /// If absent, return all hash fields.
    #[serde(rename = "key")]
    pub field: Option<String>,
    #[serde(rename = "msgId")]
    pub msg_id: Option<String>,
}

/// Single entry inside a `read-reply` property array.
#[derive(Serialize)]
pub struct ReadReplyProperty {
    pub source: String,
    pub device: String,
    pub data_type: String,
    /// For a keyed read: `{ key: value }`.  For a full-hash read: all field→value pairs.
    pub value: serde_json::Value,
}

/// Reply to `read-reply/{productSN}/{deviceSN}`.
/// Format matches Python netsrv: `{ timestamp, property: [...], msgId }`.
#[derive(Serialize)]
pub struct ReadReply {
    pub timestamp: i64,
    pub property: Vec<ReadReplyProperty>,
    #[serde(rename = "msgId")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub msg_id: Option<String>,
}

/// Incoming single-point write request on `write/{productSN}/{deviceSN}`.
/// Field name in JSON is `key`; `msgId` is the correlation ID.
#[derive(Debug, Deserialize)]
pub struct WriteRequest {
    pub source: String,
    pub device: String,
    pub data_type: String,
    #[serde(rename = "key")]
    pub field: String,
    pub value: serde_json::Value,
    #[serde(rename = "msgId")]
    pub msg_id: Option<String>,
}

/// Reply to `write-reply/{productSN}/{deviceSN}`.
/// Format matches Python netsrv: `{ result: "success"|"fail", msgId }`.
#[derive(Serialize)]
pub struct WriteReply {
    pub result: String,
    #[serde(rename = "msgId")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub msg_id: Option<String>,
}

/// Generic command-acknowledgement reply (call-data-reply, call-alarm-reply).
/// Format matches Python netsrv: `{ result, message, timestamp, msgId }`.
/// `call-alarm-reply` may use `result: "warning"` when alarmsrv returns a non-2xx status.
#[derive(Serialize)]
pub struct CommandReply {
    pub result: String,
    pub message: String,
    pub timestamp: i64,
    #[serde(rename = "msgId")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub msg_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

// ── Dynamic service configuration ────────────────────────────────────────────

/// MQTT 网关服务配置（`POST /netApi/mqtt/config`）
///
/// 修改后立即触发 MQTT 重连，无需重启服务。
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[schema(example = json!({
    "product_sn": "MonarchHub",
    "device_sn": "auto",
    "broker_host": "mqtt.example.com",
    "broker_port": 8883,
    "broker_keepalive_secs": 120,
    "client_id": "auto",
    "username": null,
    "password": null,
    "ssl_enabled": false,
    "reconnect_delay_secs": 10,
    "reconnect_max_attempts": 50,
    "report_interval_secs": 50,
    "report_batch_size": 50,
    "system_monitor_enabled": true,
    "system_monitor_interval_secs": 10,
    "subscribe_patterns": ["inst:*:M", "inst:*:A"],
    "exclude_patterns": [],
    "alarmsrv_url": "http://localhost:6007"
}))]
pub struct NetConfig {
    // -- Device identity --
    /// 产品序列号，用于构造 MQTT Topic 前缀，如 `status/{product_sn}/{device_sn}`
    #[schema(example = "MonarchHub")]
    pub product_sn: String,

    /// 设备序列号。填 `"auto"` 时从硬件自动读取（依次尝试 `/proc/device-tree/serial-number`、
    /// 环境变量 `DEVICE_SN`、主机名）。
    #[schema(example = "auto")]
    pub device_sn: String,

    // -- MQTT broker --
    /// MQTT Broker 主机地址（IP 或域名）
    #[schema(example = "mqtt.example.com")]
    pub broker_host: String,

    /// MQTT Broker 端口，TLS 通常为 8883，明文通常为 1883
    #[schema(example = 8883, minimum = 1, maximum = 65535)]
    pub broker_port: u16,

    /// MQTT Keep-Alive 间隔（秒），超时未收到心跳则断线重连
    #[schema(example = 120, minimum = 5)]
    pub broker_keepalive_secs: u64,

    /// MQTT 客户端 ID。填 `"auto"` 时使用已解析的 `device_sn`。
    #[schema(example = "auto")]
    pub client_id: String,

    // -- Auth --
    /// MQTT 用户名（可选，为空则不发送认证信息）
    #[schema(example = json!(null))]
    #[serde(default)]
    pub username: Option<String>,

    /// MQTT 密码（可选，与 username 配合使用）
    #[schema(example = json!(null))]
    #[serde(default)]
    pub password: Option<String>,

    // -- TLS --
    /// 是否启用 TLS/SSL 加密连接。
    /// 证书目录固定为容器内 `/app/config/cert`（可通过 `CERT_DIR` 环境变量覆盖），
    /// 启动时通过 Docker volume 挂载到宿主机目录，不可通过 API 修改。
    #[schema(example = false)]
    pub ssl_enabled: bool,

    // -- Reconnect --
    /// 断线后等待多少秒再重连
    #[schema(example = 10, minimum = 1)]
    pub reconnect_delay_secs: u64,

    /// 最大重连尝试次数，超过后停止重连直到手动触发
    #[schema(example = 50, minimum = 1)]
    pub reconnect_max_attempts: u32,

    // -- Data forwarding --
    /// 数据上报间隔（秒），每隔此时间将 Redis 数据批量发布到 MQTT
    #[schema(example = 50, minimum = 1)]
    pub report_interval_secs: u64,

    /// 单次上报最大数据点数，超出部分留到下次
    #[schema(example = 50, minimum = 1)]
    pub report_batch_size: usize,

    // -- System monitor --
    /// 是否启用系统资源监控（CPU / 内存 / 磁盘 / 网络），通过 MQTT 定期上报
    #[schema(example = true)]
    pub system_monitor_enabled: bool,

    /// 系统资源采集间隔（秒）
    #[schema(example = 10, minimum = 1)]
    pub system_monitor_interval_secs: u64,

    // -- Redis source --
    /// Redis Key 订阅模式列表（**glob 语法**，同 Redis SCAN）
    ///
    /// 只采集匹配这些模式的 Key，例如 `inst:*:M` 匹配所有遥测数据。
    #[schema(example = json!(["inst:*:M", "inst:*:A"]))]
    pub subscribe_patterns: Vec<String>,

    /// 排除模式列表（**正则表达式语法**，与 subscribe_patterns 的 glob 不同）
    ///
    /// 命中任意一条正则的 Key 不上报。示例：`["^inst:0:"]` 排除通道 0。
    #[schema(example = json!([]))]
    pub exclude_patterns: Vec<String>,

    // -- Service URLs --
    /// alarmsrv 服务地址，用于告警广播时反向通知
    #[schema(example = "http://localhost:6007")]
    pub alarmsrv_url: String,
}

impl Default for NetConfig {
    fn default() -> Self {
        Self {
            product_sn: "MonarchHub".to_string(),
            device_sn: "auto".to_string(),
            broker_host: "localhost".to_string(),
            broker_port: 8883,
            broker_keepalive_secs: 120,
            client_id: "auto".to_string(),
            username: None,
            password: None,
            ssl_enabled: false,
            reconnect_delay_secs: 10,
            reconnect_max_attempts: 50,
            report_interval_secs: 50,
            report_batch_size: 50,
            system_monitor_enabled: true,
            system_monitor_interval_secs: 10,
            subscribe_patterns: vec!["inst:*:M".to_string(), "inst:*:A".to_string()],
            exclude_patterns: vec![],
            alarmsrv_url: "http://localhost:6007".to_string(),
        }
    }
}

impl NetConfig {
    pub fn normalize(&mut self) {
        self.broker_port = self.broker_port.max(1);
        self.broker_keepalive_secs = self.broker_keepalive_secs.max(1);
        self.reconnect_delay_secs = self.reconnect_delay_secs.max(1);
        self.reconnect_max_attempts = self.reconnect_max_attempts.max(1);
        self.report_interval_secs = self.report_interval_secs.max(1);
        self.report_batch_size = self.report_batch_size.max(1);
        self.system_monitor_interval_secs = self.system_monitor_interval_secs.max(1);
    }
}

#[cfg(test)]
mod config_tests {
    use super::*;

    #[test]
    fn normalize_clamps_zero_runtime_values() {
        let mut cfg = NetConfig {
            broker_port: 0,
            broker_keepalive_secs: 0,
            reconnect_delay_secs: 0,
            reconnect_max_attempts: 0,
            report_interval_secs: 0,
            report_batch_size: 0,
            system_monitor_interval_secs: 0,
            ..NetConfig::default()
        };

        cfg.normalize();

        assert_eq!(cfg.broker_port, 1);
        assert_eq!(cfg.broker_keepalive_secs, 1);
        assert_eq!(cfg.reconnect_delay_secs, 1);
        assert_eq!(cfg.reconnect_max_attempts, 1);
        assert_eq!(cfg.report_interval_secs, 1);
        assert_eq!(cfg.report_batch_size, 1);
        assert_eq!(cfg.system_monitor_interval_secs, 1);
    }
}

// ── HTTP API models ───────────────────────────────────────────────────────────

/// 告警广播请求体（JSON 对象，内容透传至 MQTT 告警 Topic）
#[derive(Debug, Deserialize, ToSchema)]
pub struct AlarmBroadcastRequest(pub serde_json::Value);

/// TLS 证书上传表单（`POST /netApi/certificate/upload`，multipart/form-data）
///
/// 每次上传一个证书文件，通过 `cert_type` 指定类型，文件名任意。
#[allow(dead_code)]
#[derive(ToSchema)]
pub struct CertUploadForm {
    /// 证书类型，可选值：`ca_cert` | `client_cert` | `client_key`
    #[schema(example = "ca_cert")]
    pub cert_type: String,

    /// 证书文件（支持 .pem .crt .key .cer 等格式，最大 1MB）
    #[schema(format = Binary, value_type = String)]
    pub file: String,
}

/// 系统资源快照
#[derive(Debug, Serialize, ToSchema)]
pub struct SystemMetrics {
    pub cpu_usage_percent: f32,
    pub memory_total_gb: f64,
    pub memory_used_gb: f64,
    pub memory_available_gb: f64,
    pub memory_usage_percent: f64,
    pub disk_total_gb: f64,
    pub disk_used_gb: f64,
    pub disk_free_gb: f64,
    pub disk_usage_percent: f64,
    pub network_bytes_sent: u64,
    pub network_bytes_recv: u64,
    pub system_uptime_hours: f64,
}
