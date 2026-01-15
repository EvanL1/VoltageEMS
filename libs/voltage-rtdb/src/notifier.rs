//! UDS 通知发送端
//!
//! modsrv 使用此模块向 comsrv 发送 M2C 命令通知。
//! 支持优雅降级：连接失败时不阻塞，仅禁用通知。

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
    pub async fn notify(
        &mut self,
        channel_id: u32,
        point_type: PointType,
        point_id: u32,
    ) -> io::Result<()> {
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
                        return Ok(());
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
                            "ShmNotifier: all {} retries failed, marking disconnected: {}",
                            Self::MAX_RETRIES,
                            e
                        );
                        self.stream = None;
                        self.last_connect_attempt = Some(Instant::now());
                        // 不返回错误，允许降级到 TODO 队列
                        return Ok(());
                    },
                }
            }
        }
        Ok(())
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
    pub async fn notify_raw(&mut self, notification: &ShmNotification) -> io::Result<()> {
        self.try_reconnect().await;

        if let Some(ref mut stream) = self.stream {
            if let Err(e) = stream.write_all(&notification.to_bytes()).await {
                warn!("ShmNotifier: send_raw failed, marking disconnected: {}", e);
                self.stream = None;
                self.last_connect_attempt = Some(Instant::now());
            }
        }
        Ok(())
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

        // 应该静默成功
        notifier.notify(1001, PointType::Control, 0).await.unwrap();
    }

    #[tokio::test]
    async fn test_connect_nonexistent_path() {
        let notifier = ShmNotifier::connect("/tmp/nonexistent-test-socket.sock")
            .await
            .unwrap();

        // 连接失败，但返回禁用的 notifier 而不是错误
        assert!(!notifier.is_connected());
    }
}
