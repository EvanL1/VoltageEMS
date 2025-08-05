//! gRPC pluginsupportingmodular
//!
//! 提供通过 gRPC 与exteriorprotocolplugincommunicate的capability

pub mod adapter;
pub mod client;
pub mod manager;

// 重新exportmaster要type
pub use adapter::GrpcPluginAdapter;
pub use client::GrpcPluginClient;
pub use manager::PluginManager;

// protobuf 生成的代码
#[allow(clippy::all)]
pub mod proto {
    include!("proto/comsrv.plugin.v1.rs");
}
