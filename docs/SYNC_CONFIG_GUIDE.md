# 通用同步引擎配置指南

## 目录
1. [概述](#概述)
2. [快速开始](#快速开始)
3. [配置结构](#配置结构)
4. [预定义规则](#预定义规则)
5. [自定义配置](#自定义配置)
6. [操作命令](#操作命令)
7. [最佳实践](#最佳实践)

## 概述

通用同步引擎提供了一个配置驱动的数据同步机制，支持在不同服务之间灵活地同步数据。通过 JSON 配置定义同步规则，无需修改代码即可实现复杂的数据映射和转换。

### 核心概念

- **同步规则 (Sync Rule)**: 定义源数据和目标数据之间的映射关系
- **数据映射 (Data Mapping)**: 字段级别的映射配置
- **转换函数 (Transform)**: 数据转换和处理逻辑
- **反向映射 (Reverse Mapping)**: 双向同步的索引机制

## 快速开始

### 1. 加载同步引擎

```bash
# 加载核心同步引擎
redis-cli -x FUNCTION LOAD REPLACE < scripts/redis-functions/sync_engine.lua

# 加载配置初始化脚本
redis-cli -x FUNCTION LOAD REPLACE < scripts/redis-functions/sync_config_init.lua

# 初始化预定义配置
redis-cli FCALL init_sync_configs 0
```

### 2. 查看同步状态

```bash
# 查看所有同步规则状态
redis-cli FCALL get_sync_status 0

# 查看特定规则配置
redis-cli FCALL sync_config_get 1 comsrv_to_modsrv_T
```

### 3. 执行同步

```bash
# 手动触发所有活动规则的同步
redis-cli FCALL batch_sync_all 0

# 执行特定规则的同步
redis-cli FCALL sync_pattern_execute 1 comsrv_to_modsrv_T
```

## 配置结构

### 基本配置格式

```json
{
  "enabled": true,                    // 是否启用此规则
  "description": "规则描述",          // 规则说明
  "source": {                         // 源数据配置
    "pattern": "comsrv:*:T",          // 键模式，支持通配符
    "type": "hash",                   // 数据类型: hash/string/list/set
    "fields": ["field1", "field2"]   // 可选：指定要同步的字段
  },
  "target": {                         // 目标数据配置
    "pattern": "modsrv:model:$1:data", // 目标键模式，支持变量替换
    "type": "hash",
    "use_reverse_mapping": true       // 使用反向映射查找目标
  },
  "field_mapping": {                  // 字段映射
    "source_field": "target_field"    // 源字段到目标字段的映射
  },
  "transform": {                      // 数据转换
    "type": "direct",                 // 转换类型
    "config": {}                      // 转换配置
  },
  "reverse_mapping": {                // 反向映射配置
    "enabled": true,
    "pattern": "sync:reverse:$rule:$key"
  }
}
```

### 变量替换

在 pattern 中支持以下变量：

- `*` - 通配符，匹配任意字符
- `$1, $2, ...` - 位置变量，从源键中提取
- `${name}` - 命名变量
- `$channel` - 通道 ID（特定于 comsrv）
- `$point_id` - 点位 ID
- `$model_id` - 模型 ID（特定于 modsrv）
- `$timestamp` - 当前时间戳

## 预定义规则

系统提供了以下预定义的同步规则：

### 1. Comsrv → Modsrv

| 规则 ID | 描述 | 源模式 | 目标 |
|---------|------|--------|------|
| `comsrv_to_modsrv_T` | 遥测数据同步 | `comsrv:*:T` | modsrv 模型 measurement |
| `comsrv_to_modsrv_S` | 遥信数据同步 | `comsrv:*:S` | modsrv 模型 measurement |
| `comsrv_to_modsrv_C` | 控制数据同步 | `comsrv:*:C` | modsrv 模型 values |
| `comsrv_to_modsrv_A` | 调节数据同步 | `comsrv:*:A` | modsrv 模型 values |

### 2. Comsrv → Alarmsrv

| 规则 ID | 描述 | 源模式 | 目标 |
|---------|------|--------|------|
| `comsrv_to_alarmsrv` | 告警检测 | `comsrv:*:T` | `alarmsrv:channel:*:latest` |

### 3. Comsrv → Hissrv

| 规则 ID | 描述 | 源模式 | 目标 |
|---------|------|--------|------|
| `comsrv_to_hissrv` | 历史数据存储 | `comsrv:*:*` | `hissrv:batch:*` |

### 4. Modsrv → Rulesrv

| 规则 ID | 描述 | 源模式 | 目标 |
|---------|------|--------|------|
| `modsrv_to_rulesrv` | 规则引擎输入 | `modsrv:model:*:measurement` | `rulesrv:input:*` |

## 自定义配置

### 创建新的同步规则

```bash
# 定义配置
cat > my_sync_rule.json << 'EOF'
{
  "enabled": true,
  "description": "自定义同步规则",
  "source": {
    "pattern": "my_service:*:data",
    "type": "hash"
  },
  "target": {
    "pattern": "other_service:$1:processed",
    "type": "hash"
  },
  "transform": {
    "type": "numeric",
    "config": {
      "scale": 0.1,
      "offset": 273.15
    }
  }
}
EOF

# 加载配置
redis-cli FCALL sync_config_set 1 my_custom_rule "$(cat my_sync_rule.json)"
```

### 内置转换函数

1. **direct** - 直接映射，不转换
2. **numeric** - 数值转换（缩放、偏移）
3. **json_extract** - JSON 字段提取
4. **time_series** - 时序数据转换
5. **alarm_check** - 告警检测
6. **rule_input** - 规则引擎输入格式化

### 扩展转换函数

在 `sync_engine.lua` 中添加自定义转换函数：

```lua
transform_functions.my_transform = function(value, config)
    -- 自定义转换逻辑
    local result = value
    -- ... 处理逻辑 ...
    return result
end
```

## 操作命令

### 配置管理

```bash
# 设置配置
redis-cli FCALL sync_config_set 1 <rule_id> '<config_json>'

# 获取配置
redis-cli FCALL sync_config_get 1 <rule_id>

# 删除配置
redis-cli FCALL sync_config_delete 1 <rule_id>

# 启用/禁用规则
redis-cli FCALL toggle_sync_rule 1 <rule_id> true|false
```

### 同步执行

```bash
# 执行单个同步
redis-cli FCALL sync_execute 3 <rule_id> <source_key> <target_key>

# 批量同步
redis-cli FCALL sync_batch_execute 1 <rule_id> '<batch_json>'

# 基于模式同步
redis-cli FCALL sync_pattern_execute 1 <rule_id>

# 执行所有活动规则
redis-cli FCALL batch_sync_all 0
```

### 监控和统计

```bash
# 获取统计信息
redis-cli FCALL sync_stats_get 1 <rule_id>

# 重置统计
redis-cli FCALL sync_stats_reset 1 <rule_id>

# 反向查询
redis-cli FCALL sync_reverse_lookup 3 <rule_id> <target_key> <field>

# 获取所有规则状态
redis-cli FCALL get_sync_status 0
```

## 最佳实践

### 1. 性能优化

- **批量同步**: 尽量使用批量同步而非单条同步
- **异步处理**: 对于大量数据，使用异步同步模式
- **合理的批次大小**: 建议批次大小 100-1000 条

### 2. 配置管理

- **版本控制**: 将配置文件纳入版本控制
- **环境隔离**: 不同环境使用不同的规则前缀
- **文档化**: 为每个规则添加清晰的描述

### 3. 监控告警

- **定期检查统计**: 监控同步成功率和失败率
- **设置告警阈值**: 当失败率超过阈值时触发告警
- **日志记录**: 记录关键同步事件

### 4. 数据一致性

- **双向同步**: 谨慎配置双向同步，避免循环
- **事务支持**: 关键数据使用事务保证一致性
- **数据验证**: 在转换函数中添加数据验证

### 5. 故障恢复

- **重试机制**: 配置合理的重试次数和间隔
- **死信队列**: 将失败的同步任务放入死信队列
- **手动干预**: 提供手动触发同步的接口

## 示例场景

### 场景 1: 设备数据采集到模型映射

```json
{
  "enabled": true,
  "description": "将 Modbus 设备数据映射到设备模型",
  "source": {
    "pattern": "comsrv:1001:T",
    "type": "hash"
  },
  "target": {
    "use_reverse_mapping": true,
    "type": "hash"
  },
  "reverse_mapping": {
    "enabled": true,
    "pattern": "modsrv:reverse:1001:$point_id"
  },
  "transform": {
    "type": "numeric",
    "config": {
      "scale": 0.1,
      "offset": 0
    }
  }
}
```

### 场景 2: 实时告警检测

```json
{
  "enabled": true,
  "description": "温度超限告警",
  "source": {
    "pattern": "comsrv:*:T",
    "type": "hash",
    "fields": ["1", "2", "3"]  // 只监控特定点位
  },
  "target": {
    "pattern": "alarmsrv:temp_alarm:$1",
    "type": "string"
  },
  "transform": {
    "type": "alarm_check",
    "config": {
      "threshold_high": 80,
      "threshold_low": 10,
      "alarm_level": "warning"
    }
  }
}
```

### 场景 3: 聚合数据同步

```json
{
  "enabled": true,
  "description": "聚合多个通道数据",
  "source": {
    "pattern": "comsrv:[1-9]*:T",
    "type": "hash"
  },
  "target": {
    "pattern": "dashboard:summary",
    "type": "hash"
  },
  "field_mapping": {
    "1": "total_power",
    "2": "avg_temperature",
    "3": "max_pressure"
  },
  "transform": {
    "type": "aggregate",
    "config": {
      "operations": {
        "1": "sum",
        "2": "avg",
        "3": "max"
      }
    }
  }
}
```

## 故障排查

### 常见问题

1. **同步不生效**
   - 检查规则是否启用: `redis-cli FCALL sync_config_get 1 <rule_id>`
   - 查看统计信息: `redis-cli FCALL sync_stats_get 1 <rule_id>`
   - 确认源数据存在: `redis-cli KEYS <source_pattern>`

2. **数据不一致**
   - 检查转换函数配置
   - 验证字段映射是否正确
   - 查看反向映射是否建立

3. **性能问题**
   - 减小批次大小
   - 启用异步同步
   - 优化源数据查询模式

### 调试命令

```bash
# 测试模式匹配
redis-cli KEYS "comsrv:*:T"

# 手动执行同步并查看结果
redis-cli FCALL sync_execute 3 test_rule "comsrv:1001:T" "modsrv:test"

# 查看错误日志
redis-cli HGET "sync:stats:test_rule" last_error

# 验证反向映射
redis-cli GET "modsrv:reverse:1001:1"
```

## 总结

通用同步引擎提供了强大而灵活的数据同步能力。通过合理的配置和管理，可以实现：

- ✅ 服务间数据的实时同步
- ✅ 复杂的数据转换和映射
- ✅ 高性能的批量处理
- ✅ 可靠的故障恢复机制
- ✅ 完善的监控和统计

遵循本指南的最佳实践，可以构建稳定、高效的数据同步系统。