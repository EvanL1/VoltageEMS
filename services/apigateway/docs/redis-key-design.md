# VoltageEMS Redis键值设计规范

## 概述
本文档定义了VoltageEMS系统中Redis键值的完整命名规范，确保所有服务间的数据访问一致性。

## 核心设计原则

1. **层次化命名**：使用冒号(:)分隔的层次结构
2. **类型前缀**：明确的数据类型标识
3. **唯一性**：确保键值的全局唯一性
4. **可读性**：键名具有自解释性
5. **扩展性**：支持未来功能扩展

## 完整键值规范

### 1. 实时数据（comsrv生成）
```
{channelID}:m:{pointID}         # 测量值（遥测YC）
{channelID}:s:{pointID}         # 状态值（遥信YX）  
{channelID}:c:{pointID}         # 控制状态（遥控YK）
{channelID}:a:{pointID}         # 调节值（遥调YT）
```

**示例**：
- `1001:m:10001` - 通道1001的测量点10001
- `1002:s:20001` - 通道1002的信号点20001

**数据格式**：
```json
{
  "value": 123.45,
  "quality": 0,
  "timestamp": 1642694400000,
  "metadata": {
    "unit": "V",
    "name": "电压"
  }
}
```

### 2. 配置数据（各服务配置）
```
cfg:channel:{channelID}         # 通道配置
cfg:channel:{channelID}:meta    # 通道元数据
cfg:point:{channelID}:{type}:{pointID}  # 点位配置
cfg:service:{serviceName}       # 服务配置
cfg:module:{moduleName}         # 模块配置
```

**示例**：
- `cfg:channel:1001` - 通道1001的配置
- `cfg:point:1001:m:10001` - 通道1001测量点10001的配置
- `cfg:service:comsrv` - comsrv服务配置

### 3. 设备模型数据（modsrv管理）
```
model:def:{modelName}           # 模型定义
model:instance:{instanceID}     # 设备实例
model:calc:{instanceID}:{calcID} # 计算结果
model:event:{instanceID}:{eventID} # 设备事件
model:property:{instanceID}:{propertyName} # 设备属性
```

**示例**：
- `model:def:power_meter_v1` - 电表模型定义
- `model:instance:meter_001` - 电表实例001
- `model:calc:meter_001:total_power` - 电表001的总功率计算结果

**模型定义格式**：
```json
{
  "name": "power_meter_v1",
  "version": "1.0",
  "properties": {
    "rated_voltage": {"type": "float", "unit": "V"},
    "rated_current": {"type": "float", "unit": "A"}
  },
  "telemetry": {
    "voltage_a": {"type": "float", "unit": "V", "range": [0, 500]},
    "current_a": {"type": "float", "unit": "A", "range": [0, 100]}
  },
  "calculations": [
    {
      "id": "total_power",
      "expression": "voltage_a * current_a * sqrt(3)",
      "unit": "W"
    }
  ]
}
```

### 4. 告警数据（alarmsrv管理）
```
alarm:active:{alarmID}          # 活动告警
alarm:history:{alarmID}         # 历史告警
alarm:config:{ruleID}           # 告警规则配置
alarm:stats:{channelID}         # 通道告警统计
alarm:stats:global              # 全局告警统计
alarm:rule:{ruleID}:instances   # 规则实例列表
```

**示例**：
- `alarm:active:alm_001` - 活动告警001
- `alarm:config:rule_high_voltage` - 高电压告警规则
- `alarm:stats:1001` - 通道1001的告警统计

**告警数据格式**：
```json
{
  "id": "alm_001",
  "rule_id": "rule_high_voltage",
  "channel_id": 1001,
  "point_id": 10001,
  "level": "HIGH",
  "message": "电压超限",
  "value": 250.5,
  "threshold": 240.0,
  "timestamp": 1642694400000,
  "acknowledged": false,
  "status": "active"
}
```

### 5. 控制规则（rulesrv管理）
```
rule:def:{ruleID}               # 规则定义
rule:instance:{instanceID}      # 规则实例
rule:trigger:{triggerID}        # 触发器状态
rule:dag:{dagID}                # DAG定义
rule:execution:{executionID}    # 执行记录
```

**示例**：
- `rule:def:auto_switch` - 自动切换规则定义
- `rule:instance:switch_001` - 切换规则实例001

### 6. 历史数据索引（hissrv管理）
```
his:index:{channelID}:{date}    # 历史数据索引（YYYYMMDD）
his:batch:{batchID}             # 批次写入状态
his:stats:{channelID}:{date}    # 历史数据统计  
his:retention:policy            # 数据保留策略
his:query:{queryID}             # InfluxDB查询缓存
```

**示例**：
- `his:index:1001:20250717` - 通道1001在2025年7月17日的历史数据索引
- `his:batch:batch_001` - 批次写入001的状态
- `his:query:q12345` - 缓存的InfluxDB查询结果

**注意**: 实际历史数据存储在InfluxDB中，Redis仅存储索引、状态和查询缓存

### 7. 网络服务数据（netsrv管理）
```
net:cloud:{cloudID}             # 云端连接状态
net:forward:{forwardID}         # 转发任务状态
net:mqtt:{clientID}             # MQTT客户端状态
net:http:{endpointID}           # HTTP端点状态
```

**示例**：
- `net:cloud:aws_iot` - AWS IoT连接状态
- `net:forward:aliyun_001` - 阿里云转发任务001

### 8. 系统元数据
```
meta:channels:list              # 通道ID列表
meta:points:{channelID}:{type}  # 点位ID列表
meta:services:status            # 服务状态摘要
meta:models:list                # 模型列表
meta:alarms:rules               # 告警规则列表
meta:rules:list                 # 控制规则列表
```

**示例**：
- `meta:channels:list` - `[1001, 1002, 1003, 1004, 1005]`
- `meta:points:1001:m` - 通道1001的测量点ID列表

### 9. 命令和消息
```
cmd:{channelID}:control         # 控制命令通道
cmd:{channelID}:adjustment      # 调节命令通道
msg:broadcast                   # 系统广播消息
msg:service:{serviceName}       # 服务专用消息
msg:alarm:notification          # 告警通知消息
msg:config:changed              # 配置变更消息
```

### 10. 缓存数据
```
cache:cfg:{service}:*          # 配置缓存
cache:query:{queryHash}        # 查询结果缓存
cache:model:{modelName}        # 模型缓存
cache:alarm:rules              # 告警规则缓存
```

### 11. 临时数据
```
temp:request:{requestID}       # 临时请求数据
temp:session:{sessionID}       # 会话数据
temp:lock:{resourceID}         # 分布式锁
temp:task:{taskID}             # 任务状态
```

## 键值过期策略

| 数据类型 | TTL | 说明 |
|---------|-----|------|
| 实时数据 | 无过期 | 持久存储，定期清理 |
| 配置数据 | 1小时 | 定期从源服务刷新 |
| 模型数据 | 30分钟 | 相对稳定，适度缓存 |
| 告警数据 | 24小时 | 活动告警无过期，历史告警24小时 |
| 缓存数据 | 5-15分钟 | 根据数据热度调整 |
| 临时数据 | 1-10分钟 | 短期临时存储 |

## 数据访问模式

### 高频访问（直接Redis）
- 实时数据读取
- 活动告警查询
- 控制命令发送

### 中频访问（Redis缓存+HTTP回源）
- 配置数据查询
- 模型定义获取
- 规则定义查询

### 低频访问（InfluxDB/HTTP）
- 历史数据查询（InfluxDB）
- 统计报表生成（HTTP + InfluxDB）
- 复杂分析计算（HTTP + InfluxDB）

## 键值管理工具

### 监控命令
```bash
# 查看某通道的所有数据
redis-cli keys "1001:*"

# 查看配置数据
redis-cli keys "cfg:*"

# 查看活动告警
redis-cli keys "alarm:active:*"

# 查看模型数据
redis-cli keys "model:*"
```

### 清理命令
```bash
# 清理过期缓存
redis-cli keys "cache:*" | xargs redis-cli del

# 清理临时数据
redis-cli keys "temp:*" | xargs redis-cli del
```

## 最佳实践

1. **键名长度**：控制在合理范围内，避免过长
2. **命名一致性**：严格遵循命名规范
3. **数据压缩**：对大型JSON数据考虑压缩
4. **批量操作**：使用pipeline提高性能
5. **监控指标**：跟踪键值数量和内存使用
6. **数据备份**：重要配置数据定期备份