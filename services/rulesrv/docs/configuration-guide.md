# Rulesrv 配置指南

## 目录

1. [概述](#概述)
2. [配置文件结构](#配置文件结构)
3. [服务配置](#服务配置)
4. [Redis配置](#redis配置)
5. [API配置](#api配置)
6. [规则引擎配置](#规则引擎配置)
7. [订阅配置](#订阅配置)
8. [动作处理器配置](#动作处理器配置)
9. [存储配置](#存储配置)
10. [监控配置](#监控配置)
11. [日志配置](#日志配置)
12. [环境变量](#环境变量)
13. [规则定义格式](#规则定义格式)
14. [示例配置](#示例配置)

## 概述

Rulesrv是VoltageEMS系统的规则引擎服务，负责：
- 监听Redis通道的数据更新
- 评估规则条件
- 执行规则动作（发布告警、控制设备等）
- 管理规则的生命周期

配置文件位于：`services/rulesrv/config/default.yml`

## 配置文件结构

```yaml
service:          # 服务基本信息
redis:            # Redis连接配置
api:              # HTTP API配置
engine:           # 规则引擎配置
subscription:     # Redis订阅配置
actions:          # 动作处理器配置
storage:          # 存储配置
monitoring:       # 监控配置
logging:          # 日志配置
```

## 服务配置

```yaml
service:
  name: rulesrv
  version: 1.0.0
  description: "Rules Engine Service for VoltageEMS"
```

| 字段 | 类型 | 说明 | 默认值 |
|------|------|------|--------|
| name | string | 服务名称 | rulesrv |
| version | string | 服务版本 | 1.0.0 |
| description | string | 服务描述 | - |

## Redis配置

```yaml
redis:
  url: "redis://localhost:6379"
  key_prefix: "rulesrv"
  pool_size: 20
  connection_timeout: 5s
```

| 字段 | 类型 | 说明 | 默认值 |
|------|------|------|--------|
| url | string | Redis连接URL | redis://localhost:6379 |
| key_prefix | string | Redis键前缀 | rulesrv |
| pool_size | integer | 连接池大小 | 20 |
| connection_timeout | duration | 连接超时时间 | 5s |

### Redis URL格式
- 无密码：`redis://localhost:6379`
- 有密码：`redis://:password@localhost:6379`
- 指定数据库：`redis://localhost:6379/0`

## API配置

```yaml
api:
  host: "0.0.0.0"
  port: 8083
  cors:
    enabled: true
    allowed_origins:
      - "*"
  timeout:
    read: 30s
    write: 30s
    idle: 60s
```

| 字段 | 类型 | 说明 | 默认值 |
|------|------|------|--------|
| host | string | 监听地址 | 0.0.0.0 |
| port | integer | 监听端口 | 8083 |
| cors.enabled | boolean | 是否启用CORS | true |
| cors.allowed_origins | array | 允许的源 | ["*"] |
| timeout.read | duration | 读取超时 | 30s |
| timeout.write | duration | 写入超时 | 30s |
| timeout.idle | duration | 空闲超时 | 60s |

### API端点

- `GET /health` - 健康检查
- `GET /api/v1/rules` - 列出所有规则
- `POST /api/v1/rules` - 创建规则
- `GET /api/v1/rules/{rule_id}` - 获取规则
- `PUT /api/v1/rules/{rule_id}` - 更新规则
- `DELETE /api/v1/rules/{rule_id}` - 删除规则
- `POST /api/v1/rules/{rule_id}/execute` - 手动执行规则
- `GET /api/v1/rules/{rule_id}/history` - 规则执行历史

## 规则引擎配置

```yaml
engine:
  max_rules: 1000
  execution_timeout: 30s
  max_parallel_executions: 100
  cache_enabled: true
  cache_ttl: 300s
```

| 字段 | 类型 | 说明 | 默认值 |
|------|------|------|--------|
| max_rules | integer | 最大规则数量 | 1000 |
| execution_timeout | duration | 规则执行超时 | 30s |
| max_parallel_executions | integer | 最大并行执行数 | 100 |
| cache_enabled | boolean | 启用规则缓存 | true |
| cache_ttl | duration | 缓存有效期 | 300s |

## 订阅配置

```yaml
subscription:
  channels:
    - "modsrv:outputs:*"
    - "alarm:event:*"
  buffer_size: 10000
  batch_interval: 100ms
```

| 字段 | 类型 | 说明 | 默认值 |
|------|------|------|--------|
| channels | array | 订阅的通道模式 | - |
| buffer_size | integer | 消息缓冲区大小 | 10000 |
| batch_interval | duration | 批处理间隔 | 100ms |

### 支持的通道模式

- `modsrv:outputs:*` - 模型服务输出
- `alarm:event:*` - 告警事件
- `device:status:*` - 设备状态更新
- `sensor:data:*` - 传感器数据

## 动作处理器配置

```yaml
actions:
  control:
    enabled: true
    command_timeout: 30s
    retry:
      max_attempts: 3
      initial_delay: 1s
      max_delay: 30s
  
  alarm:
    enabled: true
    levels:
      - critical
      - warning
      - info
  
  notification:
    enabled: true
    channels:
      - email
      - webhook
      - sms
```

### 控制动作配置

| 字段 | 类型 | 说明 | 默认值 |
|------|------|------|--------|
| enabled | boolean | 是否启用 | true |
| command_timeout | duration | 命令超时 | 30s |
| retry.max_attempts | integer | 最大重试次数 | 3 |
| retry.initial_delay | duration | 初始延迟 | 1s |
| retry.max_delay | duration | 最大延迟 | 30s |

### 告警动作配置

| 字段 | 类型 | 说明 | 默认值 |
|------|------|------|--------|
| enabled | boolean | 是否启用 | true |
| levels | array | 告警级别 | [critical, warning, info] |

### 通知动作配置

| 字段 | 类型 | 说明 | 默认值 |
|------|------|------|--------|
| enabled | boolean | 是否启用 | true |
| channels | array | 通知渠道 | [email, webhook, sms] |

## 存储配置

```yaml
storage:
  rules:
    max_size: 1MB
    compression: true
  
  history:
    retention: 7d
    max_entries: 1000
```

| 字段 | 类型 | 说明 | 默认值 |
|------|------|------|--------|
| rules.max_size | size | 规则最大大小 | 1MB |
| rules.compression | boolean | 是否压缩 | true |
| history.retention | duration | 历史保留期 | 7d |
| history.max_entries | integer | 每个规则最大历史记录 | 1000 |

## 监控配置

```yaml
monitoring:
  metrics_port: 9090
  health_check_interval: 30s
  metrics_interval: 10s
```

| 字段 | 类型 | 说明 | 默认值 |
|------|------|------|--------|
| metrics_port | integer | 指标端口 | 9090 |
| health_check_interval | duration | 健康检查间隔 | 30s |
| metrics_interval | duration | 指标收集间隔 | 10s |

## 日志配置

```yaml
logging:
  level: info
  format: json
  output: stdout
  file:
    path: "logs/rulesrv.log"
    max_size: 100MB
    max_backups: 10
    max_age: 30d
```

| 字段 | 类型 | 说明 | 默认值 |
|------|------|------|--------|
| level | string | 日志级别 | info |
| format | string | 日志格式 | json |
| output | string | 输出目标 | stdout |
| file.path | string | 文件路径 | logs/rulesrv.log |
| file.max_size | size | 最大文件大小 | 100MB |
| file.max_backups | integer | 最大备份数 | 10 |
| file.max_age | duration | 最大保留时间 | 30d |

### 日志级别
- `trace` - 最详细
- `debug` - 调试信息
- `info` - 一般信息
- `warn` - 警告信息
- `error` - 错误信息

## 环境变量

所有配置项都可以通过环境变量覆盖：

```bash
# Redis配置
REDIS_URL=redis://localhost:6379
REDIS_KEY_PREFIX=rulesrv

# API配置
API_HOST=0.0.0.0
API_PORT=8083

# 日志配置
RUST_LOG=info,rulesrv=debug

# 规则引擎配置
ENGINE_MAX_RULES=2000
ENGINE_EXECUTION_TIMEOUT=60s
```

环境变量优先级高于配置文件。

## 规则定义格式

### 简单规则格式

```json
{
  "id": "temperature_alarm",
  "name": "Temperature Alarm Rule",
  "description": "Trigger alarm when temperature exceeds threshold",
  "group_id": null,
  "condition": "temperature > 30",
  "actions": [
    {
      "type": "publish",
      "channel": "alarm:temperature:high",
      "message": "Temperature exceeded 30°C"
    }
  ],
  "enabled": true,
  "priority": 10
}
```

### 规则字段说明

| 字段 | 类型 | 必填 | 说明 |
|------|------|------|------|
| id | string | 是 | 规则唯一标识 |
| name | string | 是 | 规则名称 |
| description | string | 否 | 规则描述 |
| group_id | string | 否 | 规则组ID |
| condition | string | 是 | 规则条件表达式 |
| actions | array | 是 | 规则动作列表 |
| enabled | boolean | 是 | 是否启用 |
| priority | integer | 是 | 优先级（1-100） |

### 条件表达式

支持的操作符：
- `>` - 大于
- `<` - 小于
- `>=` - 大于等于
- `<=` - 小于等于
- `==` - 等于
- `!=` - 不等于

示例：
- `temperature > 30`
- `pressure <= 100`
- `status == 1`
- `speed >= 1000`

### 动作类型

#### 1. 发布动作
```json
{
  "type": "publish",
  "channel": "alarm:temperature:high",
  "message": "Temperature alarm triggered"
}
```

#### 2. 控制动作
```json
{
  "type": "control",
  "channel_id": 1001,
  "point_type": "control",
  "point_id": 10001,
  "value": 0
}
```

#### 3. 通知动作
```json
{
  "type": "notification",
  "method": "webhook",
  "url": "https://api.example.com/alerts",
  "template": "Temperature is {{value}}°C"
}
```

## 示例配置

### 完整配置示例

```yaml
# services/rulesrv/config/default.yml
service:
  name: rulesrv
  version: 1.0.0
  description: "Rules Engine Service for VoltageEMS"

redis:
  url: "redis://localhost:6379"
  key_prefix: "rulesrv"
  pool_size: 20
  connection_timeout: 5s

api:
  host: "0.0.0.0"
  port: 8083
  cors:
    enabled: true
    allowed_origins:
      - "http://localhost:3000"
      - "https://app.voltageems.com"
  timeout:
    read: 30s
    write: 30s
    idle: 60s

engine:
  max_rules: 1000
  execution_timeout: 30s
  max_parallel_executions: 100
  cache_enabled: true
  cache_ttl: 300s

subscription:
  channels:
    - "modsrv:outputs:*"
    - "alarm:event:*"
    - "device:status:*"
  buffer_size: 10000
  batch_interval: 100ms

actions:
  control:
    enabled: true
    command_timeout: 30s
    retry:
      max_attempts: 3
      initial_delay: 1s
      max_delay: 30s
  
  alarm:
    enabled: true
    levels:
      - critical
      - warning
      - info
  
  notification:
    enabled: true
    channels:
      - webhook

storage:
  rules:
    max_size: 1MB
    compression: true
  
  history:
    retention: 7d
    max_entries: 1000

monitoring:
  metrics_port: 9090
  health_check_interval: 30s
  metrics_interval: 10s

logging:
  level: info
  format: json
  output: stdout
  file:
    path: "logs/rulesrv.log"
    max_size: 100MB
    max_backups: 10
    max_age: 30d
```

### 生产环境配置

```yaml
# services/rulesrv/config/production.yml
redis:
  url: "redis://prod-redis-cluster:6379"
  pool_size: 50

api:
  cors:
    enabled: true
    allowed_origins:
      - "https://app.voltageems.com"

engine:
  max_rules: 5000
  max_parallel_executions: 200

logging:
  level: warn
  output: file
```

### 开发环境配置

```yaml
# services/rulesrv/config/development.yml
redis:
  url: "redis://localhost:6379"

api:
  cors:
    enabled: true
    allowed_origins:
      - "*"

logging:
  level: debug
  output: stdout
```

## 启动命令

```bash
# 使用默认配置
cargo run -p rulesrv -- service

# 指定配置文件
cargo run -p rulesrv -- service --config config/production.yml

# 使用环境变量覆盖
REDIS_URL=redis://192.168.1.100:6379 cargo run -p rulesrv -- service

# 开启调试日志
RUST_LOG=debug,rulesrv=trace cargo run -p rulesrv -- service
```

## 配置验证

启动时会自动验证配置：
- Redis连接性
- 端口可用性
- 文件路径权限
- 配置值范围

如果配置有误，服务会拒绝启动并显示错误信息。

## 最佳实践

1. **环境隔离**：为不同环境创建独立配置文件
2. **密钥管理**：敏感信息使用环境变量
3. **监控配置**：生产环境启用完整监控
4. **日志轮转**：配置合理的日志保留策略
5. **性能调优**：根据负载调整连接池和并发数
6. **规则限制**：设置合理的规则数量上限
7. **超时设置**：避免规则执行时间过长