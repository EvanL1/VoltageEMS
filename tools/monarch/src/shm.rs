//! Shared Memory CLI - Interactive REPL for voltage-rtdb
//!
//! Provides a mysql-cli style interactive interface for reading/writing
//! shared memory data with zero-latency access.

use anyhow::{bail, Context, Result};
use clap::Subcommand;
use colored::*;
use common::PointType;
use rustyline::completion::{Completer, Pair};
use rustyline::error::ReadlineError;
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::validate::Validator;
use rustyline::{Editor, Helper};
use std::io;
use std::time::{Duration, Instant};
use voltage_rtdb::{default_shm_path, SharedConfig, SharedVecRtdbReader};

// TUI imports
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{Block, Borders, Paragraph, Row, Table, TableState};
use ratatui::Terminal;

/// Clap subcommands (for one-shot mode)
#[derive(Subcommand)]
pub enum ShmCommands {
    /// Get point value
    Get {
        /// Key format: inst:<id>:M|A:<point_id> or ch:<id>:T|S|C|A:<point_id>
        key: String,
    },

    /// Show shared memory statistics
    Info,

    /// Watch key for changes (real-time monitoring)
    Watch {
        /// Key to watch
        key: String,

        /// Polling interval in milliseconds
        #[arg(short, long, default_value = "500")]
        interval_ms: u64,
    },

    /// Real-time TUI dashboard (like htop)
    Top,
}

/// Parsed shared memory key
#[derive(Debug, Clone)]
enum ShmKey {
    /// Instance point: inst:<id>:M|A:<point_id>
    Instance {
        instance_id: u32,
        point_type: u8, // 0=Measurement, 1=Action
        point_id: u32,
    },
    /// Channel point: ch:<id>:T|S|C|A:<point_id>
    Channel {
        channel_id: u32,
        point_type: PointType,
        point_id: u32,
    },
}

impl std::fmt::Display for ShmKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ShmKey::Instance {
                instance_id,
                point_type,
                point_id,
            } => {
                let role = if *point_type == 0 { "M" } else { "A" };
                write!(f, "inst:{}:{}:{}", instance_id, role, point_id)
            },
            ShmKey::Channel {
                channel_id,
                point_type,
                point_id,
            } => {
                let ptype = match point_type {
                    PointType::Telemetry => "T",
                    PointType::Signal => "S",
                    PointType::Control => "C",
                    PointType::Adjustment => "A",
                };
                write!(f, "ch:{}:{}:{}", channel_id, ptype, point_id)
            },
        }
    }
}

/// Parse key string into ShmKey
///
/// Formats:
/// - `inst:<id>:M:<point_id>` - Instance measurement
/// - `inst:<id>:A:<point_id>` - Instance action
/// - `ch:<id>:T:<point_id>`   - Channel telemetry
/// - `ch:<id>:S:<point_id>`   - Channel signal
/// - `ch:<id>:C:<point_id>`   - Channel control
/// - `ch:<id>:A:<point_id>`   - Channel adjustment
fn parse_key(key: &str) -> Result<ShmKey> {
    let parts: Vec<&str> = key.split(':').collect();

    match parts.as_slice() {
        ["inst", id, role, point_id] => {
            let instance_id: u32 = id.parse().context("Invalid instance ID")?;
            let point_id: u32 = point_id.parse().context("Invalid point ID")?;
            let point_type = match role.to_uppercase().as_str() {
                "M" => 0,
                "A" => 1,
                _ => bail!("Invalid role '{}'. Use M (Measurement) or A (Action)", role),
            };
            Ok(ShmKey::Instance {
                instance_id,
                point_type,
                point_id,
            })
        },
        ["ch", id, ptype, point_id] => {
            let channel_id: u32 = id.parse().context("Invalid channel ID")?;
            let point_id: u32 = point_id.parse().context("Invalid point ID")?;
            let point_type = match ptype.to_uppercase().as_str() {
                "T" => PointType::Telemetry,
                "S" => PointType::Signal,
                "C" => PointType::Control,
                "A" => PointType::Adjustment,
                _ => bail!(
                    "Invalid point type '{}'. Use T/S/C/A (Telemetry/Signal/Control/Adjustment)",
                    ptype
                ),
            };
            Ok(ShmKey::Channel {
                channel_id,
                point_type,
                point_id,
            })
        },
        _ => bail!(
            "Invalid key format '{}'\n\
             Use: inst:<id>:M|A:<point_id> or ch:<id>:T|S|C|A:<point_id>",
            key
        ),
    }
}

// ============================================================================
// Tab Completion Helper
// ============================================================================

/// REPL helper providing Tab completion for commands and keys
struct ShmHelper;

impl Helper for ShmHelper {}

impl Hinter for ShmHelper {
    type Hint = String;

    fn hint(&self, _line: &str, _pos: usize, _ctx: &rustyline::Context<'_>) -> Option<String> {
        None
    }
}

impl Highlighter for ShmHelper {}

impl Validator for ShmHelper {}

impl Completer for ShmHelper {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &rustyline::Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        let line = &line[..pos];

        // 1. Command completion (no space yet)
        if !line.contains(' ') {
            return Ok(complete_command(line));
        }

        // 2. Key completion for GET/WATCH commands
        let parts: Vec<&str> = line.split_whitespace().collect();
        if !parts.is_empty() {
            let cmd = parts[0].to_uppercase();
            if matches!(cmd.as_str(), "GET" | "WATCH") {
                // Complete key if we're still typing the second argument
                if parts.len() == 1 || (parts.len() == 2 && !line.ends_with(' ')) {
                    let key_part = parts.get(1).copied().unwrap_or("");
                    let start = line.len() - key_part.len();
                    return Ok(complete_key(key_part, start));
                }
            }
        }

        Ok((pos, vec![]))
    }
}

/// Complete command names
fn complete_command(prefix: &str) -> (usize, Vec<Pair>) {
    let commands = ["GET", "INFO", "WATCH", "HELP", "QUIT", "EXIT"];
    let prefix_upper = prefix.to_uppercase();

    let matches: Vec<Pair> = commands
        .iter()
        .filter(|cmd| cmd.starts_with(&prefix_upper))
        .map(|cmd| Pair {
            display: (*cmd).to_string(),
            replacement: (*cmd).to_string(),
        })
        .collect();

    (0, matches)
}

/// Complete key format: inst:<id>:M|A:<point_id> or ch:<id>:T|S|C|A:<point_id>
fn complete_key(key_prefix: &str, start_pos: usize) -> (usize, Vec<Pair>) {
    let parts: Vec<&str> = key_prefix.split(':').collect();

    match parts.as_slice() {
        // Empty or just started -> suggest inst: or ch:
        [] | [""] => (
            start_pos,
            vec![
                Pair {
                    display: "inst:".into(),
                    replacement: "inst:".into(),
                },
                Pair {
                    display: "ch:".into(),
                    replacement: "ch:".into(),
                },
            ],
        ),
        // Partial prefix -> complete to inst: or ch:
        [prefix] if "inst".starts_with(*prefix) || "ch".starts_with(*prefix) => {
            let mut matches = vec![];
            if "inst".starts_with(*prefix) {
                matches.push(Pair {
                    display: "inst:".into(),
                    replacement: "inst:".into(),
                });
            }
            if "ch".starts_with(*prefix) {
                matches.push(Pair {
                    display: "ch:".into(),
                    replacement: "ch:".into(),
                });
            }
            (start_pos, matches)
        },
        // inst:<id>: -> complete M or A
        ["inst", _id, ""] | ["inst", _id] if key_prefix.ends_with(':') => (
            start_pos,
            vec![
                Pair {
                    display: "M (Measurement)".into(),
                    replacement: format!("{}M:", key_prefix),
                },
                Pair {
                    display: "A (Action)".into(),
                    replacement: format!("{}A:", key_prefix),
                },
            ],
        ),
        // ch:<id>: -> complete T/S/C/A
        ["ch", _id, ""] | ["ch", _id] if key_prefix.ends_with(':') => (
            start_pos,
            vec![
                Pair {
                    display: "T (Telemetry)".into(),
                    replacement: format!("{}T:", key_prefix),
                },
                Pair {
                    display: "S (Signal)".into(),
                    replacement: format!("{}S:", key_prefix),
                },
                Pair {
                    display: "C (Control)".into(),
                    replacement: format!("{}C:", key_prefix),
                },
                Pair {
                    display: "A (Adjustment)".into(),
                    replacement: format!("{}A:", key_prefix),
                },
            ],
        ),
        _ => (start_pos, vec![]),
    }
}

/// Main entry point - handles both REPL and one-shot modes
pub fn handle_command(cmd: Option<ShmCommands>) -> Result<()> {
    match cmd {
        None => run_repl(),
        Some(cmd) => handle_single_command(cmd),
    }
}

/// Open shared memory reader
fn open_reader() -> Result<SharedVecRtdbReader> {
    let path = default_shm_path();
    let config = SharedConfig::default().with_path(path.clone());

    SharedVecRtdbReader::open(&config)
        .with_context(|| format!("Failed to open shared memory at {:?}", path))
}

/// Handle single command (one-shot mode)
fn handle_single_command(cmd: ShmCommands) -> Result<()> {
    if let ShmCommands::Top = cmd {
        return run_dashboard();
    }

    let reader = open_reader()?;

    match cmd {
        ShmCommands::Get { key } => {
            let parsed = parse_key(&key)?;
            let value = get_value(&reader, &parsed);
            match value {
                Some(v) => println!("{}", v),
                None => println!("(nil)"),
            }
        },
        ShmCommands::Info => {
            print_info(&reader);
        },
        ShmCommands::Watch { key, interval_ms } => {
            let parsed = parse_key(&key)?;
            watch_key(&reader, &parsed, interval_ms)?;
        },
        ShmCommands::Top => unreachable!(),
    }

    Ok(())
}

/// Get value from shared memory
fn get_value(reader: &SharedVecRtdbReader, key: &ShmKey) -> Option<f64> {
    match key {
        ShmKey::Instance {
            instance_id,
            point_type,
            point_id,
        } => reader.get(*instance_id, *point_type, *point_id),
        ShmKey::Channel {
            channel_id,
            point_type,
            point_id,
        } => reader.get_channel(*channel_id, *point_type, *point_id),
    }
}

/// Print shared memory info/statistics
fn print_info(reader: &SharedVecRtdbReader) {
    let stats = reader.stats();
    let path = default_shm_path();

    println!("{}", "=== Shared Memory Stats ===".bright_cyan());
    println!("Path:          {}", path.display());
    println!(
        "Instances:     {} (indexed: {})",
        stats.instance_count, stats.indexed_instances
    );
    println!(
        "Channels:      {} (indexed: {})",
        stats.channel_count, stats.indexed_channels
    );
    println!("Total Points:  {}", stats.total_points);

    // Format timestamp
    if stats.last_update_ts > 0 {
        let ts_secs = stats.last_update_ts / 1000;
        let ts_ms = stats.last_update_ts % 1000;
        // Use simple epoch seconds display (human-readable timestamp requires chrono)
        println!(
            "Last Update:   {} ({}.{:03}s since epoch)",
            format_epoch_secs(ts_secs),
            ts_secs,
            ts_ms
        );
    } else {
        println!("Last Update:   never");
    }

    // Writer heartbeat
    let heartbeat_age = voltage_rtdb::shared_impl::timestamp_ms() - stats.writer_heartbeat;
    let alive = reader.is_writer_alive(5000);
    let status = if alive {
        format!("{} ({}ms ago)", "alive".green(), heartbeat_age)
    } else {
        format!("{} ({}ms ago)", "dead/stale".red(), heartbeat_age)
    };
    println!("Writer:        {}", status);
}

/// Watch a key for changes with polling
fn watch_key(reader: &SharedVecRtdbReader, key: &ShmKey, interval_ms: u64) -> Result<()> {
    println!(
        "Watching {} ({} to stop)",
        key.to_string().bright_yellow(),
        "Ctrl+C".bright_cyan()
    );

    let interval = Duration::from_millis(interval_ms);

    loop {
        let value = get_value(reader, key);
        let now = format_current_time();

        match value {
            Some(v) => println!("[{}] {}", now, v),
            None => println!("[{}] (nil)", now),
        }

        std::thread::sleep(interval);
    }
}

/// Format current time as HH:MM:SS
fn format_current_time() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Calculate local time components (simplified - assumes UTC for now)
    let secs_in_day = now % 86400;
    let hours = secs_in_day / 3600;
    let minutes = (secs_in_day % 3600) / 60;
    let seconds = secs_in_day % 60;
    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
}

/// Format epoch seconds as a human-readable time string
fn format_epoch_secs(epoch_secs: u64) -> String {
    // Calculate time components
    let secs_in_day = epoch_secs % 86400;
    let hours = secs_in_day / 3600;
    let minutes = (secs_in_day % 3600) / 60;
    let seconds = secs_in_day % 60;
    format!("{:02}:{:02}:{:02} UTC", hours, minutes, seconds)
}

/// Interactive REPL loop
fn run_repl() -> Result<()> {
    let reader = open_reader()?;

    // Create editor with Tab completion helper
    let config = rustyline::Config::builder()
        .completion_type(rustyline::CompletionType::List)
        .build();
    let mut rl = Editor::with_config(config).context("Failed to initialize readline")?;
    rl.set_helper(Some(ShmHelper));

    println!("{}", "Voltage Shared Memory CLI".bright_cyan().bold());
    println!(
        "Type '{}' for commands, {} for completion\n",
        "help".bright_yellow(),
        "Tab".bright_cyan()
    );

    loop {
        match rl.readline("voltage-shm> ") {
            Ok(line) => {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }

                // Add to history (ignore errors)
                let _ = rl.add_history_entry(line);

                // Parse and execute
                match execute_repl_command(&reader, line) {
                    Ok(true) => continue, // Normal command, continue REPL
                    Ok(false) => break,   // QUIT command
                    Err(e) => eprintln!("{} {}", "Error:".red(), e),
                }
            },
            Err(ReadlineError::Interrupted) => {
                // Ctrl+C - ignore and continue
                println!("^C");
                continue;
            },
            Err(ReadlineError::Eof) => {
                // Ctrl+D - exit
                break;
            },
            Err(e) => {
                eprintln!("{} {}", "Readline error:".red(), e);
                break;
            },
        }
    }

    println!("Bye!");
    Ok(())
}

/// Execute a single REPL command
/// Returns Ok(true) to continue, Ok(false) to quit
fn execute_repl_command(reader: &SharedVecRtdbReader, input: &str) -> Result<bool> {
    let parts: Vec<&str> = input.split_whitespace().collect();
    let cmd = parts.first().map(|s| s.to_uppercase());

    match cmd.as_deref() {
        Some("GET") => {
            if parts.len() < 2 {
                println!("Usage: GET <key>");
                println!("  Key format: inst:<id>:M|A:<point_id> or ch:<id>:T|S|C|A:<point_id>");
            } else {
                let key = parse_key(parts[1])?;
                match get_value(reader, &key) {
                    Some(v) => println!("{}", v),
                    None => println!("(nil)"),
                }
            }
        },
        Some("INFO") => {
            print_info(reader);
        },
        Some("WATCH") => {
            if parts.len() < 2 {
                println!("Usage: WATCH <key> [interval_ms]");
            } else {
                let key = parse_key(parts[1])?;
                let interval = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(500);
                // Note: WATCH will block until Ctrl+C
                watch_key(reader, &key, interval)?;
            }
        },
        Some("HELP") | Some("?") => {
            print_help();
        },
        Some("QUIT") | Some("EXIT") | Some("Q") => {
            return Ok(false);
        },
        Some(unknown) => {
            println!(
                "Unknown command '{}'. Type '{}' for available commands.",
                unknown.red(),
                "help".bright_yellow()
            );
        },
        None => {},
    }

    Ok(true)
}

/// Print help message
fn print_help() {
    println!("{}", "=== Available Commands ===".bright_cyan());
    println!();
    println!("  {}     Read point value", "GET <key>".bright_yellow());
    println!(
        "  {}          Show shared memory statistics",
        "INFO".bright_yellow()
    );
    println!(
        "  {}   Monitor point value in real-time",
        "WATCH <key>".bright_yellow()
    );
    println!(
        "  {}          Show this help message",
        "HELP".bright_yellow()
    );
    println!("  {}          Exit the CLI", "QUIT".bright_yellow());
    println!();
    println!("{}", "=== Key Format ===".bright_cyan());
    println!();
    println!("  Instance points:");
    println!("    inst:<id>:M:<point_id>   Measurement point");
    println!("    inst:<id>:A:<point_id>   Action point");
    println!();
    println!("  Channel points:");
    println!("    ch:<id>:T:<point_id>     Telemetry point");
    println!("    ch:<id>:S:<point_id>     Signal point");
    println!("    ch:<id>:C:<point_id>     Control point");
    println!("    ch:<id>:A:<point_id>     Adjustment point");
    println!();
    println!("{}", "=== Examples ===".bright_cyan());
    println!();
    println!("  GET inst:5:M:1          Get instance 5, measurement point 1");
    println!("  GET ch:1001:T:2         Get channel 1001, telemetry point 2");
    println!("  WATCH inst:5:M:1        Watch instance 5, measurement point 1");
    println!("  WATCH inst:5:M:1 100    Watch with 100ms interval");
}

// ============================================================================
// TUI Dashboard (htop-style)
// ============================================================================

/// Point data for display in TUI
struct PointRow {
    key: String,
    kind: &'static str,
    value: f64,
}

/// Dashboard application state
struct DashboardState {
    /// Collected point data
    points: Vec<PointRow>,
    /// Table selection state
    table_state: TableState,
    /// Scroll offset
    scroll_offset: usize,
    /// Last scan time (for cache invalidation)
    last_scan: Instant,
    /// Cached instance/channel counts
    last_instance_count: u32,
    last_channel_count: u32,
}

impl DashboardState {
    fn new() -> Self {
        Self {
            points: Vec::new(),
            table_state: TableState::default(),
            scroll_offset: 0,
            last_scan: Instant::now(),
            last_instance_count: 0,
            last_channel_count: 0,
        }
    }

    fn scroll_up(&mut self) {
        if self.scroll_offset > 0 {
            self.scroll_offset -= 1;
        }
    }

    fn scroll_down(&mut self, max: usize) {
        if self.scroll_offset < max.saturating_sub(1) {
            self.scroll_offset += 1;
        }
    }
}

/// Run the TUI dashboard
fn run_dashboard() -> Result<()> {
    // Initialize terminal
    enable_raw_mode().context("Failed to enable raw mode")?;
    let mut stdout = io::stdout();
    stdout
        .execute(EnterAlternateScreen)
        .context("Failed to enter alternate screen")?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).context("Failed to create terminal")?;

    // Open shared memory reader
    let reader = open_reader()?;
    let mut state = DashboardState::new();

    // Main loop
    let tick_rate = Duration::from_millis(250); // 4 FPS

    let result = run_dashboard_loop(&mut terminal, &reader, &mut state, tick_rate);

    // Restore terminal
    disable_raw_mode().context("Failed to disable raw mode")?;
    terminal
        .backend_mut()
        .execute(LeaveAlternateScreen)
        .context("Failed to leave alternate screen")?;

    result
}

/// Main dashboard loop
fn run_dashboard_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    reader: &SharedVecRtdbReader,
    state: &mut DashboardState,
    tick_rate: Duration,
) -> Result<()> {
    let mut last_tick = Instant::now();

    loop {
        // Refresh data if needed
        refresh_point_data(reader, state);

        // Draw UI
        terminal.draw(|f| draw_dashboard(f, reader, state))?;

        // Handle input with timeout
        let timeout = tick_rate.saturating_sub(last_tick.elapsed());
        if event::poll(timeout).context("Failed to poll events")? {
            if let Event::Key(key) = event::read().context("Failed to read event")? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                        KeyCode::Up | KeyCode::Char('k') => state.scroll_up(),
                        KeyCode::Down | KeyCode::Char('j') => state.scroll_down(state.points.len()),
                        KeyCode::Char('r') => {
                            // Force refresh
                            state.last_instance_count = 0;
                            state.last_channel_count = 0;
                        },
                        _ => {},
                    }
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }
    }
}

/// Refresh point data by scanning shared memory
fn refresh_point_data(reader: &SharedVecRtdbReader, state: &mut DashboardState) {
    let stats = reader.stats();

    // Only rescan if counts changed or enough time has passed
    let should_rescan = stats.instance_count != state.last_instance_count
        || stats.channel_count != state.last_channel_count
        || state.last_scan.elapsed() > Duration::from_secs(5);

    if should_rescan {
        state.points = collect_all_points(reader);
        state.last_instance_count = stats.instance_count;
        state.last_channel_count = stats.channel_count;
        state.last_scan = Instant::now();
    } else {
        // Just update values for existing points
        update_point_values(reader, &mut state.points);
    }
}

/// Collect all points by iterating over registered instances and channels
///
/// Uses the new iteration API which only visits registered points,
/// eliminating the need to blindly scan ID ranges.
fn collect_all_points(reader: &SharedVecRtdbReader) -> Vec<PointRow> {
    let mut rows = Vec::new();

    // Iterate over all registered instances
    for inst_id in reader.instance_ids() {
        // Measurement points
        reader.iter_instance_measurements(inst_id, |point_id, value| {
            rows.push(PointRow {
                key: format!("inst:{}:M:{}", inst_id, point_id),
                kind: "M",
                value,
            });
        });
        // Action points
        reader.iter_instance_actions(inst_id, |point_id, value| {
            rows.push(PointRow {
                key: format!("inst:{}:A:{}", inst_id, point_id),
                kind: "A",
                value,
            });
        });
    }

    // Iterate over all registered channels
    for ch_id in reader.channel_ids() {
        for point_type in [
            PointType::Telemetry,
            PointType::Signal,
            PointType::Control,
            PointType::Adjustment,
        ] {
            reader.iter_channel_points(ch_id, point_type, |point_id, value| {
                rows.push(PointRow {
                    key: format!("ch:{}:{}:{}", ch_id, point_type.as_str(), point_id),
                    kind: point_type.as_str(),
                    value,
                });
            });
        }
    }

    rows
}

/// Update values for existing points (faster than full rescan)
fn update_point_values(reader: &SharedVecRtdbReader, points: &mut [PointRow]) {
    for point in points.iter_mut() {
        if let Ok(key) = parse_key(&point.key) {
            if let Some(value) = get_value(reader, &key) {
                point.value = value;
            }
        }
    }
}

/// Draw the dashboard UI
fn draw_dashboard(f: &mut ratatui::Frame, reader: &SharedVecRtdbReader, state: &DashboardState) {
    let stats = reader.stats();
    let alive = reader.is_writer_alive(5000);
    let heartbeat_age = voltage_rtdb::shared_impl::timestamp_ms() - stats.writer_heartbeat;

    // Layout: status bar + data table
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(5)])
        .split(f.area());

    // Status bar
    let writer_status = if alive {
        format!("● alive ({}ms)", heartbeat_age)
    } else {
        format!("○ dead ({}ms)", heartbeat_age)
    };

    let status_text = format!(
        " Instances: {}  Channels: {}  Points: {}  Writer: {}  │  [q]uit [↑↓]scroll [r]efresh",
        stats.instance_count,
        stats.channel_count,
        state.points.len(),
        writer_status
    );

    let status_style = if alive {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::Red)
    };

    let status = Paragraph::new(status_text).style(status_style).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Voltage Shared Memory Monitor "),
    );
    f.render_widget(status, chunks[0]);

    // Data table
    let header_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);

    let header = Row::new(["Key", "Type", "Value"])
        .style(header_style)
        .height(1);

    let visible_rows: Vec<Row> = state
        .points
        .iter()
        .skip(state.scroll_offset)
        .map(|p| {
            let value_str = format!("{:.6}", p.value);
            Row::new([p.key.clone(), p.kind.to_string(), value_str])
        })
        .collect();

    let widths = [
        Constraint::Length(20),
        Constraint::Length(6),
        Constraint::Min(15),
    ];

    let table = Table::new(visible_rows, widths)
        .header(header)
        .block(Block::default().borders(Borders::ALL).title(format!(
            " Points ({}/{}) ",
            state.scroll_offset + 1,
            state.points.len().max(1)
        )))
        .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    f.render_stateful_widget(table, chunks[1], &mut state.table_state.clone());
}
