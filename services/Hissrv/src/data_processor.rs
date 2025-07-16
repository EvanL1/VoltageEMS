use crate::error::{HisSrvError, Result};
use crate::influx_client::{DataPoint, DataValue, InfluxDBClient};
use crate::redis_client::RedisMessage;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Mutex};
use tokio::time::{interval, Instant};
use tracing::{debug, error, info, warn};

/// 数据处理统计
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct ProcessingStats {
    pub messages_received: u64,
    pub messages_processed: u64,
    pub messages_failed: u64,
    pub points_written: u64,
    #[serde(skip)]
    pub last_processed_time: Option<Instant>,
}

/// 数据处理器
pub struct DataProcessor {
    influx_client: Arc<InfluxDBClient>,
    message_receiver: Arc<Mutex<mpsc::UnboundedReceiver<RedisMessage>>>,
    stats: Arc<Mutex<ProcessingStats>>,
    flush_interval: Duration,
}

impl DataProcessor {
    /// 创建新的数据处理器
    pub fn new(
        influx_client: InfluxDBClient,
        message_receiver: mpsc::UnboundedReceiver<RedisMessage>,
        flush_interval_seconds: u64,
    ) -> Self {
        Self {
            influx_client: Arc::new(influx_client),
            message_receiver: Arc::new(Mutex::new(message_receiver)),
            stats: Arc::new(Mutex::new(ProcessingStats::default())),
            flush_interval: Duration::from_secs(flush_interval_seconds),
        }
    }

    /// 开始处理消息
    pub async fn start_processing(&self) -> Result<()> {
        info!("启动数据处理器");

        // 启动定期刷新任务
        let flush_client = Arc::clone(&self.influx_client);
        let flush_stats = Arc::clone(&self.stats);
        let flush_interval = self.flush_interval;
        
        tokio::spawn(async move {
            let mut interval_timer = interval(flush_interval);
            loop {
                interval_timer.tick().await;
                if let Err(e) = flush_client.flush().await {
                    error!("定期刷新失败: {}", e);
                } else {
                    debug!("执行定期刷新");
                }
            }
        });

        // 启动统计报告任务
        let stats_clone = Arc::clone(&self.stats);
        tokio::spawn(async move {
            let mut interval_timer = interval(Duration::from_secs(60));
            loop {
                interval_timer.tick().await;
                let stats = stats_clone.lock().await;
                info!(
                    "处理统计: 收到 {} 条消息, 处理 {} 条, 失败 {} 条, 写入 {} 个点",
                    stats.messages_received,
                    stats.messages_processed, 
                    stats.messages_failed,
                    stats.points_written
                );
            }
        });

        // 主消息处理循环
        loop {
            let message = {
                let mut receiver = self.message_receiver.lock().await;
                receiver.recv().await
            };

            match message {
                Some(message) => {
                    // 更新接收统计
                    {
                        let mut stats = self.stats.lock().await;
                        stats.messages_received += 1;
                    }

                    // 处理消息
                    if let Err(e) = self.process_message(message).await {
                        error!("处理消息失败: {}", e);
                        let mut stats = self.stats.lock().await;
                        stats.messages_failed += 1;
                    } else {
                        let mut stats = self.stats.lock().await;
                        stats.messages_processed += 1;
                        stats.last_processed_time = Some(Instant::now());
                    }
                }
                None => {
                    warn!("消息通道已关闭，停止处理");
                    break;
                }
            }
        }

        // 最终刷新
        if let Err(e) = self.influx_client.flush().await {
            error!("最终刷新失败: {}", e);
        }

        info!("数据处理器已停止");
        Ok(())
    }

    /// 处理单个消息
    async fn process_message(&self, message: RedisMessage) -> Result<()> {
        debug!("处理消息: key={}", message.key);

        // 检查是否有点数据
        let Some(ref point_data) = message.point_data else {
            debug!("消息 {} 没有点数据，跳过", message.key);
            return Ok(());
        };

        // 转换为 InfluxDB 数据点
        let data_point = self.convert_to_influx_point(&message, point_data)?;

        // 写入 InfluxDB
        self.influx_client.write_point(data_point).await?;

        // 更新统计
        {
            let mut stats = self.stats.lock().await;
            stats.points_written += 1;
        }

        debug!("成功处理消息: {}", message.key);
        Ok(())
    }

    /// 将 Redis 消息转换为 InfluxDB 数据点
    fn convert_to_influx_point(
        &self,
        message: &RedisMessage,
        point_data: &voltage_common::types::PointData,
    ) -> Result<DataPoint> {
        // 转换值类型
        let value = DataValue::from(point_data.clone());

        // 如果有通道信息，使用结构化的方式创建数据点
        if let Some(ref channel_info) = message.channel_info {
            Ok(DataPoint::from_channel_data(
                channel_info.channel_id,
                channel_info.point_id,
                &channel_info.point_type,
                value,
                point_data.timestamp,
            ))
        } else {
            // 没有通道信息，创建一个通用的数据点
            let mut tags = std::collections::HashMap::new();
            tags.insert("source".to_string(), "redis".to_string());
            tags.insert("key".to_string(), message.key.clone());


            let mut fields = std::collections::HashMap::new();
            fields.insert("value".to_string(), value);

            Ok(DataPoint::new(
                "raw_data".to_string(),
                tags,
                fields,
                point_data.timestamp,
            ))
        }
    }

    /// 获取处理统计
    pub async fn get_stats(&self) -> ProcessingStats {
        self.stats.lock().await.clone()
    }

    /// 重置统计
    pub async fn reset_stats(&self) {
        let mut stats = self.stats.lock().await;
        *stats = ProcessingStats::default();
    }

    /// 手动触发刷新
    pub async fn flush(&self) -> Result<()> {
        self.influx_client.flush().await
    }

    /// 检查处理器健康状态
    pub async fn health_check(&self) -> Result<()> {
        // 检查 InfluxDB 连接
        self.influx_client.ping().await?;

        // 检查是否有最近的活动
        let stats = self.stats.lock().await;
        if let Some(last_time) = stats.last_processed_time {
            let elapsed = last_time.elapsed();
            if elapsed > Duration::from_secs(300) {
                // 5分钟没有处理消息
                warn!("5分钟内没有处理任何消息");
            }
        }

        Ok(())
    }
}

/// 数据转换器 - 处理特定的数据转换逻辑
pub struct DataTransformer;

impl DataTransformer {
    /// 验证数据点的有效性
    pub fn validate_point_data(point_data: &voltage_common::types::PointData) -> Result<()> {
        use voltage_common::types::PointValue;

        // 检查值的有效性
        match &point_data.value {
            PointValue::Float(f) => {
                if f.is_nan() || f.is_infinite() {
                    return Err(HisSrvError::data_processing("浮点值无效"));
                }
            }
            PointValue::String(s) => {
                if s.is_empty() {
                    return Err(HisSrvError::data_processing("字符串值为空"));
                }
                if s.len() > 1000 {
                    return Err(HisSrvError::data_processing("字符串值过长"));
                }
            }
            _ => {} // Integer 和 Boolean 不需要特殊验证
        }

        Ok(())
    }

    /// 标准化测量名称
    pub fn normalize_measurement_name(point_type: &str, channel_id: u32) -> String {
        let base_name = match point_type {
            "m" => "telemetry",
            "s" => "signal", 
            "c" => "control",
            "a" => "adjustment",
            _ => "unknown",
        };

        // 可以根据通道ID添加额外的分类
        match channel_id {
            1..=1000 => format!("{}_station_a", base_name),
            1001..=2000 => format!("{}_station_b", base_name),
            _ => base_name.to_string(),
        }
    }

    /// 添加额外的标签
    pub fn add_contextual_tags(
        tags: &mut std::collections::HashMap<String, String>,
        channel_info: &crate::redis_client::ChannelInfo,
    ) {
        // 添加设备类型信息
        match channel_info.channel_id {
            1..=100 => tags.insert("device_type".to_string(), "transformer".to_string()),
            101..=200 => tags.insert("device_type".to_string(), "generator".to_string()),
            201..=300 => tags.insert("device_type".to_string(), "meter".to_string()),
            _ => tags.insert("device_type".to_string(), "unknown".to_string()),
        };

        // 添加区域信息
        let region = match channel_info.channel_id {
            1..=500 => "north",
            501..=1000 => "south", 
            1001..=1500 => "east",
            _ => "west",
        };
        tags.insert("region".to_string(), region.to_string());

        // 添加点类型的详细描述
        let point_description = match channel_info.point_type.as_str() {
            "m" => match channel_info.point_id % 10 {
                1 => "voltage",
                2 => "current", 
                3 => "power",
                4 => "frequency",
                _ => "other_measurement",
            },
            "s" => match channel_info.point_id % 10 {
                1 => "breaker_status",
                2 => "alarm_status",
                3 => "protection_status", 
                _ => "other_signal",
            },
            _ => "unknown",
        };
        tags.insert("point_description".to_string(), point_description.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::InfluxDBConfig;
    use tokio_test;
    use voltage_common::types::{PointData, PointValue};

    fn create_test_influx_config() -> InfluxDBConfig {
        InfluxDBConfig {
            enabled: true,
            url: "http://localhost:8086".to_string(),
            database: "test_db".to_string(),
            token: None,
            organization: None,
            batch_size: 10,
            flush_interval_seconds: 1,
        }
    }

    #[tokio::test]
    async fn test_data_transformer_validate_point_data() {
        // 测试有效数据
        let good_point = PointData {
            point_id: 1001,
            value: PointValue::Float(3.14),
            timestamp: chrono::Utc::now(),
            quality: None,
            metadata: None,
        };
        assert!(DataTransformer::validate_point_data(&good_point).is_ok());

        // 测试无效浮点数
        let bad_float_point = PointData {
            point_id: 1002,
            value: PointValue::Float(f64::NAN),
            timestamp: chrono::Utc::now(),
            quality: None,
            metadata: None,
        };
        assert!(DataTransformer::validate_point_data(&bad_float_point).is_err());

        // 测试空字符串
        let empty_string_point = PointData {
            point_id: 1003,
            value: PointValue::String(String::new()),
            timestamp: chrono::Utc::now(),
            quality: None,
            metadata: None,
        };
        assert!(DataTransformer::validate_point_data(&empty_string_point).is_err());
    }

    #[test]
    fn test_data_transformer_normalize_measurement_name() {
        assert_eq!(
            DataTransformer::normalize_measurement_name("m", 100),
            "telemetry_station_a"
        );
        assert_eq!(
            DataTransformer::normalize_measurement_name("s", 1500),
            "signal_station_b"
        );
        assert_eq!(
            DataTransformer::normalize_measurement_name("c", 5000),
            "control"
        );
    }

    #[test]
    fn test_data_transformer_add_contextual_tags() {
        use crate::redis_client::ChannelInfo;
        
        let channel_info = ChannelInfo {
            channel_id: 150,
            point_id: 10001,
            point_type: "m".to_string(),
        };

        let mut tags = std::collections::HashMap::new();
        DataTransformer::add_contextual_tags(&mut tags, &channel_info);

        assert_eq!(tags.get("device_type"), Some(&"generator".to_string()));
        assert_eq!(tags.get("region"), Some(&"north".to_string()));
        assert_eq!(tags.get("point_description"), Some(&"voltage".to_string()));
    }
}