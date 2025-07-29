//! gRPC 插件支持模块
//!
//! 提供通过 gRPC 与外部协议插件通信的能力

pub mod adapter;
pub mod client;
pub mod manager;

// 重新导出主要类型
pub use adapter::GrpcPluginAdapter;
pub use client::GrpcPluginClient;
pub use manager::PluginManager;

// protobuf 生成的代码
#[allow(clippy::all)]
pub mod proto {
    include!("proto/comsrv.plugin.v1.rs");
}
