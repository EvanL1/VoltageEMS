//! # voltage-config
//!
//! Cross-platform configuration schema for VoltageEMS services.
//!
//! This crate contains the YAML / SQLite-backed configuration structs
//! consumed by `comsrv`, `modsrv`, and the `monarch` management CLI.
//! It is deliberately **platform-agnostic** (no SHM, UDS, or service
//! runtime code) so that `monarch.exe` can be built on Windows without
//! pulling in the Linux-only service runtimes.
//!
//! ## Layout
//!
//! - [`comsrv`] — `ComsrvConfig`, `ChannelConfig`, point definitions
//! - [`modsrv`] — `ModsrvConfig`, `RulesConfig`, rule metadata
//!
//! Each service crate re-exports from this crate to preserve existing
//! internal paths (`comsrv::core::config::*`, `modsrv::config::*`).

pub mod comsrv;
pub mod modsrv;
