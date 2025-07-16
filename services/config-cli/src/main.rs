use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::Colorize;
use std::path::PathBuf;

mod commands;
mod utils;

use commands::{validate, generate, migrate, show, diff, export};

/// VoltageEMS Configuration Management CLI
#[derive(Parser)]
#[command(name = "voltage-config")]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Verbose output
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Subcommands
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Validate a configuration file
    Validate {
        /// Service name (alarmsrv, hissrv, comsrv, modsrv, netsrv)
        #[arg(short, long)]
        service: String,

        /// Configuration file path
        #[arg(short, long)]
        file: PathBuf,

        /// Show warnings in addition to errors
        #[arg(short, long)]
        warnings: bool,
    },

    /// Generate a default configuration file
    Generate {
        /// Service name (alarmsrv, hissrv, comsrv, modsrv, netsrv)
        #[arg(short, long)]
        service: String,

        /// Output file path
        #[arg(short, long)]
        output: PathBuf,

        /// Configuration format (yaml, toml, json)
        #[arg(short, long, default_value = "yaml")]
        format: String,

        /// Include comments and documentation
        #[arg(short, long)]
        comments: bool,
    },

    /// Migrate configuration from old format to new format
    Migrate {
        /// Source configuration file
        #[arg(short, long)]
        from: PathBuf,

        /// Target configuration file
        #[arg(short, long)]
        to: PathBuf,

        /// Service name (alarmsrv, hissrv, comsrv, modsrv, netsrv)
        #[arg(short, long)]
        service: String,

        /// Create backup of source file
        #[arg(short, long)]
        backup: bool,

        /// Dry run (don't write files)
        #[arg(short, long)]
        dry_run: bool,
    },

    /// Show configuration in a readable format
    Show {
        /// Service name (alarmsrv, hissrv, comsrv, modsrv, netsrv)
        #[arg(short, long)]
        service: String,

        /// Configuration file path
        #[arg(short, long)]
        file: Option<PathBuf>,

        /// Output format (yaml, json, toml, table)
        #[arg(short, long, default_value = "yaml")]
        format: String,

        /// Show only specific section
        #[arg(long)]
        section: Option<String>,
    },

    /// Compare two configuration files
    Diff {
        /// First configuration file
        file1: PathBuf,

        /// Second configuration file
        file2: PathBuf,

        /// Output format (unified, context, side-by-side)
        #[arg(short, long, default_value = "unified")]
        format: String,

        /// Ignore whitespace changes
        #[arg(short, long)]
        ignore_whitespace: bool,
    },

    /// Export configuration as environment variables
    Export {
        /// Service name (alarmsrv, hissrv, comsrv, modsrv, netsrv)
        #[arg(short, long)]
        service: String,

        /// Configuration file path
        #[arg(short, long)]
        file: Option<PathBuf>,

        /// Environment file output path
        #[arg(short, long)]
        env_file: PathBuf,

        /// Variable prefix
        #[arg(short, long)]
        prefix: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let log_level = if cli.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(log_level)
        .init();

    // Print header
    println!("{}", "VoltageEMS Configuration Management Tool".bold().cyan());
    println!("{}", "==========================================".cyan());
    println!();

    // Execute command
    let result = match cli.command {
        Commands::Validate { service, file, warnings } => {
            validate::execute(&service, &file, warnings).await
        }
        
        Commands::Generate { service, output, format, comments } => {
            generate::execute(&service, &output, &format, comments).await
        }
        
        Commands::Migrate { from, to, service, backup, dry_run } => {
            migrate::execute(&from, &to, &service, backup, dry_run).await
        }
        
        Commands::Show { service, file, format, section } => {
            show::execute(&service, file.as_ref(), &format, section.as_ref()).await
        }
        
        Commands::Diff { file1, file2, format, ignore_whitespace } => {
            diff::execute(&file1, &file2, &format, ignore_whitespace).await
        }
        
        Commands::Export { service, file, env_file, prefix } => {
            export::execute(&service, file.as_ref(), &env_file, prefix.as_ref()).await
        }
    };

    // Handle result
    match result {
        Ok(_) => {
            println!();
            println!("{}", "✅ Operation completed successfully!".green());
            Ok(())
        }
        Err(e) => {
            println!();
            eprintln!("{} {}", "❌ Error:".red(), e);
            std::process::exit(1);
        }
    }
}