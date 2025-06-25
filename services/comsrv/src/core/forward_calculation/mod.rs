/// 转发计算模块 - Forward Calculation Module
/// 
/// 该模块提供虚拟通道的转发计算功能，支持：
/// - 四遥类型+点位的精确定义
/// - 四则运算和逻辑运算
/// - 灵活的配置管理
/// - 实时计算执行

pub mod config;

#[cfg(test)]
mod demo_tests;

// 重新导出主要的公共接口
pub use config::*; 