//! Service Layer
//!
//! 提供serving生命periodmanaging、reconnection机制和maintainingtask

pub mod lifecycle;
pub mod reconnect;

// 重新export常用type
pub use lifecycle::{shutdown_handler, start_cleanup_task, start_communication_service};
pub use reconnect::{ReconnectError, ReconnectHelper, ReconnectPolicy, ReconnectState};
