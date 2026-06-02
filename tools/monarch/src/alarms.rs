//! Alarm management module
//!
//! Provides read-only access to alarmsrv: active alerts, alarm rules,
//! historical alert events, statistics, and monitor status.

use anyhow::Result;
use clap::Subcommand;
use reqwest::Client;
use serde_json::Value;

#[derive(Subcommand)]
pub enum AlarmCommands {
    /// List active alerts
    #[command(about = "List currently active alerts")]
    List {
        /// Filter by channel ID
        #[arg(long)]
        channel: Option<i64>,
        /// Filter by warning level (1=low, 2=medium, 3=high)
        #[arg(long)]
        level: Option<i64>,
        /// Keyword search (rule name, channel, point)
        #[arg(long)]
        keyword: Option<String>,
        /// Page number (1-based)
        #[arg(long, default_value = "1")]
        page: i64,
        /// Page size
        #[arg(long, default_value = "50")]
        size: i64,
    },

    /// Get a single active alert by ID
    #[command(about = "Get details of a specific active alert")]
    Get {
        /// Alert ID
        id: i64,
    },

    /// List alarm rules
    #[command(about = "List alarm rules")]
    Rules {
        /// Filter by channel ID
        #[arg(long)]
        channel: Option<i64>,
        /// Show only enabled rules
        #[arg(long)]
        enabled: bool,
        /// Filter by warning level (1=low, 2=medium, 3=high)
        #[arg(long)]
        level: Option<i64>,
        /// Keyword search
        #[arg(long)]
        keyword: Option<String>,
        /// Page number (1-based)
        #[arg(long, default_value = "1")]
        page: i64,
        /// Page size
        #[arg(long, default_value = "50")]
        size: i64,
    },

    /// Get a single alarm rule by ID
    #[command(about = "Get details of a specific alarm rule")]
    RuleGet {
        /// Rule ID
        id: i64,
    },

    /// List historical alert events (trigger + recovery)
    #[command(about = "List historical alert events")]
    Events {
        /// Filter by rule ID
        #[arg(long)]
        rule: Option<i64>,
        /// Filter by event type: trigger or recovery
        #[arg(long)]
        event_type: Option<String>,
        /// Filter by warning level (1=low, 2=medium, 3=high)
        #[arg(long)]
        level: Option<i64>,
        /// Keyword search
        #[arg(long)]
        keyword: Option<String>,
        /// Page number (1-based)
        #[arg(long, default_value = "1")]
        page: i64,
        /// Page size
        #[arg(long, default_value = "50")]
        size: i64,
    },

    /// Show alert statistics
    #[command(about = "Show alert count and rule statistics")]
    Stats,

    /// Show alarm monitor status
    #[command(about = "Show alarm monitor loop status")]
    Monitor,
}

pub async fn handle_command(cmd: AlarmCommands, base_url: &str, json: bool) -> Result<()> {
    let client = AlarmClient::new(base_url)?;

    match cmd {
        AlarmCommands::List {
            channel,
            level,
            keyword,
            page,
            size,
        } => {
            let data = client
                .list_alerts(channel, level, keyword.as_deref(), page, size)
                .await?;
            if json {
                crate::output::print_success(&data);
            } else {
                print_alerts_table(&data);
            }
        },

        AlarmCommands::Get { id } => {
            let data = client.get_alert(id).await?;
            if json {
                crate::output::print_success(&data);
            } else {
                println!("{}", serde_json::to_string_pretty(&data)?);
            }
        },

        AlarmCommands::Rules {
            channel,
            enabled,
            level,
            keyword,
            page,
            size,
        } => {
            let enabled_filter = if enabled { Some(true) } else { None };
            let data = client
                .list_rules(
                    channel,
                    enabled_filter,
                    level,
                    keyword.as_deref(),
                    page,
                    size,
                )
                .await?;
            if json {
                crate::output::print_success(&data);
            } else {
                print_rules_table(&data);
            }
        },

        AlarmCommands::RuleGet { id } => {
            let data = client.get_rule(id).await?;
            if json {
                crate::output::print_success(&data);
            } else {
                println!("{}", serde_json::to_string_pretty(&data)?);
            }
        },

        AlarmCommands::Events {
            rule,
            event_type,
            level,
            keyword,
            page,
            size,
        } => {
            let data = client
                .list_events(
                    rule,
                    event_type.as_deref(),
                    level,
                    keyword.as_deref(),
                    page,
                    size,
                )
                .await?;
            if json {
                crate::output::print_success(&data);
            } else {
                print_events_table(&data);
            }
        },

        AlarmCommands::Stats => {
            let data = client.get_statistics().await?;
            if json {
                crate::output::print_success(&data);
            } else {
                println!("{}", serde_json::to_string_pretty(&data)?);
            }
        },

        AlarmCommands::Monitor => {
            let data = client.get_monitor_status().await?;
            if json {
                crate::output::print_success(&data);
            } else {
                println!("{}", serde_json::to_string_pretty(&data)?);
            }
        },
    }

    Ok(())
}

// ── Human-readable table printers ────────────────────────────────────────────

fn print_alerts_table(data: &Value) {
    let list = data
        .get("data")
        .and_then(|d| d.get("list"))
        .and_then(|l| l.as_array());

    let total = data
        .get("data")
        .and_then(|d| d.get("total"))
        .and_then(|t| t.as_i64())
        .unwrap_or(0);

    match list {
        None => {
            println!("No active alerts.");
        },
        Some(items) if items.is_empty() => {
            println!("No active alerts.");
        },
        Some(items) => {
            println!(
                "{:<6} {:<20} {:<8} {:<10} {:<8} {:<8} Triggered",
                "ID", "Rule", "Level", "Channel", "Type", "Point"
            );
            println!("{}", "-".repeat(80));
            for item in items {
                let id = item.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
                let rule = item
                    .get("rule_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("-");
                let level = item
                    .get("warning_level")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);
                let level_label = match level {
                    1 => "low",
                    2 => "medium",
                    3 => "high",
                    _ => "?",
                };
                let channel = item.get("channel_id").and_then(|v| v.as_i64()).unwrap_or(0);
                let dtype = item
                    .get("data_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("-");
                let point = item.get("point_id").and_then(|v| v.as_i64()).unwrap_or(0);
                let triggered = item
                    .get("triggered_at")
                    .and_then(|v| v.as_i64())
                    .map(|ts| {
                        chrono::DateTime::from_timestamp(ts, 0)
                            .map(|dt| dt.format("%m-%d %H:%M:%S").to_string())
                            .unwrap_or_else(|| ts.to_string())
                    })
                    .unwrap_or_else(|| "-".to_string());

                println!(
                    "{:<6} {:<20} {:<8} {:<10} {:<8} {:<8} {}",
                    id,
                    truncate(rule, 19),
                    level_label,
                    channel,
                    dtype,
                    point,
                    triggered
                );
            }
            println!("\nTotal: {}", total);
        },
    }
}

fn print_rules_table(data: &Value) {
    let list = data
        .get("data")
        .and_then(|d| d.get("list"))
        .and_then(|l| l.as_array());

    let total = data
        .get("data")
        .and_then(|d| d.get("total"))
        .and_then(|t| t.as_i64())
        .unwrap_or(0);

    match list {
        None => {
            println!("No alarm rules found.");
        },
        Some(items) if items.is_empty() => {
            println!("No alarm rules found.");
        },
        Some(items) => {
            println!(
                "{:<6} {:<22} {:<8} {:<10} {:<8} {:<5} {:<5} Enabled",
                "ID", "Name", "Level", "Channel", "Type", "Op", "Thresh"
            );
            println!("{}", "-".repeat(80));
            for item in items {
                let id = item.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
                let name = item
                    .get("rule_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("-");
                let level = item
                    .get("warning_level")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);
                let channel = item.get("channel_id").and_then(|v| v.as_i64()).unwrap_or(0);
                let dtype = item
                    .get("data_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("-");
                let op = item.get("operator").and_then(|v| v.as_str()).unwrap_or("-");
                let value = item
                    .get("value")
                    .and_then(|v| v.as_f64())
                    .map(|f| format!("{:.2}", f))
                    .unwrap_or_else(|| "-".to_string());
                let enabled = item
                    .get("enabled")
                    .and_then(|v| v.as_bool())
                    .map(|b| if b { "yes" } else { "no" })
                    .unwrap_or("-");

                println!(
                    "{:<6} {:<22} {:<8} {:<10} {:<8} {:<5} {:<5} {}",
                    id,
                    truncate(name, 21),
                    level,
                    channel,
                    dtype,
                    op,
                    value,
                    enabled
                );
            }
            println!("\nTotal: {}", total);
        },
    }
}

fn print_events_table(data: &Value) {
    let list = data
        .get("data")
        .and_then(|d| d.get("list"))
        .and_then(|l| l.as_array());

    let total = data
        .get("data")
        .and_then(|d| d.get("total"))
        .and_then(|t| t.as_i64())
        .unwrap_or(0);

    match list {
        None => {
            println!("No alert events found.");
        },
        Some(items) if items.is_empty() => {
            println!("No alert events found.");
        },
        Some(items) => {
            println!(
                "{:<6} {:<20} {:<10} {:<8} {:<10} {:<12}",
                "ID", "Rule", "EventType", "Level", "Duration", "TriggeredAt"
            );
            println!("{}", "-".repeat(80));
            for item in items {
                let id = item.get("id").and_then(|v| v.as_i64()).unwrap_or(0);
                let rule = item
                    .get("rule_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("-");
                let etype = item
                    .get("event_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("-");
                let level = item
                    .get("warning_level")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);
                let duration = item
                    .get("duration")
                    .and_then(|v| v.as_i64())
                    .map(|s| format!("{}s", s))
                    .unwrap_or_else(|| "-".to_string());
                let triggered = item
                    .get("triggered_at")
                    .and_then(|v| v.as_i64())
                    .map(|ts| {
                        chrono::DateTime::from_timestamp(ts, 0)
                            .map(|dt| dt.format("%m-%d %H:%M").to_string())
                            .unwrap_or_else(|| ts.to_string())
                    })
                    .unwrap_or_else(|| "-".to_string());

                println!(
                    "{:<6} {:<20} {:<10} {:<8} {:<10} {}",
                    id,
                    truncate(rule, 19),
                    etype,
                    level,
                    duration,
                    triggered
                );
            }
            println!("\nTotal: {}", total);
        },
    }
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max { s } else { &s[..max] }
}

// ── HTTP client ───────────────────────────────────────────────────────────────

struct AlarmClient {
    client: Client,
    base_url: String,
}

impl AlarmClient {
    fn new(base_url: &str) -> Result<Self> {
        Ok(Self {
            client: Client::new(),
            base_url: base_url.to_string(),
        })
    }

    async fn list_alerts(
        &self,
        channel: Option<i64>,
        level: Option<i64>,
        keyword: Option<&str>,
        page: i64,
        size: i64,
    ) -> Result<Value> {
        let mut params = vec![
            ("page".to_string(), page.to_string()),
            ("page_size".to_string(), size.to_string()),
        ];
        if let Some(ch) = channel {
            params.push(("channel_id".to_string(), ch.to_string()));
        }
        if let Some(lv) = level {
            params.push(("warning_level".to_string(), lv.to_string()));
        }
        if let Some(kw) = keyword {
            params.push(("keyword".to_string(), kw.to_string()));
        }

        let resp = self
            .client
            .get(format!("{}/alarmApi/alerts", self.base_url))
            .query(&params)
            .send()
            .await?;

        if resp.status().is_success() {
            Ok(resp.json().await?)
        } else {
            Err(anyhow::anyhow!(
                "Failed to list alerts: {} — ensure alarmsrv is running",
                resp.status()
            ))
        }
    }

    async fn get_alert(&self, id: i64) -> Result<Value> {
        let resp = self
            .client
            .get(format!("{}/alarmApi/alerts/{}", self.base_url, id))
            .send()
            .await?;

        if resp.status().is_success() {
            Ok(resp.json().await?)
        } else {
            Err(anyhow::anyhow!(
                "Alert {} not found (HTTP {})",
                id,
                resp.status()
            ))
        }
    }

    async fn list_rules(
        &self,
        channel: Option<i64>,
        enabled: Option<bool>,
        level: Option<i64>,
        keyword: Option<&str>,
        page: i64,
        size: i64,
    ) -> Result<Value> {
        let mut params = vec![
            ("page".to_string(), page.to_string()),
            ("page_size".to_string(), size.to_string()),
        ];
        if let Some(ch) = channel {
            params.push(("channel_id".to_string(), ch.to_string()));
        }
        if let Some(en) = enabled {
            params.push(("enabled".to_string(), en.to_string()));
        }
        if let Some(lv) = level {
            params.push(("warning_level".to_string(), lv.to_string()));
        }
        if let Some(kw) = keyword {
            params.push(("keyword".to_string(), kw.to_string()));
        }

        let resp = self
            .client
            .get(format!("{}/alarmApi/rules", self.base_url))
            .query(&params)
            .send()
            .await?;

        if resp.status().is_success() {
            Ok(resp.json().await?)
        } else {
            Err(anyhow::anyhow!(
                "Failed to list alarm rules: {} — ensure alarmsrv is running",
                resp.status()
            ))
        }
    }

    async fn get_rule(&self, id: i64) -> Result<Value> {
        let resp = self
            .client
            .get(format!("{}/alarmApi/rules/{}", self.base_url, id))
            .send()
            .await?;

        if resp.status().is_success() {
            Ok(resp.json().await?)
        } else {
            Err(anyhow::anyhow!(
                "Rule {} not found (HTTP {})",
                id,
                resp.status()
            ))
        }
    }

    async fn list_events(
        &self,
        rule: Option<i64>,
        event_type: Option<&str>,
        level: Option<i64>,
        keyword: Option<&str>,
        page: i64,
        size: i64,
    ) -> Result<Value> {
        let mut params = vec![
            ("page".to_string(), page.to_string()),
            ("page_size".to_string(), size.to_string()),
        ];
        if let Some(r) = rule {
            params.push(("rule_id".to_string(), r.to_string()));
        }
        if let Some(et) = event_type {
            params.push(("event_type".to_string(), et.to_string()));
        }
        if let Some(lv) = level {
            params.push(("warning_level".to_string(), lv.to_string()));
        }
        if let Some(kw) = keyword {
            params.push(("keyword".to_string(), kw.to_string()));
        }

        let resp = self
            .client
            .get(format!("{}/alarmApi/alert-events", self.base_url))
            .query(&params)
            .send()
            .await?;

        if resp.status().is_success() {
            Ok(resp.json().await?)
        } else {
            Err(anyhow::anyhow!(
                "Failed to list alert events: {} — ensure alarmsrv is running",
                resp.status()
            ))
        }
    }

    async fn get_statistics(&self) -> Result<Value> {
        let resp = self
            .client
            .get(format!("{}/alarmApi/alert-statistics", self.base_url))
            .send()
            .await?;

        if resp.status().is_success() {
            Ok(resp.json().await?)
        } else {
            Err(anyhow::anyhow!(
                "Failed to get alarm statistics: {}",
                resp.status()
            ))
        }
    }

    async fn get_monitor_status(&self) -> Result<Value> {
        let resp = self
            .client
            .get(format!("{}/alarmApi/monitor/status", self.base_url))
            .send()
            .await?;

        if resp.status().is_success() {
            Ok(resp.json().await?)
        } else {
            Err(anyhow::anyhow!(
                "Failed to get monitor status: {}",
                resp.status()
            ))
        }
    }
}
