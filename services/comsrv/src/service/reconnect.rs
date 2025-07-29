//! 重连机制实现
//!
//! 提供通用的重连助手，支持指数退避和抖动

use rand::Rng;
use std::future::Future;
use std::time::{Duration, Instant};
use thiserror::Error;
use tracing::{debug, info, warn};

/// 重连错误
#[derive(Error, Debug)]
pub enum ReconnectError {
    /// 达到最大重试次数
    #[error("Maximum reconnection attempts exceeded")]
    MaxAttemptsExceeded,

    /// 连接失败
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    /// 重连被取消
    #[error("Reconnection cancelled")]
    Cancelled,
}

/// 连接状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// 已连接
    Connected,
    /// 断开连接
    Disconnected,
    /// 正在重连
    Reconnecting,
    /// 重连失败（达到最大次数）
    Failed,
}

/// 重连策略配置
#[derive(Debug, Clone)]
pub struct ReconnectPolicy {
    /// 最大重试次数（0 表示无限）
    pub max_attempts: u32,
    /// 初始延迟
    pub initial_delay: Duration,
    /// 最大延迟
    pub max_delay: Duration,
    /// 退避倍数
    pub backoff_multiplier: f64,
    /// 是否添加抖动
    pub jitter: bool,
}

impl Default for ReconnectPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(60),
            backoff_multiplier: 2.0,
            jitter: true,
        }
    }
}

impl ReconnectPolicy {
    /// 从配置值创建
    pub fn from_config(
        max_attempts: u32,
        initial_delay_ms: u64,
        max_delay_ms: u64,
        backoff_multiplier: f64,
    ) -> Self {
        Self {
            max_attempts,
            initial_delay: Duration::from_millis(initial_delay_ms),
            max_delay: Duration::from_millis(max_delay_ms),
            backoff_multiplier,
            jitter: true,
        }
    }
}

/// 重连状态
#[derive(Debug, Clone)]
pub struct ReconnectState {
    /// 当前重试次数
    pub current_attempt: u32,
    /// 上次重试时间
    pub last_attempt: Option<Instant>,
    /// 下次重试时间
    pub next_attempt: Option<Instant>,
    /// 连接状态
    pub connection_state: ConnectionState,
}

impl Default for ReconnectState {
    fn default() -> Self {
        Self {
            current_attempt: 0,
            last_attempt: None,
            next_attempt: None,
            connection_state: ConnectionState::Disconnected,
        }
    }
}

/// 重连统计信息
#[derive(Debug, Default, Clone)]
pub struct ReconnectStats {
    /// 总重连次数
    pub total_attempts: u64,
    /// 成功重连次数
    pub successful_reconnects: u64,
    /// 失败重连次数
    pub failed_reconnects: u64,
    /// 最后成功连接时间
    pub last_connected: Option<Instant>,
    /// 连接开始时间
    pub connection_start: Option<Instant>,
}

/// 通用重连助手
#[derive(Debug)]
pub struct ReconnectHelper {
    /// 重连策略
    policy: ReconnectPolicy,
    /// 当前状态
    state: ReconnectState,
    /// 统计信息
    stats: ReconnectStats,
}

impl ReconnectHelper {
    /// 创建新的重连助手
    pub fn new(policy: ReconnectPolicy) -> Self {
        Self {
            policy,
            state: ReconnectState::default(),
            stats: ReconnectStats::default(),
        }
    }

    /// 获取当前连接状态
    pub fn connection_state(&self) -> ConnectionState {
        self.state.connection_state
    }

    /// 获取统计信息
    pub fn stats(&self) -> &ReconnectStats {
        &self.stats
    }

    /// 重置重连状态
    pub fn reset(&mut self) {
        self.state.current_attempt = 0;
        self.state.last_attempt = None;
        self.state.next_attempt = None;
        if self.state.connection_state != ConnectionState::Connected {
            self.state.connection_state = ConnectionState::Disconnected;
        }
    }

    /// 标记连接成功
    pub fn mark_connected(&mut self) {
        self.state.connection_state = ConnectionState::Connected;
        self.state.current_attempt = 0;
        self.stats.last_connected = Some(Instant::now());
        self.stats.connection_start = Some(Instant::now());
        debug!("Connection marked as successful");
    }

    /// 标记连接断开
    pub fn mark_disconnected(&mut self) {
        self.state.connection_state = ConnectionState::Disconnected;
        self.stats.connection_start = None;
        debug!("Connection marked as disconnected");
    }

    /// 计算下次重试延迟
    pub fn calculate_next_delay(&self) -> Duration {
        let attempt = self.state.current_attempt.saturating_sub(1);
        let base_delay = self.policy.initial_delay;
        let multiplier = self.policy.backoff_multiplier;

        // 指数退避：delay = initial_delay * (multiplier ^ attempt)
        let mut delay = base_delay.mul_f64(multiplier.powi(attempt as i32));

        // 限制最大延迟
        if delay > self.policy.max_delay {
            delay = self.policy.max_delay;
        }

        // 添加抖动（±25%）
        if self.policy.jitter {
            let jitter_range = delay.as_millis() as f64 * 0.25;
            let jitter = rand::thread_rng().gen_range(-jitter_range..jitter_range);
            let delay_ms = (delay.as_millis() as f64 + jitter).max(0.0);
            delay = Duration::from_millis(delay_ms as u64);
        }

        delay
    }

    /// 执行重连
    pub async fn execute_reconnect<F, Fut, E>(
        &mut self,
        mut connect_fn: F,
    ) -> Result<(), ReconnectError>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = Result<(), E>>,
        E: std::fmt::Display,
    {
        // 检查是否已达到最大重试次数
        if self.policy.max_attempts > 0 && self.state.current_attempt >= self.policy.max_attempts {
            self.state.connection_state = ConnectionState::Failed;
            warn!(
                "Maximum reconnection attempts ({}) exceeded",
                self.policy.max_attempts
            );
            return Err(ReconnectError::MaxAttemptsExceeded);
        }

        // 更新状态
        self.state.connection_state = ConnectionState::Reconnecting;
        self.state.current_attempt += 1;
        self.stats.total_attempts += 1;

        info!(
            "Starting reconnection attempt {}/{}",
            self.state.current_attempt,
            if self.policy.max_attempts == 0 {
                "∞".to_string()
            } else {
                self.policy.max_attempts.to_string()
            }
        );

        // 如果不是第一次尝试，计算并等待延迟
        if self.state.current_attempt > 1 {
            let delay = self.calculate_next_delay();
            info!("Waiting {:?} before reconnection attempt", delay);
            tokio::time::sleep(delay).await;
        }

        // 记录尝试时间
        let start_time = Instant::now();
        self.state.last_attempt = Some(start_time);

        // 尝试连接
        match connect_fn().await {
            Ok(()) => {
                // 连接成功
                let reconnect_time = start_time.elapsed();
                info!(
                    "Reconnection successful after {:?} (attempt {})",
                    reconnect_time, self.state.current_attempt
                );

                self.mark_connected();
                self.stats.successful_reconnects += 1;

                Ok(())
            }
            Err(e) => {
                // 连接失败
                warn!(
                    "Reconnection attempt {} failed: {}",
                    self.state.current_attempt, e
                );

                self.stats.failed_reconnects += 1;

                // 如果还有重试机会，保持 Reconnecting 状态
                if self.policy.max_attempts == 0
                    || self.state.current_attempt < self.policy.max_attempts
                {
                    self.state.connection_state = ConnectionState::Disconnected;
                } else {
                    self.state.connection_state = ConnectionState::Failed;
                }

                Err(ReconnectError::ConnectionFailed(e.to_string()))
            }
        }
    }

    /// 获取下次重试延迟（用于显示）
    pub fn next_delay(&self) -> Option<Duration> {
        if self.state.connection_state == ConnectionState::Failed {
            return None;
        }

        if self.policy.max_attempts > 0 && self.state.current_attempt >= self.policy.max_attempts {
            return None;
        }

        Some(self.calculate_next_delay())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_exponential_backoff() {
        let policy = ReconnectPolicy {
            max_attempts: 5,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(10),
            backoff_multiplier: 2.0,
            jitter: false,
        };

        let mut helper = ReconnectHelper::new(policy);

        // 第一次尝试没有延迟
        assert_eq!(helper.state.current_attempt, 0);

        // 设置当前尝试次数并验证延迟
        helper.state.current_attempt = 1;
        assert_eq!(helper.calculate_next_delay(), Duration::from_millis(100));

        helper.state.current_attempt = 2;
        assert_eq!(helper.calculate_next_delay(), Duration::from_millis(200));

        helper.state.current_attempt = 3;
        assert_eq!(helper.calculate_next_delay(), Duration::from_millis(400));

        helper.state.current_attempt = 4;
        assert_eq!(helper.calculate_next_delay(), Duration::from_millis(800));
    }

    #[tokio::test]
    async fn test_max_delay_limit() {
        let policy = ReconnectPolicy {
            max_attempts: 10,
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(5),
            backoff_multiplier: 2.0,
            jitter: false,
        };

        let mut helper = ReconnectHelper::new(policy);

        // 测试延迟不超过最大值
        helper.state.current_attempt = 10;
        let delay = helper.calculate_next_delay();
        assert!(delay <= Duration::from_secs(5));
    }

    #[tokio::test]
    async fn test_max_attempts() {
        let policy = ReconnectPolicy {
            max_attempts: 2,
            initial_delay: Duration::from_millis(10),
            max_delay: Duration::from_secs(1),
            backoff_multiplier: 2.0,
            jitter: false,
        };

        let mut helper = ReconnectHelper::new(policy);

        // 模拟失败的连接函数
        let connect_fn = || async { Err::<(), _>("Connection failed") };

        // 第一次尝试
        let result = helper.execute_reconnect(connect_fn).await;
        assert!(result.is_err());
        assert_eq!(helper.state.current_attempt, 1);

        // 第二次尝试
        let result = helper.execute_reconnect(connect_fn).await;
        assert!(result.is_err());
        assert_eq!(helper.state.current_attempt, 2);

        // 第三次尝试应该立即失败
        let result = helper.execute_reconnect(connect_fn).await;
        assert!(matches!(result, Err(ReconnectError::MaxAttemptsExceeded)));
        assert_eq!(helper.state.connection_state, ConnectionState::Failed);
    }

    #[tokio::test]
    async fn test_successful_reconnect() {
        let policy = ReconnectPolicy::default();
        let mut helper = ReconnectHelper::new(policy);

        // 模拟成功的连接函数
        let connect_fn = || async { Ok::<(), &str>(()) };

        let result = helper.execute_reconnect(connect_fn).await;
        assert!(result.is_ok());
        assert_eq!(helper.state.connection_state, ConnectionState::Connected);
        assert_eq!(helper.state.current_attempt, 0);
        assert_eq!(helper.stats.successful_reconnects, 1);
    }
}
