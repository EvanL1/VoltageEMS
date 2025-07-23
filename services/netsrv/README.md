# netsrv - 云网关服务

## 概述

netsrv 是 VoltageEMS 的云网关服务，负责将系统数据转发到外部云平台和第三方系统。服务从 Redis 读取实时数据和告警信息，通过多种协议（MQTT、HTTP、AWS IoT、阿里云 IoT）将数据推送到目标系统。所有数值保持 6 位小数精度。

## 主要特性

- **多协议支持**: MQTT、HTTP/HTTPS、AWS IoT Core、阿里云 IoT
- **灵活的数据格式**: JSON 和 ASCII 格式，可自定义模板
- **数据过滤**: 支持通道、点位、数据类型的灵活过滤
- **批量传输**: 自动批量打包数据，优化网络传输
- **断线重连**: 自动处理网络故障，确保数据传输可靠性
- **标准化精度**: 所有浮点数保持 6 位小数精度

## 快速开始

### 运行服务

```bash
cd services/netsrv
cargo run
```

### 配置文件

主配置文件位于 `config/default.yml`：

```yaml
service:
  name: "netsrv"
  host: "0.0.0.0"
  port: 8086
  
redis:
  url: "redis://localhost:6379"
  
network:
  targets:
    - name: "cloud_mqtt"
      type: "mqtt"
      enabled: true
      config:
        broker: "mqtt://broker.example.com:1883"
        client_id: "voltage_ems_001"
        topic_prefix: "ems/data"
        
    - name: "http_api"
      type: "http"
      enabled: true
      config:
        endpoint: "https://api.example.com/data"
        method: "POST"
        headers:
          Authorization: "Bearer ${API_TOKEN}"
          
monitoring:
  sources:
    - pattern: "comsrv:*:m"       # 测量数据
    - pattern: "comsrv:*:s"       # 信号数据
    - pattern: "modsrv:*:measurement"  # 计算结果
    - pattern: "alarm:*"          # 告警数据
    
logging:
  level: "info"
  file: "logs/netsrv.log"
```

## 数据源配置

### 监听 Redis 数据

netsrv 支持监听多种数据源：

```yaml
# 实时数据监听
data_sources:
  # comsrv 原始数据
  - source: "comsrv"
    patterns:
      - "comsrv:*:m"  # 测量值
      - "comsrv:*:s"  # 信号值
    format: "raw"
    
  # modsrv 计算结果
  - source: "modsrv"
    patterns:
      - "modsrv:*:measurement"
      - "modsrv:*:control"
    format: "calculated"
    
  # 告警数据
  - source: "alarmsrv"
    patterns:
      - "alarm:*"
    format: "alarm"
    subscribe_events: true  # 订阅告警事件
```

### 数据过滤规则

```yaml
filters:
  # 按通道过滤
  - type: "channel"
    include: [1001, 1002, 1003]
    exclude: [2001]
    
  # 按数据类型过滤
  - type: "data_type"
    include: ["m", "s"]  # 只要测量和信号
    
  # 按值范围过滤
  - type: "value_range"
    field: "voltage"
    min: 180.0
    max: 250.0
    
  # 按时间过滤
  - type: "time_window"
    start_hour: 8
    end_hour: 18
    weekdays: [1, 2, 3, 4, 5]  # 工作日
```

## 目标配置

### MQTT 配置

```yaml
mqtt_targets:
  - name: "cloud_mqtt"
    broker: "mqtt://broker.hivemq.com:1883"
    client_id: "voltage_ems_${DEVICE_ID}"
    username: "${MQTT_USER}"
    password: "${MQTT_PASS}"
    qos: 1
    retain: false
    
    # 主题映射
    topics:
      telemetry: "devices/${device_id}/telemetry"
      alarms: "devices/${device_id}/alarms"
      status: "devices/${device_id}/status"
      
    # 数据格式
    payload_format:
      type: "json"
      template: |
        {
          "timestamp": "${timestamp}",
          "channel": ${channel_id},
          "point": ${point_id},
          "value": ${value},
          "unit": "${unit}"
        }
```

### HTTP API 配置

```yaml
http_targets:
  - name: "rest_api"
    endpoint: "https://api.example.com/v1/data"
    method: "POST"
    
    headers:
      Content-Type: "application/json"
      Authorization: "Bearer ${API_TOKEN}"
      X-Device-ID: "${DEVICE_ID}"
      
    # 批量设置
    batch:
      enabled: true
      max_size: 100
      max_wait_ms: 5000
      
    # 重试策略
    retry:
      max_attempts: 3
      backoff_ms: 1000
      exponential: true
      
    # 请求体格式
    body_format:
      type: "json"
      structure: "array"  # array 或 object
```

### AWS IoT Core 配置

```yaml
aws_iot:
  - name: "aws_cloud"
    region: "us-west-2"
    endpoint: "${AWS_IOT_ENDPOINT}"
    
    # 认证方式
    auth:
      type: "certificate"
      cert_path: "/certs/device.pem.crt"
      key_path: "/certs/private.pem.key"
      ca_path: "/certs/root-CA.crt"
      
    # 影子设备
    thing_name: "voltage_ems_001"
    update_shadow: true
    
    # 主题规则
    topics:
      telemetry: "$aws/things/${thing_name}/shadow/update"
      alarms: "dt/alarms/${thing_name}"
```

### 阿里云 IoT 配置

```yaml
aliyun_iot:
  - name: "aliyun_cloud"
    product_key: "${ALIYUN_PRODUCT_KEY}"
    device_name: "${ALIYUN_DEVICE_NAME}"
    device_secret: "${ALIYUN_DEVICE_SECRET}"
    
    region: "cn-shanghai"
    
    # 物模型映射
    property_mapping:
      temperature: "Temperature"
      voltage: "Voltage"
      current: "Current"
      power: "ActivePower"
```

## 数据格式化

### JSON 格式化器

```rust
// 标准 JSON 格式
{
  "device_id": "voltage_ems_001",
  "timestamp": "2025-07-23T10:00:00Z",
  "data": [
    {
      "channel": 1001,
      "type": "m",
      "point": 10001,
      "value": 220.123456,
      "name": "voltage_a"
    }
  ]
}

// 扁平化 JSON 格式
{
  "device_id": "voltage_ems_001",
  "timestamp": "2025-07-23T10:00:00Z",
  "channel_1001_m_10001": 220.123456,
  "channel_1001_m_10002": 380.654321
}
```

### ASCII 格式化器

```yaml
ascii_format:
  template: "${timestamp},${channel},${point},${value}\n"
  delimiter: ","
  line_ending: "\n"
  precision: 6
```

### 自定义模板

```yaml
custom_templates:
  - name: "iec61850"
    format: |
      <DataSet name="${dataset}">
        <Timestamp>${timestamp}</Timestamp>
        <MMXU>
          <PhV>
            <phsA>${voltage_a}</phsA>
            <phsB>${voltage_b}</phsB>
            <phsC>${voltage_c}</phsC>
          </PhV>
        </MMXU>
      </DataSet>
```

## 批量传输

### 批量配置

```yaml
batch_config:
  # 批量大小
  max_batch_size: 100
  
  # 最大等待时间
  max_wait_time_ms: 5000
  
  # 压缩选项
  compression:
    enabled: true
    algorithm: "gzip"
    level: 6
    
  # 分组策略
  grouping:
    by: ["channel", "type"]
    preserve_order: true
```

### 批量数据示例

```json
{
  "batch_id": "550e8400-e29b-41d4",
  "device_id": "voltage_ems_001",
  "timestamp": "2025-07-23T10:00:00Z",
  "count": 50,
  "data": [
    {
      "channel": 1001,
      "measurements": {
        "10001": 220.123456,
        "10002": 221.234567,
        "10003": 219.345678
      }
    }
  ]
}
```

## 告警转发

### 告警订阅

```rust
// 订阅告警事件
pub async fn subscribe_alarm_events(&self) -> Result<()> {
    let mut pubsub = self.redis_client.get_async_pubsub().await?;
    pubsub.subscribe("alarm:events").await?;
    
    while let Some(msg) = pubsub.on_message().next().await {
        let event: AlarmEvent = serde_json::from_str(&msg.get_payload()?)?;
        self.forward_alarm(event).await?;
    }
    
    Ok(())
}
```

### 告警格式化

```json
{
  "alarm_id": "550e8400-e29b-41d4",
  "type": "temperature_high",
  "level": "critical",
  "title": "高温告警",
  "description": "温度超过阈值",
  "source": {
    "channel": 1001,
    "point": 10001,
    "value": 45.500000
  },
  "timestamp": "2025-07-23T10:00:00Z"
}
```

## 连接管理

### 断线重连

```yaml
connection:
  # 重连策略
  reconnect:
    enabled: true
    max_attempts: -1  # 无限重试
    initial_delay_ms: 1000
    max_delay_ms: 60000
    exponential_backoff: true
    
  # 心跳检测
  keepalive:
    enabled: true
    interval_seconds: 30
    timeout_seconds: 10
    
  # 连接池（HTTP）
  pool:
    max_idle_connections: 10
    idle_timeout_seconds: 90
```

### 故障转移

```yaml
failover:
  # 主备配置
  targets:
    - name: "primary"
      priority: 1
      health_check_interval: 30
      
    - name: "backup"
      priority: 2
      activate_on_failure: true
```

## 监控指标

通过 `/metrics` 端点暴露 Prometheus 指标：

- `netsrv_messages_sent_total` - 发送消息总数
- `netsrv_messages_failed_total` - 发送失败总数
- `netsrv_connection_status` - 连接状态
- `netsrv_send_duration_seconds` - 发送耗时
- `netsrv_batch_size` - 批量大小

## API 接口

### 连接状态

```bash
# 获取所有连接状态
GET /connections

# 获取特定连接
GET /connections/{name}

# 手动重连
POST /connections/{name}/reconnect

# 暂停/恢复连接
POST /connections/{name}/pause
POST /connections/{name}/resume
```

### 统计信息

```bash
# 获取传输统计
GET /stats

# 清空统计
DELETE /stats
```

## 故障排查

### 连接问题

```bash
# 检查网络连接
curl -v https://api.example.com/health

# 测试 MQTT 连接
mosquitto_sub -h broker.example.com -t test/#

# 查看连接日志
tail -f logs/netsrv.log | grep connection
```

### 数据问题

```bash
# 监控 Redis 数据
redis-cli monitor | grep "comsrv"

# 检查数据格式
curl http://localhost:8086/debug/last-payload
```

## 环境变量

- `RUST_LOG` - 日志级别
- `NETSRV_CONFIG` - 配置文件路径
- `REDIS_URL` - Redis 连接地址
- 支持在配置中使用 `${VAR_NAME}` 引用环境变量

## 相关文档

- [架构设计](docs/architecture.md)
- [协议适配器](docs/protocol-adapters.md)
- [数据格式化](docs/data-formatting.md)