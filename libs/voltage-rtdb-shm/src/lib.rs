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
//! - **ring_buffer**: High-frequency data recording
//! - **notifier**: UDS event notification for M2C dispatch
//! - **instance_index**: Dynamic instance management with shared slots
//! - **unified_shm**: Unified shared memory (Header + PointSlots)
//! - **channel_index**: Dynamic channel management with ArcSwap
//! - **snapshot**: Periodic SHM snapshot management
//! - **shared_config**: SharedConfig, ChannelToSlotIndex, utility functions

pub mod vec_impl;

pub mod slot_bitmap;

pub mod notification;

pub mod ring_buffer;

pub mod notifier;

pub mod instance_index;

pub mod unified_shm;

pub mod channel_index;

pub mod snapshot;

pub mod shared_config;

pub mod batch_direct;

// Re-exports for convenience
pub use instance_index::{DynamicInstanceLayout, InstanceIndex, SharedSlotRef};
pub use notification::ShmNotification;
pub use notifier::{NotifyResult, ShmNotifier, UdsHealth, DEFAULT_UDS_PATH};
pub use ring_buffer::{
    current_timestamp_us, DataPoint, HighFreqRingBuffer, RingBufferConfig, SharedRingBuffer,
};
pub use slot_bitmap::{BitmapStats, SlotAllocation, SlotBitmap, SlotBitmapHeader};
pub use vec_impl::PointSlot;

// Unified shared memory
pub use unified_shm::{
    allocate_layouts, calculate_file_size, ChannelLayout, UnifiedHeader, UnifiedReader,
    UnifiedWriter, UNIFIED_MAGIC, UNIFIED_VERSION,
};

// Channel index for dynamic channel management
pub use channel_index::{ChannelIndex, DynamicChannelLayout};

// Snapshot management
pub use snapshot::{snapshot_exists, snapshot_info, SnapshotConfig, SnapshotInfo, SnapshotManager};

// Shared memory configuration and utilities
pub use shared_config::{
    default_shm_path, is_shm_available, timestamp_ms, ChannelToSlotIndex, SharedConfig,
    DEFAULT_SHM_PATH, SHARED_MAGIC,
};

// Direct SHM batch write (bridges SHM + routing + RTDB)
pub use batch_direct::write_channel_batch_direct;
