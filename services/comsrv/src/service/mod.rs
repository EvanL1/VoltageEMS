//! Service Layer
//!
//! 提供服务生命周期管理、重连机制和维护任务

pub mod lifecycle;
pub mod reconnect;

// 重新导出常用类型
pub use lifecycle::{shutdown_handler, start_cleanup_task, start_communication_service};
pub use reconnect::{ReconnectError, ReconnectHelper, ReconnectPolicy, ReconnectState};
