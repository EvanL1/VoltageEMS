# VoltageEMS Redis数据结构规范

**版本**: v3.2
**更新日期**: 2025-07-23
**适用系统**: VoltageEMS v2.x

## 重要更新 v3.2

- **标准化浮点精度**: 所有浮点数值强制使用6位小数精度格式化
- **modsrv数据简化**: modsrv存储值不再包含时间戳，仅存储计算值
- **通用数据类型**: 引入 `voltage_libs::types::StandardFloat` 和 `PointData`
- **库级别标准化**: 提供通用格式化方法，各服务按需选择

## 1. 概览

### 1.1 系统架构

VoltageEMS采用Redis作为中央消息总线和实时数据存储，实现各服务间的高效通信和数据共享。

```
┌─────────────┐    ┌──────────────┐    ┌─────────────┐
│   comsrv    │───▶│    Redis     │◄──▶│   modsrv    │
│ 设备数据采集  │    │ 中央数据总线  │    │ 模型计算引擎 │
└─────────────┘    └──────┬───────┘    └─────────────┘
                          │
           ┌──────────────┼──────────────┐
           ▼              ▼              ▼
    ┌─────────────┐ ┌─────────────┐ ┌─────────────┐
    │  alarmsrv   │ │   hissrv    │ │  rulesrv    │
    │   告警管理   │ │  历史存储    │ │  规则引擎   │
    └─────────────┘ └─────────────┘ └─────────────┘
```

### 1.2 核心设计原则

- **统一键格式**: `{service}:{entity}:{type}:{id}`
- **命名空间隔离**: 每个服务独立的键空间
- **点位级精确访问**: 支持O(1)查询性能
- **Pub/Sub一致性**: 存储键与发布通道格式一致
- **扩展性**: 支持百万级点位实时处理
- **标准化数值精度**: 所有浮点数值强制使用6位小数格式 (例: "25.123456")

### 1.3 数值格式标准

**标准化数据类型** (`voltage_libs::types`):

```rust
// 标准化浮点数 - 强制6位小数精度
pub struct StandardFloat(f64);

// 点位数据结构
pub struct PointData {
    pub value: StandardFloat,   // 标准化数值
    pub timestamp: i64,         // 时间戳(毫秒)
}
```

**通用格式化方法**:
```rust
// 通用方法 - 各服务按需选择
point_data.to_redis_value()              // → "25.123456"
point_data.to_redis_with_timestamp()     // → "25.123456:1642592400000"

// 解析方法
PointData::from_redis_value("25.123456")
PointData::from_redis_with_timestamp("25.123456:1642592400000")
```

**服务实际使用**:
- **comsrv**: 使用 `to_redis_value()` - 仅存储数值
- **modsrv**: 使用 `to_redis_value()` - 仅存储计算值
- **hissrv**: 根据需要选择带或不带时间戳
- **所有发布消息**: 统一使用6位小数格式

### 1.4 数据流向

```
设备数据 → comsrv → Redis存储/发布 → 其他服务订阅处理
               ↓
         实时数据键存储
               ↓
         批量转存InfluxDB
```

## 2. 标准化库使用指南

### 2.1 引入标准化类型

各服务应使用统一的数据类型：

```rust
use voltage_libs::types::{StandardFloat, PointData};

// 创建标准化数值
let value = StandardFloat::new(25.123456789);  // 自动格式化为6位小数
let point = PointData::new(25.123456789);      // 包含时间戳

// 类型转换
let std_float: StandardFloat = 25.12_f64.into();
let raw_value: f64 = std_float.into();
```

### 2.2 Redis存储格式选择

```rust
// 各服务根据需求选择格式
let point = PointData::new(25.123456);

// comsrv & modsrv: 仅存储值
let redis_value = point.to_redis_value();           // "25.123456"

// hissrv或需要时间戳的场景: 值+时间戳  
let redis_full = point.to_redis_with_timestamp();   // "25.123456:1642592400000"

// 解析回数据结构
let parsed = PointData::from_redis_value("25.123456")?;
let parsed_full = PointData::from_redis_with_timestamp("25.123456:1642592400000")?;
```

### 2.3 服务特定使用模式

**comsrv示例**:
```rust
// 存储到Redis Hash
let hash_key = format!("comsrv:{}:{}", channel_id, point_type);
let field = point_id.to_string();
let value = point_data.to_redis_value();  // "25.123456"
redis_client.hset(&hash_key, &field, value).await?;

// 发布消息 
let message = format!("{}:{}", point_id, point_data.value);  // "10001:25.123456"
redis_client.publish(&channel, &message).await?;
```

**modsrv示例**:
```rust
// 存储计算结果（无时间戳）
let hash_key = format!("modsrv:{}:measurement", model_name);
let field = "total_power";
let calculated_value = PointData::new(1200.5);
let value = calculated_value.to_redis_value();  // "1200.500000"
redis_client.hset(&hash_key, field, value).await?;
```

### 2.4 强制精度保证

所有浮点数值在系统中自动维持6位小数精度：

```rust
// 输入任意精度的数值
StandardFloat::new(25.1)        // → 显示为 "25.100000"
StandardFloat::new(25.123456789) // → 显示为 "25.123457" (四舍五入)
StandardFloat::new(0.000001)    // → 显示为 "0.000001"
```

## 3. 统一键格式规范

### 3.1 键命名约定

**基本格式**: `{service}:{entity}:{type}:{id}`

- **service**: 服务标识符 (comsrv/alarmsrv/modsrv/rulesrv)
- **entity**: 实体标识符 (channelID/modelID/alarmID等)
- **type**: 数据类型标识符
- **id**: 具体对象ID (pointID等)

**字符约束**:
- 仅使用字母数字和下划线: `[a-zA-Z0-9_]`
- 冒号`:`用作分隔符，不可在字段中使用
- 总长度不超过256字符

### 3.2 点位级精确订阅

Redis键与Pub/Sub通道保持一致，实现点位级精确订阅：

```bash
# 精确订阅单个点位
SUBSCRIBE comsrv:1001:m:10001

# 订阅某通道所有遥测点
PSUBSCRIBE comsrv:1001:m:*

# 订阅某通道所有数据
PSUBSCRIBE comsrv:1001:*

# 订阅所有comsrv数据
PSUBSCRIBE comsrv:*
```

## 4. 服务数据结构定义

### 4.1 comsrv (通信服务)

**职责**: 设备数据采集、协议转换、实时数据发布

#### 3.1.1 键格式规范

**Hash键**: `comsrv:{channelID}:{type}`
**发布通道**: `comsrv:{channelID}:{type}`

**类型映射**:
- `m`: 测量值 (YC - Yao Ce)
- `s`: 信号值 (YX - Yao Xin)
- `c`: 控制值 (YK - Yao Kong)
- `a`: 调节值 (YT - Yao Tiao)

#### 3.1.2 数据格式

**Hash存储结构**:
```
键: comsrv:{channelID}:{type}
字段: {pointID}
值: "{value:.6}"
示例: comsrv:1001:m → {10001: "25.123456", 10002: "26.789012"}
```

**发布消息** (String):
```
格式: "{pointID}:{value:.6}"
示例: "10001:25.123456"
说明: 发布点位ID和数值，便于客户端更新缓存
```

#### 3.1.3 数据结构示例

```
# Hash结构
comsrv:1001:m → {
    10001: "25.123456",
    10002: "26.789012",
    10003: "24.567890"
}

comsrv:1001:s → {
    20001: "1",
    20002: "0",
    20003: "1"
}

comsrv:1001:c → {
    30001: "0",
    30002: "1"
}

comsrv:1001:a → {
    40001: "50.500000",
    40002: "75.250000"
}
```

#### 3.1.5 操作接口

**读取操作**:
```bash
# 单点查询
HGET comsrv:1001:m 10001

# 批量查询同类型点位
HMGET comsrv:1001:m 10001 10002 10003

# 获取通道所有点位
HGETALL comsrv:1001:m

# 获取字段数量
HLEN comsrv:1001:m
```

**写入操作**:
```bash
# 单点写入
HSET comsrv:1001:m 10001 "25.123456"

# 批量写入
HMSET comsrv:1001:m 10001 "25.123456" 10002 "26.789012"

# 发布通知（通道级）
PUBLISH comsrv:1001:m "10001:25.123456"
```

### 3.2 alarmsrv (告警服务)

**职责**: 告警生成、状态管理、分类索引

#### 3.2.1 键格式规范

**主数据键**: `alarm:{alarmID}`
**分类索引**: `alarm:category:{category}`
**级别索引**: `alarm:level:{level}`
**状态索引**: `alarm:status:{status}`
**时间索引**: `alarm:date:{YYYY-MM-DD}`
**小时分片**: `alarm:buckets:{YYYYMMDDHH}`
**实时索引**: `alarm:realtime`

#### 3.2.2 数据格式

**主数据** (Hash):
```json
{
    "id": "alarm_550e8400-e29b-41d4-a716-446655440000",
    "title": "高温告警",
    "description": "设备温度超过75°C阈值",
    "level": "Critical",
    "status": "New",
    "category": "temperature",
    "priority": "5",
    "tags": "[\"temperature\",\"device_001\"]",
    "source_point": "comsrv:1001:m:10001",
    "threshold_value": "75.0",
    "actual_value": "78.5",
    "created_at": "2024-01-01T10:00:00Z",
    "updated_at": "2024-01-01T10:00:00Z",
    "acknowledged_by": null,
    "acknowledged_at": null,
    "resolved_at": null,
    "data": "{...}"
}
```

#### 3.2.3 告警级别定义

| 级别 | 数值 | 颜色 | 说明 |
|------|------|------|------|
| Critical | 5 | 红色 | 严重告警，需要立即处理 |
| High | 4 | 橙色 | 高级告警，优先处理 |
| Medium | 3 | 黄色 | 中级告警，及时处理 |
| Low | 2 | 蓝色 | 低级告警，日常维护 |
| Info | 1 | 绿色 | 信息告警，记录备查 |

#### 3.2.4 告警状态流转

```
New → Acknowledged → Resolved
 ↓         ↓           ↓
Escalated → Suppressed → Closed
```

#### 3.2.5 索引结构

**分类索引** (Set):
```
alarm:category:temperature → {alarm_id_1, alarm_id_2, ...}
alarm:category:pressure → {alarm_id_3, alarm_id_4, ...}
```

**时间分片** (Hash):
```
alarm:buckets:2024010110 → {
    "alarm_id_1": "compressed_alarm_data",
    "alarm_id_2": "compressed_alarm_data"
}
```

**实时索引** (Hash):
```
alarm:realtime → {
    "temperature:alarm_123": "{\"id\":\"alarm_123\",\"level\":\"Critical\",\"created_at\":\"...\"}",
    "pressure:alarm_456": "{\"id\":\"alarm_456\",\"level\":\"High\",\"created_at\":\"...\"}"
}
```

#### 3.2.6 TTL策略

- **活跃告警**: 无过期时间
- **已解决告警**: 30天后清理（可配置）
- **索引数据**: 跟随主数据清理
- **分片数据**: 按配置保留期清理

### 4.3 modsrv (模型服务)

**职责**: 数据模型计算、监视值管理、控制命令执行

#### 3.3.1 键格式规范

**Hash键**: `modsrv:{modelname}:{type}`
**控制命令**: `cmd:{commandID}`
**命令列表**: `cmd:list:{modelname}`

**类型映射**:
- `measurement`: 测量值（计算结果、监视值）
- `control`: 控制值（控制命令、设定值）

#### 3.3.2 数据格式

**Hash存储结构**:
```
键: modsrv:{modelname}:{type}
字段: 有意义的属性名
值: "{value}" (标准6位小数格式，不含时间戳)
```

**测量值示例**:
```
modsrv:power_calc:measurement → {
    "total_power": "1200.500000",      # 仅计算值，6位小数精度
    "efficiency": "0.856000",          # 效率值
    "load_factor": "0.780000",         # 负载率
    "reactive_power": "350.200000"     # 无功功率
}
```

**控制值示例**:
```
modsrv:power_calc:control → {
    "enable": "1:1642592400000:operator",
    "max_power": "1500.0:1642592400000:operator",
    "mode": "auto:1642592400000:system",
    "target_pf": "0.95:1642592400000:optimizer"
}
```

**控制命令** (Hash):
```json
{
    "id": "cmd_550e8400-e29b-41d4-a716-446655440000",
    "model_name": "power_calc",
    "field_name": "max_power",
    "value": "1600.0",
    "status": "pending",
    "created_at": "1642592400000",
    "updated_at": "1642592400000",
    "message": null,
    "source": "api_request"
}
```

#### 3.3.3 命令状态定义

| 状态 | 说明 |
|------|------|
| pending | 等待执行 |
| executing | 正在执行 |
| success | 执行成功 |
| failed | 执行失败 |
| cancelled | 已取消 |
| timeout | 执行超时 |

#### 3.3.4 操作示例

**读取操作**:
```bash
# 获取单个值
HGET modsrv:power_calc:measurement total_power

# 获取多个值
HMGET modsrv:power_calc:measurement total_power efficiency

# 获取所有测量值
HGETALL modsrv:power_calc:measurement
```

**写入操作**:
```bash
# 单个更新
HSET modsrv:power_calc:measurement total_power "1250.500000"

# 批量更新
HMSET modsrv:power_calc:measurement \
    total_power "1250.500000" \
    efficiency "0.862000"

# 发布通知
PUBLISH modsrv:power_calc:measurement "total_power:1250.500000"
```

#### 3.3.5 TTL策略

- **测量/控制值**: 无过期时间（实时数据）
- **控制命令**: 24小时过期
- **命令列表**: 保留最近1000条

### 3.4 rulesrv (规则服务)

**职责**: 规则管理、条件评估、动作执行

#### 3.4.1 键格式规范

**规则定义**: `rulesrv:rule:{ruleID}`
**规则组**: `rulesrv:group:{groupID}`
**规则列表**: `rulesrv:rules`
**组列表**: `rulesrv:groups`
**组规则映射**: `rulesrv:group:{groupID}:rules`
**执行历史**: `rulesrv:history:{ruleID}`

#### 3.4.2 数据格式

**规则定义** (JSON String):
```json
{
    "id": "rule_temperature_monitor",
    "name": "温度监控规则",
    "description": "监控设备温度超过阈值",
    "group_id": "group_temperature",
    "enabled": true,
    "priority": 5,
    "conditions": [
        {
            "field": "temperature",
            "operator": ">",
            "value": "75.0",
            "source": "comsrv:1001:m:10001"
        }
    ],
    "actions": [
        {
            "type": "create_alarm",
            "parameters": {
                "level": "Critical",
                "title": "温度告警"
            }
        }
    ],
    "created_at": "2024-01-01T10:00:00Z",
    "updated_at": "2024-01-01T10:00:00Z"
}
```

**执行历史** (List of JSON):
```json
[
    {
        "id": "exec_550e8400-e29b-41d4-a716-446655440000",
        "rule_id": "rule_temperature_monitor",
        "timestamp": 1642592400000,
        "triggered": true,
        "conditions_result": {
            "temperature": {"matched": true, "actual_value": 78.5}
        },
        "actions_executed": ["create_alarm"],
        "success": true,
        "duration_ms": 15,
        "error_message": null,
        "context": {
            "source_data": "comsrv:1001:m:10001",
            "trigger_value": 78.5
        }
    }
]
```

#### 3.4.3 条件操作符

| 操作符 | 说明 | 适用类型 |
|--------|------|----------|
| `>` | 大于 | 数值 |
| `<` | 小于 | 数值 |
| `>=` | 大于等于 | 数值 |
| `<=` | 小于等于 | 数值 |
| `==` | 等于 | 数值/字符串 |
| `!=` | 不等于 | 数值/字符串 |
| `contains` | 包含 | 字符串 |
| `regex` | 正则匹配 | 字符串 |

#### 3.4.4 动作类型

| 动作类型 | 说明 | 参数 |
|----------|------|------|
| `create_alarm` | 创建告警 | level, title, description |
| `send_control` | 发送控制命令 | channel_id, point_id, value |
| `send_notification` | 发送通知 | recipients, message |
| `log_event` | 记录事件 | level, message |

#### 3.4.5 TTL策略

- **规则定义**: 无过期时间
- **规则组**: 无过期时间
- **执行历史**: 7天过期
- **临时数据**: 1小时过期

### 4.5 hissrv (历史数据服务)

**职责**: 历史数据采集、存储转换、查询服务

#### 3.5.1 数据流模式

```
Redis实时数据 → hissrv订阅 → 批量处理 → InfluxDB存储
```

#### 3.5.2 订阅配置

**默认订阅模式**: `comsrv:*`

**订阅处理逻辑**:
- 解析通道格式: `comsrv:{channelID}:{type}` (Hash键)
- 数据类型映射: m(测量) → measurement, s(信号) → signal
- 批量聚合: 配置化批量大小和超时
- 单次获取整个通道数据（HGETALL）

#### 3.5.3 批量处理配置

```yaml
batch_config:
  size: 1000              # 批量大小
  timeout_ms: 5000        # 批量超时
  type_filter: ["m", "s"] # 类型过滤
```

#### 3.5.4 Redis临时存储

hissrv主要作为数据中继，不在Redis中存储大量历史数据，仅使用临时缓存：

- **处理队列**: 内存队列，不持久化
- **错误重试**: 临时存储失败数据
- **状态监控**: 服务状态和统计信息

## 4. 性能优化

### 4.1 查询优化

#### 4.1.1 查询模式分类

**Hash字段查询** (O(1)):
```bash
HGET comsrv:1001:m 10001
```

**Hash批量查询** (O(N)):
```bash
HMGET comsrv:1001:m 10001 10002 10003
```

**Hash全量查询** (O(N)):
```bash
# 获取整个通道数据
HGETALL comsrv:1001:m
```

**索引查询** (O(log N)):
```bash
# Set索引查询
SMEMBERS alarm:category:temperature
# Hash索引查询
HGETALL alarm:realtime
```

#### 4.1.2 Pipeline批处理

**批量读取示例**:
```python
pipe = redis.pipeline()
pipe.hget("comsrv:1001:m", "10001")
pipe.hget("comsrv:1001:m", "10002")
pipe.hget("comsrv:1001:m", "10003")
results = pipe.execute()
```

**批量写入示例**:
```python
pipe = redis.pipeline()
pipe.hset("comsrv:1001:m", "10001", "25.123456")
pipe.hset("comsrv:1001:m", "10002", "26.789012")
pipe.publish("comsrv:1001:m", "10001:25.123456")
pipe.execute()
```

### 4.2 内存优化

#### 4.2.1 数据压缩策略

- **数值精度控制**: 6位小数精度
- **时间戳优化**: 毫秒时间戳
- **字符串复用**: 相同值引用优化
- **过期清理**: 自动TTL清理

#### 4.2.2 容量估算

**Hash结构存储成本**:
- Hash键名: ~20字节 (comsrv:1001:m)
- 字段名: ~5字节 (10001)
- 数值: ~15字节 ("25.123456")
- 单点位: ~20字节

**百万点位容量对比**:
- 旧结构 (String): ~45MB
- 新结构 (Hash): ~30MB
- 节省: 33%

**实际键数量**:
- 旧: 1,000,000个键 (100万点位)
- 新: ~1,000个键 (假设1000通道)
- 减少: 99.9%

### 4.3 扩展策略

#### 4.3.1 水平扩展 (Redis Cluster)

**分片策略**:
- 按通道ID分片: `{channelID}` hash slot
- 按服务分片: `{service}` hash slot
- 索引复制: 关键索引多节点复制

**分片示例**:
```
Shard 1: {comsrv}:*     # 通信服务数据
Shard 2: {modsrv}:*     # 模型服务数据
Shard 3: {alarm}:*      # 告警服务数据
```

#### 4.3.2 垂直扩展

**内存优化**:
- 增加Redis服务器内存
- 启用内存压缩
- 配置内存淘汰策略

**CPU优化**:
- 多实例部署
- 读写分离配置
- 计算密集操作异步化

## 5. 运维管理

### 5.1 监控指标

#### 5.1.1 关键性能指标

**内存使用**:
```bash
INFO memory
# used_memory: 已使用内存
# used_memory_peak: 内存峰值
# used_memory_rss: RSS内存
# mem_fragmentation_ratio: 内存碎片率
```

**键空间统计**:
```bash
INFO keyspace
# db0:keys=1000000,expires=100000,avg_ttl=86400000
```

**操作统计**:
```bash
INFO stats
# total_commands_processed: 总命令数
# instantaneous_ops_per_sec: 当前QPS
# keyspace_hits: 键空间命中数
# keyspace_misses: 键空间未命中数
```

**Pub/Sub统计**:
```bash
INFO replication
# connected_slaves: 连接的从节点数
PUBSUB CHANNELS comsrv:*
# 活跃通道列表
PUBSUB NUMSUB comsrv:1001:m
# 通道订阅者数量
```

#### 5.1.2 业务监控指标

**数据更新频率**:
- 各通道数据更新QPS
- 数据延迟统计

**告警统计**:
- 各级别告警数量
- 告警处理时长
- 告警解决率

**规则执行统计**:
- 规则触发频率
- 规则执行时长
- 规则成功率

#### 5.1.3 告警阈值建议

| 指标 | 警告阈值 | 严重阈值 |
|------|----------|----------|
| 内存使用率 | 70% | 85% |
| 键空间大小 | 80万 | 100万 |
| 内存碎片率 | 1.5 | 2.0 |
| QPS | 5000 | 8000 |
| 数据延迟 | 1秒 | 5秒 |

### 5.2 数据生命周期管理

#### 5.2.1 TTL策略总览

| 数据类型 | TTL | 清理策略 |
|----------|-----|----------|
| comsrv实时数据 | 永久 | 手动清理 |
| modsrv监视值 | 永久 | 手动清理 |
| modsrv控制命令 | 24小时 | 自动过期 |
| modsrv模型输出 | 7天 | 自动过期 |
| alarmsrv活跃告警 | 永久 | 状态驱动 |
| alarmsrv已解决告警 | 30天 | 自动过期 |
| rulesrv规则定义 | 永久 | 手动清理 |
| rulesrv执行历史 | 7天 | 自动过期 |

#### 5.2.2 清理脚本示例

**过期数据清理**:
```bash
#!/bin/bash
# 清理过期的控制命令
redis-cli --scan --pattern "cmd:*" | while read key; do
    ttl=$(redis-cli TTL "$key")
    if [ "$ttl" -eq -2 ]; then
        redis-cli DEL "$key"
        echo "Deleted expired key: $key"
    fi
done
```

**批量清理指定模式**:
```bash
# 清理特定通道的历史数据
redis-cli --scan --pattern "comsrv:1001:*" | xargs redis-cli DEL
```

### 5.3 备份和恢复

#### 5.3.1 备份策略

**RDB备份** (定期全量):
```bash
# 每日凌晨备份
0 2 * * * redis-cli BGSAVE
```

**AOF备份** (实时增量):
```bash
# 启用AOF持久化
appendonly yes
appendfsync everysec
```

**业务数据导出**:
```bash
# 导出特定服务数据
redis-cli --scan --pattern "comsrv:*" > comsrv_keys.txt
redis-cli --scan --pattern "alarmsrv:*" > alarmsrv_keys.txt
```

#### 5.3.2 恢复策略

**完整恢复**:
1. 停止Redis服务
2. 恢复RDB文件到数据目录
3. 重启Redis服务
4. 验证数据完整性

**增量恢复**:
1. 从AOF文件恢复增量数据
2. 重放指定时间段的操作
3. 验证关键业务数据

#### 5.3.3 灾难恢复

**主从切换**:
```bash
# 从节点提升为主节点
redis-cli SLAVEOF NO ONE

# 重新配置从节点
redis-cli SLAVEOF new_master_ip new_master_port
```

**集群恢复**:
```bash
# 节点故障转移
redis-cli CLUSTER FAILOVER

# 手动故障转移
redis-cli CLUSTER FAILOVER FORCE
```

### 5.4 故障排查

#### 5.4.1 常见问题诊断

**内存不足**:
```bash
# 检查内存使用
INFO memory
# 检查大键
redis-cli --bigkeys
# 检查键过期情况
INFO keyspace
```

**性能下降**:
```bash
# 检查慢查询
SLOWLOG GET 10
# 检查客户端连接
INFO clients
# 检查网络状态
INFO stats
```

**数据不一致**:
```bash
# 检查主从同步
INFO replication
# 检查键是否存在
EXISTS key_name
# 检查键类型和值
TYPE key_name
GET key_name
```

#### 5.4.2 调试命令

**键空间分析**:
```bash
# 统计Hash键数量
redis-cli --scan --pattern "comsrv:*" | wc -l
redis-cli --scan --pattern "alarm:*" | wc -l
redis-cli --scan --pattern "modsrv:*" | wc -l

# 统计Hash字段数量
redis-cli HLEN comsrv:1001:m
redis-cli HLEN modsrv:power_calc:measurement
```

**数据完整性检查**:
```bash
# 检查点位数据连续性
redis-cli HMGET comsrv:1001:m 10001 10002 10003

# 检查告警索引一致性
redis-cli SMEMBERS alarm:category:temperature

# 检查模型数据
redis-cli HGETALL modsrv:power_calc:measurement
```

**性能测试**:
```bash
# Redis基准测试
redis-benchmark -h localhost -p 6379 -n 100000 -c 50

# 自定义测试脚本
redis-cli --eval performance_test.lua comsrv:test , 1000
```

## 6. 质量保证

### 6.1 数据验证规则

#### 6.1.1 键格式验证

**正则表达式**:
```regex
# Hash键格式
^(comsrv|modsrv):[a-zA-Z0-9_]+:(m|s|c|a|measurement|control)$

# String键格式(告警)
^alarm:[a-zA-Z0-9_-]+$
```

**验证函数示例**:
```python
import re

def validate_hash_key(key: str) -> bool:
    """验证Hash键格式"""
    patterns = [
        r'^comsrv:\d+:(m|s|c|a)$',  # comsrv:1001:m
        r'^modsrv:[a-zA-Z0-9_]+:(measurement|control)$',  # modsrv:power_calc:measurement
    ]
    return any(re.match(pattern, key) for pattern in patterns)

def validate_alarm_key(key: str) -> bool:
    """验证告警键格式"""
    pattern = r'^alarm:[a-zA-Z0-9_-]+$'
    return re.match(pattern, key) is not None
```

#### 6.1.2 数据类型验证

**数值验证**:
```python
def validate_numeric_value(value: str) -> bool:
    """验证数值格式"""
    try:
        float_val = float(value)
        # 检查精度不超过6位小数
        decimal_places = len(value.split('.')[-1]) if '.' in value else 0
        return decimal_places <= 6
    except ValueError:
        return False
```

**时间戳验证**:
```python
def validate_timestamp(timestamp: int) -> bool:
    """验证时间戳格式"""
    # 检查是否为合理的毫秒时间戳
    return 1000000000000 <= timestamp <= 9999999999999
```

### 6.2 自动化测试

#### 6.2.1 单元测试

**键格式测试**:
```python
class TestKeyFormat:
    def test_hash_key_valid(self):
        assert validate_hash_key("comsrv:1001:m")
        assert validate_hash_key("modsrv:power_calc:measurement")

    def test_alarm_key_valid(self):
        assert validate_alarm_key("alarm:550e8400-e29b-41d4")
        assert not validate_alarm_key("alarmsrv:123")
```

**数据操作测试**:
```python
class TestDataOperations:
    def test_hash_data_crud(self):
        # Create
        redis_client.hset("comsrv:1001:m", "10001", "25.123456")

        # Read
        value = redis_client.hget("comsrv:1001:m", "10001")
        assert value == "25.123456"

        # Update
        redis_client.hset("comsrv:1001:m", "10001", "26.789012")

        # Delete field
        redis_client.hdel("comsrv:1001:m", "10001")
```

#### 6.2.2 集成测试

**Pub/Sub测试**:
```python
class TestPubSub:
    def test_channel_publish_subscribe(self):
        # 订阅通道
        pubsub = redis_client.pubsub()
        pubsub.subscribe("comsrv:1001:m")

        # 发布消息
        redis_client.publish("comsrv:1001:m", "10001:25.123456")

        # 验证接收
        received = pubsub.get_message()
        assert received is not None
        assert "10001:25.123456" in str(received['data'])
```

**批量操作测试**:
```python
class TestBatchOperations:
    def test_hash_batch_operations(self):
        # 批量写入
        pipe = redis_client.pipeline()
        for i in range(1000):
            pipe.hset("comsrv:1001:m", str(10000+i), f"{25.0+i:.6f}")
        pipe.execute()

        # 批量读取验证
        fields = [str(10000+i) for i in range(1000)]
        values = redis_client.hmget("comsrv:1001:m", fields)
        assert len(values) == 1000
        assert all(v is not None for v in values)

        # 获取所有字段
        all_data = redis_client.hgetall("comsrv:1001:m")
        assert len(all_data) == 1000
```

#### 6.2.3 性能测试

**并发写入测试**:
```python
import concurrent.futures
import time

def write_points_batch(channel_id: int, start_id: int, count: int):
    """批量写入点位数据"""
    pipe = redis_client.pipeline()
    hash_key = f"comsrv:{channel_id}:m"
    for i in range(count):
        field = str(start_id + i)
        value = f"{25.0 + i:.6f}"
        pipe.hset(hash_key, field, value)
    pipe.execute()

def test_concurrent_writes():
    """并发写入性能测试"""
    start_time = time.time()

    with concurrent.futures.ThreadPoolExecutor(max_workers=10) as executor:
        futures = []
        for i in range(10):
            future = executor.submit(write_points_batch, i * 1000, 1000)
            futures.append(future)

        # 等待所有任务完成
        concurrent.futures.wait(futures)

    end_time = time.time()
    duration = end_time - start_time
    ops_per_second = 10000 / duration

    print(f"写入10000个点位耗时: {duration:.2f}秒")
    print(f"写入速度: {ops_per_second:.0f} ops/sec")
```

**查询性能测试**:
```python
def test_query_performance():
    """查询性能测试"""
    # Hash单字段查询
    start_time = time.time()
    for i in range(1000):
        redis_client.hget("comsrv:1001:m", str(10000+i))
    single_query_time = time.time() - start_time

    # Hash全量查询
    start_time = time.time()
    all_data = redis_client.hgetall("comsrv:1001:m")
    batch_query_time = time.time() - start_time

    print(f"1000次Hash字段查询耗时: {single_query_time:.3f}秒")
    print(f"Hash全量查询耗时: {batch_query_time:.3f}秒")
    print(f"全量查询提升: {single_query_time/batch_query_time:.1f}倍")
```

## 7. 最佳实践

### 7.1 开发规范

#### 7.1.1 键命名规范

**强制要求**:
- Hash键格式: `{service}:{entity}:{type}`
- String键格式: `{service}:{id}`
- 使用小写字母和下划线
- 避免使用特殊字符和空格
- 键名总长度不超过256字符

**推荐做法**:
- 使用有意义的名称
- 保持命名一致性
- 使用标准的缩写
- 添加必要的注释

**示例对比**:
```
✅ 正确: comsrv:1001:m (Hash键)
✅ 正确: alarm:550e8400-e29b (String键)
❌ 错误: comsrv:1001:m:10001 (不再使用点位级键)
❌ 错误: comsrv-1001-m (使用了连字符)
```

#### 7.1.2 数据类型选择

**String类型** - 适用于:
- 简单数值存储
- 原子操作需求
- 高频读写场景

**Hash类型** - 适用于:
- 结构化数据
- 部分字段更新
- 相关数据聚合

**Set类型** - 适用于:
- 唯一值集合
- 成员关系判断
- 集合运算需求

**List类型** - 适用于:
- 有序数据序列
- FIFO/LIFO操作
- 历史记录存储

#### 7.1.3 事务处理

**原子操作示例**:
```python
def atomic_hash_update_with_publish(hash_key: str, field: str, value: str, channel: str):
    """原子更新Hash字段并发布"""
    pipe = redis_client.pipeline(transaction=True)
    pipe.multi()
    pipe.hset(hash_key, field, value)
    pipe.publish(channel, f"{field}:{value}")
    return pipe.execute()
```

**条件更新示例**:
```python
def conditional_update(key: str, expected_value: str, new_value: str) -> bool:
    """条件更新"""
    with redis_client.pipeline() as pipe:
        while True:
            try:
                pipe.watch(key)
                current_value = pipe.get(key)
                if current_value == expected_value:
                    pipe.multi()
                    pipe.set(key, new_value)
                    pipe.execute()
                    return True
                else:
                    pipe.unwatch()
                    return False
            except redis.WatchError:
                # 重试
                continue
```

### 7.2 性能优化

#### 7.2.1 批量操作优化

**Pipeline批处理**:
```python
def batch_update_channel(channel_key: str, updates: Dict[str, str]):
    """批量更新通道数据"""
    pipe = redis_client.pipeline(transaction=False)
    for field, value in updates.items():
        pipe.hset(channel_key, field, value)
    # 发布通道级通知
    pipe.publish(channel_key, f"updated:{len(updates)}")
    return pipe.execute()
```

**批量发布优化**:
```python
def batch_publish_updates(updates: List[Tuple[str, dict]]):
    """批量发布更新"""
    pipe = redis_client.pipeline(transaction=False)
    for channel, message in updates:
        pipe.publish(channel, json.dumps(message))
    return pipe.execute()
```

#### 7.2.2 内存使用优化

**值压缩策略**:
```python
def compress_point_value(value: float, timestamp: int) -> str:
    """压缩点位值"""
    # 限制精度减少存储空间
    compressed_value = f"{value:.6f}"
    return f"{compressed_value}:{timestamp}"

def decompress_point_value(compressed: str) -> Tuple[float, int]:
    """解压点位值"""
    parts = compressed.split(':')
    value = float(parts[0])
    timestamp = int(parts[1])
    return value, timestamp
```

#### 7.2.3 查询优化

**索引策略**:
```python
def create_time_index(service: str, timestamp: int):
    """创建时间索引"""
    date_key = f"{service}:date:{timestamp // 86400000}"  # 按天分组
    hour_key = f"{service}:hour:{timestamp // 3600000}"   # 按小时分组
    redis_client.sadd(date_key, timestamp)
    redis_client.sadd(hour_key, timestamp)

def query_by_time_range(service: str, start_ts: int, end_ts: int) -> List[int]:
    """按时间范围查询"""
    start_hour = start_ts // 3600000
    end_hour = end_ts // 3600000

    result_keys = []
    for hour in range(start_hour, end_hour + 1):
        hour_key = f"{service}:hour:{hour}"
        result_keys.append(hour_key)

    if result_keys:
        return redis_client.sunion(result_keys)
    return []
```

### 7.3 错误处理

#### 7.3.1 连接异常处理

```python
import redis
import time
from typing import Optional

class ResilientRedisClient:
    def __init__(self, url: str, max_retries: int = 3):
        self.url = url
        self.max_retries = max_retries
        self.client: Optional[redis.Redis] = None
        self._connect()

    def _connect(self):
        """建立Redis连接"""
        try:
            self.client = redis.from_url(self.url)
            self.client.ping()
        except redis.RedisError as e:
            print(f"Redis连接失败: {e}")
            self.client = None

    def execute_with_retry(self, operation, *args, **kwargs):
        """带重试的操作执行"""
        for attempt in range(self.max_retries):
            try:
                if not self.client:
                    self._connect()

                if self.client:
                    return operation(*args, **kwargs)

            except redis.ConnectionError:
                print(f"连接错误，重试 {attempt + 1}/{self.max_retries}")
                time.sleep(2 ** attempt)  # 指数退避
                self.client = None

            except redis.TimeoutError:
                print(f"超时错误，重试 {attempt + 1}/{self.max_retries}")
                time.sleep(1)

        raise redis.RedisError(f"操作失败，已重试{self.max_retries}次")

    def set(self, key: str, value: str):
        return self.execute_with_retry(self.client.set, key, value)

    def get(self, key: str):
        return self.execute_with_retry(self.client.get, key)
```

#### 7.3.2 数据一致性检查

```python
def verify_data_consistency(key: str, expected_value: str) -> bool:
    """验证数据一致性"""
    try:
        actual_value = redis_client.get(key)
        if actual_value != expected_value:
            print(f"数据不一致: {key}, 期望: {expected_value}, 实际: {actual_value}")
            return False
        return True
    except redis.RedisError as e:
        print(f"数据验证失败: {e}")
        return False

def repair_inconsistent_data(key: str, correct_value: str):
    """修复不一致的数据"""
    try:
        redis_client.set(key, correct_value)
        print(f"数据已修复: {key} = {correct_value}")
    except redis.RedisError as e:
        print(f"数据修复失败: {e}")
```

### 7.4 安全考虑

#### 7.4.1 访问控制

**ACL配置示例**:
```bash
# 创建只读用户
ACL SETUSER readonly on +@read -@dangerous >readonly_password

# 创建服务专用用户
ACL SETUSER comsrv_user on +@all ~comsrv:* >comsrv_password
ACL SETUSER modsrv_user on +@all ~modsrv:* >modsrv_password
ACL SETUSER alarm_user on +@all ~alarm:* >alarm_password
```

#### 7.4.2 数据加密

**敏感数据加密**:
```python
import hashlib
import hmac

def encrypt_sensitive_value(value: str, key: str) -> str:
    """加密敏感数据"""
    return hmac.new(
        key.encode('utf-8'),
        value.encode('utf-8'),
        hashlib.sha256
    ).hexdigest()

def verify_sensitive_value(value: str, encrypted: str, key: str) -> bool:
    """验证敏感数据"""
    expected = encrypt_sensitive_value(value, key)
    return hmac.compare_digest(expected, encrypted)
```

## 8. 总结

### 8.1 核心特性

VoltageEMS Redis数据结构规范v3.0提供了：

1. **Hash结构优化**: 大幅减少键数量，提升批量操作效率
2. **通道级数据组织**: `{service}:{entity}:{type}` Hash键格式
3. **内存优化**: 相比旧结构节省30%+内存
4. **简化的Pub/Sub**: 通道级发布，减少消息数量
5. **更好的扩展性**: 从百万键减少到千级键

### 8.2 适用场景

本规范适用于：
- 工业IoT数据采集系统
- 实时监控和告警平台
- 数据模型计算引擎
- 规则引擎和自动化控制
- 历史数据存储和分析

### 8.3 维护更新

本文档将随系统演进持续更新，包括：
- 新增服务的数据结构定义
- 性能优化策略调整
- 运维实践经验积累
- 最佳实践案例更新

---

**文档版本**: v3.0
**最后更新**: 2025-07-23
**主要变更**: 从String键迁移到Hash结构，删除数据质量字段
**维护团队**: VoltageEMS开发团队
