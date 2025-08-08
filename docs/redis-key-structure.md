# Redis Key Structure - VoltageEMS

## Complete Redis Key Structure Tree

```
Alarm System (alarmsrv):
├── alarm:rule:{id}              # Alarm rule configuration (Hash)
├── alarm:rule:config:{id}       # Complete alarm rule JSON config (String)
├── alarm:rule:index             # All alarm rules index (Set)
├── alarm:{rule_id}              # Alarm instance status (Hash)
├── idx:alarm:active             # Active alarms index (Set)
├── idx:alarm:watch:{source}:{field}  # Data point monitoring index (Set)
└── alarm:events                 # Alarm event queue (List)

Rule Engine (rulesrv):
├── rule:{id}                    # Business rule configuration (String/JSON)
├── rule:index                   # Business rules index (Set)
├── rule:status                  # Rule enable status (Hash)
├── rule:execution:{id}          # Rule execution history (List)
└── rule:statistics              # Rule execution statistics (Hash)

Communication Service Real-time Data (comsrv):
├── comsrv:{channel_id}:T        # Telemetry data (Hash)
│   ├── {point_id} -> value      # Data point values
│   └── _updated_at -> timestamp # Last update timestamp
├── comsrv:{channel_id}:C        # Control commands (Hash)
│   ├── {point_id} -> value      # Control point values
│   ├── _status -> pending/completed/failed
│   └── _updated_at -> timestamp
├── comsrv:{channel_id}:status   # Channel status (Hash)
│   ├── online -> true/false
│   ├── last_update -> timestamp
│   └── error_count -> number
└── comsrv:cmd:{channel_id}:{cmd_id}  # Command details (Hash)
    ├── point_id -> id
    ├── value -> value
    ├── status -> pending/executing/completed/failed
    └── timestamp -> timestamp

Model Service Data (modsrv):
├── model:{model_id}:measurement      # Measurement data (String/JSON)
├── model:{model_id}:action          # Action data (String/JSON)
├── model:{model_id}:status          # Model status (Hash)
│   ├── state -> normal/warning/fault
│   ├── last_update -> timestamp
│   └── error_msg -> message
├── model:{model_id}:config          # Model configuration (String/JSON)
├── model:index                      # All models index (Set)
├── model:template:{template_id}     # Model template (String/JSON)
└── template:index                   # Template index (Set)

Historical Data Service (hissrv):
├── history:{source}:{id}:{field}:{hour}  # Hourly data cache (Sorted Set)
│   └── score:timestamp -> member:value
├── history:batch:{batch_id}         # Batch historical data (String/JSON)
├── history:index:{date}              # Date index (Set)
└── history:statistics:{source}:{id} # Statistics info (Hash)
    ├── min -> value
    ├── max -> value
    ├── avg -> value
    └── count -> number

Configuration Data (config):
├── config:channel:{channel_id}      # Channel configuration (String/JSON)
├── config:model:{model_id}          # Model configuration (String/JSON)
├── config:service:{service_name}    # Service configuration (String/JSON)
├── config:system                    # System configuration (Hash)
└── config:version                   # Configuration version (String)

System Status (status):
├── status:service:{service_name}    # Service status (Hash)
│   ├── status -> running/stopped/error
│   ├── last_heartbeat -> timestamp
│   ├── version -> version_string
│   └── uptime -> seconds
├── status:channel:{channel_id}      # Channel status (Hash)
│   ├── status -> online/offline/error
│   ├── last_update -> timestamp
│   ├── error_count -> number
│   └── success_count -> number
└── status:system                    # Overall system status (Hash)
    ├── total_channels -> number
    ├── active_channels -> number
    ├── total_models -> number
    └── active_alarms -> number

Event Queues (events):
├── control:events                   # Control command events (List)
├── control:queue:{channel_id}       # Channel control queue (List)
├── measurement:sync                 # Measurement sync queue (List)
├── action:queue:{action_type}       # Action queue (List)
├── action:execute:{action_type}     # Action execution queue (List)
├── event:queue:{event_type}         # Generic event queue (List)
└── event:last:{event_type}          # Last event cache (String)

Synchronization and Coordination (sync):
├── sync:comsrv_to_modsrv            # Data sync mapping (Hash)
│   ├── {source} -> {target}
│   └── ...
├── sync:pattern:{pattern_id}        # Sync pattern config (String/JSON)
├── sync:schedule:{job_id}           # Scheduled jobs (Hash)
│   ├── pattern -> pattern_id
│   ├── interval -> seconds
│   └── last_run -> timestamp
└── sync:lock:{resource}             # Distributed lock (String)
    └── owner:timestamp

Temporary Data (temp):
├── temp:calculation:{id}            # Temporary calculation results (String)
├── temp:cache:{key}                 # Generic cache (String)
└── temp:session:{session_id}        # Session data (Hash)

Index Structures (idx):
├── idx:alarm:active                 # Active alarms (Set)
├── idx:alarm:watch:{source}:{field} # Alarm monitoring points (Set)
├── idx:channel:protocol:{protocol}  # Channels by protocol index (Set)
├── idx:model:type:{type}            # Models by type index (Set)
├── idx:point:channel:{channel_id}   # All points of a channel (Set)
└── idx:time:{date}:{hour}           # Time-based index (Set)
```

## 数据类型说明

- **Hash**: 键值对集合，适合存储对象
- **String**: 单个值或JSON字符串
- **Set**: 无序不重复集合，适合索引
- **Sorted Set**: 有序集合，适合时序数据
- **List**: 列表，适合队列和事件流

## 命名规范

1. **服务前缀**：`comsrv:`, `modsrv:`, `alarmsrv:`, `rulesrv:`, `hissrv:`
2. **分隔符**：使用冒号 `:` 分隔层级
3. **ID格式**：`{type}:{id}` 或 `{type}_{id}`
4. **索引前缀**：`idx:` 开头
5. **配置前缀**：`config:` 开头
6. **状态前缀**：`status:` 开头
7. **临时数据**：`temp:` 开头

## 使用示例

```bash
# 写入遥测数据
HSET comsrv:1:T 101 "85.5"
HSET comsrv:1:T 102 "3.2"
HSET comsrv:1:T _updated_at "1754530782"

# 创建告警规则
HSET alarm:rule:temp_high_1 source_key "comsrv:1:T"
HSET alarm:rule:temp_high_1 field "101"
HSET alarm:rule:temp_high_1 threshold "85"
HSET alarm:rule:temp_high_1 operator ">"

# 添加到索引
SADD alarm:rule:index "temp_high_1"
SADD idx:alarm:watch:comsrv:1:T:101 "temp_high_1"

# 触发告警
HSET alarm:temp_high_1 status "active"
HSET alarm:temp_high_1 trigger_value "86.2"
SADD idx:alarm:active "temp_high_1"

# 查询活动告警
SMEMBERS idx:alarm:active

# 获取告警详情
HGETALL alarm:temp_high_1
```

## 性能优化建议

1. **使用Pipeline**：批量操作时使用管道减少网络往返
2. **合理设置TTL**：临时数据和缓存设置过期时间
3. **使用索引**：避免KEYS命令，使用预建索引
4. **限制队列长度**：使用LTRIM限制List长度
5. **分片策略**：大数据量时按channel_id或model_id分片
