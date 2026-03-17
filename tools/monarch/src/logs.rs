//! Log management commands for Monarch CLI
//!
//! Provides commands for dynamically adjusting log levels in running services.

use anyhow::{Context, Result};
use clap::Subcommand;
use colored::*;
use serde::{Deserialize, Serialize};

/// Log management commands
#[derive(Subcommand, Debug)]
pub enum LogCommands {
    /// Set log level for a service
    #[command(about = "Set log level for a service (debug, info, warn, error, trace)")]
    Level {
        /// Service name (comsrv, modsrv, all)
        service: String,

        /// Log level (trace, debug, info, warn, error)
        /// or full filter spec (e.g., "info,comsrv=debug")
        level: String,
    },

    /// Get current log level for a service
    #[command(about = "Get current log level for a service")]
    Get {
        /// Service name (comsrv, modsrv, all)
        service: String,
    },
}

/// Response from log level API
#[derive(Debug, Serialize, Deserialize)]
struct LogLevelResponse {
    level: String,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

/// Request to set log level
#[derive(Debug, Serialize)]
struct SetLogLevelRequest {
    level: String,
}

/// Get service port by name
fn get_service_port(service: &str) -> Result<u16> {
    voltage_model::service_ports::default_port_for(&service.to_lowercase()).ok_or_else(|| {
        anyhow::anyhow!(
            "Unknown service: {}. Use 'comsrv', 'modsrv', or 'all'",
            service
        )
    })
}

/// Set log level for a service
async fn set_log_level(service: &str, level: &str, host: Option<&str>) -> Result<()> {
    let port = get_service_port(service)?;
    let addr = host.unwrap_or("127.0.0.1");
    let url = format!("http://{addr}:{port}/api/admin/logs/level");

    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .json(&SetLogLevelRequest {
            level: level.to_string(),
        })
        .send()
        .await
        .with_context(|| format!("Failed to connect to {} at port {}", service, port))?;

    if resp.status().is_success() {
        let body: LogLevelResponse = resp.json().await?;
        println!(
            "  {} {} → {}",
            "✓".green(),
            service.bright_cyan(),
            body.level.bright_yellow()
        );
        Ok(())
    } else {
        let body: LogLevelResponse = resp.json().await?;
        let error_msg = body.error.unwrap_or_else(|| "Unknown error".to_string());
        anyhow::bail!("{}: {}", service, error_msg)
    }
}

/// Get log level for a service
async fn get_log_level(service: &str, host: Option<&str>) -> Result<String> {
    let port = get_service_port(service)?;
    let addr = host.unwrap_or("127.0.0.1");
    let url = format!("http://{addr}:{port}/api/admin/logs/level");

    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .send()
        .await
        .with_context(|| format!("Failed to connect to {} at port {}", service, port))?;

    if resp.status().is_success() {
        let body: LogLevelResponse = resp.json().await?;
        Ok(body.level)
    } else {
        anyhow::bail!("Failed to get log level from {}", service)
    }
}

/// Handle log commands
pub async fn handle_command(command: LogCommands, json: bool, host: Option<&str>) -> Result<()> {
    match command {
        LogCommands::Level { service, level } => {
            if !json {
                println!("{}", "Setting log level...".bright_cyan());
            }

            if service.to_lowercase() == "all" {
                let services = ["comsrv", "modsrv"];
                let mut errors = Vec::new();

                for svc in services {
                    if let Err(e) = set_log_level(svc, &level, host).await {
                        errors.push(format!("{}: {}", svc, e));
                    }
                }

                if !errors.is_empty() {
                    if !json {
                        println!();
                        for err in &errors {
                            println!("  {} {}", "✗".red(), err);
                        }
                    }
                    if errors.len() == services.len() {
                        anyhow::bail!("Failed to set log level for all services");
                    }
                }
            } else {
                set_log_level(&service, &level, host).await?;
            }

            if json {
                crate::output::print_success(serde_json::json!({
                    "service": service,
                    "level": level,
                }));
            } else {
                println!();
                println!("{}", "Log level updated successfully!".green());
            }
        },

        LogCommands::Get { service } => {
            let mut results = Vec::new();

            let services: Vec<&str> = if service.to_lowercase() == "all" {
                vec!["comsrv", "modsrv"]
            } else {
                vec![service.as_str()]
            };

            if !json {
                println!("{}", "Current log levels:".bright_cyan());
            }

            for svc in &services {
                match get_log_level(svc, host).await {
                    Ok(level) => {
                        results.push(serde_json::json!({"service": svc, "level": level}));
                        if !json {
                            println!("  {} {}", svc.bright_cyan(), level.bright_yellow());
                        }
                    },
                    Err(e) => {
                        results.push(serde_json::json!({
                            "service": svc, "level": null, "error": e.to_string()
                        }));
                        if !json {
                            println!("  {} {} ({})", svc.bright_cyan(), "unavailable".red(), e);
                        }
                    },
                }
            }

            if json {
                crate::output::print_success(&results);
            }
        },
    }

    Ok(())
}
