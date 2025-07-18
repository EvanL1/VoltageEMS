//! ComBase架构重构集成测试模块

pub mod combase_integration_test;

// 重新导出测试函数，方便外部调用
pub use combase_integration_test::{
    combase_basic_test,
    four_telemetry_test, 
    storage_pubsub_test,
    command_subscription_test,
};