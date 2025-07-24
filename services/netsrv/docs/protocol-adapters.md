# 协议适配器配置指南

## 概述

netsrv 支持多种通信协议，通过插件化的协议适配器实现灵活的数据转发。本文档详细介绍各协议适配器的配置和使用方法。

## MQTT 适配器

### 基本配置

```yaml
mqtt:
  - name: "primary_mqtt"
    enabled: true
    broker: "mqtt://broker.hivemq.com:1883"
    client_id: "voltage_ems_${DEVICE_ID}"
    username: "${MQTT_USERNAME}"
    password: "${MQTT_PASSWORD}"
    
    # 连接选项
    connection:
      keep_alive: 30
      clean_session: true
      auto_reconnect: true
      reconnect_interval: 5
      
    # QoS 和保留设置
    qos: 1  # 0, 1, 2
    retain: false
    
    # SSL/TLS 配置
    tls:
      enabled: false
      ca_cert: "/certs/ca.pem"
      client_cert: "/certs/client.pem"
      client_key: "/certs/client.key"
      verify_server: true
```

### 主题映射

```yaml
mqtt:
  topics:
    # 静态主题
    telemetry: "devices/${device_id}/telemetry"
    alarms: "devices/${device_id}/alarms"
    status: "devices/${device_id}/status"
    
    # 动态主题模板
    dynamic:
      - pattern: "comsrv:*:m"
        template: "data/channel/${channel}/measurements"
      - pattern: "comsrv:*:s"
        template: "data/channel/${channel}/signals"
      - pattern: "modsrv:*:*"
        template: "calculated/${model}/${type}"
```

### 数据分片

对于大量数据，支持按主题分片：

```yaml
mqtt:
  sharding:
    enabled: true
    strategy: "hash"  # hash, round-robin, range
    shards:
      - topic_suffix: "/shard0"
        channels: [1001, 1002]
      - topic_suffix: "/shard1"
        channels: [1003, 1004]
```

### MQTT 5.0 特性

```yaml
mqtt:
  version: 5
  properties:
    # 消息过期
    message_expiry_interval: 3600
    
    # 用户属性
    user_properties:
      source: "voltage_ems"
      version: "1.0"
      
    # 响应主题
    response_topic: "devices/${device_id}/response"
    correlation_data: true
```

## HTTP/HTTPS 适配器

### 基本配置

```yaml
http:
  - name: "cloud_api"
    enabled: true
    endpoint: "https://api.example.com/v1/data"
    method: "POST"
    
    # 认证
    auth:
      type: "bearer"  # basic, bearer, api_key, custom
      token: "${API_TOKEN}"
      
    # 请求头
    headers:
      Content-Type: "application/json"
      X-Device-ID: "${DEVICE_ID}"
      X-API-Version: "1.0"
      
    # 超时设置
    timeout:
      connect: 10
      request: 30
      idle: 90
```

### 批量请求

```yaml
http:
  batch:
    enabled: true
    max_size: 100
    max_bytes: 1048576  # 1MB
    flush_interval: 5000  # ms
    
    # 批量格式
    format:
      type: "array"  # array, ndjson, custom
      wrapper:
        batch_id: "${batch_id}"
        timestamp: "${timestamp}"
        count: "${count}"
        data: "${data}"
```

### 重试策略

```yaml
http:
  retry:
    enabled: true
    max_attempts: 3
    
    # 退避策略
    backoff:
      type: "exponential"  # fixed, linear, exponential
      initial_delay: 1000
      max_delay: 60000
      multiplier: 2
      
    # 重试条件
    retry_on:
      status_codes: [408, 429, 500, 502, 503, 504]
      network_errors: true
      timeout: true
```

### 请求签名

```yaml
http:
  signature:
    enabled: true
    algorithm: "hmac-sha256"
    secret: "${SIGNING_SECRET}"
    
    # 签名内容
    include:
      - method
      - path
      - timestamp
      - body
      
    # 签名头
    header: "X-Signature"
```

### 负载均衡

```yaml
http:
  load_balancing:
    enabled: true
    strategy: "round_robin"  # round_robin, least_connections, weighted
    
    endpoints:
      - url: "https://api1.example.com/data"
        weight: 3
      - url: "https://api2.example.com/data"
        weight: 2
      - url: "https://api3.example.com/data"
        weight: 1
```

## AWS IoT Core 适配器

### 设备配置

```yaml
aws_iot:
  - name: "aws_primary"
    enabled: true
    region: "us-west-2"
    
    # 设备信息
    thing:
      name: "${AWS_THING_NAME}"
      type: "VoltageEMS"
      attributes:
        location: "factory_01"
        firmware: "1.0.0"
```

### 认证方式

#### 证书认证

```yaml
aws_iot:
  auth:
    type: "certificate"
    cert_path: "/certs/device-certificate.pem.crt"
    key_path: "/certs/private.pem.key"
    ca_path: "/certs/root-CA.crt"
    
    # 证书轮换
    rotation:
      enabled: true
      check_interval: 86400  # 每天检查
      expire_warning: 2592000  # 30天警告
```

#### IAM 角色认证

```yaml
aws_iot:
  auth:
    type: "iam_role"
    role_arn: "arn:aws:iam::123456789012:role/IoTDeviceRole"
    session_name: "voltage_ems_session"
```

### 设备影子

```yaml
aws_iot:
  shadow:
    enabled: true
    
    # 影子文档结构
    reported:
      # 状态数据
      state:
        - source: "comsrv:1001:s"
          path: "status.channel_1001"
          
      # 测量数据
      measurements:
        - source: "comsrv:1001:m:10001"
          path: "telemetry.voltage"
          interval: 60  # 更新间隔（秒）
          
    # 期望状态处理
    desired:
      enabled: true
      handlers:
        - path: "config.threshold"
          action: "update_config"
```

### 规则引擎集成

```yaml
aws_iot:
  rules:
    # 数据路由规则
    - name: "route_telemetry"
      sql: "SELECT * FROM 'topic/telemetry' WHERE value > 100"
      actions:
        - type: "dynamodb"
          table: "telemetry_data"
        - type: "sns"
          topic: "arn:aws:sns:us-west-2:123456789012:alerts"
```

### 批量上传

```yaml
aws_iot:
  batch:
    enabled: true
    topic: "$aws/rules/batch_upload/topic"
    format: "json_lines"
    compression: "gzip"
    max_size: 5242880  # 5MB
```

## 阿里云 IoT 适配器

### 设备认证

```yaml
aliyun_iot:
  - name: "aliyun_primary"
    enabled: true
    
    # 三元组
    product_key: "${ALIYUN_PRODUCT_KEY}"
    device_name: "${ALIYUN_DEVICE_NAME}"
    device_secret: "${ALIYUN_DEVICE_SECRET}"
    
    # 区域设置
    region: "cn-shanghai"
    
    # 连接模式
    mode: "direct"  # direct, gateway
```

### 物模型映射

```yaml
aliyun_iot:
  thing_model:
    # 属性映射
    properties:
      - identifier: "Temperature"
        source: "comsrv:1001:m:10001"
        dataType: "float"
        
      - identifier: "Voltage"
        source: "comsrv:1001:m:10002"
        dataType: "float"
        
    # 事件映射
    events:
      - identifier: "HighTempAlarm"
        source: "alarm:temperature_high"
        type: "warning"
        
    # 服务映射
    services:
      - identifier: "SetThreshold"
        handler: "update_threshold"
```

### 数据格式

```yaml
aliyun_iot:
  format:
    # Alink 协议
    type: "alink"
    version: "1.0"
    
    # 自定义格式
    custom:
      enabled: false
      script: "/scripts/aliyun_formatter.js"
```

### OTA 升级

```yaml
aliyun_iot:
  ota:
    enabled: true
    check_interval: 3600  # 每小时检查
    download_path: "/tmp/ota"
    
    # 升级策略
    strategy:
      auto_upgrade: false
      verify_signature: true
      rollback_on_failure: true
```

## 自定义协议适配器

### 开发指南

```rust
use async_trait::async_trait;
use crate::protocol::{ProtocolAdapter, AdapterConfig};

pub struct CustomAdapter {
    config: CustomConfig,
    client: Option<CustomClient>,
}

#[async_trait]
impl ProtocolAdapter for CustomAdapter {
    async fn connect(&mut self) -> Result<()> {
        // 实现连接逻辑
        let client = CustomClient::new(&self.config)?;
        client.connect().await?;
        self.client = Some(client);
        Ok(())
    }
    
    async fn send(&self, data: &[u8]) -> Result<()> {
        // 实现发送逻辑
        if let Some(client) = &self.client {
            client.send(data).await?;
        }
        Ok(())
    }
    
    async fn disconnect(&mut self) -> Result<()> {
        // 实现断开逻辑
        if let Some(client) = self.client.take() {
            client.disconnect().await?;
        }
        Ok(())
    }
}
```

### 配置架构

```yaml
custom_protocol:
  - name: "my_protocol"
    enabled: true
    adapter_class: "CustomAdapter"
    
    # 自定义配置
    config:
      server: "custom://server.example.com:9000"
      auth:
        method: "custom_auth"
        credentials: "${CUSTOM_CREDS}"
        
      # 协议特定选项
      options:
        buffer_size: 8192
        compression: true
        encryption: "aes256"
```

### 注册适配器

```rust
// 在 main.rs 中注册
let adapter_registry = AdapterRegistry::new();
adapter_registry.register(
    "custom",
    Box::new(|| Box::new(CustomAdapter::new()))
);
```

## 性能调优

### 连接池配置

```yaml
performance:
  connection_pool:
    min_connections: 5
    max_connections: 20
    idle_timeout: 300
    
  # 发送队列
  send_queue:
    size: 10000
    overflow_policy: "drop_oldest"  # drop_oldest, drop_newest, block
    
  # 工作线程
  workers:
    count: 4
    queue_size: 1000
```

### 内存优化

```yaml
memory:
  # 缓冲区复用
  buffer_pool:
    enabled: true
    size: 100
    buffer_size: 65536
    
  # 消息去重
  deduplication:
    enabled: true
    window: 60  # 秒
    max_entries: 10000
```

## 监控和诊断

### 健康检查

```yaml
health_check:
  enabled: true
  interval: 30
  timeout: 10
  
  # 检查项
  checks:
    - type: "connectivity"
      threshold: 3  # 连续失败次数
    - type: "latency"
      threshold: 1000  # ms
    - type: "throughput"
      threshold: 100  # msg/s
```

### 诊断信息

```bash
# 获取适配器状态
GET /adapters/{name}/status

# 获取连接统计
GET /adapters/{name}/stats

# 测试连接
POST /adapters/{name}/test

# 获取最后错误
GET /adapters/{name}/last-error
```

## 故障处理

### 常见问题

1. **MQTT 连接失败**
   ```yaml
   # 检查防火墙和端口
   # 验证证书有效性
   # 确认客户端 ID 唯一性
   ```

2. **HTTP 429 错误**
   ```yaml
   # 启用限流
   rate_limit:
     enabled: true
     requests_per_second: 10
   ```

3. **AWS IoT 限制**
   ```yaml
   # 遵守服务限制
   throttling:
     messages_per_second: 100
     payload_size: 128KB
   ```

### 调试模式

```yaml
debug:
  enabled: true
  log_payload: true
  log_headers: true
  save_failed_messages: true
  failed_message_dir: "/tmp/failed_messages"
```