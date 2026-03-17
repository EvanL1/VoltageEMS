//! Rule management module
//!
//! Provides functionality to manage business rules via HTTP API

use anyhow::Result;
use clap::Subcommand;
use reqwest::Client;
use serde_json::Value;
use tracing::info;

#[derive(Subcommand)]
pub enum RuleCommands {
    /// List all rules
    #[command(about = "List all configured business rules")]
    List {
        /// Show only enabled rules
        #[arg(long)]
        enabled: bool,
    },

    /// Get rule details
    #[command(about = "Show detailed information about a rule")]
    Get {
        /// Rule ID
        rule_id: i64,
    },

    /// Enable a rule
    #[command(about = "Enable a business rule")]
    Enable {
        /// Rule ID
        rule_id: i64,
    },

    /// Disable a rule
    #[command(about = "Disable a business rule")]
    Disable {
        /// Rule ID
        rule_id: i64,
    },

    /// Test a rule
    #[command(about = "Test rule conditions without executing actions")]
    Test {
        /// Rule ID
        rule_id: i64,
    },

    /// Execute a rule
    #[command(about = "Execute a rule (evaluate and execute if conditions met)")]
    Execute {
        /// Rule ID
        rule_id: i64,
        /// Force execution even if conditions not met
        #[arg(short, long)]
        force: bool,
    },

    /// Show recent rule executions
    #[command(about = "Display recent rule execution history")]
    Executions {
        /// Rule ID (optional, shows all if not specified)
        rule_id: Option<i64>,
        /// Limit number of results
        #[arg(long, default_value = "10")]
        limit: usize,
    },
}

pub async fn handle_command(cmd: RuleCommands, base_url: &str, json: bool) -> Result<()> {
    let client = RuleClient::new(base_url)?;

    match cmd {
        RuleCommands::List { enabled } => {
            let rules = client.list_rules().await?;

            let rules = if enabled {
                if let serde_json::Value::Array(arr) = rules {
                    let filtered = arr
                        .into_iter()
                        .filter(|r| r.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false))
                        .collect::<Vec<_>>();
                    serde_json::Value::from(filtered)
                } else {
                    rules
                }
            } else {
                rules
            };

            if json {
                crate::output::print_success(&rules);
            } else {
                println!("Rules: {}", serde_json::to_string_pretty(&rules)?);
            }
        },
        RuleCommands::Get { rule_id } => {
            let rule = client.get_rule(rule_id).await?;
            if json {
                crate::output::print_success(&rule);
            } else {
                println!(
                    "Rule '{}': {}",
                    rule_id,
                    serde_json::to_string_pretty(&rule)?
                );
            }
        },
        RuleCommands::Enable { rule_id } => {
            client.enable_rule(rule_id).await?;
            if json {
                crate::output::print_ok();
            } else {
                info!("Rule '{}' enabled", rule_id);
            }
        },
        RuleCommands::Disable { rule_id } => {
            client.disable_rule(rule_id).await?;
            if json {
                crate::output::print_ok();
            } else {
                info!("Rule '{}' disabled", rule_id);
            }
        },
        RuleCommands::Test { rule_id } => {
            let result = client.test_rule(rule_id).await?;
            if json {
                crate::output::print_success(&result);
            } else {
                println!(
                    "Test result for rule '{}': {}",
                    rule_id,
                    serde_json::to_string_pretty(&result)?
                );
            }
        },
        RuleCommands::Execute { rule_id, force } => {
            let result = client.execute_rule(rule_id, force).await?;
            if json {
                crate::output::print_success(&result);
            } else {
                println!(
                    "Execution result for rule '{}': {}",
                    rule_id,
                    serde_json::to_string_pretty(&result)?
                );
            }
        },
        RuleCommands::Executions { rule_id, limit } => {
            let executions = client.list_executions(rule_id, limit).await?;
            if json {
                crate::output::print_success(&executions);
            } else {
                println!("Executions: {}", serde_json::to_string_pretty(&executions)?);
            }
        },
    }

    Ok(())
}

// HTTP client for rule management
struct RuleClient {
    client: Client,
    base_url: String,
}

impl RuleClient {
    fn new(base_url: &str) -> Result<Self> {
        Ok(Self {
            client: Client::new(),
            base_url: base_url.to_string(),
        })
    }

    async fn list_rules(&self) -> Result<Value> {
        let response = self
            .client
            .get(format!("{}/api/rules", self.base_url))
            .send()
            .await?;

        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            Err(anyhow::anyhow!(
                "Failed to list rules: {} - ensure modsrv is running (monarch services start)",
                response.status()
            ))
        }
    }

    async fn get_rule(&self, rule_id: i64) -> Result<Value> {
        let response = self
            .client
            .get(format!("{}/api/rules/{}", self.base_url, rule_id))
            .send()
            .await?;

        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            Err(anyhow::anyhow!("Failed to get rule: {}", response.status()))
        }
    }

    async fn enable_rule(&self, rule_id: i64) -> Result<()> {
        let response = self
            .client
            .post(format!("{}/api/rules/{}/enable", self.base_url, rule_id))
            .send()
            .await?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "Failed to enable rule: {}",
                response.status()
            ))
        }
    }

    async fn disable_rule(&self, rule_id: i64) -> Result<()> {
        let response = self
            .client
            .post(format!("{}/api/rules/{}/disable", self.base_url, rule_id))
            .send()
            .await?;

        if response.status().is_success() {
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "Failed to disable rule: {}",
                response.status()
            ))
        }
    }

    async fn test_rule(&self, rule_id: i64) -> Result<Value> {
        let response = self
            .client
            .post(format!("{}/api/rules/{}/test", self.base_url, rule_id))
            .send()
            .await?;

        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            Err(anyhow::anyhow!(
                "Failed to test rule: {}",
                response.status()
            ))
        }
    }

    #[allow(clippy::disallowed_methods)] // json! macro internally uses unwrap (safe for known valid JSON)
    async fn execute_rule(&self, rule_id: i64, force: bool) -> Result<Value> {
        let response = self
            .client
            .post(format!("{}/api/rules/{}/execute", self.base_url, rule_id))
            .json(&serde_json::json!({ "force": force }))
            .send()
            .await?;

        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            Err(anyhow::anyhow!(
                "Failed to execute rule: {}",
                response.status()
            ))
        }
    }

    async fn list_executions(&self, rule_id: Option<i64>, limit: usize) -> Result<Value> {
        let url = if let Some(id) = rule_id {
            format!(
                "{}/api/rules/{}/executions?limit={}",
                self.base_url, id, limit
            )
        } else {
            format!("{}/api/executions?limit={}", self.base_url, limit)
        };

        let response = self.client.get(url).send().await?;

        if response.status().is_success() {
            Ok(response.json().await?)
        } else {
            Err(anyhow::anyhow!(
                "Failed to list executions: {}",
                response.status()
            ))
        }
    }
}
