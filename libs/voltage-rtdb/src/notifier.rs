//! UDS 通知发送端
//!
//! modsrv 使用此模块向 comsrv 发送 M2C 命令通知。
//! 支持优雅降级：连接失败时不阻塞，仅禁用通知。
//!
//! ## 可靠性增强
//!
//! `notify()` 返回 `NotifyResult` 而非简单的 `io::Result<()>`，
//! 允许调用方区分：
//! - 成功发送 (`uds_sent = true`)
//! - 降级到轮询 (`fallback_used = true`)
//! - 完全禁用 (`disabled = true`)

use std::io;
use std::path::Path;
use std::time::{Duration, Instant};

use tokio::io::AsyncWriteExt;
use tokio::net::UnixStream;
use tracing::{debug, info, warn};
use voltage_model::PointType;

use crate::notification::ShmNotification;

/// UDS 默认路径
pub const DEFAULT_UDS_PATH: &str = "/tmp/voltage-m2c.sock";

// ============================================================================
// NotifyResult - 通知结果状态
// ============================================================================

/// UDS 通知结果
///
/// 提供详细的发送状态，允许调用方根据状态采取措施。
#[derive(Debug, Clone, Copy, Default)]
pub struct NotifyResult {
    /// UDS 发送成功
    pub uds_sent: bool,
    /// 降级到轮询备份（UDS 发送失败）
    pub fallback_used: bool,
    /// 通知完全禁用（路径为空或未配置）
    pub disabled: bool,
}

impl NotifyResult {
    /// 检查是否成功发送（通过 UDS）
    #[inline]
    pub fn is_success(&self) -> bool {
        self.uds_sent
    }

    /// 检查是否需要立即轮询（降级情况）
    #[inline]
    pub fn needs_immediate_poll(&self) -> bool {
        self.fallback_used
    }
}

// ============================================================================
// UdsHealth - 连接健康状态
// ============================================================================

/// UDS 连接健康状态
#[derive(Debug, Clone)]
pub enum UdsHealth {
    /// 已连接
    Connected,
    /// 已断开
    Disconnected {
        /// 断开时间
        since: Option<Instant>,
        /// 当前退避时间（毫秒）
        backoff_ms: u64,
    },
    /// 已禁用（路径为空）
    Disabled,
}

/// SHM 命令通知发送器
///
/// 通过 Unix Domain Socket 向 comsrv 发送 M2C 命令通知。
/// 支持优雅降级：如果连接失败，通知将被静默忽略。
/// 支持自动重连：断开后使用指数退避策略自动重连。
pub struct ShmNotifier {
    stream: Option<UnixStream>,
    path: String,
    /// 上次连接尝试时间
    last_connect_attempt: Option<Instant>,
    /// 当前退避时间（毫秒）
    backoff_ms: u64,
}

impl ShmNotifier {
    /// 最小退避时间（毫秒）
    const MIN_BACKOFF_MS: u64 = 1000; // 1 秒
    /// 最大退避时间（毫秒）
    const MAX_BACKOFF_MS: u64 = 30000; // 30 秒
    /// 发送重试次数
    const MAX_RETRIES: u32 = 3;
    /// 重试间隔（毫秒）
    const RETRY_DELAY_MS: u64 = 10;

    /// 连接到 UDS 监听器
    ///
    /// 如果连接失败，返回一个禁用的 notifier（通知会被忽略）。
    /// 后续调用 `notify()` 时会自动尝试重连。
    pub async fn connect(path: impl AsRef<Path>) -> io::Result<Self> {
        let path_str = path.as_ref().to_string_lossy().to_string();

        match UnixStream::connect(&path_str).await {
            Ok(stream) => {
                debug!("ShmNotifier connected to {}", path_str);
                Ok(Self {
                    stream: Some(stream),
                    path: path_str,
                    last_connect_attempt: None,
                    backoff_ms: Self::MIN_BACKOFF_MS,
                })
            },
            Err(e) => {
                warn!(
                    "ShmNotifier: UDS connect failed ({}), will retry on notify: {}",
                    path_str, e
                );
                Ok(Self {
                    stream: None,
                    path: path_str,
                    last_connect_attempt: Some(Instant::now()),
                    backoff_ms: Self::MIN_BACKOFF_MS,
                })
            },
        }
    }

    /// 使用默认路径连接
    pub async fn connect_default() -> io::Result<Self> {
        Self::connect(DEFAULT_UDS_PATH).await
    }

    /// 创建一个禁用的 notifier（用于测试或不需要通知的场景）
    pub fn disabled() -> Self {
        Self {
            stream: None,
            path: String::new(),
            last_connect_attempt: None,
            backoff_ms: Self::MIN_BACKOFF_MS,
        }
    }

    /// 检查是否已连接
    pub fn is_connected(&self) -> bool {
        self.stream.is_some()
    }

    /// 获取连接路径
    pub fn path(&self) -> &str {
        &self.path
    }

    /// 发送通知
    ///
    /// 如果未连接，会尝试重连（使用指数退避）。
    /// 发送失败时最多重试 3 次，全部失败后标记断开，下次调用时触发重连。
    ///
    /// ## 返回值
    ///
    /// 返回 `NotifyResult` 而非 `io::Result<()>`，允许调用方区分：
    /// - `uds_sent = true`: UDS 发送成功，命令将通过低延迟路径处理
    /// - `fallback_used = true`: UDS 失败，降级到轮询备份（延迟增加）
    /// - `disabled = true`: 通知功能完全禁用
    ///
    /// ## 使用示例
    ///
    /// ```rust,ignore
    /// let result = notifier.notify(channel_id, point_type, point_id).await;
    /// if result.needs_immediate_poll() {
    ///     // UDS 失败，可能需要触发立即轮询
    ///     poller.trigger_immediate_check();
    /// }
    /// ```
    pub async fn notify(
        &mut self,
        channel_id: u32,
        point_type: PointType,
        point_id: u32,
    ) -> NotifyResult {
        // 如果路径为空，通知被禁用
        if self.path.is_empty() {
            return NotifyResult {
                disabled: true,
                ..Default::default()
            };
        }

        // 如果未连接，尝试重连
        self.try_reconnect().await;

        if let Some(ref mut stream) = self.stream {
            let notification = ShmNotification::new(channel_id, point_type, point_id);
            let bytes = notification.to_bytes();

            // 重试逻辑：最多重试 MAX_RETRIES 次
            for attempt in 0..Self::MAX_RETRIES {
                match stream.write_all(&bytes).await {
                    Ok(_) => {
                        debug!(
                            "ShmNotifier: sent notification channel={} type={:?} point={}",
                            channel_id, point_type, point_id
                        );
                        return NotifyResult {
                            uds_sent: true,
                            ..Default::default()
                        };
                    },
                    Err(e) if attempt < Self::MAX_RETRIES - 1 => {
                        warn!(
                            "ShmNotifier: send attempt {} failed: {}, retrying...",
                            attempt + 1,
                            e
                        );
                        tokio::time::sleep(Duration::from_millis(Self::RETRY_DELAY_MS)).await;
                    },
                    Err(e) => {
                        // 所有重试都失败，标记断开
                        warn!(
                            "ShmNotifier: all {} retries failed for ch{}:{}:{}, \
                             falling back to poller: {}",
                            Self::MAX_RETRIES,
                            channel_id,
                            point_type.as_str(),
                            point_id,
                            e
                        );
                        self.stream = None;
                        self.last_connect_attempt = Some(Instant::now());
                        // 返回降级状态，让调用方可以触发立即轮询
                        return NotifyResult {
                            fallback_used: true,
                            ..Default::default()
                        };
                    },
                }
            }
        }

        // 未连接且重连失败，返回降级状态
        NotifyResult {
            fallback_used: true,
            ..Default::default()
        }
    }

    /// 尝试重连（如果断开且退避时间已过）
    async fn try_reconnect(&mut self) {
        // 已连接或路径为空，跳过
        if self.stream.is_some() || self.path.is_empty() {
            return;
        }

        // 检查退避时间
        if let Some(last_attempt) = self.last_connect_attempt {
            if last_attempt.elapsed().as_millis() < self.backoff_ms as u128 {
                return; // 退避期内，跳过
            }
        }

        // 尝试重连
        match UnixStream::connect(&self.path).await {
            Ok(stream) => {
                self.stream = Some(stream);
                self.backoff_ms = Self::MIN_BACKOFF_MS;
                self.last_connect_attempt = None;
                info!("ShmNotifier: reconnected to {}", self.path);
            },
            Err(_) => {
                // 增加退避时间（指数退避）
                self.backoff_ms = (self.backoff_ms * 2).min(Self::MAX_BACKOFF_MS);
                self.last_connect_attempt = Some(Instant::now());
            },
        }
    }

    /// 发送预构建的通知（带自动重连）
    ///
    /// 返回 `NotifyResult`，与 `notify()` 行为一致。
    pub async fn notify_raw(&mut self, notification: &ShmNotification) -> NotifyResult {
        // 如果路径为空，通知被禁用
        if self.path.is_empty() {
            return NotifyResult {
                disabled: true,
                ..Default::default()
            };
        }

        self.try_reconnect().await;

        if let Some(ref mut stream) = self.stream {
            match stream.write_all(&notification.to_bytes()).await {
                Ok(_) => {
                    return NotifyResult {
                        uds_sent: true,
                        ..Default::default()
                    };
                },
                Err(e) => {
                    warn!("ShmNotifier: send_raw failed, marking disconnected: {}", e);
                    self.stream = None;
                    self.last_connect_attempt = Some(Instant::now());
                },
            }
        }

        NotifyResult {
            fallback_used: true,
            ..Default::default()
        }
    }

    /// 检查 UDS 连接健康状态
    ///
    /// 用于监控和诊断。
    pub fn health_check(&self) -> UdsHealth {
        if self.path.is_empty() {
            return UdsHealth::Disabled;
        }

        if self.stream.is_some() {
            UdsHealth::Connected
        } else {
            UdsHealth::Disconnected {
                since: self.last_connect_attempt,
                backoff_ms: self.backoff_ms,
            }
        }
    }

    /// 手动强制重新连接（绕过退避机制）
    pub async fn reconnect(&mut self) -> io::Result<bool> {
        if self.path.is_empty() {
            return Ok(false);
        }

        match UnixStream::connect(&self.path).await {
            Ok(stream) => {
                info!("ShmNotifier: force reconnected to {}", self.path);
                self.stream = Some(stream);
                self.backoff_ms = Self::MIN_BACKOFF_MS;
                self.last_connect_attempt = None;
                Ok(true)
            },
            Err(e) => {
                warn!("ShmNotifier: force reconnect failed: {}", e);
                self.last_connect_attempt = Some(Instant::now());
                Ok(false)
            },
        }
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Test code - unwrap is acceptable
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_disabled_notifier() {
        let mut notifier = ShmNotifier::disabled();
        assert!(!notifier.is_connected());

        // 禁用的 notifier 应返回 disabled = true
        let result = notifier.notify(1001, PointType::Control, 0).await;
        assert!(result.disabled);
        assert!(!result.uds_sent);
        assert!(!result.fallback_used);
    }

    #[tokio::test]
    async fn test_connect_nonexistent_path() {
        let notifier = ShmNotifier::connect("/tmp/nonexistent-test-socket.sock")
            .await
            .unwrap();

        // 连接失败，但返回禁用的 notifier 而不是错误
        assert!(!notifier.is_connected());
    }

    #[tokio::test]
    async fn test_notify_result_helpers() {
        // 成功
        let success = NotifyResult {
            uds_sent: true,
            ..Default::default()
        };
        assert!(success.is_success());
        assert!(!success.needs_immediate_poll());

        // 降级
        let fallback = NotifyResult {
            fallback_used: true,
            ..Default::default()
        };
        assert!(!fallback.is_success());
        assert!(fallback.needs_immediate_poll());

        // 禁用
        let disabled = NotifyResult {
            disabled: true,
            ..Default::default()
        };
        assert!(!disabled.is_success());
        assert!(!disabled.needs_immediate_poll());
    }

    #[tokio::test]
    async fn test_health_check() {
        // 禁用状态
        let notifier = ShmNotifier::disabled();
        assert!(matches!(notifier.health_check(), UdsHealth::Disabled));

        // 断开状态
        let notifier = ShmNotifier::connect("/tmp/nonexistent-test-socket.sock")
            .await
            .unwrap();
        assert!(matches!(
            notifier.health_check(),
            UdsHealth::Disconnected { .. }
        ));
    }
}
