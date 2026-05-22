//! VoltageEMS Shared Memory Subsystem
//!
//! Provides shared memory (SHM) and IPC components for zero-latency
//! cross-process data sharing between comsrv and modsrv containers.
//!
//! # Key Components
//!
//! - **vec_impl**: Vector-based point storage with atomic PointSlot
//! - **slot_bitmap**: Dynamic slot allocation bitmap
//! - **notification**: M2C command notification struct
//! - **notifier**: UDS event notification for M2C dispatch
//! - **instance_index**: Dynamic instance management with shared slots
//! - **unified_shm**: Unified shared memory (Header + PointSlots)
//! - **channel_index**: Dynamic channel management with ArcSwap
//! - **snapshot**: Periodic SHM snapshot management
//! - **shared_config**: SharedConfig, ChannelToSlotIndex, utility functions

pub mod channel_points;

pub mod vec_impl;

pub mod slot_bitmap;

pub mod notification;

#[cfg(unix)]
pub mod notifier;

pub mod instance_index;

pub mod unified_shm;

pub mod channel_index;

pub mod snapshot;

pub mod shared_config;

pub mod reverse_index;

pub mod shm_handle;

pub mod batch_direct;

#[cfg(unix)]
pub mod dispatch;

// Re-exports for convenience
pub use instance_index::{DynamicInstanceLayout, InstanceIndex, SharedSlotRef};
pub use notification::ShmNotification;
#[cfg(unix)]
pub use notifier::{DEFAULT_UDS_PATH, NotifyResult, ShmNotifier};
pub use slot_bitmap::{BitmapStats, SlotAllocation, SlotBitmap, SlotBitmapHeader};
pub use vec_impl::PointSlot;

// Channel point counts (routing-independent SHM layout data source)
pub use channel_points::ChannelPointCounts;

// Unified shared memory
pub use unified_shm::{
    ChannelLayout, UNIFIED_MAGIC, UNIFIED_VERSION, UnifiedHeader, UnifiedReader, UnifiedWriter,
    allocate_layouts, calculate_file_size,
};

// Channel index for dynamic channel management
pub use channel_index::{ChannelIndex, DynamicChannelLayout};

// Snapshot management
pub use snapshot::{SnapshotConfig, SnapshotManager, snapshot_exists};

// Shared memory configuration and utilities
pub use shared_config::{
    ChannelToSlotIndex, DEFAULT_SHM_PATH, SHARED_MAGIC, SharedConfig, default_shm_path,
    is_shm_available, timestamp_ms,
};

// Reverse slot index: slot → (channel_id, point_type, point_id)
pub use reverse_index::{ReverseSlotIndex, SlotOrigin};

// Runtime-swappable SHM handle
pub use shm_handle::{ShmHandle, ShmLayout};

// Direct SHM batch write (bridges SHM + routing + RTDB)
pub use batch_direct::write_channel_batch_direct;

// SHM/UDS action dispatch — shared by modsrv HTTP path and rules executor
#[cfg(unix)]
pub use dispatch::{ActionDispatch, DispatchOutcome, NoopDispatch, ShmDispatch};
