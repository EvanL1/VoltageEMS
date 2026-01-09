//! Health check and diagnostics for VoltageEMS system

use anyhow::Result;
use colored::*;
use serde::Serialize;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use crate::utils::check_database_status;

/// Check result status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckStatus {
    Ok,
    Warning,
    Error,
}

/// Single check result
#[derive(Debug, Serialize)]
pub struct CheckResult {
    pub name: String,
    pub status: CheckStatus,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
}

impl CheckResult {
    fn ok(name: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: CheckStatus::Ok,
            message: message.into(),
            suggestion: None,
            duration_ms: None,
        }
    }

    fn warning(
        name: impl Into<String>,
        message: impl Into<String>,
        suggestion: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            status: CheckStatus::Warning,
            message: message.into(),
            suggestion: Some(suggestion.into()),
            duration_ms: None,
        }
    }

    fn error(
        name: impl Into<String>,
        message: impl Into<String>,
        suggestion: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            status: CheckStatus::Error,
            message: message.into(),
            suggestion: Some(suggestion.into()),
            duration_ms: None,
        }
    }

    fn with_duration(mut self, duration: Duration) -> Self {
        self.duration_ms = Some(duration.as_millis() as u64);
        self
    }
}

/// Health response from service /health endpoint
#[derive(Debug, serde::Deserialize)]
struct HealthResponse {
    status: String,
    #[serde(default)]
    checks: Option<serde_json::Value>,
}

/// Run all health checks
pub async fn run_doctor(
    config_path: impl AsRef<Path>,
    db_path: impl AsRef<Path>,
    verbose: bool,
    json_output: bool,
) -> Result<()> {
    let config_path = config_path.as_ref();
    let db_path = db_path.as_ref();
    let mut results = Vec::new();

    // Run all checks
    results.push(check_docker().await);
    results.push(check_redis().await);
    results.push(check_service("comsrv", 6001).await);
    results.push(check_service("modsrv", 6002).await);
    results.push(check_database(db_path).await);
    results.push(check_config_files(config_path).await);
    results.push(check_shared_memory().await);

    if json_output {
        println!("{}", serde_json::to_string_pretty(&results)?);
    } else {
        print_results(&results, verbose);
    }

    // Exit with error code if any check failed
    let has_errors = results.iter().any(|r| r.status == CheckStatus::Error);
    if has_errors {
        std::process::exit(1);
    }

    Ok(())
}

/// Check Docker engine status
async fn check_docker() -> CheckResult {
    let start = Instant::now();

    let output = Command::new("docker")
        .args(["info", "--format", "{{.ServerVersion}}"])
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let version = String::from_utf8_lossy(&out.stdout).trim().to_string();
            CheckResult::ok("Docker Engine", format!("Running (v{})", version))
                .with_duration(start.elapsed())
        },
        Ok(_) => CheckResult::error(
            "Docker Engine",
            "Not running",
            "Start Docker Desktop or run: sudo systemctl start docker",
        )
        .with_duration(start.elapsed()),
        Err(_) => CheckResult::error(
            "Docker Engine",
            "Not installed",
            "Install Docker: https://docs.docker.com/get-docker/",
        )
        .with_duration(start.elapsed()),
    }
}

/// Check Redis container status
async fn check_redis() -> CheckResult {
    let start = Instant::now();

    // First check if container is running
    let container_check = Command::new("docker")
        .args(["inspect", "-f", "{{.State.Running}}", "voltage-redis"])
        .output();

    match container_check {
        Ok(out) if out.status.success() => {
            let running = String::from_utf8_lossy(&out.stdout).trim() == "true";
            if !running {
                return CheckResult::error(
                    "voltage-redis",
                    "Container stopped",
                    "monarch services start voltage-redis",
                )
                .with_duration(start.elapsed());
            }

            // Check Redis connectivity
            let ping = Command::new("docker")
                .args(["exec", "voltage-redis", "redis-cli", "ping"])
                .output();

            match ping {
                Ok(p) if p.status.success() => {
                    let response = String::from_utf8_lossy(&p.stdout).trim().to_string();
                    if response == "PONG" {
                        CheckResult::ok("voltage-redis", "Healthy (6379)")
                            .with_duration(start.elapsed())
                    } else {
                        CheckResult::warning(
                            "voltage-redis",
                            "Container running but not responding",
                            "monarch services logs voltage-redis",
                        )
                        .with_duration(start.elapsed())
                    }
                },
                _ => CheckResult::warning(
                    "voltage-redis",
                    "Container running but ping failed",
                    "monarch services logs voltage-redis",
                )
                .with_duration(start.elapsed()),
            }
        },
        _ => CheckResult::error(
            "voltage-redis",
            "Container not found",
            "monarch services start voltage-redis",
        )
        .with_duration(start.elapsed()),
    }
}

/// Check service health via HTTP endpoint
async fn check_service(name: &str, port: u16) -> CheckResult {
    let start = Instant::now();
    let url = format!("http://localhost:{}/health", port);

    let client = match reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
    {
        Ok(c) => c,
        Err(_) => {
            return CheckResult::error(
                name,
                "Failed to create HTTP client",
                "Check system configuration",
            )
            .with_duration(start.elapsed());
        },
    };

    match client.get(&url).send().await {
        Ok(response) => {
            if response.status().is_success() {
                match response.json::<HealthResponse>().await {
                    Ok(health) => {
                        let extra = if let Some(checks) = &health.checks {
                            // Extract useful info from checks
                            if let Some(obj) = checks.as_object() {
                                if name == "comsrv" {
                                    if let Some(ch) = obj.get("channels") {
                                        if let Some(msg) =
                                            ch.get("message").and_then(|m| m.as_str())
                                        {
                                            format!(" - {}", msg)
                                        } else {
                                            String::new()
                                        }
                                    } else {
                                        String::new()
                                    }
                                } else if name == "modsrv" {
                                    if let Some(inst) = obj.get("instances") {
                                        if let Some(count) =
                                            inst.get("count").and_then(|c| c.as_i64())
                                        {
                                            format!(" - {} instances", count)
                                        } else {
                                            String::new()
                                        }
                                    } else {
                                        String::new()
                                    }
                                } else {
                                    String::new()
                                }
                            } else {
                                String::new()
                            }
                        } else {
                            String::new()
                        };

                        if health.status == "healthy" {
                            CheckResult::ok(name, format!("Healthy ({}){}", port, extra))
                                .with_duration(start.elapsed())
                        } else {
                            CheckResult::warning(
                                name,
                                format!("Degraded ({}){}", port, extra),
                                format!("monarch services logs {}", name),
                            )
                            .with_duration(start.elapsed())
                        }
                    },
                    Err(_) => CheckResult::warning(
                        name,
                        format!("Running ({}) but invalid health response", port),
                        format!("monarch services logs {}", name),
                    )
                    .with_duration(start.elapsed()),
                }
            } else {
                CheckResult::warning(
                    name,
                    format!("Unhealthy ({}) - status {}", port, response.status()),
                    format!("monarch services logs {}", name),
                )
                .with_duration(start.elapsed())
            }
        },
        Err(e) => {
            let msg = if e.is_connect() {
                "Not running or not reachable"
            } else if e.is_timeout() {
                "Connection timeout"
            } else {
                "Connection failed"
            };
            CheckResult::error(name, msg, format!("monarch services start {}", name))
                .with_duration(start.elapsed())
        },
    }
}

/// Check SQLite database status
async fn check_database(db_path: &Path) -> CheckResult {
    let start = Instant::now();
    let db_file = db_path.join("monarch.db");

    match check_database_status(&db_file).await {
        Ok(status) => {
            if !status.exists {
                CheckResult::error(
                    "SQLite Database",
                    "Not found",
                    "monarch init && monarch sync",
                )
                .with_duration(start.elapsed())
            } else if !status.initialized {
                CheckResult::warning("SQLite Database", "Not initialized", "monarch init")
                    .with_duration(start.elapsed())
            } else {
                let sync_info = status
                    .last_sync
                    .map(|t| format!(", synced {}", t))
                    .unwrap_or_default();
                CheckResult::ok("SQLite Database", format!("Initialized{}", sync_info))
                    .with_duration(start.elapsed())
            }
        },
        Err(e) => CheckResult::error(
            "SQLite Database",
            format!("Error: {}", e),
            "Check database file permissions",
        )
        .with_duration(start.elapsed()),
    }
}

/// Check configuration files
async fn check_config_files(config_path: &Path) -> CheckResult {
    let start = Instant::now();
    let required_files = [
        "channels.yaml",
        "points_telemetry.csv",
        "points_signal.csv",
        "points_control.csv",
        "points_adjustment.csv",
    ];

    let mut missing = Vec::new();
    for file in &required_files {
        if !config_path.join(file).exists() {
            missing.push(*file);
        }
    }

    if missing.is_empty() {
        CheckResult::ok("Config Files", "All present").with_duration(start.elapsed())
    } else if missing.len() == required_files.len() {
        CheckResult::error(
            "Config Files",
            "No config files found",
            format!("Create config files in: {}", config_path.display()),
        )
        .with_duration(start.elapsed())
    } else {
        CheckResult::warning(
            "Config Files",
            format!("Missing: {}", missing.join(", ")),
            "Some config files are missing, sync may be incomplete",
        )
        .with_duration(start.elapsed())
    }
}

/// Check shared memory availability
async fn check_shared_memory() -> CheckResult {
    let start = Instant::now();
    let shm_path = Path::new("/dev/shm/voltage-rtdb.shm");

    if shm_path.exists() {
        let metadata = std::fs::metadata(shm_path);
        match metadata {
            Ok(m) => {
                let size_mb = m.len() as f64 / 1024.0 / 1024.0;
                CheckResult::ok("Shared Memory", format!("Available ({:.1} MB)", size_mb))
                    .with_duration(start.elapsed())
            },
            Err(_) => CheckResult::warning(
                "Shared Memory",
                "File exists but not readable",
                "Check file permissions: ls -la /dev/shm/voltage-rtdb.shm",
            )
            .with_duration(start.elapsed()),
        }
    } else {
        // Shared memory is optional, so this is just a warning
        CheckResult::warning(
            "Shared Memory",
            "Not available",
            "Set ENABLE_SHARED_MEMORY=true and restart services",
        )
        .with_duration(start.elapsed())
    }
}

/// Print results in a nice table format
fn print_results(results: &[CheckResult], verbose: bool) {
    println!();
    println!(
        "{}",
        "┌─────────────────────────────────────────────────────────┐".bright_blue()
    );
    println!(
        "{}",
        "│          VoltageEMS System Health Check                 │".bright_blue()
    );
    println!(
        "{}",
        "├─────────────────────────────────────────────────────────┤".bright_blue()
    );

    for result in results {
        let icon = match result.status {
            CheckStatus::Ok => "✓".green(),
            CheckStatus::Warning => "⚠".yellow(),
            CheckStatus::Error => "✗".red(),
        };

        let name = format!("{:<18}", result.name);
        let message = if verbose {
            if let Some(ms) = result.duration_ms {
                format!("{} ({}ms)", result.message, ms)
            } else {
                result.message.clone()
            }
        } else {
            result.message.clone()
        };

        println!("│ {} {} {:<35} │", icon, name, message);

        if let Some(ref suggestion) = result.suggestion {
            println!("│   {} {:<52} │", "→".cyan(), suggestion.dimmed());
        }
    }

    println!(
        "{}",
        "├─────────────────────────────────────────────────────────┤".bright_blue()
    );

    let ok_count = results
        .iter()
        .filter(|r| r.status == CheckStatus::Ok)
        .count();
    let total = results.len();
    let summary = format!("{}/{} checks passed", ok_count, total);

    let summary_colored = if ok_count == total {
        summary.green()
    } else if ok_count >= total - 1 {
        summary.yellow()
    } else {
        summary.red()
    };

    println!("│ {:<55} │", summary_colored);
    println!(
        "{}",
        "└─────────────────────────────────────────────────────────┘".bright_blue()
    );
    println!();
}
