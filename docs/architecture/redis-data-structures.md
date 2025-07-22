# VoltageEMS Redis数据结构规范

**版本**: v2.0
**更新日期**: 2025-07-22
**适用系统**: VoltageEMS v2.x

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

### 1.3 数据流向

```
设备数据 → comsrv → Redis存储/发布 → 其他服务订阅处理
               ↓
         实时数据键存储
               ↓
         批量转存InfluxDB
```

## 2. 统一键格式规范

### 2.1 键命名约定

**基本格式**: `{service}:{entity}:{type}:{id}`

- **service**: 服务标识符 (comsrv/alarmsrv/modsrv/rulesrv)
- **entity**: 实体标识符 (channelID/modelID/alarmID等)
- **type**: 数据类型标识符
- **id**: 具体对象ID (pointID等)

**字符约束**:
- 仅使用字母数字和下划线: `[a-zA-Z0-9_]`
- 冒号`:`用作分隔符，不可在字段中使用
- 总长度不超过256字符

### 2.2 点位级精确订阅

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

## 3. 服务数据结构定义

### 3.1 comsrv (通信服务)

**职责**: 设备数据采集、协议转换、实时数据发布

#### 3.1.1 键格式规范

**存储键**: `comsrv:{channelID}:{type}:{pointID}`
**发布通道**: `comsrv:{channelID}:{type}:{pointID}`

**类型映射**:
- `m`: 测量值 (YC - Yao Ce)
- `s`: 信号值 (YX - Yao Xin)
- `c`: 控制值 (YK - Yao Kong)
- `a`: 调节值 (YT - Yao Tiao)

#### 3.1.2 数据格式

**存储值** (String):
```
格式: "{value:.6}"
示例: "25.123456"
说明: 6位小数精度，确保工业测量精度
```

**发布消息** (JSON):
```json
{
    "point_id": 10001,
    "value": 25.123456,
    "timestamp": 1642592400000,
    "quality": 192,
    "raw_value": 25.1
}
```

#### 3.1.3 质量标识

| 质量码 | 含义 | 说明 |
|--------|------|------|
| 192 | GOOD | 数据质量良好 |
| 64 | UNCERTAIN | 数据质量不确定 |
| 0 | BAD | 数据质量差或无效 |

#### 3.1.4 键示例

```
comsrv:1001:m:10001 → "25.123456"    # 通道1001测量点10001
comsrv:1001:m:10002 → "26.789012"    # 通道1001测量点10002
comsrv:1001:s:20001 → "1"            # 通道1001信号点20001(开关状态)
comsrv:1002:c:30001 → "0"            # 通道1002控制点30001
comsrv:1002:a:40001 → "50.500000"    # 通道1002调节点40001
```

#### 3.1.5 操作接口

**读取操作**:
```bash
# 单点查询
GET comsrv:1001:m:10001

# 批量查询同类型点位
MGET comsrv:1001:m:10001 comsrv:1001:m:10002

# 模式查询(需要使用SCAN)
SCAN 0 MATCH comsrv:1001:m:*
```

**写入操作**:
```bash
# 单点写入
SET comsrv:1001:m:10001 "25.123456"

# 批量写入
MSET comsrv:1001:m:10001 "25.123456" comsrv:1001:m:10002 "26.789012"

# 发布通知
PUBLISH comsrv:1001:m:10001 '{"point_id":10001,"value":25.123456,"timestamp":1642592400000,"quality":192}'
```

### 3.2 alarmsrv (告警服务)

**职责**: 告警生成、状态管理、分类索引

#### 3.2.1 键格式规范

**主数据键**: `alarmsrv:{alarmID}`
**分类索引**: `alarmsrv:category:{category}`
**级别索引**: `alarmsrv:level:{level}`
**状态索引**: `alarmsrv:status:{status}`
**时间索引**: `alarmsrv:date:{YYYY-MM-DD}`
**小时分片**: `alarmsrv:buckets:{YYYYMMDDHH}`
**实时索引**: `alarmsrv:realtime`

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
alarmsrv:category:temperature → {alarm_id_1, alarm_id_2, ...}
alarmsrv:category:pressure → {alarm_id_3, alarm_id_4, ...}
```

**时间分片** (Hash):
```
alarmsrv:buckets:2024010110 → {
    "alarm_id_1": "compressed_alarm_data",
    "alarm_id_2": "compressed_alarm_data"
}
```

**实时索引** (Hash):
```
alarmsrv:realtime → {
    "temperature:alarm_123": "{\"id\":\"alarm_123\",\"level\":\"Critical\",\"created_at\":\"...\"}",
    "pressure:alarm_456": "{\"id\":\"alarm_456\",\"level\":\"High\",\"created_at\":\"...\"}"
}
```

#### 3.2.6 TTL策略

- **活跃告警**: 无过期时间
- **已解决告警**: 30天后清理（可配置）
- **索引数据**: 跟随主数据清理
- **分片数据**: 按配置保留期清理

### 3.3 modsrv (模型服务)

**职责**: 数据模型计算、监视值管理、控制命令执行

#### 3.3.1 键格式规范

**监视值**: `modsrv:{modelID}:{monitorType}:{pointID}`
**控制命令**: `cmd:{commandID}`
**命令列表**: `cmd:list:{modelID}`
**模型输出**: `modsrv:{modelID}:output`

**监视类型映射**:
- `mv:m`: 监视测量值 (Monitor Value: Measurement)
- `mv:s`: 监视信号值 (Monitor Value: Signal)
- `mo`: 模型输出 (Model Output)
- `mi`: 中间计算值 (Model Intermediate)

**控制类型映射**:
- `cc:c`: 控制命令 (Control Command: Control)
- `cc:a`: 调节命令 (Control Command: Adjust)

#### 3.3.2 数据格式

**监视值** (String):
```
格式: "{value}:{timestamp}:{quality}:{source}"
示例: "25.123456:1642592400000:192:model_power_calc"
说明: value(6位小数):timestamp(毫秒):quality:source
```

**控制命令** (Hash):
```json
{
    "id": "cmd_550e8400-e29b-41d4-a716-446655440000",
    "channel_id": "1001",
    "point_id": "20001",
    "command_type": "cc:c",
    "value": "1.0",
    "status": "pending",
    "created_at": "1642592400000",
    "updated_at": "1642592400000",
    "message": null,
    "source_model": "model_power_control"
}
```

**模型输出** (JSON):
```json
{
    "model_id": "model_power_calc",
    "outputs": {
        "total_power": 1200.5,
        "efficiency": 0.856,
        "load_factor": 0.78
    },
    "timestamp": 1642592400000,
    "execution_time_ms": 45
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

#### 3.3.4 键示例

```
# 监视值
modsrv:power_calc:mv:m:10001 → "25.123456:1642592400000:192:model_power_calc"
modsrv:power_calc:mv:s:20001 → "1:1642592400000:192:model_power_calc"
modsrv:power_calc:mo:result → "1200.500000:1642592400000:192:model_power_calc"

# 控制命令
cmd:cmd_550e8400-e29b-41d4-a716-446655440000 → {Hash结构}

# 模型输出
modsrv:power_calc:output → {JSON字符串}
```

#### 3.3.5 TTL策略

- **监视值**: 无过期时间（实时数据）
- **控制命令**: 24小时过期
- **模型输出**: 7天过期
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

### 3.5 hissrv (历史数据服务)

**职责**: 历史数据采集、存储转换、查询服务

#### 3.5.1 数据流模式

```
Redis实时数据 → hissrv订阅 → 批量处理 → InfluxDB存储
```

#### 3.5.2 订阅配置

**默认订阅模式**: `comsrv:*`

**订阅处理逻辑**:
- 解析通道格式: `comsrv:{channelID}:{type}:{pointID}`
- 数据类型映射: m(测量) → measurement, s(信号) → signal
- 批量聚合: 配置化批量大小和超时
- 质量过滤: 仅存储GOOD质量数据

#### 3.5.3 批量处理配置

```yaml
batch_config:
  size: 1000              # 批量大小
  timeout_ms: 5000        # 批量超时
  quality_filter: [192]   # 质量过滤
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

**点查询** (O(1)):
```bash
GET comsrv:1001:m:10001
```

**批量查询** (O(N)):
```bash
MGET comsrv:1001:m:10001 comsrv:1001:m:10002 comsrv:1001:m:10003
```

**模式查询** (O(N)):
```bash
# 使用SCAN避免阻塞
SCAN 0 MATCH comsrv:1001:m:* COUNT 100
```

**索引查询** (O(log N)):
```bash
# Set索引查询
SMEMBERS alarmsrv:category:temperature
# Hash索引查询
HGETALL alarmsrv:realtime
```

#### 4.1.2 Pipeline批处理

**批量读取示例**:
```python
pipe = redis.pipeline()
pipe.get("comsrv:1001:m:10001")
pipe.get("comsrv:1001:m:10002")
pipe.get("comsrv:1001:m:10003")
results = pipe.execute()
```

**批量写入示例**:
```python
pipe = redis.pipeline()
pipe.set("comsrv:1001:m:10001", "25.123456")
pipe.publish("comsrv:1001:m:10001", message)
pipe.execute()
```

### 4.2 内存优化

#### 4.2.1 数据压缩策略

- **数值精度控制**: 6位小数精度
- **时间戳优化**: 毫秒时间戳
- **字符串复用**: 相同值引用优化
- **过期清理**: 自动TTL清理

#### 4.2.2 容量估算

**单点位存储成本**:
- 键名: ~30字节
- 数值: ~15字节
- 总计: ~45字节/点位

**百万点位容量**:
- 实时数据: ~45MB
- 索引数据: ~10MB
- 元数据: ~5MB
- 总计: ~60MB

### 4.3 扩展策略

#### 4.3.1 水平扩展 (Redis Cluster)

**分片策略**:
- 按通道ID分片: `{channelID}` hash slot
- 跨服务分布: 每个服务独立分片
- 索引复制: 关键索引多节点复制

**分片示例**:
```
Shard 1: comsrv:1001-1999:*
Shard 2: comsrv:2000-2999:*
Shard 3: comsrv:3000-3999:*
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
PUBSUB NUMSUB comsrv:1001:m:10001
# 通道订阅者数量
```

#### 5.1.2 业务监控指标

**数据更新频率**:
- 各通道数据更新QPS
- 异常数据质量比例
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
# 统计键模式
redis-cli --scan --pattern "comsrv:*" | wc -l
redis-cli --scan --pattern "alarmsrv:*" | wc -l
redis-cli --scan --pattern "modsrv:*" | wc -l
```

**数据完整性检查**:
```bash
# 检查点位数据连续性
redis-cli MGET comsrv:1001:m:10001 comsrv:1001:m:10002 comsrv:1001:m:10003

# 检查告警索引一致性
redis-cli SMEMBERS alarmsrv:category:temperature
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
^(comsrv|alarmsrv|modsrv|rulesrv):[a-zA-Z0-9_]+:[a-zA-Z0-9_:]+:[a-zA-Z0-9_]+$
```

**验证函数示例**:
```python
import re

def validate_key_format(key: str) -> bool:
    """验证Redis键格式"""
    pattern = r'^(comsrv|alarmsrv|modsrv|rulesrv):[a-zA-Z0-9_]+:[a-zA-Z0-9_:]+:[a-zA-Z0-9_]+$'
    return re.match(pattern, key) is not None

def validate_comsrv_key(key: str) -> bool:
    """验证comsrv键格式"""
    pattern = r'^comsrv:\d+:(m|s|c|a):\d+$'
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

**质量码验证**:
```python
def validate_quality_code(quality: int) -> bool:
    """验证质量码"""
    valid_qualities = [0, 64, 192]
    return quality in valid_qualities
```

### 6.2 自动化测试

#### 6.2.1 单元测试

**键格式测试**:
```python
class TestKeyFormat:
    def test_comsrv_key_valid(self):
        assert validate_comsrv_key("comsrv:1001:m:10001")
        assert validate_comsrv_key("comsrv:2000:s:20001")

    def test_comsrv_key_invalid(self):
        assert not validate_comsrv_key("comsrv:1001:x:10001")
        assert not validate_comsrv_key("invalid:1001:m:10001")
```

**数据操作测试**:
```python
class TestDataOperations:
    def test_point_data_crud(self):
        # Create
        redis_client.set("comsrv:1001:m:10001", "25.123456")

        # Read
        value = redis_client.get("comsrv:1001:m:10001")
        assert value == "25.123456"

        # Update
        redis_client.set("comsrv:1001:m:10001", "26.789012")

        # Delete
        redis_client.delete("comsrv:1001:m:10001")
```

#### 6.2.2 集成测试

**Pub/Sub测试**:
```python
class TestPubSub:
    def test_point_publish_subscribe(self):
        # 订阅通道
        pubsub = redis_client.pubsub()
        pubsub.subscribe("comsrv:1001:m:10001")

        # 发布消息
        message = {
            "point_id": 10001,
            "value": 25.123456,
            "timestamp": 1642592400000,
            "quality": 192
        }
        redis_client.publish("comsrv:1001:m:10001", json.dumps(message))

        # 验证接收
        received = pubsub.get_message()
        assert received is not None
```

**批量操作测试**:
```python
class TestBatchOperations:
    def test_batch_write_read(self):
        # 批量写入
        pipe = redis_client.pipeline()
        for i in range(1000):
            pipe.set(f"comsrv:1001:m:{10000+i}", f"{25.0+i:.6f}")
        pipe.execute()

        # 批量读取验证
        keys = [f"comsrv:1001:m:{10000+i}" for i in range(1000)]
        values = redis_client.mget(keys)
        assert len(values) == 1000
        assert all(v is not None for v in values)
```

#### 6.2.3 性能测试

**并发写入测试**:
```python
import concurrent.futures
import time

def write_points_batch(start_id: int, count: int):
    """批量写入点位数据"""
    pipe = redis_client.pipeline()
    for i in range(count):
        key = f"comsrv:1001:m:{start_id + i}"
        value = f"{25.0 + i:.6f}"
        pipe.set(key, value)
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
    # 单点查询
    start_time = time.time()
    for _ in range(1000):
        redis_client.get("comsrv:1001:m:10001")
    single_query_time = time.time() - start_time

    # 批量查询
    keys = [f"comsrv:1001:m:{10000+i}" for i in range(1000)]
    start_time = time.time()
    redis_client.mget(keys)
    batch_query_time = time.time() - start_time

    print(f"1000次单点查询耗时: {single_query_time:.3f}秒")
    print(f"1000点批量查询耗时: {batch_query_time:.3f}秒")
    print(f"批量查询提升: {single_query_time/batch_query_time:.1f}倍")
```

## 7. 最佳实践

### 7.1 开发规范

#### 7.1.1 键命名规范

**强制要求**:
- 严格遵循命名格式: `{service}:{entity}:{type}:{id}`
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
✅ 正确: comsrv:1001:m:10001
❌ 错误: CoMsRv:1001:Measurement:10001
❌ 错误: 1001:measurement:10001
❌ 错误: comsrv-1001-m-10001
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
def atomic_update_with_publish(key: str, value: str, channel: str, message: dict):
    """原子更新并发布"""
    pipe = redis_client.pipeline(transaction=True)
    pipe.multi()
    pipe.set(key, value)
    pipe.publish(channel, json.dumps(message))
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
def batch_update_points(updates: List[Tuple[str, str]]):
    """批量更新点位"""
    pipe = redis_client.pipeline(transaction=False)
    for key, value in updates:
        pipe.set(key, value)
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
def compress_point_value(value: float, quality: int, timestamp: int) -> str:
    """压缩点位值"""
    # 限制精度减少存储空间
    compressed_value = f"{value:.6f}"
    if quality == 192:  # GOOD质量，省略质量码
        return f"{compressed_value}:{timestamp}"
    else:
        return f"{compressed_value}:{timestamp}:{quality}"

def decompress_point_value(compressed: str) -> Tuple[float, int, int]:
    """解压点位值"""
    parts = compressed.split(':')
    value = float(parts[0])
    timestamp = int(parts[1])
    quality = int(parts[2]) if len(parts) > 2 else 192
    return value, quality, timestamp
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
ACL SETUSER alarmsrv_user on +@all ~alarmsrv:* >alarmsrv_password
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

VoltageEMS Redis数据结构规范提供了：

1. **统一的键格式**: `{service}:{entity}:{type}:{id}`
2. **点位级精确访问**: O(1)查询性能
3. **命名空间隔离**: 避免服务间数据冲突
4. **Pub/Sub一致性**: 存储与通信格式统一
5. **扩展性设计**: 支持百万级点位实时处理

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

**文档版本**: v2.0
**最后更新**: 2025-07-22
**维护团队**: VoltageEMS开发团队
