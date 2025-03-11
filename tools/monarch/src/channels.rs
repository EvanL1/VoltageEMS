//! Channel management module (formerly comsrv-cli)
//!
//! Provides functionality to manage communication channels

use anyhow::Result;
use clap::Subcommand;
use reqwest::Client;
use serde_json::Value;
use tracing::{info, warn};

#[cfg(feature = "lib-mode")]
use crate::{context::ServiceContext, lib_api};

#[derive(Subcommand)]
pub enum ChannelCommands {
    /// List all channels
    #[command(about = "List all configured communication channels")]
    List,

    /// Get channel status
    #[command(about = "Get status of a specific channel")]
    Status {
        /// Channel ID
        channel_id: u16,
    },

    /// Send control command
    #[command(about = "Send control command to a channel")]
    Control {
        /// Channel ID
        channel_id: u16,
        /// Point ID
        point_id: u32,
        /// Value (0 or 1)
        value: u8,
    },

    /// Send adjustment command
    #[command(about = "Send adjustment value to a channel")]
    Adjust {
        /// Channel ID
        channel_id: u16,
        /// Point ID
        point_id: u32,
        /// Value
        value: f64,
    },

    /// Reload channel configuration
    #[command(about = "Reload all channel configurations")]
    Reload,

    /// Check service health
    #[command(about = "Check communication service health")]
    Health,
}

pub async fn handle_command(
    cmd: ChannelCommands,
    service_ctx: Option<&ServiceContext>,
    base_url: Option<&str>,
) -> Result<()> {
    // Determine which mode to use
    #[cfg(feature = "lib-mode")]
    let use_lib_api = service_ctx.is_some();
    #[cfg(not(feature = "lib-mode"))]
    let use_lib_api = false;

    if use_lib_api {
        #[cfg(feature = "lib-mode")]
        {
            // Offline mode: use lib API
            let ctx = service_ctx.expect("ServiceContext should be available in lib-mode");
            let comsrv = ctx.comsrv()?;
            let service = lib_api::channels::ChannelsService::new(comsrv);

            match cmd {
                ChannelCommands::List => {
                    let channels = service.list().await?;
                    println!("Channels: {}", serde_json::to_string_pretty(&channels)?);
                },
                ChannelCommands::Status { channel_id } => {
                    let status = service.get_status(channel_id).await?;
                    println!(
                        "Channel {} status: {}",
                        channel_id,
                        serde_json::to_string_pretty(&status)?
                    );
                },
                ChannelCommands::Control {
                    channel_id,
                    point_id,
                    value,
                } => {
                    service.send_control(channel_id, point_id, value).await?;
                    info!(
                        "Control command sent to channel {} point {}",
                        channel_id, point_id
                    );
                },
                ChannelCommands::Adjust {
                    channel_id,
                    point_id,
                    value,
                } => {
                    service.send_adjustment(channel_id, point_id, value).await?;
                    info!(
                        "Adjustment sent to channel {} point {}: {}",
                        channel_id, point_id, value
                    );
                },
                ChannelCommands::Reload => {
                    let result = service.reload().await?;
                    info!("Configuration reload: {}", result);
                },
                ChannelCommands::Health => {
                    warn!("Health check not available in offline mode (lib API)");
                },
            }
        }
    } else {
        // Online mode: use HTTP API
        let url = base_url.ok_or_else(|| {
            anyhow::anyhow!(
                "Base URL required for online mode. Please set COMSRV_URL or use --offline"
            )
        })?;
        let client = ChannelClient::new(url)?;

        match cmd {
            ChannelCommands::List => {
                let channels = client.list_channels().await?;
                println!("Channels: {}", serde_json::to_string_pretty(&channels)?);
            },
            ChannelCommands::Status { channel_id } => {
                let status = client.get_channel_status(channel_id).await?;
                println!(
                    "Channel {} status: {}",
                    channel_id,
                    serde_json::to_string_pretty(&status)?
                );
            },
            ChannelCommands::Control {
                channel_id,
                point_id,
                value,
            } => {
                client.send_control(channel_id, point_id, value).await?;
                info!(
                    "Control command sent to channel {} point {}",
                    channel_id, point_id
                );
            },
            ChannelCommands::Adjust {
                channel_id,
                point_id,
                value,
            } => {
                client.send_adjustment(channel_id, point_id, value).await?;
                info!(
                    "Adjustment sent to channel {} point {}: {}",
                    channel_id, point_id, value
                );
            },
            ChannelCommands::Reload => {
                client.reload_config().await?;
                info!("Configuration reloaded");
            },
            ChannelCommands::Health => {
                let health = client.check_health().await?;
                println!("Service health: {}", serde_json::to_string_pretty(&health)?);
            },
        }
    }

    Ok(())
}

// HTTP client for channel management
struct ChannelClient {
    client: Client,
    base_url: String,
}

impl ChannelClient {
    fn new(base_url: &str) -> Result<Self> {
        Ok(Self {
            client: Client::new(),
            base_url: base_url.to_string(),
        })
    }

    async fn list_channels(&self) -> Result<Value> {
        let response = self
            .client
            .get(format!("{}/api/channels", self.base_url))
            .send()
            .await?;

        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            Err(anyhow::anyhow!(
                "Failed to get channels: {}",
                response.status()
            ))
        }
    }

    async fn get_channel_status(&self, channel_id: u16) -> Result<Value> {
        let response = self
            .client
            .get(format!(
                "{}/api/channels/{}/status",
                self.base_url, channel_id
            ))
            .send()
            .await?;

        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            Err(anyhow::anyhow!(
                "Failed to get channel status: {}",
                response.status()
            ))
        }
    }

    #[allow(clippy::disallowed_methods)] // json! macro internally uses unwrap (safe for known valid JSON)
    async fn send_control(&self, channel_id: u16, point_id: u32, value: u8) -> Result<()> {
        let response = self
            .client
            .post(format!(
                "{}/api/channels/{}/control",
                self.base_url, channel_id
            ))
            .json(&serde_json::json!({
                "point_id": point_id,
                "value": value
            }))
            .send()
            .await?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "Failed to send control: {}",
                response.status()
            ))
        }
    }

    #[allow(clippy::disallowed_methods)] // json! macro internally uses unwrap (safe for known valid JSON)
    async fn send_adjustment(&self, channel_id: u16, point_id: u32, value: f64) -> Result<()> {
        // Align with service route: /api/channels/{channel_id}/points/{point_id}/adjustment
        let response = self
            .client
            .post(format!(
                "{}/api/channels/{}/points/{}/adjustment",
                self.base_url, channel_id, point_id
            ))
            .json(&serde_json::json!({
                "value": value
            }))
            .send()
            .await?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "Failed to send adjustment: {}",
                response.status()
            ))
        }
    }

    async fn reload_config(&self) -> Result<()> {
        let response = self
            .client
            .post(format!("{}/api/channels/reload", self.base_url))
            .send()
            .await?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "Failed to reload config: {}",
                response.status()
            ))
        }
    }

    async fn check_health(&self) -> Result<Value> {
        let response = self
            .client
            .get(format!("{}/health", self.base_url))
            .send()
            .await?;

        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            Err(anyhow::anyhow!("Service unhealthy: {}", response.status()))
        }
    }
}
