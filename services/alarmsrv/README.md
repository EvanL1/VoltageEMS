# alarmsrv - 告警管理服务

## 概述

alarmsrv 是 VoltageEMS 的智能告警管理服务，负责监控系统数据、检测异常状态、生成和管理告警。服务采用简化的 Redis 键结构设计，支持告警分类、优先级管理和自动升级机制。

## 主要特性

- **实时告警检测**: 监控 Redis 数据变化，自动触发告警
- **智能分类**: 基于规则的告警自动分类（环境、电力、通信、系统、安全）
- **简化存储**: 使用 `alarm:{id}` 的扁平化键结构
- **生命周期管理**: 完整的告警创建、确认、解决流程
- **自动升级**: 基于时间的告警级别自动升级
- **标准化精度**: 所有数值保持 6 位小数精度

## 快速开始

### 运行服务

```bash
cd services/alarmsrv
cargo run
```

### 配置文件

主配置文件位于 `config/default.yml`：

```yaml
service:
  name: "alarmsrv"
  host: "0.0.0.0"
  port: 8084
  
redis:
  url: "redis://localhost:6379"
  
alarm:
  auto_classify: true
  auto_escalate: true
  retention_days: 30
  
monitoring:
  patterns:
    - "comsrv:*:m"  # 监控测量值
    - "comsrv:*:s"  # 监控信号值
    - "modsrv:*:measurement"  # 监控计算结果
    
logging:
  level: "info"
  file: "logs/alarmsrv.log"
```

## Redis 数据结构

### 告警存储

**键格式**: `alarm:{alarmID}`

**值格式** (JSON):
```json
{
  "id": "550e8400-e29b-41d4-a716-446655440000",
  "title": "高温告警",
  "description": "服务器机房温度超过阈值",
  "category": "Environmental",
  "level": "Critical",
  "status": "Active",
  "source": {
    "channel_id": 1001,
    "point_id": 10001,
    "value": 45.500000
  },
  "created_at": "2025-07-23T10:00:00Z",
  "updated_at": "2025-07-23T10:00:00Z",
  "acknowledged_at": null,
  "resolved_at": null
}
```

### 索引结构

```
# 按状态索引
alarm:index:active → SET of alarm IDs
alarm:index:acknowledged → SET of alarm IDs
alarm:index:resolved → SET of alarm IDs

# 按级别索引
alarm:index:level:critical → SET of alarm IDs
alarm:index:level:major → SET of alarm IDs
alarm:index:level:minor → SET of alarm IDs

# 按分类索引
alarm:index:category:environmental → SET of alarm IDs
alarm:index:category:power → SET of alarm IDs

# 按日期索引
alarm:index:date:2025-07-23 → SET of alarm IDs
```

## 告警分类

### 分类规则

```yaml
categories:
  Environmental:
    patterns: ["温度", "temperature", "湿度", "humidity", "°C"]
    description: "环境监测相关告警"
    
  Power:
    patterns: ["电压", "voltage", "电流", "current", "功率", "power"]
    description: "电力系统相关告警"
    
  Communication:
    patterns: ["通信", "connection", "超时", "timeout", "离线"]
    description: "通信和网络相关告警"
    
  System:
    patterns: ["CPU", "内存", "memory", "磁盘", "disk", "服务"]
    description: "系统性能相关告警"
    
  Security:
    patterns: ["访问", "access", "认证", "auth", "安全", "security"]
    description: "安全相关告警"
```

### 严重级别

- **Critical** (严重): 需要立即处理，系统面临故障
- **Major** (主要): 严重影响，需要紧急处理
- **Minor** (次要): 有限影响，计划维护
- **Warning** (警告): 潜在问题，需要监控
- **Info** (信息): 信息性质，无需行动

## 告警触发机制

### 阈值配置

```yaml
thresholds:
  - point_type: "m"
    field: "temperature"
    conditions:
      - operator: ">"
        value: 40.0
        level: "Warning"
      - operator: ">"
        value: 45.0
        level: "Critical"
        
  - point_type: "m"
    field: "voltage"
    conditions:
      - operator: "<"
        value: 180.0
        level: "Major"
      - operator: ">"
        value: 250.0
        level: "Major"
```

### 数据监控

```rust
// 监控 comsrv 数据变化
pub async fn monitor_data_changes(&self) {
    let mut pubsub = self.redis_client.get_async_pubsub().await?;
    
    // 订阅数据通道
    pubsub.psubscribe("comsrv:*:*").await?;
    
    while let Some(msg) = pubsub.on_message().next().await {
        let channel: String = msg.get_channel_name()?;
        let payload: String = msg.get_payload()?;
        
        // 解析并检查告警条件
        if let Some(alarm) = self.check_alarm_conditions(&channel, &payload).await {
            self.create_alarm(alarm).await?;
        }
    }
}
```

## API 接口

### 告警管理

```bash
# 获取告警列表
GET /alarms?status=active&level=critical&limit=50

# 获取单个告警
GET /alarms/{id}

# 确认告警
POST /alarms/{id}/acknowledge
{
  "acknowledged_by": "operator1",
  "notes": "正在处理"
}

# 解决告警
POST /alarms/{id}/resolve
{
  "resolved_by": "operator1",
  "resolution": "已更换故障设备"
}
```

### 统计信息

```bash
# 获取告警统计
GET /stats
```

响应：
```json
{
  "total": 150,
  "by_status": {
    "active": 10,
    "acknowledged": 5,
    "resolved": 135
  },
  "by_level": {
    "critical": 2,
    "major": 3,
    "minor": 5,
    "warning": 8,
    "info": 132
  },
  "by_category": {
    "environmental": 45,
    "power": 60,
    "communication": 20,
    "system": 15,
    "security": 10
  }
}
```

### 配置管理

```bash
# 获取阈值配置
GET /config/thresholds

# 更新阈值配置
PUT /config/thresholds
Content-Type: application/json

# 重载配置
POST /config/reload
```

## 自动升级机制

### 升级规则

```yaml
escalation_rules:
  - from_level: "Warning"
    to_level: "Minor"
    after_minutes: 60
    condition: "not_acknowledged"
    
  - from_level: "Minor"
    to_level: "Major"
    after_minutes: 30
    condition: "not_acknowledged"
    
  - from_level: "Major"
    to_level: "Critical"
    after_minutes: 15
    condition: "not_resolved"
```

### 升级处理

```rust
pub async fn check_escalations(&self) -> Result<()> {
    let active_alarms = self.get_active_alarms().await?;
    
    for alarm in active_alarms {
        if let Some(new_level) = self.should_escalate(&alarm) {
            self.escalate_alarm(&alarm.id, new_level).await?;
            self.notify_escalation(&alarm, new_level).await?;
        }
    }
    
    Ok(())
}
```

## 通知集成

### 发布告警事件

告警事件发布到 Redis 供其他服务（如 netsrv）处理：

```rust
// 发布格式
let event = AlarmEvent {
    alarm_id: alarm.id.clone(),
    event_type: "created", // created, acknowledged, resolved, escalated
    alarm_data: alarm.clone(),
    timestamp: Utc::now(),
};

// 发布到通道
redis_client.publish(
    "alarm:events",
    serde_json::to_string(&event)?,
).await?;
```

## 数据清理

### 自动清理策略

```rust
pub async fn cleanup_old_alarms(&self) -> Result<()> {
    let cutoff_date = Utc::now() - Duration::days(self.config.retention_days);
    
    // 查找过期的已解决告警
    let old_alarms = self.find_resolved_alarms_before(cutoff_date).await?;
    
    for alarm_id in old_alarms {
        // 从所有索引中移除
        self.remove_from_indexes(&alarm_id).await?;
        
        // 删除告警数据
        self.redis_client.del(format!("alarm:{}", alarm_id)).await?;
    }
    
    info!("Cleaned up {} old alarms", old_alarms.len());
    Ok(())
}
```

## 监控指标

通过 `/metrics` 端点暴露 Prometheus 指标：

- `alarmsrv_alarms_total` - 告警总数
- `alarmsrv_alarms_active` - 活跃告警数
- `alarmsrv_alarm_response_time` - 告警响应时间
- `alarmsrv_escalations_total` - 升级次数

## 故障排查

### 告警未触发

1. 检查监控模式是否正确配置
2. 验证阈值设置是否合理
3. 查看日志中的错误信息

### Redis 连接问题

```bash
# 检查 Redis 连接
redis-cli ping

# 查看告警数据
redis-cli keys "alarm:*" | head -10

# 查看索引
redis-cli smembers "alarm:index:active"
```

## 环境变量

- `RUST_LOG` - 日志级别
- `ALARMSRV_CONFIG` - 配置文件路径
- `REDIS_URL` - Redis 连接地址

## 相关文档

- [架构设计](docs/architecture.md)
- [告警规则配置](docs/alarm-rules.md)