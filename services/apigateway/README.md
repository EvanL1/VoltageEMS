# apigateway - API 网关服务

## 概述

apigateway 是 VoltageEMS 的统一 API 网关，为前端应用提供一致的 RESTful API 和 WebSocket 接口。服务采用智能路由策略，根据数据类型自动选择最优的访问路径，支持实时数据、配置管理、历史查询等多种场景。所有数值保持 6 位小数精度。

## 主要特性

- **统一入口**: 所有外部请求的单一入口点
- **智能路由**: 根据数据类型自动选择 Redis、InfluxDB 或 HTTP 路径
- **JWT 认证**: 安全的用户身份验证和授权
- **WebSocket 支持**: 实时数据推送和双向通信
- **混合数据访问**: 多层缓存架构，优化性能
- **标准化精度**: 所有浮点数保持 6 位小数精度

## 快速开始

### 运行服务

```bash
cd services/apigateway
cargo run
```

### 配置文件

主配置文件位于 `apigateway.yaml`：

```yaml
server:
  host: "0.0.0.0"
  port: 8080
  workers: 4
  
redis:
  url: "redis://localhost:6379"
  pool_size: 10
  timeout_seconds: 5
  
influxdb:
  url: "http://localhost:8086"
  org: "voltage-ems"
  bucket: "ems_data"
  token: "${INFLUXDB_TOKEN}"
  
services:
  comsrv:
    url: "http://localhost:8081"
    timeout_seconds: 30
  modsrv:
    url: "http://localhost:8082"
    timeout_seconds: 30
  hissrv:
    url: "http://localhost:8083"
    timeout_seconds: 30
  alarmsrv:
    url: "http://localhost:8084"
    timeout_seconds: 30
  rulesrv:
    url: "http://localhost:8085"
    timeout_seconds: 30
  netsrv:
    url: "http://localhost:8086"
    timeout_seconds: 30
    
auth:
  jwt_secret: "${JWT_SECRET}"
  token_expiry: 3600  # 1小时
  refresh_expiry: 86400  # 24小时
  
cors:
  allowed_origins:
    - "http://localhost:3000"
    - "http://localhost:5173"
  allowed_methods: ["GET", "POST", "PUT", "DELETE", "OPTIONS"]
  max_age: 3600
  
logging:
  level: "info"
  file: "logs/apigateway.log"
```

## 数据访问策略

### 1. 实时数据 (Redis 直接访问)

```rust
// 直接从 Redis Hash 读取
// 键格式: {service}:{channelID}:{type}
// 示例: comsrv:1001:m, modsrv:power_meter:measurement

策略: RedisOnly
延迟: < 5ms
用途: 实时遥测、信号、计算结果
```

### 2. 配置数据 (Redis 缓存 + HTTP 回源)

```rust
// 优先从 Redis 读取，未命中则 HTTP 回源
// 键格式: cfg:{type}:{id}
// 示例: cfg:channel:1001, cfg:model:power_meter

策略: RedisWithHttpFallback
延迟: 缓存命中 < 5ms，回源 < 100ms
用途: 通道配置、设备模型、服务配置
```

### 3. 历史数据 (InfluxDB 查询)

```rust
// 直接查询 InfluxDB
// 使用 Flux 查询语言

策略: InfluxDBQuery
延迟: 10-500ms (根据数据量)
用途: 历史趋势、统计分析、报表
```

### 4. 复杂查询 (HTTP 服务)

```rust
// 转发到后端服务处理
// 路径: /api/{service}/*

策略: HttpOnly
延迟: 50-500ms
用途: 业务逻辑、复杂计算、管理操作
```

## API 接口

### 认证管理

```bash
# 用户登录
POST /auth/login
Content-Type: application/json
{
  "username": "admin",
  "password": "password"
}

# 响应
{
  "access_token": "eyJ...",
  "refresh_token": "eyJ...",
  "expires_in": 3600
}

# 刷新令牌
POST /auth/refresh
Authorization: Bearer {refresh_token}

# 用户登出
POST /auth/logout
Authorization: Bearer {access_token}

# 获取当前用户
GET /auth/me
Authorization: Bearer {access_token}
```

### 通道数据

```bash
# 获取通道列表
GET /api/channels
Authorization: Bearer {token}

# 获取通道详情
GET /api/channels/{channel_id}

# 获取实时遥测
GET /api/channels/{channel_id}/telemetry
响应:
{
  "channel_id": 1001,
  "data": {
    "10001": "220.123456",
    "10002": "221.234567",
    "10003": "219.345678"
  },
  "timestamp": "2025-07-23T10:00:00Z"
}

# 获取实时信号
GET /api/channels/{channel_id}/signals

# 发送控制命令
POST /api/channels/{channel_id}/control
{
  "point_id": 30001,
  "value": 1.0,
  "source": "web_ui"
}

# 发送调节命令
POST /api/channels/{channel_id}/adjustment
{
  "point_id": 40001,
  "value": 220.500000,
  "source": "web_ui"
}
```

### 设备模型

```bash
# 获取模型列表
GET /api/device-models

# 获取模型实例
GET /api/device-models/{model_name}/instances

# 获取实例数据
GET /api/device-models/{model_name}/instances/{instance_id}

# 获取模型计算结果
GET /api/device-models/{model_name}/measurements
```

### 历史数据

```bash
# 查询历史数据
GET /api/historical?channel={channel_id}&point={point_id}&start={start_time}&end={end_time}

# 聚合查询
GET /api/historical/aggregate?channel={channel_id}&point={point_id}&window=1h&function=mean

# 批量查询
POST /api/historical/batch
{
  "queries": [
    {
      "channel": 1001,
      "point": 10001,
      "start": "2025-07-23T00:00:00Z",
      "end": "2025-07-23T23:59:59Z",
      "aggregation": {
        "window": "5m",
        "function": "mean"
      }
    }
  ]
}
```

### 告警管理

```bash
# 获取活跃告警
GET /api/alarms?status=active&level=critical

# 获取告警详情
GET /api/alarms/{alarm_id}

# 确认告警
POST /api/alarms/{alarm_id}/acknowledge
{
  "notes": "正在处理"
}

# 解决告警
POST /api/alarms/{alarm_id}/resolve
{
  "resolution": "已更换故障设备"
}

# 获取告警统计
GET /api/alarms/stats
```

### 规则管理

```bash
# 获取规则列表
GET /api/rules?enabled=true

# 获取规则详情
GET /api/rules/{rule_id}

# 创建规则
POST /api/rules
{
  "name": "温度监控",
  "type": "threshold",
  "config": {...}
}

# 启用/禁用规则
POST /api/rules/{rule_id}/enable
POST /api/rules/{rule_id}/disable

# 手动执行规则
POST /api/rules/{rule_id}/execute
```

### 配置管理

```bash
# 获取配置
GET /api/configs/{key}

# 更新配置
PUT /api/configs/{key}
{
  "value": {...}
}

# 同步配置
POST /api/configs/sync/{service}

# 清理缓存
POST /api/configs/cache/clear
```

### 系统信息

```bash
# 获取系统信息
GET /api/system/info

# 获取服务状态
GET /api/system/services

# 获取性能指标
GET /api/system/metrics
```

### 健康检查

```bash
# 简单健康检查
GET /health

# 详细健康检查
GET /health/detailed
响应:
{
  "status": "healthy",
  "services": {
    "redis": "healthy",
    "influxdb": "healthy",
    "comsrv": "healthy",
    "modsrv": "healthy"
  },
  "latency": {
    "redis": "2ms",
    "influxdb": "15ms"
  }
}
```

## WebSocket 接口

### 连接建立

```javascript
// 建立连接
const ws = new WebSocket('ws://localhost:8080/ws');

// 认证
ws.send(JSON.stringify({
  type: 'auth',
  token: 'Bearer {access_token}'
}));
```

### 订阅数据

```javascript
// 订阅通道数据
ws.send(JSON.stringify({
  type: 'subscribe',
  channels: [1001, 1002],
  data_types: ['m', 's']
}));

// 订阅告警
ws.send(JSON.stringify({
  type: 'subscribe_alarms',
  levels: ['critical', 'major']
}));

// 订阅模型数据
ws.send(JSON.stringify({
  type: 'subscribe_models',
  models: ['power_meter', 'env_monitor']
}));
```

### 接收消息

```javascript
ws.onmessage = (event) => {
  const message = JSON.parse(event.data);
  
  switch(message.type) {
    case 'data':
      // 实时数据更新
      console.log('Channel:', message.channel);
      console.log('Data:', message.data);
      break;
      
    case 'alarm':
      // 告警通知
      console.log('New alarm:', message.alarm);
      break;
      
    case 'model_update':
      // 模型数据更新
      console.log('Model:', message.model);
      console.log('Data:', message.data);
      break;
  }
};
```

### 发送控制命令

```javascript
// 通过 WebSocket 发送控制命令
ws.send(JSON.stringify({
  type: 'control',
  channel: 1001,
  point: 30001,
  value: 1.0
}));
```

## 服务代理

API Gateway 为后端服务提供统一代理：

```
/api/comsrv/*    → http://comsrv:8081/*
/api/modsrv/*    → http://modsrv:8082/*
/api/hissrv/*    → http://hissrv:8083/*
/api/alarmsrv/*  → http://alarmsrv:8084/*
/api/rulesrv/*   → http://rulesrv:8085/*
/api/netsrv/*    → http://netsrv:8086/*
```

## 缓存策略

### 多层缓存架构

```
请求 → L1 本地缓存 → L2 Redis 缓存 → 数据源
         ↓               ↓              ↓
      内存 LRU        分布式缓存      后端服务
      (1000项)         (TTL控制)
```

### 缓存配置

```yaml
cache:
  # L1 本地缓存
  local:
    max_entries: 1000
    ttl_seconds: 60
    
  # L2 Redis 缓存
  redis:
    default_ttl: 300
    config_ttl: 3600
    
  # 缓存键前缀
  prefixes:
    config: "cache:config:"
    model: "cache:model:"
    stats: "cache:stats:"
```

## 性能优化

### 批量操作

```rust
// 批量读取多个通道数据
GET /api/channels/batch?ids=1001,1002,1003

// 批量查询历史数据
POST /api/historical/batch
```

### 连接池

```yaml
# HTTP 客户端连接池
http_client:
  pool_idle_timeout: 90
  pool_max_idle_per_host: 10
  timeout: 30
  
# Redis 连接池
redis:
  pool_size: 10
  min_idle: 5
```

### 并发控制

```rust
// 使用信号量限制并发
let semaphore = Arc::new(Semaphore::new(100));

// 限流配置
rate_limit:
  requests_per_second: 1000
  burst: 2000
```

## 监控指标

通过 `/api/system/metrics` 端点暴露 Prometheus 指标：

- `apigateway_requests_total` - 请求总数
- `apigateway_request_duration_seconds` - 请求耗时
- `apigateway_active_connections` - 活跃连接数
- `apigateway_cache_hits_total` - 缓存命中数
- `apigateway_cache_misses_total` - 缓存未命中数

## 故障排查

### 认证问题

```bash
# 检查 JWT 密钥配置
echo $JWT_SECRET

# 验证 token
curl -H "Authorization: Bearer {token}" http://localhost:8080/auth/me
```

### 连接问题

```bash
# 检查 Redis 连接
redis-cli ping

# 检查后端服务
curl http://localhost:8081/health
```

### 性能问题

```bash
# 查看慢查询日志
tail -f logs/apigateway.log | grep "slow_request"

# 监控内存使用
ps aux | grep apigateway
```

## 环境变量

- `RUST_LOG` - 日志级别
- `JWT_SECRET` - JWT 签名密钥
- `INFLUXDB_TOKEN` - InfluxDB 访问令牌
- `REDIS_URL` - Redis 连接地址

## 相关文档

- [架构设计](docs/architecture.md)
- [API 规范](docs/api-specification.md)
- [WebSocket 协议](docs/websocket-protocol.md)