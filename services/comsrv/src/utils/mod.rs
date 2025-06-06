//! Utility Functions and Common Components
//! 
//! This module provides essential utilities, error handling, logging, and shared
//! components used throughout the communication service library.
//! 
//! # Modules
//! 
//! - [`error`] - Comprehensive error types and error handling utilities
//! - [`logger`] - Structured logging configuration and management
//! - [`pool`] - Object and buffer pooling for memory efficiency
//! 
//! # Key Components
//! 
//! ## Error Handling
//! 
//! The [`ComSrvError`] enum provides comprehensive error classification for all
//! possible error conditions in the system. The [`ErrorExt`] trait adds convenient
//! error conversion methods to `Result` types.
//! 
//! ## Object Pooling
//! 
//! Object pools help reduce memory allocation overhead for frequently used objects
//! like buffers and temporary data structures.
//! 
//! # Examples
//! 
//! ```rust
//! use comsrv::utils::{ComSrvError, Result, ErrorExt};
//! 
//! fn example_function() -> Result<String> {
//!     std::fs::read_to_string("config.yaml")
//!         .config_error("Failed to read configuration file")
//! }
//! ```

pub mod error;
pub mod logger;
pub mod pool;

pub use error::{ComSrvError, Result, ErrorExt};
pub use pool::{ObjectPool, BufferPool, get_global_buffer_pool};
