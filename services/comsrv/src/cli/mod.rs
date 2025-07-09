//! Protocol Development CLI Tools
//!
//! This module provides command-line tools for protocol plugin development,
//! including scaffolding, testing, and management utilities.

use clap::{Parser, Subcommand};
use std::path::PathBuf;

use crate::utils::Result;

pub mod commands;
pub mod template_generator;
pub mod test_framework;
pub mod config_migration;

/// Protocol development CLI
#[derive(Parser)]
#[command(name = "comsrv-cli")]
#[command(about = "Protocol development tools for VoltageEMS communication service")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

/// Available CLI commands
#[derive(Subcommand)]
pub enum Commands {
    /// Create a new protocol plugin
    New {
        /// Protocol name (e.g., "modbus_tcp")
        name: String,
        /// Output directory
        #[arg(short, long, default_value = ".")]
        output: PathBuf,
        /// Use a specific template
        #[arg(short, long)]
        template: Option<String>,
    },
    
    /// List available protocol plugins
    List {
        /// Show detailed information
        #[arg(short, long)]
        verbose: bool,
    },
    
    /// Generate configuration for a protocol
    Config {
        /// Protocol ID
        protocol: String,
        /// Output format
        #[arg(short, long, value_enum, default_value = "yaml")]
        format: ConfigFormat,
        /// Output file (stdout if not specified)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    
    /// Validate a configuration file
    Validate {
        /// Configuration file to validate
        config: PathBuf,
        /// Protocol ID
        #[arg(short, long)]
        protocol: Option<String>,
    },
    
    /// Test a protocol implementation
    Test {
        /// Protocol ID or plugin path
        protocol: String,
        /// Test configuration file
        #[arg(short, long)]
        config: Option<PathBuf>,
        /// Run specific test
        #[arg(short, long)]
        test: Option<String>,
    },
    
    /// Generate protocol documentation
    Docs {
        /// Protocol ID
        protocol: String,
        /// Output directory
        #[arg(short, long, default_value = "./docs")]
        output: PathBuf,
    },
    
    /// Start protocol simulator
    Simulate {
        /// Protocol ID
        protocol: String,
        /// Simulator configuration
        #[arg(short, long)]
        config: Option<PathBuf>,
        /// Bind address
        #[arg(short, long, default_value = "127.0.0.1:8080")]
        bind: String,
    },
    
    /// Analyze protocol performance
    Analyze {
        /// Log file or capture file
        input: PathBuf,
        /// Protocol ID
        #[arg(short, long)]
        protocol: Option<String>,
        /// Output report file
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}

/// Configuration output format
#[derive(clap::ValueEnum, Clone, Debug)]
pub enum ConfigFormat {
    Yaml,
    Json,
    Toml,
}

/// Execute CLI command
pub async fn execute(cli: Cli) -> Result<()> {
    match cli.command {
        Commands::New { name, output, template } => {
            commands::new_protocol(&name, &output, template.as_deref()).await
        }
        Commands::List { verbose } => {
            commands::list_protocols(verbose).await
        }
        Commands::Config { protocol, format, output } => {
            commands::generate_config(&protocol, format, output.as_deref()).await
        }
        Commands::Validate { config, protocol } => {
            commands::validate_config(&config, protocol.as_deref()).await
        }
        Commands::Test { protocol, config, test } => {
            commands::test_protocol(&protocol, config.as_deref(), test.as_deref()).await
        }
        Commands::Docs { protocol, output } => {
            commands::generate_docs(&protocol, &output).await
        }
        Commands::Simulate { protocol, config, bind } => {
            commands::simulate_protocol(&protocol, config.as_deref(), &bind).await
        }
        Commands::Analyze { input, protocol, output } => {
            commands::analyze_protocol(&input, protocol.as_deref(), output.as_deref()).await
        }
    }
}