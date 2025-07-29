//! 核心轮询器 - 简单可靠的数据采集和处理

use crate::config::{Config, DataMapping, TagRule};
use crate::Result;
use hissrv::anyhow;
use redis::{AsyncCommands, Client};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use voltage_libs::influxdb::{FieldValue, InfluxClient, LineProtocolBuilder};

/// Redis 数据类型
#[derive(Debug)]
pub enum RedisData {
    /// 从列表中获取的 JSON 数据
    List(String),
    /// 从 Hash 中获取的键值对
    Hash(String, HashMap<String, String>),
}

/// 数据轮询器
pub struct Poller {
    redis: Client,
    influx: InfluxClient,
    config: Arc<RwLock<Config>>,
    buffer: Vec<String>, // Line protocol buffer
    config_update_rx: Option<tokio::sync::mpsc::Receiver<()>>,
}

impl Poller {
    /// 创建新的轮询器
    pub async fn new(config: Arc<RwLock<Config>>) -> Result<Self> {
        // 读取配置创建客户端
        let (redis_url, influx_config, buffer_size) = {
            let cfg = config
                .read()
                .map_err(|_| anyhow!("Failed to read config"))?;
            (
                cfg.redis.url.clone(),
                voltage_libs::influxdb::InfluxConfig {
                    url: cfg.influxdb.url.clone(),
                    org: cfg.influxdb.org.clone(),
                    bucket: cfg.influxdb.bucket.clone(),
                    token: cfg.influxdb.token.clone(),
                    timeout_seconds: cfg.influxdb.write_timeout.as_secs(),
                    database: None,
                    username: None,
                    password: None,
                },
                cfg.influxdb.batch_size,
            )
        };

        // 创建 Redis 客户端
        let redis =
            Client::open(redis_url).map_err(|e| anyhow!("Failed to create Redis client: {}", e))?;

        // 测试 Redis 连接
        let mut conn = redis
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| anyhow!("Failed to connect to Redis: {}", e))?;
        let _: String = redis::cmd("PING")
            .query_async(&mut conn)
            .await
            .map_err(|e| anyhow!("Redis ping failed: {}", e))?;
        tracing::info!("Redis connection established");

        // 创建 InfluxDB 客户端
        let influx = InfluxClient::from_config(influx_config)?;

        // 测试 InfluxDB 连接
        influx
            .ping()
            .await
            .map_err(|e| anyhow!("InfluxDB ping failed: {}", e))?;
        tracing::info!("InfluxDB connection established");

        Ok(Self {
            redis,
            influx,
            config,
            buffer: Vec::with_capacity(buffer_size),
            config_update_rx: None,
        })
    }

    /// 创建带配置更新通道的轮询器
    pub async fn with_update_channel(
        config: Arc<RwLock<Config>>,
        rx: tokio::sync::mpsc::Receiver<()>,
    ) -> Result<Self> {
        let mut poller = Self::new(config).await?;
        poller.config_update_rx = Some(rx);
        Ok(poller)
    }

    /// 运行主轮询循环
    pub async fn run(mut self) -> Result<()> {
        let polling_interval = {
            let cfg = self
                .config
                .read()
                .map_err(|_| anyhow!("Failed to read config"))?;
            cfg.service.polling_interval
        };

        let mut interval = tokio::time::interval(polling_interval);
        tracing::info!("Starting polling with interval: {:?}", polling_interval);

        loop {
            // 检查配置更新通知
            if let Some(rx) = &mut self.config_update_rx {
                match rx.try_recv() {
                    Ok(()) => {
                        tracing::info!("Received configuration update notification");
                        if let Err(e) = self.reload_config().await {
                            tracing::error!("Failed to reload configuration: {}", e);
                        }
                    }
                    Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {
                        // 没有更新，继续正常处理
                    }
                    Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                        tracing::warn!("Configuration update channel disconnected");
                        self.config_update_rx = None;
                    }
                }
            }

            interval.tick().await;

            // 获取数据
            match self.fetch_data().await {
                Ok(data_items) => {
                    let count = data_items.len();
                    if count > 0 {
                        tracing::debug!("Fetched {} data items", count);

                        // 处理每个数据项
                        for item in data_items {
                            if let Err(e) = self.process_data_item(item).await {
                                tracing::error!("Failed to process data item: {}", e);
                            }
                        }

                        // 检查是否需要刷新缓冲区
                        let batch_size = {
                            let cfg = self
                                .config
                                .read()
                                .map_err(|_| anyhow!("Failed to read config"))?;
                            cfg.influxdb.batch_size
                        };

                        if self.buffer.len() >= batch_size {
                            self.flush_buffer().await?;
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to fetch data: {}", e);
                }
            }

            // 定期刷新缓冲区（即使未满）
            if !self.buffer.is_empty() {
                self.flush_buffer().await?;
            }
        }

        #[allow(unreachable_code)]
        Ok(())
    }

    /// 从 Redis 获取数据
    async fn fetch_data(&mut self) -> Result<Vec<RedisData>> {
        let mut conn = self.redis.get_multiplexed_async_connection().await?;
        let mut results = Vec::new();

        let data_keys = {
            let cfg = self
                .config
                .read()
                .map_err(|_| anyhow!("Failed to read config"))?;
            cfg.redis.data_keys.clone()
        };

        for key_config in &data_keys {
            match key_config.data_type.as_str() {
                "list" => {
                    // 使用 RPOPLPUSH 原子操作，确保数据不丢失
                    let processing_key = format!("{}_processing", &key_config.pattern);
                    while let Some(data) = conn
                        .rpoplpush::<_, _, Option<String>>(&key_config.pattern, &processing_key)
                        .await?
                    {
                        results.push(RedisData::List(data.clone()));
                        // 处理成功后删除
                        let _: () = conn.lrem(&processing_key, 1, &data).await?;
                    }
                }
                "hash" => {
                    // 扫描匹配的 Hash keys
                    let pattern = key_config.pattern.clone();
                    let keys = self.scan_keys(&pattern).await?;
                    for key in keys {
                        let data: HashMap<String, String> = conn.hgetall(&key).await?;
                        if !data.is_empty() {
                            results.push(RedisData::Hash(key.clone(), data));
                            // 处理后删除
                            let _: () = conn.del(&key).await?;
                        }
                    }
                }
                _ => {
                    tracing::warn!("Unknown data type: {}", key_config.data_type);
                }
            }
        }

        Ok(results)
    }

    /// 扫描匹配的键
    async fn scan_keys(&self, pattern: &str) -> Result<Vec<String>> {
        let mut conn = self.redis.get_multiplexed_async_connection().await?;
        let mut keys = Vec::new();
        let mut cursor = 0;

        loop {
            let (new_cursor, batch): (u64, Vec<String>) = redis::cmd("SCAN")
                .arg(cursor)
                .arg("MATCH")
                .arg(pattern)
                .arg("COUNT")
                .arg(100)
                .query_async(&mut conn)
                .await?;

            keys.extend(batch);
            cursor = new_cursor;

            if cursor == 0 {
                break;
            }
        }

        Ok(keys)
    }

    /// 处理单个数据项
    async fn process_data_item(&mut self, item: RedisData) -> Result<()> {
        match item {
            RedisData::List(json_str) => {
                // 解析 JSON 数据
                let data: Value = serde_json::from_str(&json_str)
                    .map_err(|e| anyhow!("Failed to parse JSON from list: {}", e))?;
                if let Some(line) = self.convert_json_to_line_protocol(data)? {
                    self.buffer.push(line);
                }
            }
            RedisData::Hash(key, data) => {
                // 查找映射规则
                let mapping_opt = {
                    let cfg = self
                        .config
                        .read()
                        .map_err(|_| anyhow!("Failed to read config"))?;
                    cfg.mappings
                        .iter()
                        .find(|m| key.starts_with(&m.source.replace("*", "")))
                        .cloned()
                };

                if let Some(mapping) = mapping_opt {
                    if let Some(line) = self.convert_hash_to_line_protocol(&key, data, &mapping)? {
                        self.buffer.push(line);
                    }
                } else {
                    tracing::warn!("No mapping found for key: {}", key);
                }
            }
        }
        Ok(())
    }

    /// 将 JSON 数据转换为 Line Protocol
    fn convert_json_to_line_protocol(&self, data: Value) -> Result<Option<String>> {
        // 期望的 JSON 格式:
        // {
        //   "timestamp": 1234567890,
        //   "measurement": "metrics",
        //   "tags": {"tag1": "value1"},
        //   "fields": {"field1": 123.45}
        // }

        let obj = data
            .as_object()
            .ok_or_else(|| anyhow!("Expected JSON object"))?;

        let measurement = obj
            .get("measurement")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing measurement"))?;

        let mut builder = LineProtocolBuilder::new(measurement);

        // 添加标签
        if let Some(tags) = obj.get("tags").and_then(|v| v.as_object()) {
            for (key, value) in tags {
                if let Some(v) = value.as_str() {
                    builder = builder.tag(key, v);
                }
            }
        }

        // 添加字段
        if let Some(fields) = obj.get("fields").and_then(|v| v.as_object()) {
            for (key, value) in fields {
                let field_value = self.json_to_field_value(value)?;
                builder = builder.field(key, field_value);
            }
        }

        // 添加时间戳
        if let Some(ts) = obj.get("timestamp").and_then(|v| v.as_i64()) {
            builder = builder.timestamp(ts * 1_000_000_000); // 秒转纳秒
        }

        Ok(Some(builder.build()))
    }

    /// 将 Hash 数据转换为 Line Protocol
    fn convert_hash_to_line_protocol(
        &self,
        key: &str,
        data: HashMap<String, String>,
        mapping: &DataMapping,
    ) -> Result<Option<String>> {
        let mut builder = LineProtocolBuilder::new(&mapping.measurement);

        // 处理标签
        for tag_rule in &mapping.tags {
            match tag_rule {
                TagRule::Extract { field } => {
                    // 从 key 中提取标签（例如 "archive:1m:1001" 提取 channel=1001）
                    if let Some(value) = self.extract_from_key(key, field) {
                        builder = builder.tag(field, &value);
                    }
                }
                TagRule::Static { value } => {
                    // 静态标签（例如 "interval=1m"）
                    if let Some((k, v)) = value.split_once('=') {
                        builder = builder.tag(k, v);
                    }
                }
            }
        }

        // 处理字段
        for field_mapping in &mapping.fields {
            if let Some(value_str) = data.get(&field_mapping.name) {
                let field_value = self.parse_field_value(value_str, &field_mapping.field_type)?;
                builder = builder.field(&field_mapping.name, field_value);
            }
        }

        // 添加时间戳（如果数据中包含）
        if let Some(ts_str) = data.get("timestamp") {
            if let Ok(ts) = ts_str.parse::<i64>() {
                builder = builder.timestamp(ts * 1_000_000_000);
            }
        } else {
            // 使用当前时间
            builder = builder.timestamp(chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0));
        }

        Ok(Some(builder.build()))
    }

    /// 从键中提取值
    fn extract_from_key(&self, key: &str, _field: &str) -> Option<String> {
        // 简单实现：提取最后一个冒号后的部分
        // 例如 "archive:1m:1001" 提取 "1001"
        key.split(':').last().map(|s| s.to_string())
    }

    /// 解析字段值
    fn parse_field_value(&self, value: &str, field_type: &str) -> Result<FieldValue> {
        match field_type {
            "float" => Ok(FieldValue::Float(
                value
                    .parse()
                    .map_err(|_| anyhow!("Failed to parse float"))?,
            )),
            "int" => Ok(FieldValue::Integer(
                value
                    .parse()
                    .map_err(|_| anyhow!("Failed to parse integer"))?,
            )),
            "bool" => Ok(FieldValue::Boolean(
                value
                    .parse()
                    .map_err(|_| anyhow!("Failed to parse boolean"))?,
            )),
            "string" => Ok(FieldValue::String(value.to_string())),
            _ => Ok(FieldValue::String(value.to_string())),
        }
    }

    /// JSON 值转换为 FieldValue
    fn json_to_field_value(&self, value: &Value) -> Result<FieldValue> {
        match value {
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Ok(FieldValue::Integer(i))
                } else if let Some(f) = n.as_f64() {
                    Ok(FieldValue::Float(f))
                } else {
                    anyhow::bail!("Unsupported number type")
                }
            }
            Value::Bool(b) => Ok(FieldValue::Boolean(*b)),
            Value::String(s) => Ok(FieldValue::String(s.clone())),
            _ => anyhow::bail!("Unsupported field value type"),
        }
    }

    /// 刷新缓冲区到 InfluxDB
    async fn flush_buffer(&mut self) -> Result<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }

        let line_protocol = self.buffer.join("\n");
        let batch_size = self.buffer.len();

        match self.influx.write_line_protocol(&line_protocol).await {
            Ok(_) => {
                tracing::info!("Successfully wrote {} data points to InfluxDB", batch_size);
                self.buffer.clear();
                Ok(())
            }
            Err(e) => {
                tracing::error!("Failed to write to InfluxDB: {}", e);
                // 保留数据在缓冲区中以便重试
                Err(e.into())
            }
        }
    }

    /// 重新加载配置
    async fn reload_config(&mut self) -> Result<()> {
        tracing::info!("Reloading configuration...");

        // 尝试加载新配置
        let new_config = Config::reload()?;

        // 验证新配置
        new_config.validate()?;

        // 检查关键连接参数是否改变
        let need_reconnect = {
            let current_config = self
                .config
                .read()
                .map_err(|_| anyhow!("Failed to read config"))?;

            current_config.redis.url != new_config.redis.url
                || current_config.influxdb.url != new_config.influxdb.url
                || current_config.influxdb.org != new_config.influxdb.org
                || current_config.influxdb.bucket != new_config.influxdb.bucket
                || current_config.influxdb.token != new_config.influxdb.token
        };

        // 如果需要重新连接
        if need_reconnect {
            tracing::info!("Connection parameters changed, reconnecting...");

            // 重新创建 Redis 客户端
            let redis = Client::open(new_config.redis.url.clone())
                .map_err(|e| anyhow!("Failed to create Redis client: {}", e))?;

            // 测试连接
            let mut conn = redis
                .get_multiplexed_async_connection()
                .await
                .map_err(|e| anyhow!("Failed to connect to Redis: {}", e))?;
            let _: String = redis::cmd("PING")
                .query_async(&mut conn)
                .await
                .map_err(|e| anyhow!("Redis ping failed: {}", e))?;

            // 重新创建 InfluxDB 客户端
            let influx_config = voltage_libs::influxdb::InfluxConfig {
                url: new_config.influxdb.url.clone(),
                org: new_config.influxdb.org.clone(),
                bucket: new_config.influxdb.bucket.clone(),
                token: new_config.influxdb.token.clone(),
                timeout_seconds: new_config.influxdb.write_timeout.as_secs(),
                database: None,
                username: None,
                password: None,
            };
            let influx = InfluxClient::from_config(influx_config)?;

            // 测试连接
            influx
                .ping()
                .await
                .map_err(|e| anyhow!("InfluxDB ping failed: {}", e))?;

            // 更新客户端
            self.redis = redis;
            self.influx = influx;
        }

        // 更新配置
        {
            let mut config = self
                .config
                .write()
                .map_err(|_| anyhow!("Failed to write config"))?;
            *config = new_config;
        }

        tracing::info!("Configuration reloaded successfully");
        Ok(())
    }
}
