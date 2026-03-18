use std::sync::Arc;
use std::sync::atomic::Ordering;

use axum::{
    extract::{Multipart, Path, State},
    http::StatusCode,
    response::Json,
    routing::{delete, get, post},
    Router,
};
use serde_json::{json, Value};
use tracing::error;
use utoipa::OpenApi;
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
        .with_state(state);

    api.merge(
        SwaggerUi::new("/docs")
            .url("/openapi.json", ApiDoc::openapi())
            .config(
                Config::default()
                    .default_model_rendering("model")
                    .default_models_expand_depth(1),
            ),
    )
}

// ============================================================================
// OpenAPI document
// ============================================================================

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

#[utoipa::path(get, path = "/", tag = "Health",
    responses((status = 200, description = "服务基本信息")))]
async fn root() -> Json<Value> {
    Json(json!({
        "service": "netsrv",
        "version": env!("CARGO_PKG_VERSION"),
        "status": "running"
    }))
}

#[utoipa::path(get, path = "/ping", tag = "Health",
    responses((status = 200, description = "pong")))]
async fn ping() -> &'static str {
    "pong"
}

// ============================================================================
// Health
// ============================================================================

#[utoipa::path(get, path = "/netApi/health", tag = "Health",
    responses((status = 200, description = "MQTT 连接状态与设备身份信息")))]
async fn health(State(state): State<Arc<AppState>>) -> Json<Value> {
    let mqtt_ok = state.mqtt_connected.load(Ordering::Relaxed);
    Json(json!({
        "status": if mqtt_ok { "healthy" } else { "degraded" },
        "mqtt_connected": mqtt_ok,
        "product_sn": state.device.product_sn,
        "device_sn": state.device.device_sn,
    }))
}

// ============================================================================
// Alarm
// ============================================================================

/// 将告警 JSON 透传发布到 MQTT 告警 Topic。
/// 请求体为任意 JSON 对象，内容不做校验，直接转发。
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
        .map(|_| Json(json!({"success": true, "message": "告警已广播"})))
        .map_err(|e| {
            error!("Alarm broadcast failed: {}", e);
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({"success": false, "error": e.to_string()})),
            )
        })
}

#[utoipa::path(get, path = "/netApi/alarm/config", tag = "Alarm",
    responses((status = 200, description = "告警 Topic 名称与 MQTT 连接状态")))]
async fn alarm_config(State(state): State<Arc<AppState>>) -> Json<Value> {
    Json(json!({
        "alarm_topic": state.topics.alarm,
        "mqtt_connected": state.mqtt_connected.load(Ordering::Relaxed),
    }))
}

// ============================================================================
// MQTT config
// ============================================================================

#[utoipa::path(get, path = "/netApi/mqtt/config", tag = "MQTT",
    responses((status = 200, description = "当前 MQTT 配置", body = NetConfig)))]
async fn mqtt_get_config(State(state): State<Arc<AppState>>) -> Json<Value> {
    let cfg = state.config.read().await.clone();
    Json(json!({ "success": true, "data": cfg }))
}

/// 更新 MQTT 配置并立即触发重连，无需重启服务。
#[utoipa::path(post, path = "/netApi/mqtt/config", tag = "MQTT",
    request_body = NetConfig,
    responses(
        (status = 200, description = "配置已保存，正在重连"),
        (status = 500, description = "保存失败"),
    ))]
async fn mqtt_update_config(
    State(state): State<Arc<AppState>>,
    Json(new_cfg): Json<NetConfig>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    if let Err(e) = db_config::save_config(&state.sqlite, &new_cfg).await {
        error!("Save config failed: {}", e);
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "error": e.to_string()})),
        ));
    }
    *state.config.write().await = new_cfg;
    state.reconnect_signal.notify_one();
    Ok(Json(json!({"success": true, "message": "配置已更新，正在重连"})))
}

#[utoipa::path(get, path = "/netApi/mqtt/status", tag = "MQTT",
    responses((status = 200, description = "当前 MQTT 连接状态、Broker 地址与设备信息")))]
async fn mqtt_status(State(state): State<Arc<AppState>>) -> Json<Value> {
    let cfg = state.config.read().await;
    Json(json!({
        "connected": state.mqtt_connected.load(Ordering::Relaxed),
        "broker": format!("{}:{}", cfg.broker_host, cfg.broker_port),
        "ssl": cfg.ssl_enabled,
        "product_sn": state.device.product_sn,
        "device_sn": state.device.device_sn,
    }))
}

#[utoipa::path(post, path = "/netApi/mqtt/disconnect", tag = "MQTT",
    responses((status = 200, description = "断开指令已发送")))]
async fn mqtt_disconnect(State(state): State<Arc<AppState>>) -> Json<Value> {
    state.reconnect_signal.notify_one();
    Json(json!({"success": true, "message": "断开指令已发送"}))
}

#[utoipa::path(post, path = "/netApi/mqtt/reconnect", tag = "MQTT",
    responses((status = 200, description = "重连指令已发送，后台异步执行")))]
async fn mqtt_reconnect(State(state): State<Arc<AppState>>) -> Json<Value> {
    state.reconnect_signal.notify_one();
    Json(json!({"success": true, "message": "重连指令已发送"}))
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
                    (StatusCode::BAD_REQUEST, Json(json!({"success": false, "error": e.to_string()})))
                })?;
                cert_type_val = Some(text);
            }
            "file" => {
                let orig_name = field.file_name().unwrap_or("").to_string();
                let data = field.bytes().await.map_err(|e| {
                    (StatusCode::BAD_REQUEST, Json(json!({"success": false, "error": e.to_string()})))
                })?;
                file_data = Some((orig_name, data.to_vec()));
            }
            _ => {}
        }
    }

    // Validate cert_type.
    let cert_type = cert_type_val.ok_or_else(|| (
        StatusCode::BAD_REQUEST,
        Json(json!({"success": false, "error": "缺少 cert_type 字段，可选值：ca_cert | client_cert | client_key"})),
    ))?;

    let save_name = match cert_type.as_str() {
        "ca_cert"     => "AmazonRootCA1.pem",
        "client_cert" => "certificate.pem.crt",
        "client_key"  => "private.pem.key",
        _ => return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"success": false, "error": format!("不支持的 cert_type: '{}'. 可选：ca_cert | client_cert | client_key", cert_type)})),
        )),
    };

    // Validate file.
    let (orig_name, data) = file_data.ok_or_else(|| (
        StatusCode::BAD_REQUEST,
        Json(json!({"success": false, "error": "缺少 file 字段"})),
    ))?;

    if orig_name.is_empty() {
        return Err((StatusCode::BAD_REQUEST, Json(json!({"success": false, "error": "文件名不能为空"}))));
    }

    let ext = std::path::Path::new(&orig_name)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| format!(".{}", e.to_lowercase()))
        .unwrap_or_default();
    if !ALLOWED_EXT.contains(&ext.as_str()) {
        return Err((StatusCode::BAD_REQUEST, Json(json!({
            "success": false,
            "error": format!("不支持的文件格式 '{}'，支持：{}", ext, ALLOWED_EXT.join(", "))
        }))));
    }

    if data.is_empty() {
        return Err((StatusCode::BAD_REQUEST, Json(json!({"success": false, "error": "文件内容为空"}))));
    }
    if data.len() > MAX_SIZE {
        return Err((StatusCode::BAD_REQUEST, Json(json!({
            "success": false,
            "error": format!("文件超过 1MB 限制（当前 {} bytes）", data.len())
        }))));
    }

    // Ensure directory exists.
    std::fs::create_dir_all(&cert_dir).map_err(|e| (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({
            "success": false,
            "error": format!("无法创建证书目录 '{}': {}（请检查路径权限，或通过 POST /netApi/mqtt/config 修改 cert_dir）", cert_dir, e)
        })),
    ))?;

    // Save file.
    let dest = format!("{}/{}", cert_dir, save_name);
    std::fs::write(&dest, &data).map_err(|e| (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"success": false, "error": format!("写入文件失败: {}", e)})),
    ))?;

    Ok(Json(json!({
        "success": true,
        "message": format!("证书上传成功"),
        "cert_type": cert_type,
        "saved_as": save_name,
        "path": dest,
    })))
}

#[utoipa::path(get, path = "/netApi/certificate/info", tag = "Certificate",
    responses((status = 200, description = "证书目录路径及各证书文件是否存在")))]
async fn cert_info(State(state): State<Arc<AppState>>) -> Json<Value> {
    let cert_dir = state.env.cert_dir.clone();
    let files = ["AmazonRootCA1.pem", "certificate.pem.crt", "private.pem.key"];
    let info: Vec<Value> = files
        .iter()
        .map(|f| {
            let exists = std::path::Path::new(&format!("{}/{}", cert_dir, f)).exists();
            json!({ "file": f, "exists": exists })
        })
        .collect();
    Json(json!({ "cert_dir": cert_dir, "files": info }))
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
        "ca_cert"     => "AmazonRootCA1.pem",
        "client_cert" => "certificate.pem.crt",
        "client_key"  => "private.pem.key",
        _ => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(json!({"success": false, "error": "未知的 cert_type，可选：ca_cert | client_cert | client_key"})),
            ))
        }
    };

    match std::fs::remove_file(format!("{}/{}", cert_dir, filename)) {
        Ok(_) => Ok(Json(json!({"success": true, "deleted": filename}))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Json(
            json!({"success": true, "message": "文件不存在，无需删除"}),
        )),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "error": e.to_string()})),
        )),
    }
}
