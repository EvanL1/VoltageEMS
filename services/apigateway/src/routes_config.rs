use std::io::{self, Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;

use axum::{
    body::Body,
    extract::Multipart,
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use tracing::{error, info};

const CONFIG_DIR: &str = "/opt/MonarchEdge/data";
const UPGRADE_DIR: &str = "/opt/MonarchEdge/upgrade";
const UPGRADE_STATUS_FILE: &str = "/opt/MonarchEdge/upgrade/upgrade_status.json";
const UPGRADE_LOG_FILE: &str = "/opt/MonarchEdge/upgrade/upgrade.log";

// In-memory upgrade state (also mirrored to UPGRADE_STATUS_FILE for persistence across restarts)
static UPGRADE_PID: Mutex<Option<u32>> = Mutex::new(None);
static UPGRADE_RUNNING: Mutex<bool> = Mutex::new(false);

// Upload progress tracking (reset on each new upload attempt)
static UPLOAD_RECEIVED_BYTES: AtomicU64 = AtomicU64::new(0);
static UPLOAD_TOTAL_BYTES: AtomicU64 = AtomicU64::new(0);
// Set while an upload stream is in progress; cleared when it ends (success or error).
static UPLOAD_IN_PROGRESS: AtomicBool = AtomicBool::new(false);
// Set by abort_upgrade to signal the streaming loop to stop early.
static UPLOAD_ABORT_REQUESTED: AtomicBool = AtomicBool::new(false);

fn write_upgrade_status(data: &serde_json::Value) {
    if let Ok(s) = serde_json::to_string_pretty(data) {
        let _ = std::fs::write(UPGRADE_STATUS_FILE, s);
    }
}

/// Called once at apigateway startup.
///
/// If the status file shows `"running"` but `UPGRADE_RUNNING` is false (fresh
/// process), the previous upgrade was interrupted by a container restart — most
/// likely because the installer replaced the voltageems image and brought the
/// service back up.  Mark the status as "completed_or_restarted" so the status
/// endpoint never returns the contradictory `running=false` + `status="running"`.
pub fn reconcile_upgrade_status_on_startup() {
    if let Ok(content) = std::fs::read_to_string(UPGRADE_STATUS_FILE) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&content) {
            if v.get("status").and_then(|s| s.as_str()) == Some("running") {
                write_upgrade_status(&json!({
                    "status": "completed_or_restarted",
                    "message": "Service was restarted during upgrade (likely by the installer). Check the upgrade log for the actual outcome.",
                    "recovered_at": chrono::Utc::now().to_rfc3339(),
                }));
                info!("Recovered stale upgrade status: 'running' → 'completed_or_restarted'");
            }
        }
    }
}

async fn read_upgrade_status() -> serde_json::Value {
    tokio::fs::read_to_string(UPGRADE_STATUS_FILE)
        .await
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| json!({"status": "idle"}))
}

// ── GET /api/v1/config/check ──────────────────────────────────────────────────

#[utoipa::path(get, path = "/api/v1/config/check", tag = "Config",
    security(("bearer_auth" = [])),
    responses((status = 200, description = "配置目录检查结果")))]
pub async fn check_config() -> impl IntoResponse {
    let dir = Path::new(CONFIG_DIR);
    if !dir.exists() {
        return Json(json!({
            "success": false,
            "message": format!("Config directory not found: {}", CONFIG_DIR),
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
        "message": "Config directory check completed",
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
            Json(json!({"success": false, "message": "Config directory not found"})),
        )
            .into_response();
    }

    match create_zip_archive(dir) {
        Ok(data) => {
            let filename = format!("config_{}.zip", chrono::Utc::now().format("%Y%m%d_%H%M%S"));
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/zip")
                .header(
                    header::CONTENT_DISPOSITION,
                    format!("attachment; filename=\"{}\"", filename),
                )
                .body(Body::from(data))
                .unwrap()
        },
        Err(e) => {
            error!("Export config error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(
                    json!({"success": false, "message": format!("Failed to export config: {}", e)}),
                ),
            )
                .into_response()
        },
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

    loop {
        match multipart.next_field().await {
            Ok(None) => break,
            Err(e) => {
                let msg = classify_upload_error(&e.to_string(), 64);
                error!("Multipart parse error: {}", e);
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"success": false, "message": msg})),
                )
                    .into_response();
            },
            Ok(Some(field)) => {
                let name = field.name().unwrap_or("").to_string();
                if name == "file" || name == "zip" || zip_data.is_none() {
                    match field.bytes().await {
                        Ok(data) => zip_data = Some(data.to_vec()),
                        Err(e) => {
                            let msg = classify_upload_error(&e.to_string(), 64);
                            error!("Read upload error: {}", e);
                            return (
                                StatusCode::BAD_REQUEST,
                                Json(json!({"success": false, "message": msg})),
                            )
                                .into_response();
                        },
                    }
                }
            },
        }
    }

    let data = match zip_data {
        Some(d) if !d.is_empty() => d,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"success": false, "message": "No config file received. Please select a ZIP file and try again."})),
            )
                .into_response();
        },
    };

    let target = Path::new(CONFIG_DIR);
    if let Err(e) = std::fs::create_dir_all(target) {
        error!("Create config dir error: {}", e);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": format!("Failed to create config directory: {}", e)})),
        )
            .into_response();
    }

    match extract_zip(&data, target) {
        Ok(count) => {
            info!("Config imported: {} files", count);

            // Restart all services so they pick up the new database
            let services = [
                "voltageems-comsrv",
                "voltageems-modsrv",
                "voltageems-hissrv",
                "voltageems-netsrv",
                "voltageems-alarmsrv",
            ];
            let mut restart_results = Vec::new();
            for svc in &services {
                match std::process::Command::new("docker")
                    .args(["restart", svc])
                    .output()
                {
                    Ok(o) if o.status.success() => {
                        info!("Restarted service: {}", svc);
                        restart_results.push(json!({"service": svc, "success": true}));
                    },
                    Ok(o) => {
                        let err = String::from_utf8_lossy(&o.stderr).to_string();
                        error!("Restart {} failed: {}", svc, err);
                        restart_results
                            .push(json!({"service": svc, "success": false, "error": err}));
                    },
                    Err(e) => {
                        error!("Docker command error for {}: {}", svc, e);
                        restart_results.push(
                            json!({"service": svc, "success": false, "error": e.to_string()}),
                        );
                    },
                }
            }

            Json(json!({
                "success": true,
                "message": "Config imported successfully, services restarted",
                "data": {
                    "files_extracted": count,
                    "restart_results": restart_results,
                }
            }))
            .into_response()
        },
        Err(e) => {
            error!("Extract zip error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": format!("Failed to extract config: {}", e)})),
            )
                .into_response()
        },
    }
}

fn extract_zip(data: &[u8], target: &Path) -> io::Result<usize> {
    let cursor = io::Cursor::new(data);
    let mut archive =
        zip::ZipArchive::new(cursor).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

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
    let services = [
        "voltageems-comsrv",
        "voltageems-modsrv",
        "voltageems-hissrv",
        "voltageems-netsrv",
        "voltageems-alarmsrv",
    ];
    let mut results = Vec::new();

    for svc in &services {
        let output = std::process::Command::new("docker")
            .args(["restart", svc])
            .output();

        match output {
            Ok(o) if o.status.success() => {
                info!("Restarted service: {}", svc);
                results.push(json!({"service": svc, "success": true}));
            },
            Ok(o) => {
                let err = String::from_utf8_lossy(&o.stderr).to_string();
                error!("Restart {} failed: {}", svc, err);
                results.push(json!({"service": svc, "success": false, "error": err}));
            },
            Err(e) => {
                error!("Docker command error for {}: {}", svc, e);
                results.push(json!({"service": svc, "success": false, "error": e.to_string()}));
            },
        }
    }

    let all_ok = results
        .iter()
        .all(|r| r["success"].as_bool().unwrap_or(false));
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
pub async fn start_upgrade(headers: HeaderMap, mut multipart: Multipart) -> impl IntoResponse {
    use tokio::io::AsyncWriteExt;

    // Log immediately so that even if the connection is later dropped mid-upload,
    // there is a record that this handler was invoked.
    info!("Upgrade upload request received");

    {
        let running = UPGRADE_RUNNING.lock().unwrap();
        if *running {
            return (
                StatusCode::CONFLICT,
                Json(json!({"success": false, "message": "Upgrade already in progress"})),
            )
                .into_response();
        }
    }

    // Run all prerequisite checks before touching the request body so that
    // obvious failures are reported immediately without streaming any bytes.
    let docker_socket = Path::new("/var/run/docker.sock");
    if !docker_socket.exists() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({
                "success": false,
                "message": "Docker Socket unavailable — ensure /var/run/docker.sock is mounted in docker-compose.yml",
                "hint": "volumes:\\n  - /var/run/docker.sock:/var/run/docker.sock"
            })),
        )
            .into_response();
    }

    let upgrade_dir = Path::new(UPGRADE_DIR);
    if let Err(e) = std::fs::create_dir_all(upgrade_dir) {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": format!("Failed to create upgrade directory: {}", e)})),
        )
            .into_response();
    }

    // Clean up any leftover upgrade packages from previous runs before writing a
    // new one.  Without this, multiple 500 MB .run files accumulate on disk,
    // rapidly filling the partition and causing the next upload to slow to a crawl
    // or fail with ENOSPC mid-transfer.
    match std::fs::read_dir(upgrade_dir) {
        Ok(entries) => {
            for entry in entries.flatten() {
                let path = entry.path();
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_lowercase();
                if ext == "run" || ext == "sh" {
                    if let Err(e) = std::fs::remove_file(&path) {
                        error!("Failed to remove old upgrade package {:?}: {}", path, e);
                    } else {
                        info!("Removed old upgrade package: {:?}", path);
                    }
                }
            }
        },
        Err(e) => {
            error!("Failed to list upgrade directory for cleanup: {}", e);
        },
    }

    // Refuse early if there is less than 2 GB of free space on the upgrade
    // partition — a typical .run package is ~500 MB and the installer needs
    // additional headroom for Docker image layers and extraction.  2 GB gives
    // comfortable margin even when the previous upgrade left temporary files.
    {
        let df_out = std::process::Command::new("df")
            .args(["--output=avail", "-B1", UPGRADE_DIR])
            .output();
        if let Ok(out) = df_out {
            // df output: header line + data line, avail is in bytes (-B1)
            let stdout = String::from_utf8_lossy(&out.stdout);
            if let Some(avail_bytes) = stdout
                .lines()
                .nth(1)
                .and_then(|l| l.trim().parse::<u64>().ok())
            {
                const MIN_FREE: u64 = 2_000 * 1024 * 1024; // 2 GB
                info!(
                    "Disk space check (after cleanup): {:.1} MB free on {}",
                    avail_bytes as f64 / (1024.0 * 1024.0),
                    UPGRADE_DIR
                );
                if avail_bytes < MIN_FREE {
                    return (
                        StatusCode::INSUFFICIENT_STORAGE,
                        Json(json!({
                            "success": false,
                            "message": format!(
                                "Insufficient disk space: {:.1} MB free, at least 2 GB required",
                                avail_bytes as f64 / (1024.0 * 1024.0)
                            )
                        })),
                    )
                        .into_response();
                }
            }
        }
    }

    // Get the upload field.
    let field = match multipart.next_field().await {
        Ok(None) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"success": false, "message": "No upgrade package received. Please select an upgrade file and try again."})),
            )
                .into_response();
        },
        Err(e) => {
            let msg = classify_upload_error(&e.to_string(), 1024);
            error!("Upgrade multipart parse error: {}", e);
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({"success": false, "message": msg})),
            )
                .into_response();
        },
        Ok(Some(f)) => f,
    };

    let pkg_name = field.file_name().unwrap_or("upgrade.run").to_string();
    let pkg_path = upgrade_dir.join(&pkg_name);

    // Extract Content-Length from the request headers for progress tracking.
    // This is the full multipart body size (slightly larger than the file itself
    // due to multipart boundaries), but accurate enough for a progress indicator.
    let total_bytes = headers
        .get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);

    UPLOAD_RECEIVED_BYTES.store(0, Ordering::Relaxed);
    UPLOAD_TOTAL_BYTES.store(total_bytes, Ordering::Relaxed);
    UPLOAD_ABORT_REQUESTED.store(false, Ordering::Relaxed);
    UPLOAD_IN_PROGRESS.store(true, Ordering::Relaxed);

    write_upgrade_status(&json!({
        "status": "uploading",
        "filename": pkg_name,
        "total_bytes": total_bytes,
        "started_at": chrono::Utc::now().to_rfc3339(),
    }));

    info!(
        "Streaming upgrade package to disk: {} → {:?}",
        pkg_name, pkg_path
    );

    // Create the output file wrapped in a 4 MB BufWriter. Multipart chunks from
    // the network are typically 8–64 KB; without buffering each chunk triggers
    // a separate write syscall. On slow eMMC / SD storage that creates severe
    // I/O backpressure which stalls the TCP receive window and makes the upload
    // appear "slow". The 4 MB buffer batches ~64–512 chunks into one large
    // write, dramatically reducing syscall overhead.
    let mut out_file = match tokio::fs::File::create(&pkg_path).await {
        Ok(f) => tokio::io::BufWriter::with_capacity(4 * 1024 * 1024, f),
        Err(e) => {
            error!("Failed to create upgrade file: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "message": format!("Failed to create upgrade file: {}", e)})),
            )
                .into_response();
        },
    };

    // Stream the upload directly to disk chunk-by-chunk instead of buffering
    // the entire file in memory.  A 30-minute absolute deadline handles even
    // very slow links; the same deadline is reused across iterations so the
    // total allowed time is capped, not the per-chunk time.
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(30 * 60);
    let mut received_bytes: u64 = 0;
    let mut field = field;

    // Macro to consolidate the repeated cleanup on error paths inside the loop.
    macro_rules! upload_abort_cleanup {
        ($pkg_path:expr) => {{
            UPLOAD_IN_PROGRESS.store(false, Ordering::Relaxed);
            UPLOAD_ABORT_REQUESTED.store(false, Ordering::Relaxed);
            UPLOAD_RECEIVED_BYTES.store(0, Ordering::Relaxed);
            UPLOAD_TOTAL_BYTES.store(0, Ordering::Relaxed);
            drop(out_file);
            let _ = std::fs::remove_file(&$pkg_path);
        }};
    }

    loop {
        // Check for a user-initiated abort before requesting the next chunk.
        if UPLOAD_ABORT_REQUESTED.load(Ordering::Relaxed) {
            upload_abort_cleanup!(pkg_path);
            write_upgrade_status(&json!({
                "status": "aborted",
                "aborted_at": chrono::Utc::now().to_rfc3339(),
                "message": "Upload aborted by user"
            }));
            info!(
                "Upgrade upload aborted by user after {} bytes",
                received_bytes
            );
            return (
                StatusCode::GONE,
                Json(json!({"success": false, "message": "Upload aborted by user"})),
            )
                .into_response();
        }

        match tokio::time::timeout_at(deadline, field.chunk()).await {
            Err(_elapsed) => {
                upload_abort_cleanup!(pkg_path);
                write_upgrade_status(&json!({"status": "idle"}));
                error!("Upgrade upload timed out after 30 minutes");
                return (
                    StatusCode::REQUEST_TIMEOUT,
                    Json(json!({
                        "success": false,
                        "message": "Upload timed out after 30 minutes. Please check your network connection and retry."
                    })),
                )
                    .into_response();
            },
            Ok(Err(e)) => {
                upload_abort_cleanup!(pkg_path);
                write_upgrade_status(&json!({"status": "idle"}));
                let msg = classify_upload_error(&e.to_string(), 1024);
                error!("Upgrade upload read error: {}", e);
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"success": false, "message": msg})),
                )
                    .into_response();
            },
            Ok(Ok(None)) => break,
            Ok(Ok(Some(chunk))) => {
                if let Err(e) = out_file.write_all(&chunk).await {
                    upload_abort_cleanup!(pkg_path);
                    write_upgrade_status(&json!({"status": "idle"}));
                    error!("Upgrade file write error: {}", e);
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({"success": false, "message": format!("Failed to write upgrade package: {}", e)})),
                    )
                        .into_response();
                }
                received_bytes += chunk.len() as u64;
                UPLOAD_RECEIVED_BYTES.store(received_bytes, Ordering::Relaxed);
            },
        }
    }

    UPLOAD_IN_PROGRESS.store(false, Ordering::Relaxed);

    if let Err(e) = out_file.flush().await {
        drop(out_file);
        let _ = std::fs::remove_file(&pkg_path);
        UPLOAD_RECEIVED_BYTES.store(0, Ordering::Relaxed);
        UPLOAD_TOTAL_BYTES.store(0, Ordering::Relaxed);
        error!("Upgrade file flush error: {}", e);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"success": false, "message": format!("Failed to flush upgrade package: {}", e)})),
        )
            .into_response();
    }
    drop(out_file);

    if received_bytes == 0 {
        let _ = std::fs::remove_file(&pkg_path);
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"success": false, "message": "No upgrade package received. Please select an upgrade file and try again."})),
        )
            .into_response();
    }

    info!(
        "Upgrade package received: {} ({:.2} MB)",
        pkg_name,
        received_bytes as f64 / (1024.0 * 1024.0)
    );

    // Lock upload counters at 100% so the status endpoint shows the completed
    // upload size during the install phase (instead of confusing 0/0 values).
    UPLOAD_RECEIVED_BYTES.store(received_bytes, Ordering::Relaxed);
    UPLOAD_TOTAL_BYTES.store(received_bytes, Ordering::Relaxed);

    // Prepare upgrade: chmod +x so the .run file is executable
    let pkg_name_lower = pkg_name.to_lowercase();
    if pkg_name_lower.ends_with(".run") || pkg_name_lower.ends_with(".sh") {
        let _ = std::process::Command::new("chmod")
            .args(["+x"])
            .arg(&pkg_path)
            .output();
    }

    // Set UPGRADE_RUNNING=true and write status file BEFORE spawning the blocking task.
    // If we set it inside spawn_blocking, the tokio thread pool may not schedule the task
    // immediately — a status poll arriving in that window would incorrectly see running=false.
    *UPGRADE_RUNNING.lock().unwrap() = true;

    let size_mb = received_bytes as f64 / (1024.0 * 1024.0);
    write_upgrade_status(&json!({
        "status": "running",
        "filename": pkg_name,
        "size_mb": format!("{:.2}", size_mb),
        "started_at": chrono::Utc::now().to_rfc3339(),
    }));
    let log_header = format!(
        "=== VoltageEMS Upgrade Log ===\nStarted at: {}\nPackage: {}\nFile size: {:.2} MB\n{}\n\n",
        chrono::Utc::now().to_rfc3339(),
        pkg_name,
        size_mb,
        "=".repeat(60)
    );
    let _ = std::fs::write(UPGRADE_LOG_FILE, log_header);

    tokio::task::spawn_blocking(move || {
        // UPGRADE_RUNNING is already true (set before this task was spawned)

        // Remove any leftover upgrader container before starting a new one.
        // This handles the case where a previous run was killed mid-flight or the container
        // name is still registered after the last session.
        let _ = std::process::Command::new("docker")
            .args(["rm", "-f", "voltageems-upgrader"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();

        // Build the upgrade command first, then attach log redirection before spawning.
        // This ensures ALL output (stdout + stderr) from the upgrade process is captured
        // into upgrade.log so the status API can return meaningful progress to the frontend.
        //
        // IMPORTANT: do NOT use -d (detached). We run synchronously so that child.wait()
        // tracks the real upgrade completion and UPGRADE_RUNNING stays true throughout.
        let mut cmd = if pkg_name_lower.ends_with(".run") || pkg_name_lower.ends_with(".sh") {
            let docker_available = std::process::Command::new("docker")
                .args(["info"])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false);

            if docker_available {
                let mut c = std::process::Command::new("docker");
                c.args([
                    "run",
                    "--rm",
                    "--name",
                    "voltageems-upgrader",
                    "--pid",
                    "host",
                    "--privileged",
                    "-v",
                    "/opt/MonarchEdge:/opt/MonarchEdge",
                    "alpine:latest",
                    "nsenter",
                    "--target",
                    "1",
                    "--mount",
                    "--uts",
                    "--ipc",
                    "--net",
                    "--pid",
                    "--",
                    "bash",
                ])
                .arg(&pkg_path)
                .args(["--", "--auto"]);
                c
            } else {
                info!("Docker unavailable; running upgrade directly in container");
                let mut c = std::process::Command::new("bash");
                c.arg(&pkg_path).args(["--", "--auto"]);
                c
            }
        } else if pkg_name_lower.ends_with(".tar.gz")
            || pkg_name_lower.ends_with(".tgz")
            || pkg_name_lower.ends_with(".tar.bz2")
            || pkg_name_lower.ends_with(".tar.xz")
        {
            let mut c = std::process::Command::new("tar");
            c.args(["-xaf"]).arg(&pkg_path).arg("-C").arg(upgrade_dir);
            c
        } else {
            let mut c = std::process::Command::new("bash");
            c.arg(&pkg_path);
            c
        };

        // Redirect stdout + stderr → upgrade.log (append mode)
        // Both handles must be separate File objects (can't share one fd for stdout+stderr).
        match std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(UPGRADE_LOG_FILE)
        {
            Ok(log_out) => match log_out.try_clone() {
                Ok(log_err) => {
                    cmd.stdout(std::process::Stdio::from(log_out))
                        .stderr(std::process::Stdio::from(log_err));
                },
                Err(e) => error!("Failed to clone log fd: {}", e),
            },
            Err(e) => error!("Failed to open upgrade log for writing: {}", e),
        }

        let result = cmd.spawn();

        match result {
            Ok(mut child) => {
                *UPGRADE_PID.lock().unwrap() = Some(child.id());
                let exit_code = child.wait().ok().and_then(|s| s.code()).unwrap_or(-1);
                if exit_code == 0 {
                    info!("Upgrade completed successfully");
                    write_upgrade_status(&json!({
                        "status": "completed",
                        "finished_at": chrono::Utc::now().to_rfc3339(),
                        "exit_code": 0,
                        "message": "Upgrade successful"
                    }));
                } else {
                    error!("Upgrade failed with exit code {}", exit_code);
                    write_upgrade_status(&json!({
                        "status": "failed",
                        "finished_at": chrono::Utc::now().to_rfc3339(),
                        "exit_code": exit_code,
                        "message": format!("Upgrade failed (exit code {})", exit_code)
                    }));
                }
            },
            Err(e) => {
                error!("Upgrade command error: {}", e);
                write_upgrade_status(&json!({
                    "status": "failed",
                    "finished_at": chrono::Utc::now().to_rfc3339(),
                    "message": format!("Failed to start upgrade: {}", e)
                }));
            },
        }

        // Delete the .run / .sh package now that the installer has finished.
        // Leaving it on disk is the primary cause of slow subsequent uploads:
        // the next upload's pre-upload cleanup still has to contend with a
        // nearly-full disk during the actual transfer.  Deleting it here frees
        // ~500 MB immediately, well before the user starts the next upgrade.
        match std::fs::read_dir(UPGRADE_DIR) {
            Ok(entries) => {
                for entry in entries.flatten() {
                    let path = entry.path();
                    let ext = path
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("")
                        .to_lowercase();
                    if ext == "run" || ext == "sh" {
                        if let Err(e) = std::fs::remove_file(&path) {
                            error!(
                                "Failed to delete upgrade package after install {:?}: {}",
                                path, e
                            );
                        } else {
                            info!("Deleted upgrade package after install: {:?}", path);
                        }
                    }
                }
            },
            Err(e) => error!("Post-install cleanup read_dir failed: {}", e),
        }

        *UPGRADE_RUNNING.lock().unwrap() = false;
        *UPGRADE_PID.lock().unwrap() = None;
    });

    Json(json!({
        "success": true,
        "message": "Upgrade started",
        "data": { "package": pkg_name }
    }))
    .into_response()
}

// ── POST /api/v1/config/upgrade/abort ────────────────────────────────────────

#[utoipa::path(post, path = "/api/v1/config/upgrade/abort", tag = "Config",
    security(("bearer_auth" = [])),
    responses((status = 200, description = "升级已中断")))]
pub async fn abort_upgrade() -> impl IntoResponse {
    let uploading = UPLOAD_IN_PROGRESS.load(Ordering::Relaxed);
    let pid = UPGRADE_PID.lock().unwrap().take();
    let installing = *UPGRADE_RUNNING.lock().unwrap();

    if !uploading && !installing && pid.is_none() {
        return Json(json!({"success": false, "message": "No upgrade in progress"}))
            .into_response();
    }

    // ── Phase 1: abort an in-progress upload ──────────────────────────────────
    // Setting this flag causes the streaming loop in start_upgrade to break on
    // its next iteration, clean up the partial file, and write "aborted" status.
    if uploading {
        UPLOAD_ABORT_REQUESTED.store(true, Ordering::Relaxed);
        info!("Abort requested: signalled upload streaming loop to stop");
        // Give the streaming loop a moment to honour the flag and clean up,
        // then return immediately — the status endpoint will reflect "aborted".
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }

    // ── Phase 2: abort an in-progress install ─────────────────────────────────
    if installing || pid.is_some() {
        let _ = tokio::process::Command::new("docker")
            .args(["stop", "voltageems-upgrader"])
            .output()
            .await;

        if let Some(pid) = pid {
            let _ = tokio::process::Command::new("kill")
                .args(["-TERM", &pid.to_string()])
                .output()
                .await;
        }

        *UPGRADE_RUNNING.lock().unwrap() = false;
        *UPGRADE_PID.lock().unwrap() = None;

        write_upgrade_status(&json!({
            "status": "aborted",
            "aborted_at": chrono::Utc::now().to_rfc3339(),
            "message": "Upgrade aborted by user"
        }));
    }

    Json(json!({"success": true, "message": "Upgrade aborted"})).into_response()
}

// ── GET /api/v1/config/upgrade/status ────────────────────────────────────────

#[utoipa::path(get, path = "/api/v1/config/upgrade/status", tag = "Config",
    security(("bearer_auth" = [])),
    responses((status = 200, description = "升级状态")))]
pub async fn upgrade_status() -> impl IntoResponse {
    let file_status = read_upgrade_status().await;
    let mem_running = *UPGRADE_RUNNING.lock().unwrap();
    let mem_pid = *UPGRADE_PID.lock().unwrap();

    // Cross-check: if memory says not running but status file says "running",
    // verify whether the upgrader container is actually still alive on the host.
    // Use tokio::process::Command (non-blocking) with a 3-second timeout so that
    // a slow or unavailable docker daemon never stalls the status response.
    let container_running = tokio::time::timeout(
        std::time::Duration::from_secs(3),
        tokio::process::Command::new("docker")
            .args(["inspect", "-f", "{{.State.Running}}", "voltageems-upgrader"])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .output(),
    )
    .await
    .ok()
    .and_then(|r| r.ok())
    .and_then(|o| String::from_utf8(o.stdout).ok())
    .map(|s| s.trim() == "true")
    .unwrap_or(false);

    let running = mem_running || container_running;

    // If the status file still says "running" but we can confirm nothing is
    // actually running (not in memory, no upgrader container), the previous
    // run was cut short by a service restart.  Fix the file now so subsequent
    // queries and the current response are consistent.
    let file_status = if !running
        && file_status.get("status").and_then(|s| s.as_str()) == Some("running")
    {
        let recovered = json!({
            "status": "completed_or_restarted",
            "message": "Service was restarted during upgrade (likely by the installer). Check the upgrade log for the actual outcome.",
            "recovered_at": chrono::Utc::now().to_rfc3339(),
        });
        write_upgrade_status(&recovered);
        info!("Status inconsistency detected: status file was 'running' but no upgrade in progress; corrected to 'completed_or_restarted'");
        recovered
    } else {
        file_status
    };

    let log_content = tokio::fs::read_to_string(UPGRADE_LOG_FILE)
        .await
        .map(|raw| clean_log(&raw))
        .unwrap_or_default();

    // Upload progress (only meaningful while detail.status == "uploading")
    let received = UPLOAD_RECEIVED_BYTES.load(Ordering::Relaxed);
    let total = UPLOAD_TOTAL_BYTES.load(Ordering::Relaxed);
    let upload_progress_pct: Option<f64> = if total > 0 {
        Some((received as f64 / total as f64 * 100.0).min(100.0))
    } else {
        None
    };

    Json(json!({
        "success": true,
        "message": "OK",
        "data": {
            "running": running,
            "pid": mem_pid,
            "log": log_content,
            "detail": file_status,
            "upload": {
                "received_bytes": received,
                "total_bytes": total,
                "progress_pct": upload_progress_pct,
            }
        }
    }))
    .into_response()
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Strip ANSI escape sequences and collapse backspace-based progress-bar overwriting.
///
/// Makeself progress bars work by writing e.g. "  0% \b\b\b\b\b  1% \b\b\b\b\b  2%"
/// which looks correct in a terminal (each % overwrites the previous) but produces
/// garbage when captured to a file.  This function simulates the terminal rendering
/// so each "line" only keeps the final visible content.
fn clean_log(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let bytes = raw.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        match bytes[i] {
            // ANSI escape: ESC '[' <params> <final-byte>
            0x1b if i + 1 < bytes.len() && bytes[i + 1] == b'[' => {
                i += 2;
                while i < bytes.len() && !bytes[i].is_ascii_alphabetic() {
                    i += 1;
                }
                i += 1; // skip the final letter
            },
            // Backspace: erase the last character written on this line
            0x08 => {
                let trim_to = out
                    .char_indices()
                    .rev()
                    .find(|(_, c)| *c != '\n')
                    .map(|(idx, _)| idx);
                if let Some(t) = trim_to {
                    out.truncate(t);
                }
                i += 1;
            },
            // Carriage return without newline: reset to start of line
            0x0d if i + 1 < bytes.len() && bytes[i + 1] != 0x0a => {
                if let Some(nl) = out.rfind('\n') {
                    out.truncate(nl + 1);
                } else {
                    out.clear();
                }
                i += 1;
            },
            // Normal byte: append as-is
            _ => {
                // Preserve multi-byte UTF-8 sequences intact
                let ch_len = if bytes[i] < 0x80 {
                    1
                } else if bytes[i] < 0xE0 {
                    2
                } else if bytes[i] < 0xF0 {
                    3
                } else {
                    4
                };
                let end = (i + ch_len).min(bytes.len());
                if let Ok(s) = std::str::from_utf8(&bytes[i..end]) {
                    out.push_str(s);
                    i = end;
                } else {
                    i += 1;
                }
            },
        }
    }
    out
}

/// Map a multipart/body read error to a user-facing Chinese message.
/// `limit_mb` is the configured limit for this endpoint (for display only).
fn classify_upload_error(err: &str, limit_mb: usize) -> String {
    let lower = err.to_lowercase();
    if lower.contains("length limit")
        || lower.contains("too large")
        || lower.contains("size exceeded")
        || lower.contains("payload too large")
        || lower.contains("body limit")
    {
        format!(
            "File too large. Maximum upload size is {} MB. Please compress the file and try again.",
            limit_mb
        )
    } else {
        format!("Failed to parse uploaded file: {}", err)
    }
}
