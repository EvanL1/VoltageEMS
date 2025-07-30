# NetSrv 改进方案

## 当前状态
- 云端网关服务，负责数据转发
- 支持 MQTT、HTTP 协议
- 配置化的数据格式转换
- 基础的断线重传机制

## 改进建议

### 1. 智能路由系统

```rust
pub struct SmartRouter {
    routes: Vec<Route>,
    load_balancer: LoadBalancer,
    circuit_breaker: CircuitBreaker,
}

pub struct Route {
    name: String,
    matcher: DataMatcher,
    destinations: Vec<Destination>,
    transform: Option<Transform>,
    priority: u8,
}

impl SmartRouter {
    pub async fn route(&self, data: &DataPacket) -> Result<()> {
        // 根据数据类型和内容智能选择路由
        let matched_routes = self.routes.iter()
            .filter(|r| r.matcher.matches(data))
            .sorted_by_key(|r| r.priority);
            
        for route in matched_routes {
            // 选择健康的目标
            if let Some(dest) = self.load_balancer.select(&route.destinations) {
                // 应用数据转换
                let transformed = route.transform
                    .as_ref()
                    .map(|t| t.apply(data))
                    .unwrap_or_else(|| data.clone());
                    
                // 发送数据
                self.send_with_circuit_breaker(dest, transformed).await?;
            }
        }
        Ok(())
    }
}
```

### 2. 高级数据转换

```yaml
# 转换模板配置
transforms:
  - name: "aws_iot_format"
    type: "template"
    template: |
      {
        "deviceId": "{{ channel_id }}",
        "timestamp": {{ timestamp | date: "%Y-%m-%dT%H:%M:%S.%3fZ" }},
        "telemetry": {
          {% for key, value in measurements %}
          "{{ key }}": {{ value }}{% if not loop.last %},{% endif %}
          {% endfor %}
        },
        "metadata": {
          "source": "VoltageEMS",
          "version": "{{ version }}"
        }
      }
      
  - name: "azure_telemetry"
    type: "function"
    script: |
      function transform(data) {
        return {
          body: [{
            "timeStamp": new Date(data.timestamp * 1000).toISOString(),
            "deviceId": data.channel_id,
            "measurements": data.measurements
          }]
        };
      }
```

### 3. 断线缓存队列

```rust
pub struct ReliableQueue {
    memory_queue: Arc<RwLock<VecDeque<QueueItem>>>,
    disk_queue: DiskQueue,
    max_memory_items: usize,
}

pub struct QueueItem {
    id: Uuid,
    data: Vec<u8>,
    destination: String,
    retry_count: u32,
    created_at: Instant,
    priority: Priority,
}

impl ReliableQueue {
    pub async fn enqueue(&self, item: QueueItem) -> Result<()> {
        let mut queue = self.memory_queue.write().await;
        
        if queue.len() >= self.max_memory_items {
            // 内存满时写入磁盘
            self.spill_to_disk(&mut queue).await?;
        }
        
        // 按优先级插入
        let pos = queue.iter()
            .position(|i| i.priority < item.priority)
            .unwrap_or(queue.len());
            
        queue.insert(pos, item);
        Ok(())
    }
    
    pub async fn process_queue(&self) -> Result<()> {
        loop {
            let item = self.dequeue_next().await?;
            
            match self.send_item(&item).await {
                Ok(_) => self.mark_success(&item.id).await?,
                Err(e) if e.is_retryable() => {
                    self.requeue_with_backoff(item).await?;
                }
                Err(_) => self.move_to_dlq(item).await?,
            }
        }
    }
}
```

### 4. 协议适配器框架

```rust
#[async_trait]
pub trait ProtocolAdapter: Send + Sync {
    async fn connect(&mut self, config: &Config) -> Result<()>;
    async fn send(&self, data: &[u8]) -> Result<()>;
    async fn batch_send(&self, data: &[Vec<u8>]) -> Result<()>;
    fn supports_batch(&self) -> bool;
    fn max_batch_size(&self) -> usize;
}

// MQTT 适配器示例
pub struct MqttAdapter {
    client: MqttClient,
    qos: QoS,
}

#[async_trait]
impl ProtocolAdapter for MqttAdapter {
    async fn send(&self, data: &[u8]) -> Result<()> {
        self.client.publish(self.topic, self.qos, false, data).await
    }
    
    fn supports_batch(&self) -> bool { false }
    fn max_batch_size(&self) -> usize { 1 }
}

// HTTP 批量适配器
pub struct HttpBatchAdapter {
    client: reqwest::Client,
    endpoint: Url,
    batch_size: usize,
}

#[async_trait]
impl ProtocolAdapter for HttpBatchAdapter {
    async fn batch_send(&self, data: &[Vec<u8>]) -> Result<()> {
        let batch_request = BatchRequest { items: data };
        self.client.post(self.endpoint.clone())
            .json(&batch_request)
            .send()
            .await?;
        Ok(())
    }
    
    fn supports_batch(&self) -> bool { true }
    fn max_batch_size(&self) -> usize { self.batch_size }
}
```

### 5. 流量控制与限流

```rust
pub struct RateLimiter {
    limiters: HashMap<String, TokenBucket>,
}

pub struct TokenBucket {
    capacity: u64,
    tokens: AtomicU64,
    refill_rate: u64,
    last_refill: AtomicU64,
}

impl RateLimiter {
    pub async fn check_rate_limit(&self, destination: &str, size: u64) -> Result<()> {
        let limiter = self.limiters.get(destination)
            .ok_or_else(|| Error::NoLimiterConfigured)?;
            
        if !limiter.try_consume(size) {
            return Err(Error::RateLimitExceeded);
        }
        
        Ok(())
    }
}
```

### 6. 监控与告警

```rust
pub struct NetSrvMetrics {
    // 发送指标
    messages_sent: CounterVec,
    bytes_sent: CounterVec,
    send_duration: HistogramVec,
    
    // 错误指标
    send_errors: CounterVec,
    retry_count: CounterVec,
    
    // 队列指标
    queue_depth: GaugeVec,
    queue_age: HistogramVec,
    
    // 连接指标
    active_connections: GaugeVec,
    connection_errors: CounterVec,
}
```

## 配置示例

```yaml
netsrv:
  routes:
    # AWS IoT Core 路由
    - name: "aws_telemetry"
      match:
        data_type: ["measurement", "signal"]
        channels: ["1001", "1002"]
      destination:
        type: "mqtt"
        endpoint: "xxx.iot.amazonaws.com"
        topic: "dt/{{ channel_id }}/telemetry"
      transform: "aws_iot_format"
      rate_limit:
        messages_per_second: 100
        bytes_per_second: 10485760  # 10MB/s
        
    # 阿里云 IoT 路由
    - name: "aliyun_batch"
      match:
        data_type: ["measurement"]
      destination:
        type: "http"
        endpoint: "https://iot.aliyuncs.com/api/batch"
        batch_size: 100
        batch_timeout: 5s
      transform: "aliyun_format"
      
  queue:
    memory_size: 100000
    disk_path: "/var/lib/netsrv/queue"
    retry_policy:
      max_retries: 3
      backoff: "exponential"
      max_backoff: 300s
      
  monitoring:
    export_interval: 10s
    alert_rules:
      - name: "high_error_rate"
        condition: "error_rate > 0.05"
        severity: "warning"
      - name: "queue_overflow"
        condition: "queue_depth > 90000"
        severity: "critical"
```

## 实施优先级

1. **高**：断线缓存队列（保证数据不丢失）
2. **高**：智能路由（灵活的数据分发）
3. **中**：协议适配器框架（扩展性）
4. **低**：高级数据转换（灵活性）

## 预期效果

- 数据零丢失保证
- 支持复杂的路由规则
- 灵活的协议扩展
- 完善的监控告警