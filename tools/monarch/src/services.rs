//! Service management module for Docker operations
//!
//! Provides functionality to manage VoltageEMS services

use anyhow::Result;
use clap::Subcommand;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Subcommand)]
pub enum ServiceCommands {
    /// Start services
    #[command(about = "Start one or more VoltageEMS services")]
    Start {
        /// Service names (optional, starts all if not specified)
        services: Vec<String>,
    },

    /// Stop services
    #[command(about = "Stop one or more VoltageEMS services")]
    Stop {
        /// Service names (optional, stops all if not specified)
        services: Vec<String>,
    },

    /// Restart services
    #[command(about = "Restart one or more VoltageEMS services")]
    Restart {
        /// Service names (optional, restarts all if not specified)
        services: Vec<String>,
    },

    /// Show service status
    #[command(about = "Display status of VoltageEMS services")]
    Status {
        /// Service names (optional, shows all if not specified)
        services: Vec<String>,
    },

    /// View service logs
    #[command(about = "View logs for VoltageEMS services")]
    Logs {
        /// Service name
        service: String,
        /// Follow log output
        #[arg(short, long)]
        follow: bool,
        /// Number of lines to show from the end
        #[arg(short = 'n', long, default_value = "100")]
        tail: String,
    },

    /// Reload service configurations
    #[command(about = "Reload configurations for services")]
    Reload {
        /// Service names (optional, reloads all if not specified)
        services: Vec<String>,
    },

    /// Build Docker images
    #[command(about = "Build Docker images for services")]
    Build {
        /// Service names (optional, builds all if not specified)
        services: Vec<String>,
    },

    /// Pull Docker images
    #[command(about = "Pull latest Docker images")]
    Pull,

    /// Clean up Docker resources
    #[command(about = "Clean up Docker volumes and networks")]
    Clean {
        /// Also remove volumes
        #[arg(short, long)]
        volumes: bool,
    },

    /// Refresh Docker images by recreating containers
    #[command(about = "Force recreate containers with latest images")]
    Refresh {
        /// Service names (optional, refreshes all if not specified)
        services: Vec<String>,
        /// Also pull latest images before recreating
        #[arg(short, long)]
        pull: bool,
        /// Use smart mode (only recreate if image changed, protect Redis)
        #[arg(short, long)]
        smart: bool,
    },
}

pub async fn handle_command(cmd: ServiceCommands) -> Result<()> {
    match cmd {
        ServiceCommands::Start { services } => {
            // IMPORTANT: Ensure SHM file exists before Docker starts
            // Docker bind mount creates directory if source doesn't exist!
            ensure_shm_file_exists();
            ensure_logs_dir_exists();

            let mut args = vec!["up".to_string(), "-d".to_string()];

            // Filter out "all" keyword and add specific service names
            let filtered_services: Vec<String> = services
                .into_iter()
                .filter(|s| s.to_lowercase() != "all")
                .collect();

            args.extend(filtered_services);
            execute_docker_compose_str(&args)?;
            println!("Services started");
        },
        ServiceCommands::Stop { services } => {
            let args = build_docker_compose_args("stop", "", services);
            execute_docker_compose_str(&args)?;
            println!("Services stopped");
        },
        ServiceCommands::Restart { services } => {
            ensure_shm_file_exists();
            ensure_logs_dir_exists();

            let filtered: Vec<String> = services
                .into_iter()
                .filter(|s| !s.eq_ignore_ascii_case("all"))
                .collect();

            if filtered.is_empty() {
                // Restart all: dependency-aware order
                // comsrv writes SHM, modsrv reads it — comsrv must start first
                println!("Restarting services in dependency order...");
                println!("  [1/3] comsrv (SHM writer)");
                execute_docker_compose(&["restart", "comsrv"])?;
                println!("  [2/3] modsrv (SHM reader)");
                execute_docker_compose(&["restart", "modsrv"])?;
                println!("  [3/3] remaining services");
                execute_docker_compose(&["restart", "apigateway", "hissrv", "alarmsrv", "netsrv"])?;
            } else {
                let args = build_docker_compose_args("restart", "", filtered);
                execute_docker_compose_str(&args)?;
            }
            println!("Services restarted");
        },
        ServiceCommands::Status { services } => {
            let filtered_services: Vec<&String> = services
                .iter()
                .filter(|s| !s.eq_ignore_ascii_case("all"))
                .collect();

            let args = if filtered_services.is_empty() {
                vec!["ps"]
            } else {
                let mut args = vec!["ps"];
                for service in filtered_services {
                    args.push(service);
                }
                args
            };
            execute_docker_compose(&args)?;
        },
        ServiceCommands::Logs {
            service,
            follow,
            tail,
        } => {
            let mut args = vec!["logs"];
            if follow {
                args.push("-f");
            }
            args.push("--tail");
            args.push(&tail);
            args.push(&service);
            execute_docker_compose(&args)?;
        },
        ServiceCommands::Reload { services } => {
            let hot_reload_services = vec!["comsrv", "modsrv"];

            let services_to_reload =
                if services.is_empty() || services.iter().any(|s| s.to_lowercase() == "all") {
                    hot_reload_services.clone()
                } else {
                    services
                        .iter()
                        .filter(|s| s.to_lowercase() != "all")
                        .map(|s| s.as_str())
                        .collect()
                };

            for service in services_to_reload {
                match reload_service(service).await {
                    Ok(()) => println!("Reloaded {} configuration", service),
                    Err(e) => eprintln!("Failed to reload {}: {}", service, e),
                }
            }
        },
        ServiceCommands::Build { services } => {
            let args = build_docker_compose_args("build", "", services);
            execute_docker_compose_str(&args)?;
            println!("Images built");
        },
        ServiceCommands::Pull => {
            execute_docker_compose(&["pull"])?;
            println!("Images pulled");
        },
        ServiceCommands::Clean { volumes } => {
            if volumes {
                execute_docker_compose(&["down", "-v"])?;
                println!("Services stopped and volumes removed");
            } else {
                execute_docker_compose(&["down"])?;
                println!("Services stopped");
            }
        },
        ServiceCommands::Refresh {
            services,
            pull,
            smart,
        } => {
            ensure_shm_file_exists();
            ensure_logs_dir_exists();

            if smart {
                println!("Refreshing services with smart mode...");
                println!("Detecting image changes...");

                let target_services: Vec<String> = services
                    .iter()
                    .filter(|s| !s.eq_ignore_ascii_case("all"))
                    .cloned()
                    .collect();

                // Check if voltage-redis image has changed
                let redis_changed = check_container_image_changed("voltage-redis")?;

                // All 6 Rust services share the voltageems:latest image.
                // Check any one container — if the image changed, recreate all.
                let all_services = [
                    "comsrv",
                    "modsrv",
                    "hissrv",
                    "apigateway",
                    "netsrv",
                    "alarmsrv",
                ];
                let services_changed = if target_services.is_empty() {
                    check_container_image_changed("voltageems-comsrv")?
                } else {
                    target_services.iter().any(|svc| {
                        if all_services.contains(&svc.as_str()) {
                            let container_name = format!("voltageems-{}", svc);
                            check_container_image_changed(&container_name).unwrap_or(false)
                        } else {
                            false
                        }
                    })
                };

                // Check if frontend has changed
                let frontend_changed =
                    if target_services.is_empty() || target_services.iter().any(|s| s == "apps") {
                        check_container_image_changed("voltage-apps")?
                    } else {
                        false
                    };

                // Check if TimescaleDB has changed
                let timescaledb_changed = if target_services.is_empty()
                    || target_services.iter().any(|s| s == "timescaledb")
                {
                    check_container_image_changed("voltage-timescaledb")?
                } else {
                    false
                };

                if pull {
                    println!("Pulling latest images...");
                    execute_docker_compose(&["pull"])?;
                }

                // Handle Redis separately (with protection)
                if redis_changed {
                    println!("\n⚠️  Redis image has changed");
                    println!("   Recreating Redis will cause brief service interruption");
                    println!("   Data will be preserved (AOF + RDB persistence)");
                    println!("\nRecreate Redis container? (yes/NO): ");

                    use std::io::{Write, stdin, stdout};
                    let mut input = String::new();
                    stdout().flush()?;
                    stdin().read_line(&mut input)?;

                    if input.trim() == "yes" {
                        println!("Recreating voltage-redis...");
                        execute_docker_compose(&["up", "-d", "--force-recreate", "voltage-redis"])?;
                        println!("✓ Redis recreated");
                    } else {
                        println!("Skipped Redis recreation");
                    }
                } else {
                    println!("✓ Redis image unchanged (no recreation needed)");
                }

                // Handle all Rust services (unified voltageems:latest image)
                if services_changed {
                    println!("\nRecreating services...");
                    execute_docker_compose(&[
                        "up",
                        "-d",
                        "--force-recreate",
                        "comsrv",
                        "modsrv",
                        "hissrv",
                        "apigateway",
                        "netsrv",
                        "alarmsrv",
                    ])?;
                    println!("✓ Services recreated");
                } else {
                    println!("✓ Services unchanged (no recreation needed)");
                }

                // Handle frontend
                if frontend_changed {
                    println!("\nRecreating frontend...");
                    execute_docker_compose(&["up", "-d", "--force-recreate", "apps"])?;
                    println!("✓ Frontend recreated");
                } else {
                    println!("✓ Frontend unchanged (no recreation needed)");
                }

                // Handle TimescaleDB (with data protection)
                if timescaledb_changed {
                    println!("\n⚠️  TimescaleDB image has changed");
                    println!("   Recreating TimescaleDB may affect historical data queries");
                    println!("\nRecreate TimescaleDB container? (yes/NO): ");

                    use std::io::{Write, stdin, stdout};
                    let mut input = String::new();
                    stdout().flush()?;
                    stdin().read_line(&mut input)?;

                    if input.trim() == "yes" {
                        println!("Recreating TimescaleDB...");
                        execute_docker_compose(&["up", "-d", "--force-recreate", "timescaledb"])?;
                        println!("✓ TimescaleDB recreated");
                    } else {
                        println!("Skipped TimescaleDB recreation");
                    }
                } else {
                    println!("✓ TimescaleDB unchanged (no recreation needed)");
                }

                println!("\nSmart refresh completed successfully");
            } else {
                // Legacy mode: Force recreate all
                println!("Refreshing services with latest images (force mode)...");

                let target_services: Vec<String> = services
                    .iter()
                    .filter(|s| !s.eq_ignore_ascii_case("all"))
                    .cloned()
                    .collect();
                let full_refresh = target_services.is_empty();

                if full_refresh {
                    println!("Stopping and removing all containers...");
                    execute_docker_compose(&["down"])?;

                    if pull {
                        println!("Pulling latest images for all services...");
                        execute_docker_compose(&["pull"])?;
                    }

                    println!("Recreating all containers with latest images...");
                    execute_docker_compose(&["up", "-d", "--force-recreate"])?;
                } else {
                    println!(
                        "Stopping and removing selected services: {}",
                        target_services.join(", ")
                    );

                    let stop_args = build_docker_compose_args("stop", "", target_services.clone());
                    execute_docker_compose_str(&stop_args)?;

                    let rm_args = build_docker_compose_args("rm", "-f", target_services.clone());
                    execute_docker_compose_str(&rm_args)?;

                    if pull {
                        println!("Pulling latest images for selected services...");
                        let pull_args =
                            build_docker_compose_args("pull", "", target_services.clone());
                        execute_docker_compose_str(&pull_args)?;
                    }

                    println!("Recreating selected services with latest images...");
                    let mut up_args = vec![
                        "up".to_string(),
                        "-d".to_string(),
                        "--force-recreate".to_string(),
                    ];
                    up_args.extend(target_services);
                    execute_docker_compose_str(&up_args)?;
                }

                println!("Services refreshed successfully");
            }
        },
    }
    Ok(())
}

/// Reload a single service via its HTTP API.
async fn reload_service(service: &str) -> Result<()> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .no_proxy()
        .build()?;

    match service {
        "comsrv" => {
            let url = format!(
                "http://localhost:{}/api/channels/reload",
                voltage_model::service_ports::COMSRV_PORT
            );
            let resp = client.post(&url).send().await?;
            if !resp.status().is_success() {
                return Err(anyhow::anyhow!("HTTP {}", resp.status()));
            }
        },
        "modsrv" => {
            let url = format!(
                "http://localhost:{}/api/instances/reload",
                voltage_model::service_ports::MODSRV_PORT
            );
            let resp = client.post(&url).send().await?;
            if !resp.status().is_success() {
                return Err(anyhow::anyhow!("HTTP {}", resp.status()));
            }
        },
        _ => return Err(anyhow::anyhow!("{} does not support hot reload", service)),
    }
    Ok(())
}

/// Try to reload comsrv + modsrv after sync.
/// Silently skips if services are not running.
pub async fn try_reload_services() {
    let services = ["comsrv", "modsrv"];
    let mut any_reloaded = false;

    for service in &services {
        match reload_service(service).await {
            Ok(()) => {
                println!("  Reloaded {}", service);
                any_reloaded = true;
            },
            Err(_) => {
                // Service not running or unreachable — skip silently
            },
        }
    }

    if !any_reloaded {
        println!("  Services not running. Start with: monarch services start");
    }
}

/// Ensure logs directory exists (handles broken symlinks from external storage)
///
/// When external storage (e.g., SD card) is unavailable, the symlink to the
/// external logs directory is broken. Docker bind mount fails with
/// "mkdir: file exists" because it can't create a directory over a symlink.
/// This creates a local fallback directory so services can start without
/// external storage — file logging resumes automatically once storage is mounted
/// and services are restarted.
fn ensure_logs_dir_exists() {
    let logs_path = std::env::var("VOLTAGE_LOG_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("logs"));

    // Broken symlink: is_symlink() = true but exists() = false (target unreachable)
    if logs_path.is_symlink() && !logs_path.exists() {
        eprintln!(
            "Warning: {} is a broken symlink (external storage unavailable), creating local fallback",
            logs_path.display()
        );
        if let Err(e) = fs::remove_file(&logs_path) {
            eprintln!("Failed to remove broken symlink: {}", e);
            return;
        }
        if let Err(e) = fs::create_dir_all(&logs_path) {
            eprintln!("Failed to create logs directory: {}", e);
        }
    } else if !logs_path.exists() {
        println!("Creating logs directory: {}", logs_path.display());
        if let Err(e) = fs::create_dir_all(&logs_path) {
            eprintln!("Failed to create logs directory: {}", e);
        }
    }
}

/// Ensure shared memory file exists (not directory)
fn ensure_shm_file_exists() {
    let shm_path = Path::new("/dev/shm/voltage-rtdb.shm");

    if shm_path.is_dir() {
        eprintln!(
            "Warning: {} is a directory (Docker artifact), removing...",
            shm_path.display()
        );
        if let Err(e) = fs::remove_dir_all(shm_path) {
            eprintln!("Failed to remove directory: {}", e);
        }
    }

    if !shm_path.exists() {
        println!("Creating shared memory file: {}", shm_path.display());
        if let Err(e) = fs::File::create(shm_path) {
            eprintln!("Failed to create SHM file: {}", e);
        }
    }
}

fn build_docker_compose_args(command: &str, flag: &str, services: Vec<String>) -> Vec<String> {
    let mut args = vec![command.to_string()];
    if !flag.is_empty() {
        args.push(flag.to_string());
    }

    let filtered_services: Vec<String> = services
        .into_iter()
        .filter(|s| !s.eq_ignore_ascii_case("all"))
        .collect();

    args.extend(filtered_services);
    args
}

fn execute_docker_compose(args: &[&str]) -> Result<()> {
    let compose_file = if std::path::Path::new("/opt/MonarchEdge/docker-compose.yml").exists() {
        "/opt/MonarchEdge/docker-compose.yml"
    } else if std::path::Path::new("docker-compose.yml").exists() {
        "docker-compose.yml"
    } else {
        return Err(anyhow::anyhow!(
            "docker-compose.yml not found in /opt/MonarchEdge or current directory"
        ));
    };

    // Detect which Docker Compose version is available
    let use_v2 = Command::new("docker")
        .args(["compose", "version"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    let output = if use_v2 {
        let mut full_args = vec!["compose", "-f", compose_file];
        full_args.extend(args);
        Command::new("docker").args(&full_args).output()?
    } else {
        let mut v1_args = vec!["-f", compose_file];
        v1_args.extend(args);
        Command::new("docker-compose").args(&v1_args).output()?
    };

    if !output.status.success() {
        return Err(anyhow::anyhow!(
            "Docker compose command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    print!("{}", String::from_utf8_lossy(&output.stdout));
    Ok(())
}

fn execute_docker_compose_str(args: &[String]) -> Result<()> {
    let str_args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    execute_docker_compose(&str_args)
}

/// Check if a container's image has changed compared to the local image
fn check_container_image_changed(container_name: &str) -> Result<bool> {
    let running_image_output = Command::new("docker")
        .args(["inspect", container_name, "--format={{.Image}}"])
        .output();

    let running_image_id = match running_image_output {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        },
        _ => {
            return Ok(true); // Container doesn't exist, assume needs update
        },
    };

    // Determine the image name from container name
    let image_name = if container_name == "voltage-redis" {
        "voltage-redis:latest".to_string()
    } else if container_name == "voltage-timescaledb" {
        "timescale/timescaledb:2.25.2-pg17".to_string()
    } else if container_name.starts_with("voltageems-") {
        "voltageems:latest".to_string()
    } else if container_name.starts_with("voltage-") {
        format!("{}:latest", container_name)
    } else {
        return Ok(false); // Unknown container, assume no change
    };

    let local_image_output = Command::new("docker")
        .args(["images", &image_name, "--format={{.ID}}"])
        .output()?;

    if !local_image_output.status.success() {
        return Ok(true); // Image not found locally, assume needs update
    }

    let local_image_id = String::from_utf8_lossy(&local_image_output.stdout)
        .trim()
        .to_string();

    Ok(running_image_id != local_image_id)
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;

    #[test]
    fn test_build_docker_compose_args_basic() {
        let args = build_docker_compose_args("start", "", vec![]);
        assert_eq!(args, vec!["start"]);
    }

    #[test]
    fn test_build_docker_compose_args_with_flag() {
        let args = build_docker_compose_args("rm", "-f", vec![]);
        assert_eq!(args, vec!["rm", "-f"]);
    }

    #[test]
    fn test_build_docker_compose_args_with_services() {
        let args =
            build_docker_compose_args("stop", "", vec!["comsrv".to_string(), "modsrv".to_string()]);
        assert_eq!(args, vec!["stop", "comsrv", "modsrv"]);
    }

    #[test]
    fn test_build_docker_compose_args_filters_all() {
        let args = build_docker_compose_args("stop", "", vec!["all".to_string()]);
        assert_eq!(args, vec!["stop"]);
    }

    #[test]
    fn test_build_docker_compose_args_filters_all_case_insensitive() {
        let args = build_docker_compose_args(
            "stop",
            "",
            vec!["ALL".to_string(), "All".to_string(), "comsrv".to_string()],
        );
        assert_eq!(args, vec!["stop", "comsrv"]);
    }

    #[test]
    fn test_service_commands_start_default() {
        let cmd = ServiceCommands::Start { services: vec![] };
        match cmd {
            ServiceCommands::Start { services } => {
                assert!(services.is_empty());
            },
            _ => panic!("Expected Start command"),
        }
    }

    #[test]
    fn test_service_commands_logs_defaults() {
        let cmd = ServiceCommands::Logs {
            service: "comsrv".to_string(),
            follow: false,
            tail: "100".to_string(),
        };
        match cmd {
            ServiceCommands::Logs {
                service,
                follow,
                tail,
            } => {
                assert_eq!(service, "comsrv");
                assert!(!follow);
                assert_eq!(tail, "100");
            },
            _ => panic!("Expected Logs command"),
        }
    }

    #[test]
    fn test_service_commands_refresh_smart_mode() {
        let cmd = ServiceCommands::Refresh {
            services: vec![],
            pull: true,
            smart: true,
        };
        match cmd {
            ServiceCommands::Refresh {
                services,
                pull,
                smart,
            } => {
                assert!(services.is_empty());
                assert!(pull);
                assert!(smart);
            },
            _ => panic!("Expected Refresh command"),
        }
    }

    #[test]
    fn test_service_commands_clean_with_volumes() {
        let cmd = ServiceCommands::Clean { volumes: true };
        match cmd {
            ServiceCommands::Clean { volumes } => {
                assert!(volumes);
            },
            _ => panic!("Expected Clean command"),
        }
    }
}
