//! Modbus TCP Slave Simulator for VoltageEMS CI Testing.
//!
//! This tool simulates industrial devices (PCS, BMS, PV) as Modbus TCP slaves,
//! generating realistic waveform data for testing comsrv.
//!
//! # Usage
//!
//! ```bash
//! # Start with a scenario file
//! simulator --scenario scenarios/pcs_normal.yaml --port 5020
//!
//! # Start with fault injection enabled
//! simulator --scenario scenarios/network_fault.yaml --port 5020
//! ```

mod coils;
mod devices;
mod rtu_server;
mod scenarios;
mod server;
mod writable;

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

/// Modbus TCP/RTU Slave Simulator
#[derive(Parser, Debug)]
#[command(name = "simulator")]
#[command(about = "Modbus TCP/RTU slave simulator for VoltageEMS CI testing")]
struct Args {
    /// Scenario configuration file path
    #[arg(short, long)]
    scenario: PathBuf,

    /// TCP port to listen on (TCP mode only)
    #[arg(short, long, default_value = "5020")]
    port: u16,

    /// Bind address (TCP mode only)
    #[arg(short, long, default_value = "0.0.0.0")]
    bind: String,

    /// RTU serial port (e.g., /dev/ttyUSB0 or /dev/pts/3)
    /// If specified, runs in RTU mode instead of TCP mode
    #[arg(long)]
    rtu: Option<String>,

    /// RTU baud rate (only used with --rtu)
    #[arg(long, default_value = "9600")]
    baud: u32,

    /// Log level (trace, debug, info, warn, error)
    #[arg(short, long, default_value = "info")]
    log_level: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize logging
    let level = match args.log_level.to_lowercase().as_str() {
        "trace" => Level::TRACE,
        "debug" => Level::DEBUG,
        "info" => Level::INFO,
        "warn" => Level::WARN,
        "error" => Level::ERROR,
        _ => Level::INFO,
    };

    FmtSubscriber::builder()
        .with_max_level(level)
        .with_target(false)
        .with_thread_ids(false)
        .init();

    info!("VoltageEMS Modbus Simulator v{}", env!("CARGO_PKG_VERSION"));
    info!("Loading scenario: {:?}", args.scenario);

    // Load scenario configuration
    let scenario = scenarios::load_scenario(&args.scenario)?;
    info!(
        "Scenario '{}' loaded: {} device(s)",
        scenario.name,
        scenario.devices.len()
    );

    // Build device register map
    let device_map = devices::build_device_map(&scenario.devices)?;

    // Start server based on mode
    if let Some(rtu_port) = args.rtu {
        // RTU mode
        info!(
            "Starting Modbus RTU server on {} @ {} baud",
            rtu_port, args.baud
        );
        rtu_server::run_rtu_server(&rtu_port, args.baud, device_map, &scenario.devices).await?;
    } else {
        // TCP mode (default)
        let addr = format!("{}:{}", args.bind, args.port);
        info!("Starting Modbus TCP server on {}", addr);
        server::run_server(&addr, device_map, scenario.faults, &scenario.devices).await?;
    }

    Ok(())
}
