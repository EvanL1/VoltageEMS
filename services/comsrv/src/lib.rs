//! Communication Service Library
//!
//! Industrial communication service providing unified interface for various protocols

// Module declarations
pub mod error;
pub mod utils;

pub mod api {
    //! REST API Module
    //!
    //! Provides HTTP API endpoints for the communication service.

    pub mod dto;
    pub mod routes;

    pub mod handlers {
        pub mod channel_handlers;
        pub mod channel_management_handlers;
        pub mod control_handlers;
        pub mod health;
        pub mod mapping_handlers;
        pub mod point_handlers;
    }
}

// Inline module declarations to avoid extra thin shell files
pub mod core {
    pub mod bootstrap;
    pub mod channels;
    pub mod config;
    pub mod reload;
}

pub mod protocols {
    #[cfg(feature = "modbus")]
    pub mod modbus;
    pub mod virt;
}

pub mod runtime {
    //! Runtime Orchestration Layer
    //!
    //! Provides runtime lifecycle management, service orchestration, reconnection mechanisms,
    //! maintenance tasks, and data storage utilities for the communication service.

    pub mod cleanup_provider;
    pub mod lifecycle;
    pub mod reconnect;
    pub mod storage;

    #[cfg(test)]
    pub mod test_utils;

    // Re-export common types
    pub use cleanup_provider::ComsrvCleanupProvider;
    pub use lifecycle::{
        shutdown_handler, shutdown_services, start_cleanup_task, start_communication_service,
        wait_for_shutdown,
    };
    pub use reconnect::{ReconnectContext, ReconnectError, ReconnectHelper, ReconnectPolicy};
    pub use storage::{PluginPointUpdate, StorageManager};
}

// Re-export dto at crate root for compatibility
pub use crate::api::dto;

// Re-export commonly used types
pub use error::{ComSrvError, ErrorExt, Result};
pub use runtime::storage::PluginPointUpdate;

// Re-export core functionality
pub use core::bootstrap::ServiceArgs;
pub use core::channels::ChannelManager;
pub use core::config::ConfigManager;

// Re-export runtime helpers for convenience
pub use runtime::cleanup_provider;
pub use runtime::storage;
pub use runtime::{shutdown_services, wait_for_shutdown};

#[cfg(test)]
pub use runtime::test_utils;
