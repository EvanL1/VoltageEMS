//! SHM/UDS Action Dispatch — re-exported from voltage-rtdb-shm.
//!
//! The trait and ShmDispatch implementation live in voltage-rtdb-shm so the
//! rules executor (libs/voltage-rules) can use the same generation-checked
//! writer as modsrv's HTTP /control path. Without sharing, the rules
//! executor previously held its own raw UnifiedWriter that silently kept
//! writing to stale slots after a comsrv restart.

pub use voltage_rtdb_shm::{ActionDispatch, DispatchOutcome, NoopDispatch, ShmDispatch};
