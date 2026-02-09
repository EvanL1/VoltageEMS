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

pub mod vec_impl;

pub mod slot_bitmap;

pub mod notification;

pub mod ring_buffer;

pub mod notifier;

pub mod instance_index;

// Re-exports for convenience
pub use instance_index::{DynamicInstanceLayout, InstanceIndex, SharedSlotRef};
pub use notification::ShmNotification;
pub use notifier::{NotifyResult, ShmNotifier, UdsHealth, DEFAULT_UDS_PATH};
pub use ring_buffer::{
    current_timestamp_us, DataPoint, HighFreqRingBuffer, RingBufferConfig, SharedRingBuffer,
};
pub use slot_bitmap::{BitmapStats, SlotAllocation, SlotBitmap, SlotBitmapHeader};
pub use vec_impl::PointSlot;
