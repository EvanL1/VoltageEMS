//! Pure SHM infrastructure: slot storage + bitmap allocator.
//!
//! This module is the **infra boundary**: types here MUST NOT depend on
//! business concepts (channel, instance, point type, routing, action dispatch).
//! Anything under `core::` must be promotable to a standalone `voltage-shm-slots`
//! crate without dragging `voltage-model` / `voltage-routing` along.
//!
//! Business-aware code lives at the parent module (channel_index, instance_index,
//! reverse_index, dispatch, notifier, etc.) and consumes `core` as an adapter.

pub mod bitmap;
pub mod config;
pub mod header;
pub mod slot;
pub mod slot_io;
pub mod writer;
