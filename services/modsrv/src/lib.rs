//! # ModSrv - 模型服务
//!
//! 简洁高效的工业IoT模型服务，提供设备模型管理、数据订阅和控制接口。
//!
//! ## 核心功能
//!
//! 1. **配置读取**: 从配置文件加载模型定义，在Redis中初始化
//! 2. **数据同步**: 通过Lua脚本实现与ComsRv的双向数据同步
//! 3. **控制接口**: 提供HTTP API接口，处理外部控制命令
//!
//! ## 架构设计
//!
//! ```text
//! 配置加载 → 模型初始化 → Lua同步 → API接口
//!    ↓           ↓           ↓         ↓
//! config.rs → model.rs → EdgeRedis → api.rs
//! ```
//!
//! ## 基本使用
//!
//! ### 配置模型
//!
//! ```json
//! {
//!   "id": "power_meter",
//!   "name": "电力仪表",
//!   "description": "智能电表监控",
//!   "monitoring": {
//!     "voltage": {
//!       "description": "电压",
//!       "unit": "V"
//!     },
//!     "current": {
//!       "description": "电流",
//!       "unit": "A"
//!     }
//!   },
//!   "control": {
//!     "switch": {
//!       "description": "主开关"
//!     },
//!     "limit": {
//!       "description": "功率限制",
//!       "unit": "kW"
//!     }
//!   }
//! }
//! ```
//!
//! ### 启动服务
//!
//! ```bash
//! # 运行服务
//! modsrv service
//!
//! # 查看模型信息
//! modsrv info
//!
//! # 检查配置
//! modsrv check-config
//! ```
//!
//! ### API接口
//!
//! ```bash
//! # 健康检查
//! GET /health
//!
//! # 获取模型列表
//! GET /models
//!
//! # 获取模型实时数据
//! GET /models/{model_id}/values
//!
//! # 执行控制命令
//! POST /models/{model_id}/control/{control_name}
//! {"value": 1.0}
//!
//! # WebSocket连接
//! WS /ws/{model_id}
//! ```

#![allow(dead_code)]
#![allow(unused_imports)]

/// 配置管理模块
///
/// 提供配置文件加载、环境变量处理和配置验证功能
pub mod config;

/// 错误处理模块
///
/// 定义统一的错误类型和结果处理
pub mod error;

/// 核心模型模块
///
/// 包含模型定义、数据读取、控制命令处理等核心功能
pub mod model;

/// 点位映射管理模块
///
/// 处理ModSrv与底层comsrv的映射关系
pub mod mapping;

/// WebSocket实时推送模块
///
/// 提供WebSocket连接管理和实时数据推送
pub mod websocket;

/// REST API模块
///
/// 提供HTTP接口用于模型管理和控制操作
pub mod api;

// 重新导出常用类型
pub use api::ApiServer;
pub use config::Config;
pub use error::{ModelSrvError, Result};
pub use mapping::{MappingManager, PointMapping};
pub use model::{Model, ModelConfig, ModelManager, PointConfig};
pub use websocket::{ws_handler, WsConnectionManager};

/// 服务版本信息
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// 服务名称
pub const SERVICE_NAME: &str = "modsrv";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_info() {
        // VERSION是编译时常量，总是有值
        assert_eq!(VERSION, "2.0.0");
        assert_eq!(SERVICE_NAME, "modsrv");
    }
}
