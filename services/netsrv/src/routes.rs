use std::sync::Arc;
use std::sync::atomic::Ordering;

use axum::{
    Router,
    extract::{Multipart, Path, State},
    http::StatusCode,
    response::Json,
    routing::{delete, get, post},
};
use serde_json::{Value, json};
use tracing::error;
use utoipa::OpenApi;
#[cfg(feature = "swagger-ui")]
use utoipa_swagger_ui::{Config, SwaggerUi};

use crate::db_config;
use crate::models::{AlarmBroadcastRequest, CertUploadForm, NetConfig, SystemMetrics};
use crate::mqtt::publish_json;
use crate::state::AppState;

// ============================================================================
// Router
// ============================================================================

pub fn build_router(state: Arc<AppState>) -> Router {
    let api = Router::new()
        .route("/", get(root))
        .route("/ping", get(ping))
        .route("/netApi/health", get(health))
        // Alarm
        .route("/netApi/alarm/broadcast", post(alarm_broadcast))
        .route("/netApi/alarm/config", get(alarm_config))
        // MQTT
        .route("/netApi/mqtt/config", get(mqtt_get_config).post(mqtt_update_config))
        .route("/netApi/mqtt/status", get(mqtt_status))
        .route("/netApi/mqtt/disconnect", post(mqtt_disconnect))
        .route("/netApi/mqtt/reconnect", post(mqtt_reconnect))
        // Certificate
        .route("/netApi/certificate/upload", post(cert_upload))
        .route("/netApi/certificate/info", get(cert_info))
        .route("/netApi/certificate/{cert_type}", delete(cert_delete))
        // Admin API (shared endpoints from common lib)
        .route("/api/admin/logs/level", get(common::admin_api::get_log_level).post(common::admin_api::set_log_level))
        .route("/api/admin/logs/files", get(common::admin_api::list_log_files))
        .route("/api/admin/logs/view", get(common::admin_api::view_log_file))
        .with_state(state);

    #[cfg(feature = "swagger-ui")]
    let api = api.merge(
        SwaggerUi::new("/docs")
            .url("/openapi.json", ApiDoc::openapi())
            .config(
                Config::default()
                    .default_model_rendering("model")
                    .default_models_expand_depth(1),
            ),
    );

    api
}

// ============================================================================
// OpenAPI document (only consumed when swagger-ui feature is enabled)
// ============================================================================

#[cfg_attr(not(feature = "swagger-ui"), allow(dead_code))]
#[derive(OpenApi)]
#[openapi(
    paths(
        root,
        ping,
        health,
        alarm_broadcast,
        alarm_config,
        mqtt_get_config,
        mqtt_update_config,
        mqtt_status,
        mqtt_disconnect,
        mqtt_reconnect,
        cert_upload,
        cert_info,
        cert_delete,
    ),
    components(schemas(
        NetConfig,
        AlarmBroadcastRequest,
        CertUploadForm,
        SystemMetrics,
    )),
    tags(
        (name = "Health",      description = "健康检查与服务信息"),
        (name = "Alarm",       description = "告警广播与配置"),
        (name = "MQTT",        description = "MQTT 连接配置与控制"),
        (name = "Certificate", description = "TLS 证书管理"),
    ),
    info(
        title = "VoltageEMS Network Service",
        version = "1.0.0",
        description = "MQTT 网关服务：负责将 Redis 实时数据上报至云端 MQTT Broker，\
                       并接收云端下发的读写指令转发给本地设备。"
    )
)]
pub struct ApiDoc;

// ============================================================================
// Root / ping
// ============================================================================

/// netsrv 服务横幅。
///
/// 返回名称、版本、描述。给运维确认 netsrv 进程在线且版本对得上。
/// 不依赖 MQTT 连接 —— 即使 broker 挂了也会 200。MQTT 状态查
/// `/netApi/health` 或 `/netApi/mqtt/status`。
#[utoipa::path(get, path = "/", tag = "Health",
    responses((status = 200, description = "服务基本信息")))]
async fn root() -> Json<Value> {
    Json(json!({
        "service": "netsrv",
        "version": env!("CARGO_PKG_VERSION"),
        "status": "running"
    }))
}

/// 极简存活探测，返回字符串 "pong"。
///
/// 跟 `/` 的区别：响应体是纯字符串，不带 JSON 框架开销，适合高频
/// liveness probe / 负载均衡器探活。
#[utoipa::path(get, path = "/ping", tag = "Health",
    responses((status = 200, description = "pong")))]
async fn ping() -> &'static str {
    "pong"
}

// ============================================================================
// Health
// ============================================================================

/// 健康检查：返回 MQTT 连接状态 + 设备身份信息。
///
/// 检测实际的 MQTT broker 连接（不是缓存状态）。返回 `mqtt_connected`
/// （bool）、broker 地址、设备 client_id 等。**进程活着但 MQTT 没连
/// 上**时仍然回 200 + connected:false，运维 dashboard 据此区分"进程
/// 死了"和"进程活着但上云链路断了"。
#[utoipa::path(get, path = "/netApi/health", tag = "Health",
    responses((status = 200, description = "MQTT 连接状态与设备身份信息")))]
async fn health(State(state): State<Arc<AppState>>) -> Json<Value> {
    let mqtt_ok = state.mqtt_connected.load(Ordering::Relaxed);
    Json(json!({
        "success": mqtt_ok,
        "message": if mqtt_ok { "MQTT 连接正常" } else { "MQTT 未连接" },
        "data": {
            "mqtt_connected": mqtt_ok,
            "product_sn":     state.device.product_sn,
            "device_sn":      state.device.device_sn,
        }
    }))
}

// ============================================================================
// Alarm
// ============================================================================

/// 把告警 JSON 透传发布到 MQTT 告警 Topic。
///
/// 请求体为任意 JSON 对象，内容不做校验，整体发到配置的告警 Topic
/// （`netApi/alarm/config` 可查 Topic 名）。**云端订阅方负责解析**——
/// 上游 alarmsrv 发出的告警事件会经过这条链路上云。MQTT 没连接时本
/// 接口仍返回 200 但消息可能丢失（QoS 取决于 MQTT 配置）。
#[utoipa::path(post, path = "/netApi/alarm/broadcast", tag = "Alarm",
    request_body = AlarmBroadcastRequest,
    responses(
        (status = 200,                  description = "广播成功"),
        (status = 503, description = "MQTT 未连接，发送失败"),
    ))]
async fn alarm_broadcast(
    State(state): State<Arc<AppState>>,
    Json(AlarmBroadcastRequest(body)): Json<AlarmBroadcastRequest>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    publish_json(&state, &state.topics.alarm, &body)
        .await
        .map(|_| Json(json!({"success": true, "message": "Alarm broadcast"})))
        .map_err(|e| {
            error!("Alarm broadcast failed: {}", e);
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({"success": false, "message": e.to_string()})),
            )
        })
}

/// 查看告警上云的配置（Topic 名 + MQTT 状态）。
///
/// 返回当前 alarm broadcast 用的 MQTT Topic 名字（如 `voltageems/
/// alarm/{device_id}`）和 MQTT 连接是否在线。前端"上云配置页"用，让
/// 运维确认告警发到哪个 Topic、链路是否通。
#[utoipa::path(get, path = "/netApi/alarm/config", tag = "Alarm",
    responses((status = 200, description = "告警 Topic 名称与 MQTT 连接状态")))]
async fn alarm_config(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({
        "success": true,
        "message": "OK",
        "data": {
            "alarm_topic":    state.topics.alarm,
            "mqtt_connected": state.mqtt_connected.load(Ordering::Relaxed),
        }
    }))
}

// ============================================================================
// MQTT config
// ============================================================================

/// 查看当前 MQTT 配置（broker / 证书路径 / 主题前缀等）。
///
/// 只读。返回 `NetConfig` 结构 —— broker_host、port、client_id、是否
/// 用 TLS、证书文件名、各类 topic 模板等。**不返回**敏感信息（如证书
/// 私钥内容）。要改配置走 `POST /netApi/mqtt/config`。
#[utoipa::path(get, path = "/netApi/mqtt/config", tag = "MQTT",
    responses((status = 200, description = "当前 MQTT 配置", body = NetConfig)))]
async fn mqtt_get_config(State(state): State<Arc<AppState>>) -> Json<Value> {
    let cfg = state.config.read().await.clone();
    Json(json!({ "success": true, "message": "OK", "data": cfg }))
}

/// 更新 MQTT 配置并立即触发重连，无需重启服务。
///
/// 写入新配置后立刻 disconnect 当前 MQTT session、用新参数 reconnect。
/// 期间会有几秒 MQTT 不可用窗口 —— 期间发的告警可能丢失（看 QoS 配
/// 置）。改 broker 地址 / 证书 / TLS 开关都走这一条路。如果新参数错误
/// 导致连不上，会留在 disconnected 状态，运维要再调一次正确的配置。
#[utoipa::path(post, path = "/netApi/mqtt/config", tag = "MQTT",
    request_body = NetConfig,
    responses(
        (status = 200, description = "配置已保存，正在重连"),
        (status = 500, description = "保存失败"),
    ))]
async fn mqtt_update_config(
    State(state): State<Arc<AppState>>,
    Json(mut new_cfg): Json<NetConfig>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    // Without this the in-memory copy bypasses load_config() and chunks(0) can panic.
    new_cfg.normalize();
    if let Err(e) = db_config::save_config(&state.sqlite, &new_cfg).await {
        error!("Save config failed: {}", e);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": e.to_string()})),
        ));
    }
    *state.config.write().await = new_cfg;
    state.reconnect_signal.notify_one();
    Ok(Json(
        json!({"success": true, "message": "Config updated, reconnecting"}),
    ))
}

/// 实时 MQTT 连接状态（轮询端点）。
///
/// 返回 connected/disconnected/connecting、最后一次连接成功 / 失败时间、
/// 失败原因（如果有）、累计重连次数。前端"上云状态"小绿灯就轮询这个
/// 接口。比 `/netApi/health` 信息更细，但更新频率相同（无后台缓存）。
#[utoipa::path(get, path = "/netApi/mqtt/status", tag = "MQTT",
    responses((status = 200, description = "当前 MQTT 连接状态、Broker 地址与设备信息")))]
async fn mqtt_status(State(state): State<Arc<AppState>>) -> Json<Value> {
    let cfg = state.config.read().await;
    Json(json!({
        "success": true,
        "message": "OK",
        "data": {
            "connected":  state.mqtt_connected.load(Ordering::Relaxed),
            "broker":     format!("{}:{}", cfg.broker_host, cfg.broker_port),
            "ssl":        cfg.ssl_enabled,
            "product_sn": state.device.product_sn,
            "device_sn":  state.device.device_sn,
        }
    }))
}

/// 手动断开 MQTT 连接，并**暂停自动重连**。
///
/// 跟 `reconnect` 是对应的开关。调用后 netsrv 关闭当前 MQTT session
/// 并设置一个"禁止自动重连"标志位 —— 之后即使 broker 可达也不会主动
/// 连回去，直到 `reconnect` 显式打开开关。**运维窗口期**用：升级
/// broker、临时禁止上云告警等场景。
#[utoipa::path(post, path = "/netApi/mqtt/disconnect", tag = "MQTT",
    responses((status = 200, description = "断开 MQTT 连接，停止自动重连，直到调用 reconnect")))]
async fn mqtt_disconnect(State(state): State<Arc<AppState>>) -> Json<Value> {
    // Mark disconnection intent first, then wake the mqtt loop so it stops reconnecting.
    state
        .disconnect_requested
        .store(true, std::sync::atomic::Ordering::Relaxed);
    // Drop the current client to force the event loop to exit.
    *state.mqtt_client.lock().await = None;
    state
        .mqtt_connected
        .store(false, std::sync::atomic::Ordering::Relaxed);
    state.reconnect_signal.notify_one();
    Json(json!({"success": true, "message": "MQTT disconnected, auto-reconnect paused"}))
}

/// 触发 MQTT 重连，并**恢复自动重连**。
///
/// 跟 `disconnect` 是对应的开关。调用后清除"禁止自动重连"标志、立刻
/// 触发一次连接尝试。返回 200 不代表连上了 —— 实际是后台异步 connect，
/// 结果要查 `/netApi/mqtt/status`。运维窗口结束后调这个让上云链路恢复。
#[utoipa::path(post, path = "/netApi/mqtt/reconnect", tag = "MQTT",
    responses((status = 200, description = "触发重连，后台异步执行")))]
async fn mqtt_reconnect(State(state): State<Arc<AppState>>) -> Json<Value> {
    // Clear the disconnect flag, then wake the mqtt loop to reconnect.
    state
        .disconnect_requested
        .store(false, std::sync::atomic::Ordering::Relaxed);
    state.reconnect_signal.notify_one();
    Json(json!({"success": true, "message": "Reconnect command sent, executing in background"}))
}

// ============================================================================
// Certificate management
// ============================================================================

/// 上传单个 TLS 证书文件（multipart/form-data）
///
/// 每次上传一个证书，通过 `cert_type` 指定类型，证书文件名任意。
/// 文件将以固定名称保存到 `cert_dir`：
///
/// | cert_type    | 保存文件名            |
/// |--------------|-----------------------|
/// | `ca_cert`    | `AmazonRootCA1.pem`   |
/// | `client_cert`| `certificate.pem.crt` |
/// | `client_key` | `private.pem.key`     |
#[utoipa::path(post, path = "/netApi/certificate/upload", tag = "Certificate",
    request_body(
        content_type = "multipart/form-data",
        content = inline(CertUploadForm),
    ),
    responses(
        (status = 200, description = "上传成功"),
        (status = 400, description = "参数错误（cert_type 非法 / 文件为空 / 格式不支持 / 超过 1MB）"),
        (status = 500, description = "目录无写入权限或文件写入失败"),
    ))]
async fn cert_upload(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    const MAX_SIZE: usize = 1024 * 1024; // 1 MB
    const ALLOWED_EXT: &[&str] = &[".pem", ".crt", ".key", ".cer", ".p12", ".pfx"];

    let cert_dir = state.env.cert_dir.clone();

    // Collect all multipart fields first.
    let mut cert_type_val: Option<String> = None;
    let mut file_data: Option<(String, Vec<u8>)> = None; // (original_filename, bytes)

    while let Some(field) = multipart.next_field().await.unwrap_or(None) {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "cert_type" => {
                let text = field.text().await.map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        Json(json!({"success": false, "message": e.to_string()})),
                    )
                })?;
                cert_type_val = Some(text);
            },
            "file" => {
                let orig_name = field.file_name().unwrap_or("").to_string();
                let data = field.bytes().await.map_err(|e| {
                    (
                        StatusCode::BAD_REQUEST,
                        Json(json!({"success": false, "message": e.to_string()})),
                    )
                })?;
                file_data = Some((orig_name, data.to_vec()));
            },
            _ => {},
        }
    }

    // Validate cert_type.
    let cert_type = cert_type_val.ok_or_else(|| (
        StatusCode::BAD_REQUEST,
        Json(json!({"success": false, "message": "Missing cert_type field. Valid values: ca_cert | client_cert | client_key"})),
    ))?;

    let save_name = match cert_type.as_str() {
        "ca_cert" => "AmazonRootCA1.pem",
        "client_cert" => "certificate.pem.crt",
        "client_key" => "private.pem.key",
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(
                    json!({"success": false, "message": format!("Unsupported cert_type: '{}'. Valid: ca_cert | client_cert | client_key", cert_type)}),
                ),
            ));
        },
    };

    // Validate file.
    let (orig_name, data) = file_data.ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({"success": false, "message": "Missing file field"})),
        )
    })?;

    if orig_name.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"success": false, "message": "Filename cannot be empty"})),
        ));
    }

    let ext = std::path::Path::new(&orig_name)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| format!(".{}", e.to_lowercase()))
        .unwrap_or_default();
    if !ALLOWED_EXT.contains(&ext.as_str()) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "success": false,
                "message": format!("Unsupported file format '{}'. Supported: {}", ext, ALLOWED_EXT.join(", "))
            })),
        ));
    }

    if data.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"success": false, "message": "File content is empty"})),
        ));
    }
    if data.len() > MAX_SIZE {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "success": false,
                "message": format!("File exceeds 1MB limit (current: {} bytes)", data.len())
            })),
        ));
    }

    // Ensure directory exists.
    std::fs::create_dir_all(&cert_dir).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "success": false,
                "message": format!("Cannot create cert directory '{}': {} (check path permissions)", cert_dir, e)
            })),
        )
    })?;

    // Save file.
    let dest = format!("{}/{}", cert_dir, save_name);
    std::fs::write(&dest, &data).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": format!("Failed to write file: {}", e)})),
        )
    })?;

    Ok(Json(json!({
        "success": true,
        "message": "Certificate uploaded",
        "data": {
            "cert_type": cert_type,
            "saved_as":  save_name,
            "path":      dest,
        }
    })))
}

/// 查看证书目录状态：路径 + 各证书文件是否存在。
///
/// 检查 CA / 客户端证书 / 私钥三个文件是否在配置的证书目录里。**不返
/// 回证书内容或指纹**（避免私钥泄露风险），只返回 exists:true/false。
/// 用于"上云配置检查"页面的预检 —— 如果显示缺少私钥，运维知道要重新
/// upload。
#[utoipa::path(get, path = "/netApi/certificate/info", tag = "Certificate",
    responses((status = 200, description = "证书目录路径及各证书文件是否存在")))]
async fn cert_info(State(state): State<Arc<AppState>>) -> Json<Value> {
    let cert_dir = state.env.cert_dir.clone();
    let files = [
        "AmazonRootCA1.pem",
        "certificate.pem.crt",
        "private.pem.key",
    ];
    let info: Vec<Value> = files
        .iter()
        .map(|f| {
            let exists = std::path::Path::new(&format!("{}/{}", cert_dir, f)).exists();
            json!({ "file": f, "exists": exists })
        })
        .collect();
    Json(json!({
        "success": true,
        "message": "OK",
        "data": {
            "cert_dir": cert_dir,
            "files":    info,
        }
    }))
}

/// 删除指定类型的证书文件。
///
/// `cert_type` 可选值：`ca_cert` / `client_cert` / `client_key`
#[utoipa::path(delete, path = "/netApi/certificate/{cert_type}", tag = "Certificate",
    params(
        ("cert_type" = String, Path, description = "证书类型：ca_cert | client_cert | client_key")
    ),
    responses(
        (status = 200, description = "删除成功（或文件本就不存在）"),
        (status = 400, description = "未知的 cert_type"),
        (status = 500, description = "删除失败"),
    ))]
async fn cert_delete(
    State(state): State<Arc<AppState>>,
    Path(cert_type): Path<String>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let cert_dir = state.env.cert_dir.clone();
    let filename = match cert_type.as_str() {
        "ca_cert" => "AmazonRootCA1.pem",
        "client_cert" => "certificate.pem.crt",
        "client_key" => "private.pem.key",
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(
                    json!({"success": false, "message": "Unknown cert_type. Valid: ca_cert | client_cert | client_key"}),
                ),
            ));
        },
    };

    match std::fs::remove_file(format!("{}/{}", cert_dir, filename)) {
        Ok(_) => Ok(Json(json!({
            "success": true,
            "message": "Deleted successfully",
            "data": { "deleted": filename }
        }))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Json(
            json!({"success": true, "message": "File does not exist, nothing to delete"}),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": e.to_string()})),
        )),
    }
}
