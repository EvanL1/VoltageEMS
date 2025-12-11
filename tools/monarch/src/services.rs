//! Service management module for Docker operations
//!
//! Provides functionality to manage VoltageEMS services

use anyhow::Result;
use clap::Subcommand;
use std::process::Command;
use voltage_rtdb::Rtdb;

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

    /// Execute action with M2C routing (Unified Entry Point)
    #[command(about = "Execute action point with automatic M2C routing")]
    SetAction {
        /// Instance name (e.g., pcs_01, battery_01)
        instance_name: String,
        /// Action point ID
        point_id: String,
        /// Value to set
        value: f64,
        /// Show detailed routing information
        #[arg(short, long)]
        detailed: bool,
    },

    /// Show routing table entries
    #[command(about = "Display routing table (C2M, M2C, C2C) from cache")]
    RoutingShow {
        /// Routing type to display (c2m, m2c, c2c, or all)
        #[arg(short = 't', long, default_value = "all")]
        route_type: String,
        /// Optional prefix filter (e.g., "2:T:", "23:A:")
        #[arg(short, long)]
        prefix: Option<String>,
        /// Show detailed routing information
        #[arg(short, long)]
        detailed: bool,
        /// Limit number of results (0 = no limit)
        #[arg(short, long, default_value = "100")]
        limit: usize,
    },
}

pub async fn handle_command(
    cmd: ServiceCommands,
    service_ctx: Option<&crate::context::ServiceContext>,
) -> Result<()> {
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
                        let client = reqwest::Client::builder().no_proxy().build()?;
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

                // Check if Rust services have changed (voltageems-comsrv, voltageems-modsrv)
                let rust_services_changed = if target_services.is_empty() {
                    check_container_image_changed("voltageems-comsrv")?
                        || check_container_image_changed("voltageems-modsrv")?
                } else {
                    target_services.iter().any(|svc| match svc.as_str() {
                        "comsrv" | "modsrv" => {
                            let container_name = format!("voltageems-{}", svc);
                            check_container_image_changed(&container_name).unwrap_or(false)
                        },
                        _ => false,
                    })
                };

                // Check if Python services have changed
                let python_services_changed = if target_services.is_empty() {
                    check_container_image_changed("voltage-hissrv")?
                        || check_container_image_changed("voltage-apigateway")?
                        || check_container_image_changed("voltage-netsrv")?
                        || check_container_image_changed("voltage-alarmsrv")?
                } else {
                    target_services.iter().any(|svc| match svc.as_str() {
                        "hissrv" | "apigateway" | "netsrv" | "alarmsrv" => {
                            let container_name = format!("voltage-{}", svc);
                            check_container_image_changed(&container_name).unwrap_or(false)
                        },
                        _ => false,
                    })
                };

                // Check if frontend has changed
                let frontend_changed =
                    if target_services.is_empty() || target_services.iter().any(|s| s == "apps") {
                        check_container_image_changed("voltage-apps")?
                    } else {
                        false
                    };

                // Check if InfluxDB has changed
                let infra_changed = if target_services.is_empty()
                    || target_services.iter().any(|s| s == "influxdb")
                {
                    check_container_image_changed("voltage-influxdb")?
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

                // Handle Rust services (comsrv, modsrv)
                if rust_services_changed {
                    println!("\nRecreating Rust services...");
                    execute_docker_compose(&["up", "-d", "--force-recreate", "comsrv", "modsrv"])?;
                    println!("✓ Rust services recreated");
                } else {
                    println!("✓ Rust services unchanged (no recreation needed)");
                }

                // Handle Python services (hissrv, apigateway, netsrv, alarmsrv)
                if python_services_changed {
                    println!("\nRecreating Python services...");
                    execute_docker_compose(&[
                        "up",
                        "-d",
                        "--force-recreate",
                        "hissrv",
                        "apigateway",
                        "netsrv",
                        "alarmsrv",
                    ])?;
                    println!("✓ Python services recreated");
                } else {
                    println!("✓ Python services unchanged (no recreation needed)");
                }

                // Handle frontend (apps)
                if frontend_changed {
                    println!("\nRecreating frontend...");
                    execute_docker_compose(&["up", "-d", "--force-recreate", "apps"])?;
                    println!("✓ Frontend recreated");
                } else {
                    println!("✓ Frontend unchanged (no recreation needed)");
                }

                // Handle InfluxDB
                if infra_changed {
                    println!("\n⚠️  InfluxDB image has changed");
                    println!("   Recreating InfluxDB may affect historical data queries");
                    println!("\nRecreate InfluxDB container? (yes/NO): ");

                    use std::io::{stdin, stdout, Write};
                    let mut input = String::new();
                    stdout().flush()?;
                    stdin().read_line(&mut input)?;

                    if input.trim() == "yes" {
                        println!("Recreating InfluxDB...");
                        execute_docker_compose(&["up", "-d", "--force-recreate", "influxdb"])?;
                        println!("✓ InfluxDB recreated");
                    } else {
                        println!("Skipped InfluxDB recreation");
                    }
                } else {
                    println!("✓ InfluxDB unchanged (no recreation needed)");
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
        ServiceCommands::SetAction {
            instance_name,
            point_id,
            value,
            detailed,
        } => {
            // Direct library call to execute action with M2C routing
            println!("Executing action point with automatic M2C routing...");
            println!("  Instance: {}", instance_name);
            println!("  Point ID: {}", point_id);
            println!("  Value: {}", value);

            // Get ModsrvContext which has RoutingCache and Rtdb
            let ctx = service_ctx
                .ok_or_else(|| anyhow::anyhow!("Service context required for set-action command"))?
                .modsrv()?;

            // Resolve instance name to ID
            let instance_id_bytes = ctx
                .rtdb
                .hash_get("inst:name:index", &instance_name)
                .await?
                .ok_or_else(|| anyhow::anyhow!("Instance '{}' not found", instance_name))?;
            let instance_id = String::from_utf8_lossy(&instance_id_bytes)
                .parse::<u32>()
                .map_err(|e| anyhow::anyhow!("Invalid instance ID: {}", e))?;

            // Execute action routing using shared library (direct call)
            let outcome = voltage_routing::set_action_point(
                ctx.rtdb.as_ref(),
                ctx.instance_manager.routing_cache(),
                instance_id,
                &point_id,
                value,
            )
            .await?;

            // Display results
            if outcome.routed {
                println!("\n✓ Action executed and routed successfully");
                println!(
                    "  Route result: {}",
                    outcome.route_result.unwrap_or_else(|| "N/A".to_string())
                );

                if detailed {
                    if let Some(ctx) = outcome.route_context {
                        println!("\nRouting Context:");
                        println!("  Channel ID: {}", ctx.channel_id);
                        println!("  Point Type: {}", ctx.point_type);
                        println!("  ComSrv Point ID: {}", ctx.comsrv_point_id);
                        println!("  Queue Key: {}", ctx.queue_key);
                    }
                }
            } else {
                println!("\n⚠ Action executed but no routing found");
                println!("  (This instance/point may not be mapped to any channel)");
            }
        },
        ServiceCommands::RoutingShow {
            route_type,
            prefix,
            detailed,
            limit,
        } => {
            // Get ModsrvContext which has RoutingCache
            let ctx = service_ctx
                .ok_or_else(|| {
                    anyhow::anyhow!("Service context required for routing-show command")
                })?
                .modsrv()?;

            let routing_cache = ctx.instance_manager.routing_cache();
            let stats = routing_cache.stats();

            // Determine prefix filter (empty string means no filter)
            let filter_prefix = prefix.as_deref().unwrap_or("");

            // Collect routes based on type
            // Note: Arc<str> converted to String for display (CLI is not a hot path)
            let route_type_lower = route_type.to_lowercase();
            let mut all_routes: Vec<(String, String, &str)> = Vec::new();

            match route_type_lower.as_str() {
                "c2m" => {
                    let routes = routing_cache.get_c2m_by_prefix(filter_prefix);
                    all_routes.extend(routes.into_iter().map(|(k, v)| (k, v.to_string(), "C2M")));
                },
                "m2c" => {
                    let routes = routing_cache.get_m2c_by_prefix(filter_prefix);
                    all_routes.extend(routes.into_iter().map(|(k, v)| (k, v.to_string(), "M2C")));
                },
                "c2c" => {
                    let routes = routing_cache.get_c2c_by_prefix(filter_prefix);
                    all_routes.extend(routes.into_iter().map(|(k, v)| (k, v.to_string(), "C2C")));
                },
                "all" => {
                    let c2m = routing_cache.get_c2m_by_prefix(filter_prefix);
                    let m2c = routing_cache.get_m2c_by_prefix(filter_prefix);
                    let c2c = routing_cache.get_c2c_by_prefix(filter_prefix);
                    all_routes.extend(c2m.into_iter().map(|(k, v)| (k, v.to_string(), "C2M")));
                    all_routes.extend(m2c.into_iter().map(|(k, v)| (k, v.to_string(), "M2C")));
                    all_routes.extend(c2c.into_iter().map(|(k, v)| (k, v.to_string(), "C2C")));
                },
                _ => {
                    return Err(anyhow::anyhow!(
                        "Invalid route type '{}'. Must be one of: c2m, m2c, c2c, all",
                        route_type
                    ));
                },
            }

            // Apply limit
            let total_count = all_routes.len();
            let limited_routes: Vec<_> = if limit > 0 {
                all_routes.into_iter().take(limit).collect()
            } else {
                all_routes
            };

            // Print summary
            println!("=== Routing Cache Summary ===\n");
            println!("C2M Routes (Channel → Model): {} entries", stats.c2m_count);
            println!("M2C Routes (Model → Channel): {} entries", stats.m2c_count);
            println!(
                "C2C Routes (Channel → Channel): {} entries",
                stats.c2c_count
            );
            let total_routes = stats.c2m_count + stats.m2c_count + stats.c2c_count;
            println!("Total: {} routes\n", total_routes);

            if prefix.is_some() {
                println!("Filter: Prefix = '{}'\n", filter_prefix);
            }

            // Print detailed breakdown if requested
            if detailed {
                println!("--- Routing Cache Details ---");
                println!("Total Capacity: {} entries", total_routes);
                if total_count > limited_routes.len() {
                    println!(
                        "Showing: {} of {} matching routes (limited by --limit {})",
                        limited_routes.len(),
                        total_count,
                        limit
                    );
                } else {
                    println!("Showing: {} matching routes", limited_routes.len());
                }
                println!();
            }

            // Print routing table
            if limited_routes.is_empty() {
                println!("⚠ No routing entries found");
                if prefix.is_some() {
                    println!("  (Try removing the --prefix filter or using a different prefix)");
                }
            } else {
                println!("=== Routing Table ===");
                println!("{:<8} {:<30} → {:<30}", "Type", "Source", "Target");
                println!("{}", "─".repeat(72));

                for (source, target, route_type) in limited_routes {
                    println!("{:<8} {:<30} → {:<30}", route_type, source, target);
                }

                if total_count > limit && limit > 0 {
                    println!();
                    println!(
                        "... and {} more entries (use --limit 0 to show all)",
                        total_count - limit
                    );
                }
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

    // Detect which Docker Compose version is available
    let use_v2 = Command::new("docker")
        .args(["compose", "version"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    let output = if use_v2 {
        // Use Docker Compose V2 (docker compose)
        let mut full_args = vec!["compose", "-f", compose_file];
        full_args.extend(args);
        Command::new("docker").args(&full_args).output()?
    } else {
        // Fall back to Docker Compose V1 (docker-compose)
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
    // Map container names to their corresponding image names
    let image_name = if container_name == "voltage-redis" {
        "voltage-redis:latest".to_string()
    } else if container_name == "voltage-influxdb" {
        "influxdb:2-alpine".to_string()
    } else if container_name.starts_with("voltageems-") {
        // Rust services: voltageems-comsrv, voltageems-modsrv
        "voltageems:latest".to_string()
    } else if container_name.starts_with("voltage-") {
        // Python services and frontend: voltage-hissrv, voltage-apigateway,
        // voltage-netsrv, voltage-alarmsrv, voltage-apps
        format!("{}:latest", container_name)
    } else {
        return Ok(false); // Unknown container, assume no change
    };

    // Get the image ID of the local image
    let local_image_output = Command::new("docker")
        .args(["images", &image_name, "--format={{.ID}}"])
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
