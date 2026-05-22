//! Pure-infra contract for SHM slot I/O.
//!
//! `SlotIo` is the declaration of what a "business-unaware SHM writer" can do:
//! address slots by index, write/read seqlocked cells, track dirty slots,
//! observe header state. **Nothing on this trait mentions channels, point
//! types, instances, or routing** — that is by design.
//!
//! `UnifiedWriter` (in `unified_shm.rs`) implements `SlotIo` to expose its
//! pure-infra capabilities. Its inherent methods continue to carry the
//! channel/point-type adapters; those adapters are deliberately NOT part of
//! this trait. Any caller that only needs slot-level I/O (snapshot restore,
//! pure-infra tests, future generic tools) should program against `dyn
//! SlotIo` so that the type system rejects business coupling at compile
//! time.
//!
//! This trait is intentionally not object-safe in pursuit of one thing or
//! another — it is plain `&self` so it composes with `Arc<dyn SlotIo>` if
//! callers want dynamic dispatch.

use crate::core::header::UnifiedHeader;
use crate::core::slot::PointSlot;

/// The pure-infra view of a SHM writer/reader: slot-level I/O only.
pub trait SlotIo: Send + Sync {
    /// Number of slots currently live in this SHM.
    fn slot_count(&self) -> usize;

    /// Read-only access to a slot by index. Returns `None` if out of bounds.
    fn slot(&self, index: usize) -> Option<&PointSlot>;

    /// Write a measurement to a slot. Returns `false` if the index is out of
    /// bounds. Implementations must mark the slot as dirty so a subsequent
    /// `take_dirty_slots` will surface it.
    fn write_slot(&self, index: usize, value: f64, raw: f64, timestamp_ms: u64) -> bool;

    /// Drain and return the set of slot indices written since the last drain.
    fn take_dirty_slots(&self) -> Vec<usize>;

    /// Current writer generation. Bumped by the writer on each
    /// create/reconfigure; consumed by readers to detect writer restarts.
    fn generation(&self) -> u64;

    /// Most recent writer heartbeat timestamp (ms since UNIX epoch).
    fn writer_heartbeat(&self) -> u64;

    /// On-mmap header. Implementors that hold a mapped header expose it
    /// here so callers can read magic/version/manifest_hash without going
    /// through any adapter-specific accessor.
    fn header(&self) -> &UnifiedHeader;
}
