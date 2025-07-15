//! 测试模块
//! 
//! 包含单元测试、集成测试和测试工具

#[cfg(test)]
pub mod batch_writer_test;

#[cfg(test)]
pub mod redis_subscriber_test;

#[cfg(test)]
pub mod query_optimizer_test;

#[cfg(test)]
pub mod retention_policy_test;

#[cfg(test)]
pub mod integration_test;

#[cfg(test)]
pub mod api_test;

#[cfg(test)]
pub mod test_utils;

#[cfg(test)]
pub mod mock_storage;

#[cfg(test)]
pub mod unit;