# netsrv 架构设计

## 概述

netsrv（Network Service）是 VoltageEMS 的云端网关服务，负责将本地数据同步到云平台，支持多种云服务商和协议，实现数据的远程监控和管理。

## 架构特点

1. **多云支持**：AWS IoT、阿里云、Azure 等
2. **协议转换**：MQTT、HTTP/HTTPS、WebSocket
3. **智能缓存**：断线数据缓存和续传
4. **数据过滤**：灵活的数据筛选和聚合
5. **安全传输**：TLS 加密和认证

## 系统架构图

```
┌────────────────────────────────────────────────────────────┐
│                         netsrv                              │
├────────────────────────────────────────────────────────────┤
│                    Data Source Layer                        │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐    │
│  │Redis Monitor │  │Data Filter   │  │Aggregator    │    │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘    │
│         └──────────────────┴──────────────────┘            │
│                            │                                │
│                   Protocol Adapter Layer                    │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐    │
│  │MQTT Adapter  │  │HTTP Adapter  │  │Custom Adapter│    │
│  └──────────────┘  └──────────────┘  └──────────────┘    │
│                            │                                │
│                    Cloud Provider Layer                     │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐    │
│  │AWS IoT Core  │  │Aliyun IoT    │  │Azure IoT Hub │    │
│  └──────────────┘  └──────────────┘  └──────────────┘    │
└────────────────────────────────────────────────────────────┘
```

## 核心组件

### 1. Data Monitor（数据监控）

```rust
pub struct DataMonitor {
    redis_client: Arc<RedisClient>,
    filters: Vec<DataFilter>,
    output_channel: mpsc::Sender<FilteredData>,
}

impl DataMonitor {
    /// 监控数据更新
    pub async fn monitor_updates(&self) -> Result<()> {
        let mut pubsub = self.redis_client.get_async_pubsub().await?;
        pubsub.psubscribe("point:update:*").await?;
        
        while let Some(msg) = pubsub.on_message().next().await {
            if let Ok(update) = self.parse_update(&msg) {
                // 应用过滤器
                if self.should_forward(&update) {
                    let filtered = self.apply_filters(update);
                    self.output_channel.send(filtered).await?;
                }
            }
        }
        
        Ok(())
    }
}
```

### 2. Protocol Adapters（协议适配器）

#### MQTT 适配器
```rust
pub struct MqttAdapter {
    client: AsyncClient,
    config: MqttConfig,
    topic_mapper: TopicMapper,
}

impl MqttAdapter {
    /// 发布数据到 MQTT
    pub async fn publish_data(&self, data: &CloudData) -> Result<()> {
        let topic = self.topic_mapper.map_topic(data)?;
        let payload = self.serialize_payload(data)?;
        
        let msg = MessageBuilder::new()
            .topic(&topic)
            .payload(payload)
            .qos(self.config.qos)
            .retained(self.config.retained)
            .finalize();
        
        self.client.publish(msg).await?;
        Ok(())
    }
}
```

#### HTTP 适配器
```rust
pub struct HttpAdapter {
    client: reqwest::Client,
    config: HttpConfig,
    auth: AuthProvider,
}

impl HttpAdapter {
    /// 批量发送数据
    pub async fn send_batch(&self, batch: Vec<CloudData>) -> Result<()> {
        let request = self.client
            .post(&self.config.endpoint)
            .header("Authorization", self.auth.get_token().await?)
            .json(&batch)
            .timeout(self.config.timeout);
        
        let response = request.send().await?;
        
        if !response.status().is_success() {
            return Err(Error::HttpError(response.status()));
        }
        
        Ok(())
    }
}
```

### 3. Cloud Providers（云服务提供商）

#### AWS IoT Core
```rust
pub struct AwsIotProvider {
    mqtt_client: MqttAdapter,
    shadow_client: DeviceShadowClient,
    config: AwsConfig,
}

impl CloudProvider for AwsIotProvider {
    async fn connect(&mut self) -> Result<()> {
        // 配置 TLS 证书
        let tls_config = self.build_tls_config()?;
        
        // 连接到 AWS IoT
        self.mqtt_client.connect_with_tls(tls_config).await?;
        
        Ok(())
    }
    
    async fn send_telemetry(&self, data: TelemetryData) -> Result<()> {
        let topic = format!("$aws/things/{}/shadow/update", self.config.thing_name);
        
        let payload = json!({
            "state": {
                "reported": data.to_json()
            }
        });
        
        self.mqtt_client.publish(&topic, payload).await
    }
}
```

### 4. Data Management（数据管理）

#### 缓存管理
```rust
pub struct CacheManager {
    cache: Arc<RwLock<VecDeque<CachedData>>>,
    max_size: usize,
    persistence: DiskPersistence,
}

impl CacheManager {
    /// 缓存数据
    pub async fn cache_data(&self, data: CloudData) -> Result<()> {
        let mut cache = self.cache.write().await;
        
        // 检查容量
        if cache.len() >= self.max_size {
            // 持久化到磁盘
            let overflow = cache.drain(..self.max_size/2).collect::<Vec<_>>();
            self.persistence.save_batch(overflow).await?;
        }
        
        cache.push_back(CachedData {
            data,
            timestamp: Utc::now(),
            retry_count: 0,
        });
        
        Ok(())
    }
}
```

#### 断线重连
```rust
pub struct ConnectionManager {
    providers: HashMap<String, Box<dyn CloudProvider>>,
    reconnect_strategy: ReconnectStrategy,
}

impl ConnectionManager {
    /// 管理连接状态
    pub async fn maintain_connections(&self) -> Result<()> {
        for (name, provider) in &self.providers {
            if !provider.is_connected().await {
                info!("Provider {} disconnected, attempting reconnect", name);
                
                match self.reconnect_with_backoff(provider).await {
                    Ok(_) => info!("Reconnected to {}", name),
                    Err(e) => error!("Failed to reconnect to {}: {}", name, e),
                }
            }
        }
        
        Ok(())
    }
}
```

## 数据流处理

### 1. 数据过滤

```rust
pub struct DataFilter {
    /// 通道过滤
    channel_ids: Option<HashSet<u16>>,
    
    /// 点位类型过滤
    point_types: Option<HashSet<String>>,
    
    /// 值范围过滤
    value_range: Option<(f64, f64)>,
    
    /// 采样率
    sampling_rate: Option<f64>,
}

impl DataFilter {
    pub fn apply(&self, data: &PointData) -> Option<FilteredData> {
        // 通道过滤
        if let Some(channels) = &self.channel_ids {
            if !channels.contains(&data.channel_id) {
                return None;
            }
        }
        
        // 采样
        if let Some(rate) = self.sampling_rate {
            if rand::random::<f64>() > rate {
                return None;
            }
        }
        
        Some(FilteredData::from(data))
    }
}
```

### 2. 数据聚合

```rust
pub struct DataAggregator {
    window: Duration,
    aggregations: Vec<AggregationType>,
    buffer: Arc<RwLock<HashMap<String, Vec<f64>>>>,
}

impl DataAggregator {
    pub async fn aggregate(&self, key: String, value: f64) -> Option<AggregatedData> {
        let mut buffer = self.buffer.write().await;
        let values = buffer.entry(key.clone()).or_insert_with(Vec::new);
        values.push(value);
        
        // 检查是否到达窗口边界
        if self.should_emit(&values) {
            let aggregated = self.calculate_aggregations(&values);
            buffer.remove(&key);
            Some(aggregated)
        } else {
            None
        }
    }
}
```

## 安全设计

### 1. 认证机制

```rust
pub enum AuthMethod {
    /// X.509 证书
    Certificate {
        cert_path: String,
        key_path: String,
        ca_path: String,
    },
    
    /// Token 认证
    Token {
        token: String,
        refresh_url: String,
    },
    
    /// API Key
    ApiKey {
        key_id: String,
        secret: String,
    },
}
```

### 2. 数据加密

```rust
pub struct DataEncryption {
    cipher: Aes256Gcm,
    key_rotation_interval: Duration,
}

impl DataEncryption {
    pub fn encrypt_payload(&self, data: &[u8]) -> Result<Vec<u8>> {
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let ciphertext = self.cipher.encrypt(&nonce, data)?;
        
        // 组合 nonce 和密文
        let mut result = nonce.to_vec();
        result.extend_from_slice(&ciphertext);
        
        Ok(result)
    }
}
```

## 性能优化

### 1. 批量发送
- 聚合小消息减少请求次数
- 动态调整批次大小
- 优先级队列管理

### 2. 压缩策略
- Gzip 压缩大消息
- Protocol Buffers 序列化
- 增量更新

### 3. 连接池
- 复用 HTTP 连接
- MQTT 持久会话
- 连接健康检查

## 配置示例

```yaml
# netsrv 配置
redis:
  url: "redis://localhost:6379"
  
filters:
  - channel_ids: [1001, 1002, 1003]
    point_types: ["m", "s"]
    sampling_rate: 0.1
    
providers:
  aws_iot:
    enabled: true
    endpoint: "xxx.iot.region.amazonaws.com"
    thing_name: "voltage_ems_gateway"
    cert_path: "/certs/device.pem"
    key_path: "/certs/private.key"
    
  aliyun_iot:
    enabled: false
    product_key: "xxx"
    device_name: "gateway001"
    device_secret: "${ALIYUN_DEVICE_SECRET}"
    
cache:
  max_memory_mb: 100
  disk_path: "/var/cache/netsrv"
  
retry:
  max_attempts: 3
  initial_delay: 1s
  max_delay: 30s
```

## 监控指标

- 发送成功率
- 消息延迟
- 缓存命中率
- 连接状态
- 流量统计

## 故障处理

1. **网络中断**：本地缓存 + 自动重连
2. **认证失败**：Token 刷新 + 告警
3. **限流**：退避算法 + 队列管理
4. **数据丢失**：持久化队列 + 确认机制