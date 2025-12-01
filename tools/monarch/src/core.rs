//! Monarch Core - Configuration management core functionality
//!
//! This module provides the core functionality for managing service configurations
//! in the VoltageEMS system. It supports both read-only and read-write access modes
//! and handles the synchronization between YAML/CSV files and SQLite databases.

use anyhow::{Context, Result};
use sqlx::SqlitePool;
use std::path::{Path, PathBuf};
use tracing::info;

// Module declarations
pub mod exporter;
pub mod file_utils;
pub mod schema;
pub mod syncer;
pub mod validator;

// Re-export key types
pub use exporter::{ConfigExporter, ExportResult};
pub use syncer::{ConfigSyncer, SyncResult};
pub use validator::ConfigValidator;
pub use voltage_config::ValidationResult;

/// Access mode for the Monarch core
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AccessMode {
    /// Read-write mode (for management tools)
    ReadWrite,
}

/// Monarch core configuration management
pub struct MonarchCore {
    /// Database path
    db_path: PathBuf,
    /// Configuration files path
    config_path: PathBuf,
    /// Access mode
    mode: AccessMode,
    /// Database connection pool
    pool: Option<SqlitePool>,
}

impl MonarchCore {
    /// Create a read-write instance (for management tools)
    pub async fn readwrite(
        db_path: impl AsRef<Path>,
        config_path: impl AsRef<Path>,
        service: &str,
    ) -> Result<Self> {
        let config_path = config_path.as_ref().to_path_buf();
        let (db_dir, db_file, _explicit_file) = normalise_db_path(db_path.as_ref(), service);

        // Ensure database directory exists
        if let Some(parent) = db_file.parent() {
            std::fs::create_dir_all(parent).context("Failed to create database directory")?;
        }

        let pool = SqlitePool::connect(&format!("sqlite://{}?mode=rwc", db_file.display()))
            .await
            .context("Failed to connect to database in read-write mode")?;

        info!("Connected to database in read-write mode: {:?}", db_file);

        Ok(Self {
            db_path: db_dir,
            config_path,
            mode: AccessMode::ReadWrite,
            pool: Some(pool),
        })
    }

    /// Create an instance without connecting to database (for initialization)
    pub fn new(config_path: impl AsRef<Path>) -> Self {
        Self {
            db_path: PathBuf::from("data"),
            config_path: config_path.as_ref().to_path_buf(),
            mode: AccessMode::ReadWrite,
            pool: None,
        }
    }

    /// Validate configuration for a service
    pub async fn validate(&self, service: &str) -> Result<ValidationResult> {
        let validator = ConfigValidator::new(&self.config_path);
        validator.validate_service(service).await
    }

    /// Sync configuration from files to database
    pub async fn sync(&self, service: &str) -> Result<SyncResult> {
        self.require_write_mode()?;

        let syncer = ConfigSyncer::new(&self.config_path, &self.db_path);
        syncer.sync_service(service).await
    }

    /// Export configuration from database to files
    pub async fn export(
        &self,
        service: &str,
        output_dir: impl AsRef<Path>,
    ) -> Result<ExportResult> {
        self.require_write_mode()?;

        let pool = self
            .pool
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Database not connected"))?;

        let exporter = ConfigExporter::new(pool.clone());
        exporter.export_service(service, output_dir).await
    }

    // Private helper methods

    fn require_write_mode(&self) -> Result<()> {
        if self.mode != AccessMode::ReadWrite {
            Err(anyhow::anyhow!("Operation requires write mode"))
        } else {
            Ok(())
        }
    }
}

/// Normalise an input path into a database directory and concrete database file path.
///
/// NEW BEHAVIOR (Unified Database Architecture):
/// - All services (comsrv, modsrv, rules, global) use the unified `voltage.db`
/// - Only when an explicit database file is provided (not a directory), use that file
/// - This supports both the new unified architecture and legacy single-service mode
fn normalise_db_path(input: &Path, _service: &str) -> (PathBuf, PathBuf, bool) {
    if input.is_dir() {
        let dir = input.to_path_buf();
        // Use unified database for all services
        let file = dir.join("voltage.db");
        (dir, file, false)
    } else {
        let file = input.to_path_buf();
        let dir = input
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
        (dir, file, true)
    }
}
