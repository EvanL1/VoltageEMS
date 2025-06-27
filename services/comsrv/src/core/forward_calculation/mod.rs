/// 转发计算模块 - Forward Calculation Module
/// 
/// 该模块提供虚拟通道的转发计算功能，支持：
/// - 四遥类型+点位的精确定义
/// - 四则运算和逻辑运算
/// - 灵活的配置管理
/// - 实时计算执行

// 计算执行引擎
pub mod calculator;
// 计算执行器
// TODO: Implement executor module
// pub mod executor;

// 从config模块重新导出配置相关类型
// 重新导出计算引擎
// TODO: Re-export executor types when implemented
// pub use executor::{ForwardCalculationExecutor, DataSource, DataStore, ExecutionStats, MockDataSource, MockDataStore}; 