//! gRPC Plugin Support Module
//!
//! Provides capability to communicate with external protocol plugins through gRPC

pub mod adapter;
pub mod client;
pub mod manager;

// Re-export main types
pub use adapter::GrpcPluginAdapter;
pub use client::GrpcPluginClient;
pub use manager::PluginManager;

// protobuf generated code
#[allow(clippy::all)]
pub mod proto {
    include!("proto/comsrv.plugin.rs");
}
