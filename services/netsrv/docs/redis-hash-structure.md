# netsrv Redis Hash结构设计

## 概述
netsrv（网络服务）负责将实时数据同步到云平台，支持多种协议（MQTT、HTTP、云厂商IoT平台）。采用Hash结构存储云端同步状态，实现高效的状态管理和性能监控。

## Hash键结构

### 云端同步状态
```
Key Pattern: netsrv:cloud:status:{network_name}
```

### 数据缓冲区
```
Key Pattern: netsrv:buffer:queue:{network_name}
```

### 配置缓存
```
Key Pattern: netsrv:config:network:{network_name}
```

## 云端状态结构

### 状态数据示例
```json
Key: netsrv:cloud:status:aws-iot
Fields:
  connected: "true"
  connection_time: "2025-01-10T08:00:00Z"
  last_sync_time: "2025-01-10T10:30:00.123Z"
  last_error: ""
  last_error_time: ""
  messages_sent: "156420"
  messages_failed: "23"
  bytes_sent: "104857600"
  average_latency_ms: "45.6"
  queue_size: "0"
  retry_count: "0"
  updated_at: "2025-01-10T10:30:00.123Z"
  
  # 云平台特定信息
  endpoint: "a1b2c3d4e5f6g7.iot.cn-north-1.amazonaws.com.cn"
  thing_name: "voltage-ems-gateway-01"
  certificate_expiry: "2026-01-10T00:00:00Z"
  sdk_version: "1.5.0"
```

### 多网络状态示例
```json
Key: netsrv:cloud:status:aliyun-mqtt
Fields:
  connected: "true"
  messages_sent: "145320"
  messages_failed: "15"
  product_key: "a1234567890"
  device_name: "device001"
  
Key: netsrv:cloud:status:http-webhook  
Fields:
  connected: "true"
  messages_sent: "98765"
  messages_failed: "5"
  endpoint_url: "https://api.example.com/v1/data"
  auth_type: "bearer_token"
```

## 数据读取策略

### 从优化的Hash结构读取
```rust
pub async fn fetch_optimized_data(&mut self) -> Result<Value> {
    let mut all_data = json!({});
    
    // 读取comsrv通道数据
    for channel_id in &self.config.source_channels {
        let key = format!("comsrv:realtime:channel:{}", channel_id);
        if let Ok(data) = self.redis.hgetall(&key).await {
            all_data[format!("channel_{}", channel_id)] = parse_channel_data(data);
        }
    }
    
    // 读取modsrv模块数据
    for module_id in &self.config.source_modules {
        let key = format!("modsrv:realtime:module:{}", module_id);
        if let Ok(data) = self.redis.hgetall(&key).await {
            all_data[module_id] = parse_module_data(data);
        }
    }
    
    Ok(all_data)
}
```

### 数据过滤和转换
```rust
pub struct DataFilter {
    pub include_points: Vec<String>,      // 包含的点位
    pub exclude_points: Vec<String>,      // 排除的点位
    pub value_transform: Transform,       // 值转换
    pub sampling_rate: u32,              // 采样率
    pub change_threshold: f64,           // 变化阈值
}
```

## 云平台适配

### AWS IoT Core
```rust
pub struct AwsIotAdapter {
    // 连接配置
    endpoint: String,
    port: u16,
    client_id: String,
    
    // 证书配置
    cert_path: String,
    key_path: String,
    ca_path: String,
    
    // 主题配置
    data_topic: String,      // 数据上报主题
    command_topic: String,   // 命令接收主题
    shadow_topic: String,    // 设备影子主题
}
```

### 阿里云物联网平台
```rust
pub struct AliyunIotAdapter {
    // 三元组
    product_key: String,
    device_name: String,
    device_secret: String,
    
    // 物模型
    thing_model: ThingModel,
    
    // 主题映射
    property_post: String,   // 属性上报
    event_post: String,      // 事件上报
    service_reply: String,   // 服务响应
}
```

### HTTP Webhook
```rust
pub struct HttpWebhookAdapter {
    // 端点配置
    url: String,
    method: String,
    headers: HashMap<String, String>,
    
    // 认证配置
    auth_type: AuthType,
    credentials: Credentials,
    
    // 批量配置
    batch_size: usize,
    batch_timeout_ms: u64,
}
```

## 缓冲队列管理

### 离线缓冲
```
Key: netsrv:buffer:queue:aws-iot
Type: List
Elements: [
  {
    "timestamp": "2025-01-10T10:25:00Z",
    "data": {...},
    "retry_count": 0
  },
  ...
]
```

### 缓冲策略
```rust
pub struct BufferStrategy {
    pub max_size: usize,           // 最大缓冲条数
    pub max_memory: usize,         // 最大内存占用
    pub overflow_policy: Policy,   // 溢出策略
    pub compression: bool,         // 是否压缩
}

pub enum Policy {
    DropOldest,    // 丢弃最旧数据
    DropNewest,    // 丢弃最新数据
    Persist,       // 持久化到磁盘
}
```

## 性能监控

### 实时指标
```
Key: netsrv:metrics:realtime
Fields:
  current_qps: "523.4"          # 每秒消息数
  current_bandwidth: "2.3"      # MB/s
  active_connections: "4"       # 活跃连接数
  cpu_usage: "15.6"            # CPU使用率
  memory_usage: "256"          # MB
  goroutines: "42"            # 协程数
```

### 历史统计
```
Key: netsrv:metrics:hourly:2025011010
Fields:
  total_messages: "1876543"
  total_bytes: "1073741824"
  success_rate: "99.8"
  avg_latency: "43.2"
  peak_qps: "1250.5"
  errors: {
    "timeout": 15,
    "auth_failed": 2,
    "rate_limit": 8
  }
```

## 数据格式化

### JSON格式化
```json
{
  "deviceId": "voltage-ems-01",
  "timestamp": "2025-01-10T10:30:00.123Z",
  "data": {
    "channel_1": {
      "point_1001": {
        "value": 65.3,
        "unit": "°C",
        "quality": "good"
      }
    }
  },
  "metadata": {
    "version": "1.0",
    "source": "netsrv"
  }
}
```

### 物模型格式化
```json
{
  "id": "123456",
  "version": "1.0",
  "method": "thing.property.post",
  "params": {
    "temperature": {
      "value": 65.3,
      "time": 1641830400000
    },
    "power": {
      "value": 1234.56,
      "time": 1641830400000
    }
  }
}
```

## 安全管理

### 凭证存储
```
Key: netsrv:security:credentials:{network_name}
Fields:
  auth_type: "certificate"
  cert_fingerprint: "SHA256:1234567890abcdef"
  cert_expiry: "2026-01-10T00:00:00Z"
  last_rotation: "2025-01-10T00:00:00Z"
  rotation_status: "success"
```

### 访问控制
```rust
pub struct AccessControl {
    pub allowed_ips: Vec<IpAddr>,
    pub rate_limit: RateLimit,
    pub auth_required: bool,
    pub tls_required: bool,
}
```

## 故障处理

### 重试机制
```rust
pub struct RetryConfig {
    pub max_retries: u32,
    pub initial_delay_ms: u64,
    pub max_delay_ms: u64,
    pub backoff_multiplier: f64,
    pub jitter: bool,
}

// 指数退避算法
fn calculate_delay(attempt: u32, config: &RetryConfig) -> Duration {
    let delay = config.initial_delay_ms * 
                (config.backoff_multiplier.powi(attempt as i32)) as u64;
    let delay = delay.min(config.max_delay_ms);
    
    if config.jitter {
        // 添加随机抖动
        delay * (0.5 + rand::random::<f64>() * 0.5) as u64
    } else {
        delay
    }
}
```

### 熔断器
```rust
pub struct CircuitBreaker {
    pub failure_threshold: u32,
    pub success_threshold: u32,
    pub timeout: Duration,
    pub half_open_requests: u32,
    
    state: State,
    failure_count: u32,
    success_count: u32,
    last_failure_time: Option<Instant>,
}
```

## 配置管理

### 网络配置缓存
```
Key: netsrv:config:network:aws-iot
Fields:
  enabled: "true"
  protocol: "mqtt"
  qos: "1"
  keep_alive: "60"
  clean_session: "false"
  max_message_size: "262144"
  compression: "gzip"
  priority: "high"
```

### 动态配置更新
```rust
pub async fn reload_config(&mut self, network_name: &str) -> Result<()> {
    // 从配置中心获取最新配置
    let new_config = self.fetch_config_from_center(network_name).await?;
    
    // 更新Redis缓存
    let key = format!("netsrv:config:network:{}", network_name);
    self.redis.hset_all(&key, &new_config).await?;
    
    // 应用新配置
    self.apply_config(network_name, new_config).await?;
    
    Ok(())
}
```

## 监控告警

### 告警阈值
```yaml
alerts:
  - name: "高失败率"
    condition: "failure_rate > 5%"
    duration: "5m"
    severity: "warning"
    
  - name: "连接断开"
    condition: "connected == false"
    duration: "1m"
    severity: "critical"
    
  - name: "队列积压"
    condition: "queue_size > 10000"
    duration: "10m"
    severity: "major"
```

### 健康检查
```rust
pub async fn health_check(&self) -> HealthStatus {
    let mut status = HealthStatus::new();
    
    // 检查各网络连接状态
    for network in &self.networks {
        let net_status = self.check_network_health(network).await;
        status.add_component(network.name(), net_status);
    }
    
    // 检查Redis连接
    status.add_component("redis", self.check_redis_health().await);
    
    // 计算整体健康度
    status.calculate_overall();
    
    status
}
```

## 最佳实践

1. **合理设置缓冲区**：根据网络带宽和稳定性调整
2. **数据压缩传输**：大数据量启用压缩
3. **批量发送优化**：平衡延迟和吞吐量
4. **监控关键指标**：及时发现和处理问题
5. **安全凭证轮换**：定期更新证书和密钥
6. **灾备方案完善**：多区域部署和故障切换
7. **日志审计完整**：记录所有关键操作