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
    match service.to_lowercase().as_str() {
        "comsrv" => Ok(6001),
        "modsrv" => Ok(6002),
        _ => anyhow::bail!(
            "Unknown service: {}. Use 'comsrv', 'modsrv', or 'all'",
            service
        ),
    }
}

/// Set log level for a service
async fn set_log_level(service: &str, level: &str) -> Result<()> {
    let port = get_service_port(service)?;
    let url = format!("http://127.0.0.1:{}/api/admin/logs/level", port);

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
async fn get_log_level(service: &str) -> Result<String> {
    let port = get_service_port(service)?;
    let url = format!("http://127.0.0.1:{}/api/admin/logs/level", port);

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
pub async fn handle_command(command: LogCommands) -> Result<()> {
    match command {
        LogCommands::Level { service, level } => {
            println!("{}", "Setting log level...".bright_cyan());

            if service.to_lowercase() == "all" {
                // Set for all services
                let services = ["comsrv", "modsrv"];
                let mut errors = Vec::new();

                for svc in services {
                    if let Err(e) = set_log_level(svc, &level).await {
                        errors.push(format!("{}: {}", svc, e));
                    }
                }

                if !errors.is_empty() {
                    println!();
                    for err in &errors {
                        println!("  {} {}", "✗".red(), err);
                    }
                    if errors.len() == services.len() {
                        anyhow::bail!("Failed to set log level for all services");
                    }
                }
            } else {
                set_log_level(&service, &level).await?;
            }

            println!();
            println!("{}", "Log level updated successfully!".green());
        },

        LogCommands::Get { service } => {
            println!("{}", "Current log levels:".bright_cyan());

            if service.to_lowercase() == "all" {
                let services = ["comsrv", "modsrv"];
                for svc in services {
                    match get_log_level(svc).await {
                        Ok(level) => {
                            println!("  {} {}", svc.bright_cyan(), level.bright_yellow());
                        },
                        Err(e) => {
                            println!("  {} {} ({})", svc.bright_cyan(), "unavailable".red(), e);
                        },
                    }
                }
            } else {
                match get_log_level(&service).await {
                    Ok(level) => {
                        println!("  {} {}", service.bright_cyan(), level.bright_yellow());
                    },
                    Err(e) => {
                        println!(
                            "  {} {} ({})",
                            service.bright_cyan(),
                            "unavailable".red(),
                            e
                        );
                    },
                }
            }
        },
    }

    Ok(())
}
