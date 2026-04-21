//! Comsrv service configuration types (re-exported from voltage-config).
//!
//! Config schema lives in the platform-agnostic `voltage-config` crate so that
//! `monarch.exe` can be built on Windows without pulling in Linux-only
//! service runtime code (SHM, UDS).

pub use voltage_config::comsrv::*;
