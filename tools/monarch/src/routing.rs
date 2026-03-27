//! Routing management module
//!
//! Provides functionality to manage channel-to-instance point routing via HTTP API

use anyhow::Result;
use clap::Subcommand;
use reqwest::Client;
use serde_json::Value;
use tracing::info;

#[derive(Subcommand)]
pub enum RoutingCommands {
    /// List routing configurations
    List {
        /// Filter by instance ID
        #[arg(short = 'i', long)]
        instance: Option<u32>,
        /// Filter by channel ID
        #[arg(long)]
        channel: Option<u32>,
    },

    /// Create a single routing entry for an instance
    Create {
        /// Instance ID
        instance_id: u32,
        /// Point type: M (measurement) or A (action)
        #[arg(short = 't', long = "point-type")]
        point_type: String,
        /// Instance point ID
        #[arg(short = 'p', long = "point-id")]
        point_id: u32,
        /// Channel ID
        #[arg(long = "channel-id")]
        channel_id: u32,
        /// Four-remote type: T (telemetry), S (signal), C (control), A (adjustment)
        #[arg(short = 'r', long = "four-remote")]
        four_remote: String,
        /// Channel point ID
        #[arg(short = 'P', long = "channel-point-id")]
        channel_point_id: u32,
    },

    /// Batch upsert routing from JSON file or stdin
    Batch {
        /// Instance ID
        instance_id: u32,
        /// Path to JSON file with routing entries (use '-' for stdin)
        #[arg(long)]
        file: String,
    },

    /// Delete all routing for an instance
    DeleteInstance {
        /// Instance name
        instance_name: String,
        /// Skip confirmation
        #[arg(short, long)]
        force: bool,
    },

    /// Delete all routing for a channel
    DeleteChannel {
        /// Channel ID
        channel_id: u32,
        /// Skip confirmation
        #[arg(short, long)]
        force: bool,
    },
}

pub async fn handle_command(cmd: RoutingCommands, base_url: &str, json: bool) -> Result<()> {
    let client = RoutingClient::new(base_url)?;

    match cmd {
        RoutingCommands::List { instance, channel } => match (instance, channel) {
            (Some(_), Some(_)) => {
                anyhow::bail!("Use either --instance or --channel, not both");
            },
            (Some(id), None) => {
                let result = client.list_by_instance(id).await?;
                if json {
                    crate::output::print_success(&result);
                } else {
                    println!(
                        "Routing for instance {}: {}",
                        id,
                        serde_json::to_string_pretty(&result)?
                    );
                }
            },
            (None, Some(id)) => {
                let result = client.list_by_channel(id).await?;
                if json {
                    crate::output::print_success(&result);
                } else {
                    println!(
                        "Routing for channel {}: {}",
                        id,
                        serde_json::to_string_pretty(&result)?
                    );
                }
            },
            (None, None) => {
                let result = client.list_all().await?;
                if json {
                    crate::output::print_success(&result);
                } else {
                    println!("Routing: {}", serde_json::to_string_pretty(&result)?);
                }
            },
        },
        RoutingCommands::Create {
            instance_id,
            point_type,
            point_id,
            channel_id,
            four_remote,
            channel_point_id,
        } => {
            let entries = serde_json::json!([{
                "point_type": point_type,
                "point_id": point_id,
                "channel_id": channel_id,
                "four_remote": four_remote,
                "channel_point_id": channel_point_id,
            }]);
            let result = client.create_routing(instance_id, entries).await?;
            if json {
                crate::output::print_success(&result);
            } else {
                info!(
                    "Routing created for instance {}: {}",
                    instance_id,
                    serde_json::to_string_pretty(&result)?
                );
            }
        },
        RoutingCommands::Batch { instance_id, file } => {
            let content = if file == "-" {
                let mut buf = String::new();
                std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf)?;
                buf
            } else {
                std::fs::read_to_string(&file)
                    .map_err(|e| anyhow::anyhow!("Failed to read {}: {}", file, e))?
            };
            let entries: Value = serde_json::from_str(&content)
                .map_err(|e| anyhow::anyhow!("Invalid JSON in routing file: {}", e))?;
            let result = client.batch_routing(instance_id, entries).await?;
            if json {
                crate::output::print_success(&result);
            } else {
                info!("Batch routing upserted for instance {}", instance_id);
                println!("{}", serde_json::to_string_pretty(&result)?);
            }
        },
        RoutingCommands::DeleteInstance {
            instance_name,
            force,
        } => {
            if !force && !json {
                println!("Delete all routing for instance '{}'? [y/N]", instance_name);
                let mut input = String::new();
                std::io::stdin().read_line(&mut input)?;
                if !input.trim().eq_ignore_ascii_case("y") {
                    println!("Cancelled");
                    return Ok(());
                }
            }
            client.delete_instance_routing(&instance_name).await?;
            if json {
                crate::output::print_ok();
            } else {
                info!("Routing deleted for instance '{}'", instance_name);
            }
        },
        RoutingCommands::DeleteChannel { channel_id, force } => {
            if !force && !json {
                println!("Delete all routing for channel {}? [y/N]", channel_id);
                let mut input = String::new();
                std::io::stdin().read_line(&mut input)?;
                if !input.trim().eq_ignore_ascii_case("y") {
                    println!("Cancelled");
                    return Ok(());
                }
            }
            client.delete_channel_routing(channel_id).await?;
            if json {
                crate::output::print_ok();
            } else {
                info!("Routing deleted for channel {}", channel_id);
            }
        },
    }

    Ok(())
}

// HTTP client for routing management
struct RoutingClient {
    client: Client,
    base_url: String,
}

impl RoutingClient {
    fn new(base_url: &str) -> Result<Self> {
        Ok(Self {
            client: Client::new(),
            base_url: base_url.to_string(),
        })
    }

    async fn list_all(&self) -> Result<Value> {
        let response = self
            .client
            .get(format!("{}/api/routing", self.base_url))
            .send()
            .await?;

        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            Err(anyhow::anyhow!(
                "Failed to list routing: {} - ensure modsrv is running (monarch services start)",
                response.status()
            ))
        }
    }

    async fn list_by_instance(&self, id: u32) -> Result<Value> {
        let response = self
            .client
            .get(format!("{}/api/instances/{}/routing", self.base_url, id))
            .send()
            .await?;

        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            Err(anyhow::anyhow!(
                "Failed to list routing for instance {}: {} - ensure modsrv is running (monarch services start)",
                id,
                response.status()
            ))
        }
    }

    async fn list_by_channel(&self, id: u32) -> Result<Value> {
        let response = self
            .client
            .get(format!("{}/api/routing/by-channel/{}", self.base_url, id))
            .send()
            .await?;

        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            Err(anyhow::anyhow!(
                "Failed to list routing for channel {}: {} - ensure modsrv is running (monarch services start)",
                id,
                response.status()
            ))
        }
    }

    async fn create_routing(&self, instance_id: u32, entries: Value) -> Result<Value> {
        let response = self
            .client
            .post(format!(
                "{}/api/instances/{}/routing",
                self.base_url, instance_id
            ))
            .json(&entries)
            .send()
            .await?;

        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            Err(anyhow::anyhow!(
                "Failed to create routing for instance {}: {} - ensure modsrv is running (monarch services start)",
                instance_id,
                response.status()
            ))
        }
    }

    async fn batch_routing(&self, instance_id: u32, entries: Value) -> Result<Value> {
        let response = self
            .client
            .put(format!(
                "{}/api/instances/{}/routing",
                self.base_url, instance_id
            ))
            .json(&entries)
            .send()
            .await?;

        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            Err(anyhow::anyhow!(
                "Failed to batch upsert routing for instance {}: {} - ensure modsrv is running (monarch services start)",
                instance_id,
                response.status()
            ))
        }
    }

    async fn delete_instance_routing(&self, name: &str) -> Result<()> {
        let response = self
            .client
            .delete(format!("{}/api/routing/instances/{}", self.base_url, name))
            .send()
            .await?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "Failed to delete routing for instance '{}': {} - ensure modsrv is running (monarch services start)",
                name,
                response.status()
            ))
        }
    }

    async fn delete_channel_routing(&self, id: u32) -> Result<()> {
        let response = self
            .client
            .delete(format!("{}/api/routing/channels/{}", self.base_url, id))
            .send()
            .await?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "Failed to delete routing for channel {}: {} - ensure modsrv is running (monarch services start)",
                id,
                response.status()
            ))
        }
    }
}
