//! 通用传输层桥接适配器
//!
//! 这个模块提供协议层和传输层之间的通用桥接，让所有协议都可以使用
//! 统一的传输层接口，而不是直接使用各种第三方库

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

use crate::core::transport::Transport;
use crate::utils::Result;

/// 协议特定的配置
#[derive(Debug, Clone)]
pub struct ProtocolBridgeConfig {
    /// 协议名称
    pub protocol_name: String,
    /// 默认超时时间
    pub default_timeout: Duration,
    /// 最大接收缓冲区大小
    pub max_buffer_size: usize,
    /// 连接重试次数
    pub max_retries: u32,
    /// 协议特定配置
    pub protocol_specific: HashMap<String, String>,
}

impl Default for ProtocolBridgeConfig {
    fn default() -> Self {
        Self {
            protocol_name: "unknown".to_string(),
            default_timeout: Duration::from_secs(5),
            max_buffer_size: 4096,
            max_retries: 3,
            protocol_specific: HashMap::new(),
        }
    }
}

/// 通用传输层桥接适配器
///
/// 这个适配器让任何协议都可以使用标准的Transport trait
/// 提供统一的连接管理、数据发送/接收等功能
#[derive(Debug)]
pub struct UniversalTransportBridge {
    /// 底层传输实现
    _transport: Arc<RwLock<Box<dyn Transport>>>,
    /// 桥接配置
    config: ProtocolBridgeConfig,
    /// 连接状态
    connected: Arc<RwLock<bool>>,
    /// 统计信息
    stats: Arc<RwLock<BridgeStats>>,
}

/// 桥接统计信息
#[derive(Debug, Clone, Default)]
pub struct BridgeStats {
    /// 发送的字节数
    pub bytes_sent: u64,
    /// 接收的字节数
    pub bytes_received: u64,
    /// 成功的请求数
    pub successful_requests: u64,
    /// 失败的请求数
    pub failed_requests: u64,
    /// 连接重试次数
    pub connection_retries: u32,
    /// 最后活动时间
    pub last_activity: Option<std::time::SystemTime>,
}

impl UniversalTransportBridge {
    /// 创建新的通用传输桥接
    pub fn new(transport: Box<dyn Transport>, config: ProtocolBridgeConfig) -> Self {
        Self {
            _transport: Arc::new(RwLock::new(transport)),
            config,
            connected: Arc::new(RwLock::new(false)),
            stats: Arc::new(RwLock::new(BridgeStats::default())),
        }
    }

    /// 创建Modbus专用的桥接（便利方法）
    pub fn new_modbus(transport: Box<dyn Transport>) -> Self {
        let config = ProtocolBridgeConfig {
            protocol_name: "modbus".to_string(),
            default_timeout: Duration::from_secs(5),
            max_buffer_size: 512, // Modbus通常较小
            max_retries: 3,
            protocol_specific: {
                let mut map = HashMap::new();
                map.insert("transaction_id".to_string(), "0x0000".to_string());
                map.insert("protocol_id".to_string(), "0x0000".to_string());
                map
            },
        };
        Self::new(transport, config)
    }

    /// 创建IEC60870专用的桥接（便利方法）
    pub fn new_iec60870(transport: Box<dyn Transport>) -> Self {
        let config = ProtocolBridgeConfig {
            protocol_name: "iec60870".to_string(),
            default_timeout: Duration::from_secs(10),
            max_buffer_size: 2048,
            max_retries: 5,
            protocol_specific: HashMap::new(),
        };
        Self::new(transport, config)
    }

    /// 连接到远程端点
    pub async fn connect(&self) -> Result<()> {
        let mut retries = 0;

        while retries < self.config.max_retries {
            let mut transport = self.transport.write().await;
            let result = transport.connect().await;

            match result {
                Ok(_) => {
                    let mut connected = self.connected.write().await;
                    *connected = true;

                    // 更新统计
                    let mut stats = self.stats.write().await;
                    stats.last_activity = Some(std::time::SystemTime::now());

                    tracing::info!(
                        "Successfully connected {} protocol via transport bridge",
                        self.config.protocol_name
                    );
                    return Ok(());
                }
                Err(e) => {
                    retries += 1;
                    let mut stats = self.stats.write().await;
                    stats.connection_retries = retries;

                    if retries >= self.config.max_retries {
                        return Err(crate::utils::ComSrvError::ConnectionError(format!(
                            "Transport connection failed after {} retries: {}",
                            retries, e
                        )));
                    }

                    tracing::warn!(
                        "Connection attempt {} failed for {}: {}",
                        retries,
                        self.config.protocol_name,
                        e
                    );
                    drop(transport); // 释放锁
                    tokio::time::sleep(Duration::from_millis(1000 * retries as u64)).await;
                }
            }
        }

        Err(crate::utils::ComSrvError::ConnectionError(
            "Max retries exceeded".to_string(),
        ))
    }

    /// 断开连接
    pub async fn disconnect(&self) -> Result<()> {
        let mut transport = self.transport.write().await;
        let result = transport.disconnect().await;

        if result.is_ok() {
            let mut connected = self.connected.write().await;
            *connected = false;
            tracing::info!("Disconnected {} protocol", self.config.protocol_name);
        }

        result.map_err(|e| {
            crate::utils::ComSrvError::ConnectionError(format!("Disconnect failed: {e}"))
        })
    }

    /// 发送请求并接收响应
    pub async fn send_request(&self, data: &[u8]) -> Result<Vec<u8>> {
        self.send_request_with_timeout(data, self.config.default_timeout)
            .await
    }

    /// 发送请求并接收响应（指定超时）
    pub async fn send_request_with_timeout(
        &self,
        data: &[u8],
        timeout: Duration,
    ) -> Result<Vec<u8>> {
        let mut transport = self.transport.write().await;

        // 发送数据
        let bytes_sent = transport.send(data).await.map_err(|e| {
            let mut stats = self.stats.try_write().unwrap();
            stats.failed_requests += 1;
            crate::utils::ComSrvError::NetworkError(format!(
                "Failed to send {} data: {}",
                self.config.protocol_name, e
            ))
        })?;

        if bytes_sent != data.len() {
            let mut stats = self.stats.write().await;
            stats.failed_requests += 1;
            return Err(crate::utils::ComSrvError::NetworkError(format!(
                "Incomplete send: {} of {} bytes",
                bytes_sent,
                data.len()
            )));
        }

        // 接收响应
        let mut buffer = vec![0u8; self.config.max_buffer_size];
        let bytes_received = transport
            .receive(&mut buffer, Some(timeout))
            .await
            .map_err(|e| {
                let mut stats = self.stats.try_write().unwrap();
                stats.failed_requests += 1;
                crate::utils::ComSrvError::NetworkError(format!(
                    "Failed to receive {} response: {}",
                    self.config.protocol_name, e
                ))
            })?;

        buffer.truncate(bytes_received);

        // 更新统计
        let mut stats = self.stats.write().await;
        stats.bytes_sent += bytes_sent as u64;
        stats.bytes_received += bytes_received as u64;
        stats.successful_requests += 1;
        stats.last_activity = Some(std::time::SystemTime::now());

        tracing::debug!(
            "Successfully exchanged {} protocol data: sent {}, received {} bytes",
            self.config.protocol_name,
            bytes_sent,
            bytes_received
        );

        Ok(buffer)
    }

    /// 只发送数据（不等待响应）
    pub async fn send_only(&self, data: &[u8]) -> Result<usize> {
        let mut transport = self.transport.write().await;

        let bytes_sent = transport.send(data).await.map_err(|e| {
            crate::utils::ComSrvError::NetworkError(format!(
                "Failed to send {} data: {}",
                self.config.protocol_name, e
            ))
        })?;

        // 更新统计
        let mut stats = self.stats.write().await;
        stats.bytes_sent += bytes_sent as u64;
        stats.last_activity = Some(std::time::SystemTime::now());

        Ok(bytes_sent)
    }

    /// 检查连接状态
    pub async fn is_connected(&self) -> bool {
        let transport = self.transport.read().await;
        transport.is_connected().await
    }

    /// 获取桥接配置
    pub fn config(&self) -> &ProtocolBridgeConfig {
        &self.config
    }

    /// 获取统计信息
    pub async fn stats(&self) -> BridgeStats {
        let stats = self.stats.read().await;
        stats.clone()
    }

    /// 重置统计信息
    pub async fn reset_stats(&self) {
        let mut stats = self.stats.write().await;
        *stats = BridgeStats::default();
    }

    /// 获取传输层诊断信息
    pub async fn diagnostics(&self) -> HashMap<String, String> {
        let transport = self.transport.read().await;
        let mut diag = transport.diagnostics().await;

        // 添加桥接层信息
        diag.insert(
            "bridge_protocol".to_string(),
            self.config.protocol_name.clone(),
        );
        diag.insert(
            "bridge_connected".to_string(),
            self.is_connected().await.to_string(),
        );

        let stats = self.stats.read().await;
        diag.insert(
            "bridge_bytes_sent".to_string(),
            stats.bytes_sent.to_string(),
        );
        diag.insert(
            "bridge_bytes_received".to_string(),
            stats.bytes_received.to_string(),
        );
        diag.insert(
            "bridge_successful_requests".to_string(),
            stats.successful_requests.to_string(),
        );
        diag.insert(
            "bridge_failed_requests".to_string(),
            stats.failed_requests.to_string(),
        );

        diag
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::transport::mock::{MockTransport, MockTransportConfig};

    #[tokio::test]
    async fn test_universal_bridge_creation() {
        let mock_config = MockTransportConfig::default();
        let mock_transport = MockTransport::new(mock_config).unwrap();

        let bridge = UniversalTransportBridge::new_modbus(Box::new(mock_transport));

        assert_eq!(bridge.config().protocol_name, "modbus");
        assert!(!bridge.is_connected().await);
    }

    #[tokio::test]
    async fn test_universal_bridge_stats() {
        let mock_config = MockTransportConfig::default();
        let mock_transport = MockTransport::new(mock_config).unwrap();

        let bridge = UniversalTransportBridge::new_iec60870(Box::new(mock_transport));
        let stats = bridge.stats().await;

        assert_eq!(stats.bytes_sent, 0);
        assert_eq!(stats.bytes_received, 0);
        assert_eq!(stats.successful_requests, 0);
    }
}
