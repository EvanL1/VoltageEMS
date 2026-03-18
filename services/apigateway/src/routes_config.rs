use std::io::{self, Read, Write};
use std::path::Path;
use std::sync::Mutex;

use axum::{
    body::Body,
    extract::Multipart,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use tracing::{error, info};

const CONFIG_DIR: &str = "/opt/MonarchEdge/data";
const UPGRADE_DIR: &str = "/opt/MonarchEdge/upgrade";

// Upgrade state shared between start/abort/status handlers
static UPGRADE_PID: Mutex<Option<u32>> = Mutex::new(None);
static UPGRADE_RUNNING: Mutex<bool> = Mutex::new(false);

// ── GET /api/v1/config/check ──────────────────────────────────────────────────

#[utoipa::path(get, path = "/api/v1/config/check", tag = "Config",
    security(("bearer_auth" = [])),
    responses((status = 200, description = "配置目录检查结果")))]
pub async fn check_config() -> impl IntoResponse {
    let dir = Path::new(CONFIG_DIR);
    if !dir.exists() {
        return Json(json!({
            "success": false,
            "message": format!("配置目录不存在: {}", CONFIG_DIR),
            "data": { "exists": false, "path": CONFIG_DIR }
        }))
        .into_response();
    }

    let entries: Vec<String> = std::fs::read_dir(dir)
        .unwrap_or_else(|_| std::fs::read_dir(".").unwrap())
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();

    Json(json!({
        "success": true,
        "message": "配置目录检查完成",
        "data": {
            "exists": true,
            "path": CONFIG_DIR,
            "file_count": entries.len(),
            "files": entries,
        }
    }))
    .into_response()
}

// ── GET /api/v1/config/export ─────────────────────────────────────────────────

#[utoipa::path(get, path = "/api/v1/config/export", tag = "Config",
    security(("bearer_auth" = [])),
    responses((status = 200, description = "返回 ZIP 文件流")))]
pub async fn export_config() -> impl IntoResponse {
    let dir = Path::new(CONFIG_DIR);
    if !dir.exists() {
        return (
            StatusCode::NOT_FOUND,
            Json(json!({"success": false, "message": "配置目录不存在"})),
        )
            .into_response();
    }

    match create_zip_archive(dir) {
        Ok(data) => {
            let filename = format!(
                "config_{}.zip",
                chrono::Utc::now().format("%Y%m%d_%H%M%S")
            );
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/zip")
                .header(
                    header::CONTENT_DISPOSITION,
                    format!("attachment; filename=\"{}\"", filename),
                )
                .body(Body::from(data))
                .unwrap()
        }
        Err(e) => {
            error!("Export config error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": format!("导出配置失败: {}", e)})),
            )
                .into_response()
        }
    }
}

fn create_zip_archive(dir: &Path) -> io::Result<Vec<u8>> {
    let buf = Vec::new();
    let cursor = io::Cursor::new(buf);
    let mut zip = zip::ZipWriter::new(cursor);

    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    let base = dir;
    for entry in walkdir_simple(base) {
        let rel = entry.strip_prefix(base).unwrap();
        let rel_str = rel.to_string_lossy();

        if entry.is_dir() {
            zip.add_directory(format!("{}/", rel_str), options)?;
        } else if entry.is_file() {
            zip.start_file(rel_str.as_ref(), options)?;
            let data = std::fs::read(&entry)?;
            zip.write_all(&data)?;
        }
    }

    let cursor = zip.finish()?;
    Ok(cursor.into_inner())
}

fn walkdir_simple(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut paths = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return paths;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            paths.push(path.clone());
            paths.extend(walkdir_simple(&path));
        } else {
            paths.push(path);
        }
    }
    paths
}

// ── POST /api/v1/config/import ────────────────────────────────────────────────

#[utoipa::path(post, path = "/api/v1/config/import", tag = "Config",
    security(("bearer_auth" = [])),
    request_body(content_type = "multipart/form-data", description = "上传 ZIP 配置文件（字段名 file）"),
    responses((status = 200, description = "导入成功"), (status = 400, description = "文件格式错误")))]
pub async fn import_config(mut multipart: Multipart) -> impl IntoResponse {
    let mut zip_data: Option<Vec<u8>> = None;

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();
        if name == "file" || name == "zip" || zip_data.is_none() {
            match field.bytes().await {
                Ok(data) => zip_data = Some(data.to_vec()),
                Err(e) => {
                    error!("Read upload error: {}", e);
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(json!({"success": false, "message": "读取上传文件失败"})),
                    )
                        .into_response();
                }
            }
        }
    }

    let data = match zip_data {
        Some(d) if !d.is_empty() => d,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"success": false, "message": "未提供配置文件"})),
            )
                .into_response();
        }
    };

    let target = Path::new(CONFIG_DIR);
    if let Err(e) = std::fs::create_dir_all(target) {
        error!("Create config dir error: {}", e);
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    match extract_zip(&data, target) {
        Ok(count) => {
            info!("Config imported: {} files", count);
            Json(json!({
                "success": true,
                "message": "配置导入成功",
                "data": { "files_extracted": count }
            }))
            .into_response()
        }
        Err(e) => {
            error!("Extract zip error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": format!("解压配置失败: {}", e)})),
            )
                .into_response()
        }
    }
}

fn extract_zip(data: &[u8], target: &Path) -> io::Result<usize> {
    let cursor = io::Cursor::new(data);
    let mut archive = zip::ZipArchive::new(cursor)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    let count = archive.len();
    for i in 0..count {
        let mut file = archive
            .by_index(i)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        let name = file.name().to_string();
        let out_path = target.join(&name);

        if name.ends_with('/') {
            std::fs::create_dir_all(&out_path)?;
        } else {
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut out_file = std::fs::File::create(&out_path)?;
            let mut buf = Vec::new();
            file.read_to_end(&mut buf)?;
            out_file.write_all(&buf)?;
        }
    }

    Ok(count)
}

// ── POST /api/v1/config/restart-services ─────────────────────────────────────

#[utoipa::path(post, path = "/api/v1/config/restart-services", tag = "Config",
    security(("bearer_auth" = [])),
    responses((status = 200, description = "服务重启结果")))]
pub async fn restart_services() -> impl IntoResponse {
    let services = ["voltageems-comsrv", "voltageems-modsrv"];
    let mut results = Vec::new();

    for svc in &services {
        let output = std::process::Command::new("docker")
            .args(["restart", svc])
            .output();

        match output {
            Ok(o) if o.status.success() => {
                info!("Restarted service: {}", svc);
                results.push(json!({"service": svc, "success": true}));
            }
            Ok(o) => {
                let err = String::from_utf8_lossy(&o.stderr).to_string();
                error!("Restart {} failed: {}", svc, err);
                results.push(json!({"service": svc, "success": false, "error": err}));
            }
            Err(e) => {
                error!("Docker command error for {}: {}", svc, e);
                results.push(json!({"service": svc, "success": false, "error": e.to_string()}));
            }
        }
    }

    let all_ok = results.iter().all(|r| r["success"].as_bool().unwrap_or(false));
    Json(json!({
        "success": all_ok,
        "message": if all_ok { "服务重启成功" } else { "部分服务重启失败" },
        "data": { "results": results }
    }))
    .into_response()
}

// ── POST /api/v1/config/upgrade ───────────────────────────────────────────────

#[utoipa::path(post, path = "/api/v1/config/upgrade", tag = "Config",
    security(("bearer_auth" = [])),
    request_body(content_type = "multipart/form-data", description = "上传升级包"),
    responses((status = 200, description = "升级已启动"), (status = 409, description = "升级正在进行中")))]
pub async fn start_upgrade(mut multipart: Multipart) -> impl IntoResponse {
    {
        let running = UPGRADE_RUNNING.lock().unwrap();
        if *running {
            return (
                StatusCode::CONFLICT,
                Json(json!({"success": false, "message": "升级已在运行中"})),
            )
                .into_response();
        }
    }

    let mut pkg_data: Option<Vec<u8>> = None;
    let mut pkg_name = "upgrade.tar.gz".to_string();

    while let Ok(Some(field)) = multipart.next_field().await {
        if let Some(fname) = field.file_name() {
            pkg_name = fname.to_string();
        }
        if let Ok(data) = field.bytes().await {
            pkg_data = Some(data.to_vec());
        }
    }

    let data = match pkg_data {
        Some(d) if !d.is_empty() => d,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"success": false, "message": "未提供升级包"})),
            )
                .into_response();
        }
    };

    let upgrade_dir = Path::new(UPGRADE_DIR);
    if let Err(e) = std::fs::create_dir_all(upgrade_dir) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": format!("创建升级目录失败: {}", e)})),
        )
            .into_response();
    }

    let pkg_path = upgrade_dir.join(&pkg_name);
    if let Err(e) = std::fs::write(&pkg_path, &data) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": format!("保存升级包失败: {}", e)})),
        )
            .into_response();
    }

    // Spawn upgrade script in background
    tokio::spawn(async move {
        *UPGRADE_RUNNING.lock().unwrap() = true;

        let script = upgrade_dir.join("upgrade.sh");
        let result = if script.exists() {
            std::process::Command::new("bash")
                .arg(&script)
                .arg(&pkg_path)
                .spawn()
        } else {
            // No script: just extract
            std::process::Command::new("tar")
                .args(["-xzf"])
                .arg(&pkg_path)
                .arg("-C")
                .arg(upgrade_dir)
                .spawn()
        };

        match result {
            Ok(mut child) => {
                *UPGRADE_PID.lock().unwrap() = Some(child.id());
                let _ = child.wait();
            }
            Err(e) => error!("Upgrade command error: {}", e),
        }

        *UPGRADE_RUNNING.lock().unwrap() = false;
        *UPGRADE_PID.lock().unwrap() = None;
    });

    Json(json!({
        "success": true,
        "message": "升级已启动",
        "data": { "package": pkg_name }
    }))
    .into_response()
}

// ── POST /api/v1/config/upgrade/abort ────────────────────────────────────────

#[utoipa::path(post, path = "/api/v1/config/upgrade/abort", tag = "Config",
    security(("bearer_auth" = [])),
    responses((status = 200, description = "升级已中断")))]
pub async fn abort_upgrade() -> impl IntoResponse {
    let pid = UPGRADE_PID.lock().unwrap().take();
    match pid {
        Some(pid) => {
            // Send SIGTERM
            let _ = std::process::Command::new("kill")
                .args(["-TERM", &pid.to_string()])
                .output();
            *UPGRADE_RUNNING.lock().unwrap() = false;
            Json(json!({"success": true, "message": "升级已中断"})).into_response()
        }
        None => Json(json!({"success": false, "message": "没有正在运行的升级"})).into_response(),
    }
}

// ── GET /api/v1/config/upgrade/status ────────────────────────────────────────

#[utoipa::path(get, path = "/api/v1/config/upgrade/status", tag = "Config",
    security(("bearer_auth" = [])),
    responses((status = 200, description = "升级状态")))]
pub async fn upgrade_status() -> impl IntoResponse {
    let running = *UPGRADE_RUNNING.lock().unwrap();
    let pid = *UPGRADE_PID.lock().unwrap();

    Json(json!({
        "success": true,
        "data": {
            "running": running,
            "pid": pid,
        }
    }))
    .into_response()
}
