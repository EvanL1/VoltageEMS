//! Interactive TUI dashboard for real-time VoltageEMS monitoring
//!
//! `monarch top` — hierarchical navigation across all services:
//! ←→ switch view (Channels / Instances / Rules) | ↑↓ navigate | Enter drill | Esc back
//!
//! Drawing logic is in `top_draw.rs`.

use anyhow::Result;
use common::redis::RedisClient;
use crossterm::ExecutableCommand;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::widgets::TableState;
use reqwest::Client;
use serde_json::Value;
use std::collections::HashMap;
use std::io;
use std::sync::Arc;
use std::time::{Duration, Instant};
use voltage_rtdb::{RedisRtdb, Rtdb};

// ─── Data Model (pub(crate) for top_draw access) ────────────────────────────

pub(crate) struct ChannelInfo {
    pub id: u32,
    pub name: String,
    pub protocol: String,
    pub connected: bool,
    pub error_count: u64,
    pub read_count: u64,
    pub point_count: usize,
    pub address: String,
}

pub(crate) struct PointRow {
    pub point_id: u32,
    pub name: String,
    pub value: f64,
    pub unit: String,
    /// Millisecond epoch timestamp of last update (0 = unknown)
    pub ts_ms: u64,
}

pub(crate) struct InstanceInfo {
    pub id: u32,
    pub name: String,
    pub product: String,
}

pub(crate) struct InstPointRow {
    pub point_id: u32,
    pub kind: &'static str,
    pub name: String,
    pub value: f64,
    pub unit: String,
    /// Routing source/target, e.g. "Ch1:T.1" or "Ch1:C.3"
    pub routing: String,
    /// Millisecond epoch timestamp of last update (0 = unknown)
    pub ts_ms: u64,
}

pub(crate) struct RuleInfo {
    pub id: i64,
    pub name: String,
    pub enabled: bool,
    pub description: String,
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum View {
    Channels,
    Instances,
    Rules,
}

impl View {
    pub const ALL: [View; 3] = [View::Channels, View::Instances, View::Rules];

    pub fn index(self) -> usize {
        match self {
            View::Channels => 0,
            View::Instances => 1,
            View::Rules => 2,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            View::Channels => "Channels",
            View::Instances => "Instances",
            View::Rules => "Rules",
        }
    }

    fn next(self) -> Self {
        View::ALL[(self.index() + 1) % View::ALL.len()]
    }

    fn prev(self) -> Self {
        View::ALL[(self.index() + View::ALL.len() - 1) % View::ALL.len()]
    }
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Depth {
    List,
    Detail,
}

pub(crate) struct TopState {
    pub view: View,
    pub depth: Depth,
    pub channels: Vec<ChannelInfo>,
    pub ch_table: TableState,
    pub points: Vec<PointRow>,
    pub pt_table: TableState,
    pub hide_zero: bool,
    pub last_ch_id: Option<u32>,
    pub instances: Vec<InstanceInfo>,
    pub inst_table: TableState,
    pub inst_points: Vec<InstPointRow>,
    pub inst_pt_table: TableState,
    pub last_inst_id: Option<u32>,
    pub rules: Vec<RuleInfo>,
    pub rule_table: TableState,
    pub last_refresh: Instant,
    pub host_display: String,
    pub error_msg: Option<String>,
    pub redis_ok: bool,
}

impl TopState {
    fn new(host_display: String, redis_ok: bool) -> Self {
        Self {
            view: View::Channels,
            depth: Depth::List,
            channels: Vec::new(),
            ch_table: TableState::default(),
            points: Vec::new(),
            pt_table: TableState::default(),
            hide_zero: false,
            last_ch_id: None,
            instances: Vec::new(),
            inst_table: TableState::default(),
            inst_points: Vec::new(),
            inst_pt_table: TableState::default(),
            last_inst_id: None,
            rules: Vec::new(),
            rule_table: TableState::default(),
            last_refresh: Instant::now() - Duration::from_secs(10),
            host_display,
            error_msg: None,
            redis_ok,
        }
    }

    fn active_table(&mut self) -> &mut TableState {
        if self.depth == Depth::Detail {
            return match self.view {
                View::Channels => &mut self.pt_table,
                View::Instances => &mut self.inst_pt_table,
                View::Rules => &mut self.rule_table,
            };
        }
        match self.view {
            View::Channels => &mut self.ch_table,
            View::Instances => &mut self.inst_table,
            View::Rules => &mut self.rule_table,
        }
    }

    fn active_len(&self) -> usize {
        if self.depth == Depth::Detail {
            return match self.view {
                View::Channels => self.visible_points().len(),
                View::Instances => self.inst_points.len(),
                View::Rules => self.rules.len(),
            };
        }
        match self.view {
            View::Channels => self.channels.len(),
            View::Instances => self.instances.len(),
            View::Rules => self.rules.len(),
        }
    }

    fn move_up(&mut self) {
        let tbl = self.active_table();
        let i = tbl.selected().unwrap_or(0);
        if i > 0 {
            tbl.select(Some(i - 1));
        }
    }

    fn move_down(&mut self) {
        let max = self.active_len().saturating_sub(1);
        let tbl = self.active_table();
        let i = tbl.selected().unwrap_or(0);
        if i < max {
            tbl.select(Some(i + 1));
        }
    }

    fn selected_ch_id(&self) -> Option<u32> {
        self.ch_table
            .selected()
            .and_then(|i| self.channels.get(i))
            .map(|ch| ch.id)
    }

    fn selected_inst_id(&self) -> Option<u32> {
        self.inst_table
            .selected()
            .and_then(|i| self.instances.get(i))
            .map(|inst| inst.id)
    }

    pub fn visible_points(&self) -> Vec<&PointRow> {
        self.points
            .iter()
            .filter(|p| !self.hide_zero || p.value != 0.0)
            .collect()
    }
}

// ─── Entry Point ────────────────────────────────────────────────────────────

pub async fn run_top(comsrv_url: &str, modsrv_url: &str, redis_url: &str) -> Result<()> {
    let handle = tokio::runtime::Handle::current();
    let client = Client::builder().timeout(Duration::from_secs(5)).build()?;

    let (rtdb, redis_ok) = match RedisClient::new(redis_url).await {
        Ok(rc) => (Some(RedisRtdb::from_client(Arc::new(rc))), true),
        Err(_) => (None, false),
    };

    let host_display = comsrv_url
        .trim_start_matches("http://")
        .trim_start_matches("https://")
        .split(':')
        .next()
        .unwrap_or("localhost")
        .to_string();
    let mut state = TopState::new(host_display, redis_ok);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    stdout.execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let urls = Urls {
        comsrv: comsrv_url.to_string(),
        modsrv: modsrv_url.to_string(),
    };

    let result = tokio::task::block_in_place(|| {
        run_event_loop(&mut terminal, &client, &rtdb, &urls, &mut state, &handle)
    });

    disable_raw_mode()?;
    terminal.backend_mut().execute(LeaveAlternateScreen)?;
    result
}

struct Urls {
    comsrv: String,
    modsrv: String,
}

// ─── Event Loop ─────────────────────────────────────────────────────────────

fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    client: &Client,
    rtdb: &Option<RedisRtdb>,
    urls: &Urls,
    state: &mut TopState,
    handle: &tokio::runtime::Handle,
) -> Result<()> {
    loop {
        if state.last_refresh.elapsed() >= Duration::from_secs(1) {
            do_refresh(handle, client, rtdb, urls, state);
        }

        if state.view == View::Channels && state.selected_ch_id() != state.last_ch_id {
            reload_ch_points(handle, client, rtdb, &urls.comsrv, state);
        }
        if state.view == View::Instances
            && state.depth == Depth::Detail
            && state.selected_inst_id() != state.last_inst_id
        {
            reload_inst_points(handle, client, rtdb, &urls.modsrv, state);
        }

        terminal.draw(|f| crate::top_draw::draw_ui(f, state))?;

        if event::poll(Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
            && handle_key(key.code, state, handle, client, rtdb, urls)
        {
            return Ok(());
        }
    }
}

/// Returns true if the app should exit.
fn handle_key(
    code: KeyCode,
    state: &mut TopState,
    handle: &tokio::runtime::Handle,
    client: &Client,
    rtdb: &Option<RedisRtdb>,
    urls: &Urls,
) -> bool {
    match code {
        KeyCode::Char('q') => return true,
        // Esc always goes back one level; never exits
        KeyCode::Esc => {
            if state.depth == Depth::Detail {
                state.depth = Depth::List;
            }
            // At list level, Esc is no-op (use q to quit)
        },
        KeyCode::Left => {
            if state.depth == Depth::Detail {
                state.depth = Depth::List;
            } else {
                state.view = state.view.prev();
            }
        },
        KeyCode::Right => {
            if state.depth == Depth::List {
                state.view = state.view.next();
            }
        },
        KeyCode::Tab => {
            state.view = state.view.next();
            state.depth = Depth::List;
        },
        KeyCode::BackTab => {
            state.view = state.view.prev();
            state.depth = Depth::List;
        },
        KeyCode::Enter => {
            if state.depth == Depth::List {
                match state.view {
                    View::Channels => {
                        state.depth = Depth::Detail;
                        state.pt_table.select(Some(0));
                    },
                    View::Instances => {
                        state.depth = Depth::Detail;
                        state.inst_pt_table.select(Some(0));
                        reload_inst_points(handle, client, rtdb, &urls.modsrv, state);
                    },
                    View::Rules => {},
                }
            }
        },
        KeyCode::Up | KeyCode::Char('k') => state.move_up(),
        KeyCode::Down | KeyCode::Char('j') => state.move_down(),
        KeyCode::Char('z') => state.hide_zero = !state.hide_zero,
        KeyCode::Char('r') => {
            do_refresh(handle, client, rtdb, urls, state);
            if state.view == View::Channels {
                reload_ch_points(handle, client, rtdb, &urls.comsrv, state);
            }
            if state.view == View::Instances && state.depth == Depth::Detail {
                reload_inst_points(handle, client, rtdb, &urls.modsrv, state);
            }
        },
        KeyCode::Char('1') => {
            state.view = View::Channels;
            state.depth = Depth::List;
        },
        KeyCode::Char('2') => {
            state.view = View::Instances;
            state.depth = Depth::List;
        },
        KeyCode::Char('3') => {
            state.view = View::Rules;
            state.depth = Depth::List;
        },
        _ => {},
    }
    false
}

// ─── Refresh ────────────────────────────────────────────────────────────────

fn do_refresh(
    handle: &tokio::runtime::Handle,
    client: &Client,
    rtdb: &Option<RedisRtdb>,
    urls: &Urls,
    state: &mut TopState,
) {
    let prev_pt = state.pt_table.selected();

    if let Ok(chs) = handle.block_on(fetch_channels(client, &urls.comsrv)) {
        state.channels = chs;
        clamp_select(&mut state.ch_table, state.channels.len());
    }

    if let Some(ch_id) = state.selected_ch_id()
        && state.last_ch_id == Some(ch_id)
        && let Ok(pts) = handle.block_on(load_points(client, rtdb, &urls.comsrv, ch_id))
    {
        state.points = pts;
        let max = state.visible_points().len().saturating_sub(1);
        state.pt_table.select(prev_pt.map(|s| s.min(max)));
    }

    if state.view == View::Instances
        && state.depth == Depth::Detail
        && let Some(inst_id) = state.selected_inst_id()
    {
        let prev_ipt = state.inst_pt_table.selected();
        if let Ok(pts) = handle.block_on(fetch_inst_points(client, rtdb, &urls.modsrv, inst_id)) {
            state.inst_points = pts;
            let max = state.inst_points.len().saturating_sub(1);
            state.inst_pt_table.select(prev_ipt.map(|s| s.min(max)));
        }
    }

    if let Ok(insts) = handle.block_on(fetch_instances(client, &urls.modsrv)) {
        state.instances = insts;
        clamp_select(&mut state.inst_table, state.instances.len());
    }

    if let Ok(rules) = handle.block_on(fetch_rules(client, &urls.modsrv)) {
        state.rules = rules;
        clamp_select(&mut state.rule_table, state.rules.len());
    }

    state.error_msg = None;
    state.last_refresh = Instant::now();
}

fn reload_ch_points(
    handle: &tokio::runtime::Handle,
    client: &Client,
    rtdb: &Option<RedisRtdb>,
    comsrv_url: &str,
    state: &mut TopState,
) {
    let ch_id = state.selected_ch_id();
    state.last_ch_id = ch_id;
    if let Some(id) = ch_id
        && let Ok(pts) = handle.block_on(load_points(client, rtdb, comsrv_url, id))
    {
        state.points = pts;
        state.pt_table.select(Some(0));
    }
}

fn reload_inst_points(
    handle: &tokio::runtime::Handle,
    client: &Client,
    rtdb: &Option<RedisRtdb>,
    modsrv_url: &str,
    state: &mut TopState,
) {
    let inst_id = state.selected_inst_id();
    state.last_inst_id = inst_id;
    if let Some(id) = inst_id
        && let Ok(pts) = handle.block_on(fetch_inst_points(client, rtdb, modsrv_url, id))
    {
        state.inst_points = pts;
        state.inst_pt_table.select(Some(0));
    }
}

fn clamp_select(table: &mut TableState, len: usize) {
    if len == 0 {
        table.select(None);
    } else if table.selected().is_none() {
        table.select(Some(0));
    } else if let Some(s) = table.selected()
        && s >= len
    {
        table.select(Some(len - 1));
    }
}

// ─── Data Fetching ──────────────────────────────────────────────────────────

async fn fetch_channels(client: &Client, base_url: &str) -> Result<Vec<ChannelInfo>> {
    let json: Value = client
        .get(format!("{base_url}/api/channels"))
        .send()
        .await?
        .json()
        .await?;

    let raw = json
        .pointer("/data/list")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut out = Vec::with_capacity(raw.len());
    for ch in &raw {
        let id = ch["id"].as_u64().unwrap_or(0) as u32;
        if id == 0 {
            continue;
        }
        out.push(build_channel(client, base_url, id, ch).await);
    }
    out.sort_by_key(|c| c.id);
    Ok(out)
}

async fn build_channel(client: &Client, base_url: &str, id: u32, ch: &Value) -> ChannelInfo {
    let name = ch["name"].as_str().unwrap_or("").to_string();
    let protocol = ch["protocol"].as_str().unwrap_or("").to_string();
    let (connected, error_count, read_count, point_count, address) =
        fetch_ch_stats(client, base_url, id).await;
    ChannelInfo {
        id,
        name,
        protocol,
        connected,
        error_count,
        read_count,
        point_count,
        address,
    }
}

async fn fetch_ch_stats(
    client: &Client,
    base_url: &str,
    id: u32,
) -> (bool, u64, u64, usize, String) {
    let url = format!("{base_url}/api/channels/{id}/status");
    let Ok(resp) = client.get(&url).send().await else {
        return (false, 0, 0, 0, String::new());
    };
    let Ok(json): Result<Value, _> = resp.json().await else {
        return (false, 0, 0, 0, String::new());
    };
    let d = json.pointer("/data").unwrap_or(&Value::Null);
    let s = d.get("statistics").unwrap_or(&Value::Null);
    let x = s.get("extra").unwrap_or(&Value::Null);
    (
        d["connected"].as_bool().unwrap_or(false),
        s["error_count"].as_u64().unwrap_or(0),
        s["read_count"].as_u64().unwrap_or(0),
        x["points"].as_u64().unwrap_or(0) as usize,
        x["address"].as_str().unwrap_or("").to_string(),
    )
}

async fn load_points(
    client: &Client,
    rtdb: &Option<RedisRtdb>,
    base_url: &str,
    ch_id: u32,
) -> Result<Vec<PointRow>> {
    let defs = fetch_point_defs(client, base_url, ch_id).await?;
    let (live, ts) = match rtdb {
        Some(r) => {
            let v = fetch_live(r, ch_id).await.unwrap_or_default();
            let t = fetch_ts(r, &format!("comsrv:{ch_id}:T:ts")).await;
            (v, t)
        },
        None => (HashMap::new(), HashMap::new()),
    };
    Ok(defs
        .into_iter()
        .map(|(pid, name, unit)| PointRow {
            point_id: pid,
            value: live.get(&pid).copied().unwrap_or(0.0),
            ts_ms: ts.get(&pid).copied().unwrap_or(0),
            name,
            unit,
        })
        .collect())
}

async fn fetch_point_defs(
    client: &Client,
    base_url: &str,
    ch_id: u32,
) -> Result<Vec<(u32, String, String)>> {
    let json: Value = client
        .get(format!("{base_url}/api/channels/{ch_id}/points"))
        .send()
        .await?
        .json()
        .await?;

    let arr = json
        .pointer("/data/telemetry")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    Ok(arr
        .iter()
        .map(|p| {
            (
                p["point_id"].as_u64().unwrap_or(0) as u32,
                p["signal_name"].as_str().unwrap_or("").to_string(),
                p["unit"].as_str().unwrap_or("").to_string(),
            )
        })
        .collect())
}

async fn fetch_live(rtdb: &RedisRtdb, ch_id: u32) -> Result<HashMap<u32, f64>> {
    let raw = rtdb.hash_get_all(&format!("comsrv:{ch_id}:T")).await?;
    Ok(raw
        .into_iter()
        .filter_map(|(f, b)| Some((f.parse().ok()?, std::str::from_utf8(&b).ok()?.parse().ok()?)))
        .collect())
}

async fn fetch_instances(client: &Client, base_url: &str) -> Result<Vec<InstanceInfo>> {
    let json: Value = client
        .get(format!("{base_url}/api/instances"))
        .send()
        .await?
        .json()
        .await?;

    let arr = json
        .pointer("/data/list")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    Ok(arr
        .iter()
        .map(|i| InstanceInfo {
            id: i["instance_id"].as_u64().unwrap_or(0) as u32,
            name: i["instance_name"].as_str().unwrap_or("").to_string(),
            product: i["product_name"].as_str().unwrap_or("").to_string(),
        })
        .collect())
}

async fn fetch_rules(client: &Client, base_url: &str) -> Result<Vec<RuleInfo>> {
    let json: Value = client
        .get(format!("{base_url}/api/rules"))
        .send()
        .await?
        .json()
        .await?;

    let arr = json
        .pointer("/data/list")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    Ok(arr
        .iter()
        .map(|r| RuleInfo {
            id: r["id"].as_i64().unwrap_or(0),
            name: r["name"].as_str().unwrap_or("").to_string(),
            enabled: r["enabled"].as_bool().unwrap_or(false),
            description: r["description"].as_str().unwrap_or("").to_string(),
        })
        .collect())
}

async fn fetch_inst_points(
    client: &Client,
    rtdb: &Option<RedisRtdb>,
    modsrv_url: &str,
    inst_id: u32,
) -> Result<Vec<InstPointRow>> {
    let json: Value = client
        .get(format!("{modsrv_url}/api/instances/{inst_id}/points"))
        .send()
        .await?
        .json()
        .await?;

    let data = json.pointer("/data").unwrap_or(&Value::Null);
    let (live_m, live_a) = match rtdb {
        Some(r) => {
            let m = fetch_inst_live(r, inst_id, "M").await.unwrap_or_default();
            let a = fetch_inst_live(r, inst_id, "A").await.unwrap_or_default();
            (m, a)
        },
        None => (HashMap::new(), HashMap::new()),
    };

    // Collect unique source channel IDs to fetch their timestamps
    let mut ch_ts_cache: HashMap<u32, HashMap<u32, u64>> = HashMap::new();

    let mut rows = Vec::new();
    if let Some(arr) = data.get("measurements").and_then(|v| v.as_array()) {
        for p in arr {
            let pid = p["measurement_id"].as_u64().unwrap_or(0) as u32;
            let ts = get_routing_ts(p, rtdb, &mut ch_ts_cache).await;
            rows.push(InstPointRow {
                point_id: pid,
                kind: "M",
                name: p["name"].as_str().unwrap_or("").to_string(),
                value: live_m.get(&pid).copied().unwrap_or(0.0),
                unit: p["unit"].as_str().unwrap_or("").to_string(),
                routing: format_routing(p, "←"),
                ts_ms: ts,
            });
        }
    }
    if let Some(arr) = data.get("actions").and_then(|v| v.as_array()) {
        for p in arr {
            let pid = p["action_id"].as_u64().unwrap_or(0) as u32;
            rows.push(InstPointRow {
                point_id: pid,
                kind: "A",
                name: p["name"].as_str().unwrap_or("").to_string(),
                value: live_a.get(&pid).copied().unwrap_or(0.0),
                unit: p["unit"].as_str().unwrap_or("").to_string(),
                routing: format_routing(p, "→"),
                ts_ms: 0, // Action timestamps not tracked
            });
        }
    }
    Ok(rows)
}

/// Fetch per-point timestamps from a Redis hash (millis epoch)
async fn fetch_ts(rtdb: &RedisRtdb, key: &str) -> HashMap<u32, u64> {
    let raw = match rtdb.hash_get_all(key).await {
        Ok(r) => r,
        Err(_) => return HashMap::new(),
    };
    raw.into_iter()
        .filter_map(|(f, b)| Some((f.parse().ok()?, std::str::from_utf8(&b).ok()?.parse().ok()?)))
        .collect()
}

/// Get timestamp for an instance point via its routing source channel
async fn get_routing_ts(
    point: &Value,
    rtdb: &Option<RedisRtdb>,
    cache: &mut HashMap<u32, HashMap<u32, u64>>,
) -> u64 {
    let Some(r) = point.get("routing").filter(|v| !v.is_null()) else {
        return 0;
    };
    let Some(rtdb) = rtdb else { return 0 };
    let ch_id = r["channel_id"].as_u64().unwrap_or(0) as u32;
    let ch_type = r["channel_type"].as_str().unwrap_or("T");
    let ch_pt = r["channel_point_id"].as_u64().unwrap_or(0) as u32;
    if ch_id == 0 {
        return 0;
    }

    // Cache per-channel timestamp hash to avoid redundant Redis calls
    let ts_key = format!("comsrv:{ch_id}:{ch_type}:ts");
    cache.entry(ch_id).or_insert(fetch_ts(rtdb, &ts_key).await);
    cache
        .get(&ch_id)
        .and_then(|m| m.get(&ch_pt))
        .copied()
        .unwrap_or(0)
}

/// Format routing info from point JSON: "← Ch1:T.5" or "→ Ch1:C.3"
fn format_routing(point: &Value, arrow: &str) -> String {
    let r = match point.get("routing") {
        Some(v) if !v.is_null() => v,
        _ => return String::new(),
    };
    let ch_id = r["channel_id"].as_u64().unwrap_or(0);
    let ch_type = r["channel_type"].as_str().unwrap_or("?");
    let pt_id = r["channel_point_id"].as_u64().unwrap_or(0);
    if ch_id == 0 {
        return String::new();
    }
    format!("{arrow} Ch{ch_id}:{ch_type}.{pt_id}")
}

/// Current time in milliseconds (for staleness comparison)
pub(crate) fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

/// Format a millisecond epoch timestamp as HH:MM:SS local time
pub(crate) fn format_time(ts_ms: u64) -> String {
    if ts_ms == 0 {
        return "-".to_string();
    }
    use chrono::{Local, TimeZone};
    let secs = (ts_ms / 1000) as i64;
    match Local.timestamp_opt(secs, 0) {
        chrono::LocalResult::Single(dt) => {
            let today = Local::now().date_naive();
            if dt.date_naive() == today {
                dt.format("%H:%M:%S").to_string()
            } else {
                dt.format("%m-%d %H:%M:%S").to_string()
            }
        },
        _ => "-".to_string(),
    }
}

/// Is this timestamp stale (older than threshold)?
pub(crate) fn is_stale(ts_ms: u64, threshold_s: u64) -> bool {
    if ts_ms == 0 {
        return true;
    }
    now_ms().saturating_sub(ts_ms) / 1000 > threshold_s
}

async fn fetch_inst_live(rtdb: &RedisRtdb, inst_id: u32, kind: &str) -> Result<HashMap<u32, f64>> {
    let raw = rtdb.hash_get_all(&format!("inst:{inst_id}:{kind}")).await?;
    Ok(raw
        .into_iter()
        .filter_map(|(f, b)| Some((f.parse().ok()?, std::str::from_utf8(&b).ok()?.parse().ok()?)))
        .collect())
}
