//! 标准化数据处理器 - 严格遵循Redis数据结构规范v3.2

use crate::{
    config::{InfluxBatchConfig, StandardRedisConfig},
    error::Result,
    subscriber::{HashBatchData, StandardRedisMessage},
};
use std::collections::HashMap;
use std::time::Duration;
use tokio::{sync::mpsc, time::interval};
use voltage_libs::influxdb::{FieldValue, InfluxClient, LineProtocolBuilder};

/// 标准化数据点结构
#[derive(Debug, Clone)]
pub struct StandardDataPoint {
    /// InfluxDB measurement名称
    pub measurement: String,
    /// 标签数据
    pub tags: HashMap<String, String>,
    /// 字段数据
    pub fields: HashMap<String, FieldValue>,
    /// 时间戳(纳秒)
    pub timestamp: i64,
}

/// 标准化数据处理器
pub struct StandardDataProcessor {
    influx_client: InfluxClient,
    influx_config: InfluxBatchConfig,
    redis_config: StandardRedisConfig,
    batch_buffer: Vec<StandardDataPoint>,
}

impl StandardDataProcessor {
    /// 创建新的标准数据处理器
    pub async fn new(
        influx_config: InfluxBatchConfig,
        redis_config: StandardRedisConfig,
    ) -> Result<Self> {
        let influx_client = InfluxClient::from_config(influx_config.connection.clone())?;

        // 测试InfluxDB连接
        influx_client.ping().await?;
        tracing::info!("InfluxDB连接成功: {}", influx_config.connection.url);

        let batch_size = influx_config.batch_size;
        Ok(Self {
            influx_client,
            influx_config,
            redis_config,
            batch_buffer: Vec::with_capacity(batch_size),
        })
    }

    /// 开始标准化数据处理
    pub async fn start_standard_processing(
        mut self,
        mut message_receiver: mpsc::UnboundedReceiver<StandardRedisMessage>,
        mut batch_receiver: mpsc::UnboundedReceiver<HashBatchData>,
    ) -> Result<()> {
        tracing::info!(
            "开始标准化数据处理，批量大小: {}, 支持的数据类型: {:?}",
            self.influx_config.batch_size,
            self.redis_config.get_supported_types()
        );

        let mut flush_timer = interval(Duration::from_secs(
            self.influx_config.flush_interval_seconds,
        ));

        loop {
            tokio::select! {
                // 处理单个消息通知
                msg = message_receiver.recv() => {
                    match msg {
                        Some(standard_msg) => {
                            tracing::debug!(
                                "处理单个消息通知: {}:{}:{}",
                                standard_msg.service,
                                standard_msg.channel_id.as_deref().unwrap_or(""),
                                standard_msg.data_type
                            );
                            // 这里只记录日志，实际数据处理在batch_receiver中
                        }
                        None => {
                            tracing::info!("消息通道已关闭");
                            break;
                        }
                    }
                }

                // 处理Hash批量数据
                batch_data = batch_receiver.recv() => {
                    match batch_data {
                        Some(hash_data) => {
                            match self.process_hash_batch_data(hash_data).await {
                                Ok(mut data_points) => {
                                    self.batch_buffer.append(&mut data_points);

                                    // 检查是否需要刷新
                                    if self.batch_buffer.len() >= self.influx_config.batch_size {
                                        self.flush_batch().await?;
                                    }
                                }
                                Err(e) => {
                                    tracing::error!("处理Hash批量数据失败: {}", e);
                                }
                            }
                        }
                        None => {
                            tracing::info!("批量数据通道已关闭，刷新剩余数据");
                            if !self.batch_buffer.is_empty() {
                                self.flush_batch().await?;
                            }
                            break;
                        }
                    }
                }

                // 定时刷新
                _ = flush_timer.tick() => {
                    if !self.batch_buffer.is_empty() {
                        self.flush_batch().await?;
                    }
                }
            }
        }

        tracing::info!("标准化数据处理器已停止");
        Ok(())
    }

    /// 处理多服务批量数据
    async fn process_hash_batch_data(
        &self,
        hash_data: HashBatchData,
    ) -> Result<Vec<StandardDataPoint>> {
        tracing::debug!(
            "处理{}批量数据: {} -> {} 个点位",
            hash_data.service,
            hash_data.data_key,
            hash_data.fields.len()
        );

        let mut data_points = Vec::new();

        // 获取measurement名称
        let measurement = self
            .redis_config
            .get_measurement_for_service_type(&hash_data.service, &hash_data.data_type);

        // 创建公共标签
        let mut base_tags = HashMap::new();
        base_tags.insert("service".to_string(), hash_data.service.clone());
        base_tags.insert("data_type".to_string(), hash_data.data_type.clone());

        // 可选：添加通道ID标签
        if let Some(channel_id) = &hash_data.channel_id {
            base_tags.insert("channel_id".to_string(), channel_id.clone());
        }

        let timestamp_nanos = hash_data.timestamp.timestamp_nanos_opt().unwrap_or(0);

        // 处理每个字段(点位)
        for (point_id, value_str) in hash_data.fields {
            match self.parse_standard_value(&value_str) {
                Ok(field_value) => {
                    let mut tags = base_tags.clone();
                    tags.insert("point_id".to_string(), point_id);

                    let mut fields = HashMap::new();
                    fields.insert("value".to_string(), field_value);

                    let data_point = StandardDataPoint {
                        measurement: measurement.clone(),
                        tags,
                        fields,
                        timestamp: timestamp_nanos,
                    };

                    data_points.push(data_point);
                }
                Err(e) => {
                    tracing::warn!("解析点位值失败 {}:{}: {}", point_id, value_str, e);
                }
            }
        }

        tracing::debug!(
            "成功处理 {} 个{}数据点到measurement: {}",
            data_points.len(),
            hash_data.service,
            measurement
        );

        Ok(data_points)
    }

    /// 解析标准化数值 - 支持6位小数精度
    fn parse_standard_value(&self, value_str: &str) -> Result<FieldValue> {
        // 尝试解析浮点数 (标准6位小数格式)
        if let Ok(float_val) = value_str.parse::<f64>() {
            return Ok(FieldValue::Float(float_val));
        }

        // 尝试解析整数
        if let Ok(int_val) = value_str.parse::<i64>() {
            return Ok(FieldValue::Integer(int_val));
        }

        // 尝试解析布尔值 (信号数据)
        if let Ok(bool_val) = value_str.parse::<bool>() {
            return Ok(FieldValue::Boolean(bool_val));
        }

        // 特殊处理字符串形式的布尔值
        match value_str {
            "1" => Ok(FieldValue::Boolean(true)),
            "0" => Ok(FieldValue::Boolean(false)),
            _ => {
                // 默认作为字符串处理
                Ok(FieldValue::String(value_str.to_string()))
            }
        }
    }

    /// 刷新批量数据到InfluxDB
    async fn flush_batch(&mut self) -> Result<()> {
        if self.batch_buffer.is_empty() {
            return Ok(());
        }

        tracing::debug!("刷新批量数据: {} 个点", self.batch_buffer.len());

        // 构建线协议数据
        let mut line_protocol_lines = Vec::with_capacity(self.batch_buffer.len());

        for data_point in &self.batch_buffer {
            let mut builder = LineProtocolBuilder::new(&data_point.measurement);

            // 添加标签
            for (key, value) in &data_point.tags {
                builder = builder.tag(key, value);
            }

            // 添加字段
            for (key, value) in &data_point.fields {
                builder = builder.field(key, value.clone());
            }

            // 添加时间戳
            if data_point.timestamp > 0 {
                builder = builder.timestamp(data_point.timestamp);
            }

            line_protocol_lines.push(builder.build());
        }

        let line_protocol = line_protocol_lines.join("\n");

        // 写入InfluxDB
        match self.influx_client.write_line_protocol(&line_protocol).await {
            Ok(_) => {
                tracing::info!("成功写入 {} 个数据点到InfluxDB", self.batch_buffer.len());
                self.batch_buffer.clear();
            }
            Err(e) => {
                tracing::error!("写入InfluxDB失败: {}", e);
                return Err(e.into());
            }
        }

        Ok(())
    }
}
