//! Channel management module
//!
//! Provides functionality to manage communication channels via HTTP API

use anyhow::Result;
use clap::Subcommand;
use reqwest::Client;
use serde_json::Value;

#[derive(Subcommand)]
pub enum ChannelCommands {
    /// List all channels
    #[command(about = "List all configured communication channels")]
    List,

    /// Get channel status
    #[command(about = "Get status of a specific channel")]
    Status {
        /// Channel ID
        channel_id: u32,
    },

    /// Send control command
    #[command(about = "Send control command to a channel")]
    Control {
        /// Channel ID
        channel_id: u32,
        /// Point ID
        point_id: u32,
        /// Value (0 or 1)
        value: u8,
    },

    /// Send adjustment command
    #[command(about = "Send adjustment value to a channel")]
    Adjust {
        /// Channel ID
        channel_id: u32,
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

    /// Create a new channel
    #[command(about = "Create a new communication channel")]
    Create {
        /// Channel name (must be unique)
        #[arg(long)]
        name: String,
        /// Protocol type (modbus_tcp, modbus_rtu, virtual, di_do, can)
        #[arg(long)]
        protocol: String,
        /// Protocol parameters as JSON string (e.g. '{"host":"192.168.1.10","port":502}')
        #[arg(long)]
        params: String,
        /// Channel description
        #[arg(long)]
        description: Option<String>,
        /// Start channel immediately (default: true)
        #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
        enabled: bool,
        /// Override channel ID (auto-assigned if omitted)
        #[arg(long)]
        id: Option<u32>,
    },

    /// Update channel configuration
    #[command(about = "Update an existing channel's configuration")]
    Update {
        /// Channel ID to update
        channel_id: u32,
        /// New channel name
        #[arg(long)]
        name: Option<String>,
        /// Updated protocol parameters as JSON string
        #[arg(long)]
        params: Option<String>,
        /// Updated description
        #[arg(long)]
        description: Option<String>,
    },

    /// Delete a channel (cascades: points, mappings, routing)
    #[command(about = "Delete a channel and cascade-remove its points, mappings, and routing")]
    Delete {
        /// Channel ID to delete
        channel_id: u32,
        /// Skip confirmation prompt
        #[arg(short, long)]
        force: bool,
    },
}

pub async fn handle_command(cmd: ChannelCommands, base_url: &str, json: bool) -> Result<()> {
    let client = ChannelClient::new(base_url)?;

    match cmd {
        ChannelCommands::List => {
            let channels = client.list_channels().await?;
            if json {
                crate::output::print_success(&channels);
            } else {
                println!("Channels: {}", serde_json::to_string_pretty(&channels)?);
            }
        },
        ChannelCommands::Status { channel_id } => {
            let status = client.get_channel_status(channel_id).await?;
            if json {
                crate::output::print_success(&status);
            } else {
                println!(
                    "Channel {} status: {}",
                    channel_id,
                    serde_json::to_string_pretty(&status)?
                );
            }
        },
        ChannelCommands::Control {
            channel_id,
            point_id,
            value,
        } => {
            client.send_control(channel_id, point_id, value).await?;
            if json {
                crate::output::print_ok();
            } else {
                println!(
                    "Control command sent to channel {} point {}",
                    channel_id, point_id
                );
            }
        },
        ChannelCommands::Adjust {
            channel_id,
            point_id,
            value,
        } => {
            client.send_adjustment(channel_id, point_id, value).await?;
            if json {
                crate::output::print_ok();
            } else {
                println!(
                    "Adjustment sent to channel {} point {}: {}",
                    channel_id, point_id, value
                );
            }
        },
        ChannelCommands::Reload => {
            client.reload_config().await?;
            if json {
                crate::output::print_ok();
            } else {
                println!("Configuration reloaded");
            }
        },
        ChannelCommands::Health => {
            let health = client.check_health().await?;
            if json {
                crate::output::print_success(&health);
            } else {
                println!("Service health: {}", serde_json::to_string_pretty(&health)?);
            }
        },
        ChannelCommands::Create {
            name,
            protocol,
            params,
            description,
            enabled,
            id,
        } => {
            let parameters: Value = serde_json::from_str(&params)
                .map_err(|e| anyhow::anyhow!("--params must be valid JSON: {}", e))?;
            let result = client
                .create_channel(
                    &name,
                    &protocol,
                    parameters,
                    description.as_deref(),
                    id,
                    enabled,
                )
                .await?;
            if json {
                crate::output::print_success(&result);
            } else {
                println!(
                    "Channel created: {}",
                    result
                        .get("data")
                        .and_then(|d| d.get("channel_id"))
                        .map(|v| v.to_string())
                        .unwrap_or_default()
                );
            }
        },
        ChannelCommands::Update {
            channel_id,
            name,
            params,
            description,
        } => {
            let mut body = serde_json::Map::new();
            if let Some(n) = name {
                body.insert("name".to_string(), Value::String(n));
            }
            if let Some(p) = params {
                let parameters: Value = serde_json::from_str(&p)
                    .map_err(|e| anyhow::anyhow!("--params must be valid JSON: {}", e))?;
                body.insert("parameters".to_string(), parameters);
            }
            if let Some(d) = description {
                body.insert("description".to_string(), Value::String(d));
            }
            let result = client
                .update_channel(channel_id, Value::Object(body))
                .await?;
            if json {
                crate::output::print_success(&result);
            } else {
                println!("Channel {} updated", channel_id);
            }
        },
        ChannelCommands::Delete { channel_id, force } => {
            if !force && !json {
                println!("Delete channel {}? [y/N]", channel_id);
                let mut input = String::new();
                std::io::stdin().read_line(&mut input)?;
                if !input.trim().eq_ignore_ascii_case("y") {
                    println!("Cancelled");
                    return Ok(());
                }
            }
            client.delete_channel(channel_id).await?;
            if json {
                crate::output::print_ok();
            } else {
                println!("Channel {} deleted", channel_id);
            }
        },
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
                "Failed to get channels: {} - ensure comsrv is running (monarch services start)",
                response.status()
            ))
        }
    }

    async fn get_channel_status(&self, channel_id: u32) -> Result<Value> {
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
    async fn send_control(&self, channel_id: u32, point_id: u32, value: u8) -> Result<()> {
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
    async fn send_adjustment(&self, channel_id: u32, point_id: u32, value: f64) -> Result<()> {
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

    #[allow(clippy::disallowed_methods)] // json! macro internally uses unwrap (safe for known valid JSON)
    async fn create_channel(
        &self,
        name: &str,
        protocol: &str,
        parameters: Value,
        description: Option<&str>,
        id: Option<u32>,
        enabled: bool,
    ) -> Result<Value> {
        let mut body = serde_json::json!({
            "name": name,
            "protocol": protocol,
            "parameters": parameters,
            "enabled": enabled,
        });
        if let Some(desc) = description {
            body["description"] = Value::String(desc.to_string());
        }
        if let Some(channel_id) = id {
            body["channel_id"] = Value::Number(channel_id.into());
        }
        let response = self
            .client
            .post(format!("{}/api/channels", self.base_url))
            .json(&body)
            .send()
            .await?;

        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            Err(anyhow::anyhow!(
                "Failed to create channel: {} - {}",
                status,
                text
            ))
        }
    }

    async fn update_channel(&self, channel_id: u32, body: Value) -> Result<Value> {
        let response = self
            .client
            .put(format!("{}/api/channels/{}", self.base_url, channel_id))
            .json(&body)
            .send()
            .await?;

        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            Err(anyhow::anyhow!(
                "Failed to update channel {}: {} - {}",
                channel_id,
                status,
                text
            ))
        }
    }

    async fn delete_channel(&self, channel_id: u32) -> Result<()> {
        let response = self
            .client
            .delete(format!("{}/api/channels/{}", self.base_url, channel_id))
            .send()
            .await?;

        if response.status().is_success() {
            Ok(())
        } else {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            Err(anyhow::anyhow!(
                "Failed to delete channel {}: {} - {}",
                channel_id,
                status,
                text
            ))
        }
    }
}
