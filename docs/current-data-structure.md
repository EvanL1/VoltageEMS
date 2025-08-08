# VoltageEMS 当前数据结构设计

## 1. 实时数据结构

### 1.1 通信服务 (comsrv) - 遥测数据
```
Key: comsrv:{channel_id}:T
Type: Hash
Fields:
  {point_id} -> value       # 数据点ID -> 实时值
  _updated_at -> timestamp  # 最后更新时间

示例:
  comsrv:1:T
    101 -> "85.5"          # 温度值
    102 -> "3.2"           # 压力值
    103 -> "1"             # 开关状态
    _updated_at -> "1754530782"
```

### 1.2 通信服务 (comsrv) - 控制命令
```
Key: comsrv:{channel_id}:C
Type: Hash
Fields:
  {point_id} -> value       # 控制点ID -> 控制值
  _status -> pending/completed/failed
  _updated_at -> timestamp

示例:
  comsrv:1:C
    201 -> "1"             # 开关控制
    202 -> "50"            # 阀门开度
    _status -> "pending"
    _updated_at -> "1754530782"
```

### 1.3 模型服务 (modsrv) - 测量数据
```
Key: model:{model_id}:measurement
Type: String (JSON)
Value: {
  "field1": value,
  "field2": value,
  ...
}

示例:
  model:transformer_1:measurement
    {"temperature": 75.5, "load": 85.2, "voltage": 10.5}
```

### 1.4 模型服务 (modsrv) - 动作数据
```
Key: model:{model_id}:action
Type: String (JSON)
Value: {
  "action_type": "type",
  "parameters": {...},
  "timestamp": timestamp
}

示例:
  model:breaker_1:action
    {"action_type": "trip", "reason": "overcurrent", "timestamp": 1754530782}
```

## 2. 告警数据结构 (alarmsrv)

### 2.1 告警规则
```
Key: alarm:rule:{rule_id}
Type: Hash
Fields:
  id -> rule_id
  source_key -> "comsrv:1:T" | "model:xxx:measurement"
  field -> point_id or field_name
  threshold -> numeric_value
  operator -> ">" | "<" | "==" | ">=" | "<=" | "!="
  enabled -> "true" | "false"
  alarm_level -> "Critical" | "Major" | "Minor" | "Warning"
  alarm_title -> "描述文本"
  created_at -> timestamp

示例:
  alarm:rule:temp_high_1
    id -> "temp_high_1"
    source_key -> "comsrv:1:T"
    field -> "101"
    threshold -> "85"
    operator -> ">"
    enabled -> "true"
    alarm_level -> "Critical"
    alarm_title -> "1号变压器温度过高"
```

### 2.2 告警实例
```
Key: alarm:{rule_id}
Type: Hash
Fields:
  status -> "active" | "cleared"
  rule_id -> rule_id
  source_key -> data_source
  field -> field_name
  trigger_value -> value_when_triggered
  current_value -> latest_value
  threshold -> threshold_value
  operator -> operator
  triggered_at -> timestamp
  cleared_at -> timestamp (optional)
  updated_at -> timestamp

示例:
  alarm:temp_high_1
    status -> "active"
    rule_id -> "temp_high_1"
    source_key -> "comsrv:1:T"
    field -> "101"
    trigger_value -> "86.2"
    current_value -> "87.5"
    threshold -> "85"
    operator -> ">"
    triggered_at -> "1754530782"
```

### 2.3 告警索引
```
# 活动告警索引
Key: idx:alarm:active
Type: Set
Members: [rule_id1, rule_id2, ...]

# 告警规则索引
Key: alarm:rule:index
Type: Set
Members: [rule_id1, rule_id2, ...]

# 数据点监控索引
Key: idx:alarm:watch:{source_key}:{field}
Type: Set
Members: [rule_id1, rule_id2, ...]  # 监控该数据点的所有告警规则

# 告警事件队列
Key: alarm:events
Type: List
Elements: JSON events (最近1000条)
```

## 3. 规则引擎数据结构 (rulesrv)

### 3.1 业务规则 (与告警规则分离)
```
Key: rule:{rule_id}
Type: String (JSON)
Value: {
  "id": "rule_id",
  "name": "规则名称",
  "type": "calculation" | "control" | "sync",
  "condition": {...},
  "action": {...},
  "enabled": true
}

示例:
  rule:sync_temp_to_model
    {
      "id": "sync_temp_to_model",
      "name": "同步温度到模型",
      "type": "sync",
      "condition": {
        "source": "comsrv:1:T",
        "field": "101"
      },
      "action": {
        "target": "model:transformer_1",
        "field": "temperature"
      },
      "enabled": true
    }
```

### 3.2 规则索引
```
Key: rule:index
Type: Set
Members: [rule_id1, rule_id2, ...]

Key: rule:status
Type: Hash
Fields:
  {rule_id} -> "enabled" | "disabled"
```

## 4. 历史数据结构

### 4.1 历史数据存储 (InfluxDB)
```
Measurement: telemetry
Tags:
  channel_id = "1"
  point_id = "101"
  source = "comsrv"
Fields:
  value = 85.5
Time: 1754530782000000000

Measurement: measurement
Tags:
  model_id = "transformer_1"
  field = "temperature"
  source = "modsrv"
Fields:
  value = 75.5
Time: 1754530782000000000

Measurement: alarm_event
Tags:
  rule_id = "temp_high_1"
  event_type = "triggered" | "cleared"
Fields:
  value = 86.2
  threshold = 85.0
Time: 1754530782000000000
```

### 4.2 Redis中的历史数据缓存（可选）
```
# 最近N小时的数据缓存
Key: history:{source}:{id}:{field}:{hour}
Type: Sorted Set
Score: timestamp
Member: value

示例:
  history:comsrv:1:101:2024010112  # 2024-01-01 12:00-13:00
    1754530782 -> "85.5"
    1754530783 -> "85.6"
    ...
```

## 5. 配置结构

### 5.1 通道配置
```
Key: config:channel:{channel_id}
Type: String (JSON)
Value: {
  "id": 1,
  "name": "主变压器",
  "protocol": "modbus_tcp",
  "connection": {
    "host": "192.168.1.100",
    "port": 502
  },
  "points": [...]
}
```

### 5.2 模型配置
```
Key: config:model:{model_id}
Type: String (JSON)
Value: {
  "id": "transformer_1",
  "name": "1号变压器",
  "type": "transformer",
  "parameters": {...},
  "measurements": [...],
  "actions": [...]
}
```

## 6. 系统状态

### 6.1 服务状态
```
Key: status:service:{service_name}
Type: Hash
Fields:
  status -> "running" | "stopped" | "error"
  last_heartbeat -> timestamp
  version -> version_string
  uptime -> seconds
```

### 6.2 通道状态
```
Key: status:channel:{channel_id}
Type: Hash
Fields:
  status -> "online" | "offline" | "error"
  last_update -> timestamp
  error_count -> number
  success_count -> number
```

## 7. 事件队列

### 7.1 控制命令队列
```
Key: control:events
Type: List
Elements: JSON {type, key, point, value, timestamp}
```

### 7.2 测量同步队列
```
Key: measurement:sync
Type: List
Elements: JSON measurement data
```

### 7.3 动作执行队列
```
Key: action:execute:{action_type}
Type: List
Elements: JSON action data
```

## 8. 使用示例

### 写入实时数据
```bash
# comsrv写入遥测数据
redis-cli FCALL comsrv_write_telemetry 1 "comsrv:1:T" '{"101": 85.5, "102": 3.2}'

# modsrv同步测量数据
redis-cli FCALL modsrv_sync_measurement 1 "model:transformer_1:measurement" \
  '{"temperature": 75.5, "load": 85.2}'
```

### 创建告警规则
```bash
redis-cli FCALL alarmsrv_create_rule 0 "temp_high_1" '{
  "source_key": "comsrv:1:T",
  "field": "101",
  "threshold": 85,
  "operator": ">",
  "alarm_level": "Critical",
  "alarm_title": "温度过高"
}'
```

### 查询活动告警
```bash
redis-cli FCALL alarmsrv_list_active_alarms 0
```

### 查询历史数据（InfluxDB）
```sql
SELECT value FROM telemetry 
WHERE channel_id = '1' AND point_id = '101' 
AND time > now() - 1h
```

## 9. 数据流向

```
采集设备 → comsrv → Redis实时数据 → 告警检查
                 ↓                    ↓
            InfluxDB历史          告警事件

模型计算 → modsrv → Redis实时数据 → 告警检查
                 ↓                    ↓
            InfluxDB历史          告警事件
```

## 10. 性能考虑

1. **实时数据**：使用Redis Hash，O(1)读写
2. **告警检查**：基于索引的规则查找，O(1)
3. **历史数据**：InfluxDB时序数据库，优化的时间范围查询
4. **事件队列**：限制长度（LTRIM），防止内存溢出

## 11. 扩展性

- 支持多通道并发（channel_id分区）
- 支持多模型并发（model_id分区）
- 告警规则动态配置，无需重启
- 历史数据可分片存储（InfluxDB集群）