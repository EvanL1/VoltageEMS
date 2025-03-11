//! Service management module for Docker operations
//!
//! Provides functionality to manage VoltageEMS services

use anyhow::Result;
use clap::Subcommand;
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
            // Use --no-recreate to avoid rebuilding containers that already exist
            // This prevents unnecessary Redis restarts when images haven't changed
            let mut args = vec![
                "up".to_string(),
                "-d".to_string(),
                "--no-recreate".to_string(),
            ];

            // Filter out "all" keyword and add specific service names
            let filtered_services: Vec<String> = services
                .into_iter()
                .filter(|s| s.to_lowercase() != "all")
                .collect();

            args.extend(filtered_services);
            execute_docker_compose_str(&args)?;
            println!("Services started (using --no-recreate to preserve existing containers)");
        },
        ServiceCommands::Stop { services } => {
            let args = build_docker_compose_args("stop", "", services);
            execute_docker_compose_str(&args)?;
            println!("Services stopped");
        },
        ServiceCommands::Restart { services } => {
            let args = build_docker_compose_args("restart", "", services);
            execute_docker_compose_str(&args)?;
            println!("Services restarted");
        },
        ServiceCommands::Status { services } => {
            // Filter out "all" keyword
            let filtered_services: Vec<&String> = services
                .iter()
                .filter(|s| s.to_lowercase() != "all")
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
            // Define all services that support hot reload
            let hot_reload_services = vec!["comsrv"];

            // Determine which services to reload
            let services_to_reload =
                if services.is_empty() || services.iter().any(|s| s.to_lowercase() == "all") {
                    // Reload all hot-reload capable services
                    hot_reload_services.clone()
                } else {
                    // Filter out "all" and use the provided services
                    services
                        .iter()
                        .filter(|s| s.to_lowercase() != "all")
                        .map(|s| s.as_str())
                        .collect()
                };

            // For services that support configuration reload via API
            for service in services_to_reload {
                match service {
                    "comsrv" => {
                        // Use HTTP client to reload comsrv configuration
                        let client = reqwest::Client::new();
                        let response = client
                            .post("http://localhost:6001/api/channels/reload")
                            .send()
                            .await?;

                        if response.status().is_success() {
                            println!("Reloaded comsrv configuration");
                        } else {
                            return Err(anyhow::anyhow!(
                                "Failed to reload comsrv config: {}",
                                response.status()
                            ));
                        }
                    },
                    _ => {
                        println!(
                            "Service {} doesn't support hot reload, restart required",
                            service
                        );
                    },
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
            if smart {
                // Smart mode: Only recreate containers if images actually changed
                println!("Refreshing services with smart mode...");
                println!("Detecting image changes...");

                let target_services: Vec<String> = services
                    .iter()
                    .filter(|s| s.to_lowercase() != "all")
                    .cloned()
                    .collect();

                // Check if voltage-redis image has changed
                let redis_changed = check_container_image_changed("voltage-redis")?;

                // Check if voltageems services have changed
                let voltageems_changed = if target_services.is_empty() {
                    // Check any voltageems container
                    check_container_image_changed("voltageems-comsrv")?
                        || check_container_image_changed("voltageems-modsrv")?
                        || check_container_image_changed("voltageems-rulesrv")?
                } else {
                    // Check only specified services
                    target_services.iter().any(|svc| {
                        let container_name = format!("voltageems-{}", svc);
                        check_container_image_changed(&container_name).unwrap_or(false)
                    })
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

                    use std::io::{stdin, stdout, Write};
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

                // Handle VoltageEMS services
                if voltageems_changed {
                    println!("\nRecreating VoltageEMS services...");
                    if target_services.is_empty() {
                        execute_docker_compose(&[
                            "up",
                            "-d",
                            "--force-recreate",
                            "comsrv",
                            "modsrv",
                            "rulesrv",
                        ])?;
                    } else {
                        let mut up_args = vec![
                            "up".to_string(),
                            "-d".to_string(),
                            "--force-recreate".to_string(),
                        ];
                        up_args.extend(target_services);
                        execute_docker_compose_str(&up_args)?;
                    }
                    println!("✓ VoltageEMS services recreated");
                } else {
                    println!("✓ VoltageEMS services unchanged (no recreation needed)");
                }

                println!("\nSmart refresh completed successfully");
            } else {
                // Legacy mode: Force recreate all (original behavior)
                println!("Refreshing services with latest images (force mode)...");

                let target_services: Vec<String> = services
                    .iter()
                    .filter(|s| s.to_lowercase() != "all")
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

fn build_docker_compose_args(command: &str, flag: &str, services: Vec<String>) -> Vec<String> {
    let mut args = vec![command.to_string()];
    if !flag.is_empty() {
        args.push(flag.to_string());
    }

    // Filter out "all" keyword - when services list is empty or contains "all",
    // docker-compose will operate on all services by default
    let filtered_services: Vec<String> = services
        .into_iter()
        .filter(|s| s.to_lowercase() != "all")
        .collect();

    args.extend(filtered_services);
    args
}

fn execute_docker_compose(args: &[&str]) -> Result<()> {
    // Determine the docker-compose.yml location
    let compose_file = if std::path::Path::new("/opt/MonarchEdge/docker-compose.yml").exists() {
        "/opt/MonarchEdge/docker-compose.yml"
    } else if std::path::Path::new("docker-compose.yml").exists() {
        "docker-compose.yml"
    } else {
        return Err(anyhow::anyhow!(
            "docker-compose.yml not found in /opt/MonarchEdge or current directory"
        ));
    };

    // Build full args with -f flag
    let mut full_args = vec!["compose", "-f", compose_file];
    full_args.extend(args);

    // Try docker compose (v2) first, then fall back to docker-compose (v1)
    let output = Command::new("docker")
        .args(&full_args)
        .output()
        .or_else(|_| {
            // For docker-compose v1, adjust args format
            let mut v1_args = vec!["-f", compose_file];
            v1_args.extend(args);
            Command::new("docker-compose").args(&v1_args).output()
        })?;

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
/// Returns true if container doesn't exist OR image has changed
fn check_container_image_changed(container_name: &str) -> Result<bool> {
    // Get the image ID currently used by the running container
    let running_image_output = Command::new("docker")
        .args(["inspect", container_name, "--format={{.Image}}"])
        .output();

    let running_image_id = match running_image_output {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        },
        _ => {
            // Container doesn't exist or error occurred
            return Ok(true); // Assume needs update
        },
    };

    // Determine the image name from container name
    let image_name = if container_name == "voltage-redis" {
        "voltage-redis:latest"
    } else if container_name.starts_with("voltageems-") {
        "voltageems:latest"
    } else {
        return Ok(false); // Unknown container, assume no change
    };

    // Get the image ID of the local image
    let local_image_output = Command::new("docker")
        .args(["images", image_name, "--format={{.ID}}"])
        .output()?;

    if !local_image_output.status.success() {
        return Ok(true); // Image not found locally, assume needs update
    }

    let local_image_id = String::from_utf8_lossy(&local_image_output.stdout)
        .trim()
        .to_string();

    // Compare image IDs
    Ok(running_image_id != local_image_id)
}
