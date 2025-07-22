use crate::error::{HisSrvError, Result};
use crate::monitoring::MetricsCollector;
use crate::redis_subscriber::{ChannelInfo, MessageType, SubscriptionMessage};
use crate::storage::{DataPoint, DataValue, StorageManager};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use crate::types::{GenericPointData, PointValue};

/// 增强的消息处理器
pub struct EnhancedMessageProcessor {
    storage_manager: Arc<RwLock<StorageManager>>,
    message_receiver: mpsc::UnboundedReceiver<SubscriptionMessage>,
    metrics_collector: MetricsCollector,
    batch_size: usize,
    batch_timeout_ms: u64,
}

impl EnhancedMessageProcessor {
    pub fn new(
        storage_manager: Arc<RwLock<StorageManager>>,
        message_receiver: mpsc::UnboundedReceiver<SubscriptionMessage>,
        metrics_collector: MetricsCollector,
        batch_size: usize,
        batch_timeout_ms: u64,
    ) -> Self {
        Self {
            storage_manager,
            message_receiver,
            metrics_collector,
            batch_size,
            batch_timeout_ms,
        }
    }

    /// 开始处理消息
    pub async fn start_processing(&mut self) -> Result<()> {
        info!("Starting enhanced message processor");

        let mut batch = Vec::new();
        let mut batch_timer =
            tokio::time::interval(tokio::time::Duration::from_millis(self.batch_timeout_ms));

        loop {
            tokio::select! {
                // 接收消息
                msg = self.message_receiver.recv() => {
                    match msg {
                        Some(message) => {
                            batch.push(message);

                            // 检查批量大小
                            if batch.len() >= self.batch_size {
                                self.process_batch(&mut batch).await?;
                            }
                        }
                        None => {
                            info!("Message channel closed, stopping processor");
                            break;
                        }
                    }
                }
                // 批量超时
                _ = batch_timer.tick() => {
                    if !batch.is_empty() {
                        self.process_batch(&mut batch).await?;
                    }
                }
            }
        }

        // 处理剩余的消息
        if !batch.is_empty() {
            self.process_batch(&mut batch).await?;
        }

        Ok(())
    }

    /// 批量处理消息
    async fn process_batch(&mut self, batch: &mut Vec<SubscriptionMessage>) -> Result<()> {
        if batch.is_empty() {
            return Ok(());
        }

        let start_time = Instant::now();
        debug!("Processing batch of {} messages", batch.len());

        // 按存储后端分组消息
        let mut backend_batches: HashMap<String, Vec<DataPoint>> = HashMap::new();

        for message in batch.drain(..) {
            match self.convert_to_data_point(&message).await {
                Ok(Some((backend, data_point))) => {
                    backend_batches
                        .entry(backend)
                        .or_insert_with(Vec::new)
                        .push(data_point);
                }
                Ok(None) => {
                    debug!("Message {} skipped (no data point generated)", message.id);
                }
                Err(e) => {
                    error!("Failed to convert message {}: {}", message.id, e);
                    self.metrics_collector.record_storage_error().await;
                }
            }
        }

        // 批量写入每个后端
        let mut storage_manager = self.storage_manager.write().await;
        for (backend_name, data_points) in backend_batches {
            if let Some(backend) = storage_manager.get_backend(Some(&backend_name)) {
                match backend.store_batch(&data_points).await {
                    Ok(_) => {
                        debug!(
                            "Stored {} data points to backend {}",
                            data_points.len(),
                            backend_name
                        );
                        for _ in 0..data_points.len() {
                            self.metrics_collector.record_storage_operation().await;
                        }
                    }
                    Err(e) => {
                        error!("Failed to store batch to backend {}: {}", backend_name, e);
                        for _ in 0..data_points.len() {
                            self.metrics_collector.record_storage_error().await;
                        }
                    }
                }
            } else {
                warn!("No storage backend found: {}", backend_name);
            }
        }

        // 记录处理时间
        let duration = start_time.elapsed();
        self.metrics_collector
            .record_message_processing_time(duration)
            .await;

        Ok(())
    }

    /// 将订阅消息转换为数据点
    async fn convert_to_data_point(
        &self,
        message: &SubscriptionMessage,
    ) -> Result<Option<(String, DataPoint)>> {
        // 记录处理的消息
        self.metrics_collector.record_message_processed().await;

        // 如果有解析的点数据，使用它
        if let Some(ref point_data) = message.point_data {
            let backend = self.determine_storage_backend(&message.channel_info);
            let data_point = self.point_data_to_data_point(
                &message.channel_info,
                point_data,
                &message.metadata,
            )?;
            return Ok(Some((backend, data_point)));
        }

        // 否则尝试解析原始数据
        if let Some(ref raw_data) = message.raw_data {
            if let Some(ref channel_info) = message.channel_info {
                let backend = self.determine_storage_backend(&Some(channel_info.clone()));
                let data_point = self.parse_raw_data(
                    channel_info,
                    raw_data,
                    &message.timestamp,
                    &message.metadata,
                )?;
                return Ok(Some((backend, data_point)));
            }
        }

        // 特殊处理事件和系统状态
        if message.channel.starts_with("event:") {
            return Ok(Some((
                "influxdb".to_string(),
                self.create_event_data_point(message)?,
            )));
        } else if message.channel.starts_with("system:") {
            return Ok(Some((
                "redis".to_string(),
                self.create_system_status_data_point(message)?,
            )));
        }

        Ok(None)
    }

    /// 将 PointData 转换为 DataPoint
    fn point_data_to_data_point(
        &self,
        channel_info: &Option<ChannelInfo>,
        point_data: &GenericPointData,
        metadata: &HashMap<String, String>,
    ) -> Result<DataPoint> {
        let key = if let Some(info) = channel_info {
            format!(
                "{}:{}:{}",
                info.channel_id,
                match info.message_type {
                    MessageType::Telemetry => "m",
                    MessageType::Signal => "s",
                    MessageType::Control => "c",
                    MessageType::Adjustment => "a",
                    MessageType::Calculated => "calc",
                    _ => "unknown",
                },
                info.point_id
            )
        } else {
            format!("unknown:{}", point_data.id)
        };

        let value = match point_data.value {
            PointValue::Float(f) => DataValue::Float(f),
            PointValue::Integer(i) => DataValue::Integer(i),
            PointValue::Boolean(b) => DataValue::Boolean(b),
            PointValue::String(ref s) => DataValue::String(s.clone()),
            PointValue::Bytes(ref b) => DataValue::Bytes(b.clone()),
        };

        let mut tags = HashMap::new();
        if let Some(info) = channel_info {
            tags.insert("channel_id".to_string(), info.channel_id.to_string());
            tags.insert("point_id".to_string(), info.point_id.to_string());
            tags.insert("type".to_string(), format!("{:?}", info.message_type));
        }
        tags.insert("quality".to_string(), point_data.quality.to_string());

        let mut point_metadata = metadata.clone();
        if let Some(ref source) = point_data.source {
            point_metadata.insert("source".to_string(), source.clone());
        }

        Ok(DataPoint {
            key,
            timestamp: point_data.timestamp,
            value,
            tags,
            metadata: point_metadata,
        })
    }

    /// 解析原始数据
    fn parse_raw_data(
        &self,
        channel_info: &ChannelInfo,
        raw_data: &str,
        timestamp: &chrono::DateTime<chrono::Utc>,
        metadata: &HashMap<String, String>,
    ) -> Result<DataPoint> {
        // 尝试解析为数字
        let value = if let Ok(f) = raw_data.parse::<f64>() {
            DataValue::Float(f)
        } else if let Ok(i) = raw_data.parse::<i64>() {
            DataValue::Integer(i)
        } else if let Ok(b) = raw_data.parse::<bool>() {
            DataValue::Boolean(b)
        } else {
            DataValue::String(raw_data.to_string())
        };

        let key = format!(
            "{}:{}:{}",
            channel_info.channel_id,
            match channel_info.message_type {
                MessageType::Telemetry => "m",
                MessageType::Signal => "s",
                MessageType::Control => "c",
                MessageType::Adjustment => "a",
                MessageType::Calculated => "calc",
                _ => "unknown",
            },
            channel_info.point_id
        );

        let mut tags = HashMap::new();
        tags.insert(
            "channel_id".to_string(),
            channel_info.channel_id.to_string(),
        );
        tags.insert("point_id".to_string(), channel_info.point_id.to_string());
        tags.insert(
            "type".to_string(),
            format!("{:?}", channel_info.message_type),
        );

        Ok(DataPoint {
            key,
            timestamp: *timestamp,
            value,
            tags,
            metadata: metadata.clone(),
        })
    }

    /// 创建事件数据点
    fn create_event_data_point(&self, message: &SubscriptionMessage) -> Result<DataPoint> {
        let event_type = message.channel.strip_prefix("event:").unwrap_or("unknown");

        let mut tags = HashMap::new();
        tags.insert("type".to_string(), "event".to_string());
        tags.insert("event_type".to_string(), event_type.to_string());

        let value = if let Some(ref raw_data) = message.raw_data {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(raw_data) {
                DataValue::Json(json)
            } else {
                DataValue::String(raw_data.clone())
            }
        } else {
            DataValue::String("".to_string())
        };

        Ok(DataPoint {
            key: format!("events:{}", event_type),
            timestamp: message.timestamp,
            value,
            tags,
            metadata: message.metadata.clone(),
        })
    }

    /// 创建系统状态数据点
    fn create_system_status_data_point(&self, message: &SubscriptionMessage) -> Result<DataPoint> {
        let service = message
            .channel
            .strip_prefix("system:")
            .and_then(|s| s.split(':').next())
            .unwrap_or("unknown");

        let mut tags = HashMap::new();
        tags.insert("type".to_string(), "system_status".to_string());
        tags.insert("service".to_string(), service.to_string());

        let value = if let Some(ref raw_data) = message.raw_data {
            DataValue::String(raw_data.clone())
        } else {
            DataValue::String("unknown".to_string())
        };

        Ok(DataPoint {
            key: format!("system:{}:status", service),
            timestamp: message.timestamp,
            value,
            tags,
            metadata: message.metadata.clone(),
        })
    }

    /// 根据通道信息确定存储后端
    fn determine_storage_backend(&self, channel_info: &Option<ChannelInfo>) -> String {
        if let Some(info) = channel_info {
            match info.message_type {
                MessageType::Telemetry | MessageType::Signal => "influxdb".to_string(),
                MessageType::Control | MessageType::Adjustment => "redis".to_string(),
                MessageType::Calculated => "influxdb".to_string(),
                MessageType::Event => "influxdb".to_string(),
                MessageType::SystemStatus => "redis".to_string(),
            }
        } else {
            "influxdb".to_string() // 默认使用 InfluxDB
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::redis_subscriber::ChannelInfo;
    use crate::types::{GenericPointData, PointValue};

    #[test]
    fn test_determine_storage_backend() {
        let processor = create_test_processor();

        // 测试遥测数据
        let telemetry_info = ChannelInfo {
            channel_id: 1001,
            message_type: MessageType::Telemetry,
            point_id: 10001,
        };
        assert_eq!(
            processor.determine_storage_backend(&Some(telemetry_info)),
            "influxdb"
        );

        // 测试控制数据
        let control_info = ChannelInfo {
            channel_id: 1001,
            message_type: MessageType::Control,
            point_id: 30001,
        };
        assert_eq!(
            processor.determine_storage_backend(&Some(control_info)),
            "redis"
        );
    }

    fn create_test_processor() -> EnhancedMessageProcessor {
        let storage_manager = Arc::new(RwLock::new(StorageManager::new()));
        let (_tx, rx) = mpsc::unbounded_channel();
        let metrics = MetricsCollector::new();

        EnhancedMessageProcessor::new(storage_manager, rx, metrics, 100, 100)
    }
}
