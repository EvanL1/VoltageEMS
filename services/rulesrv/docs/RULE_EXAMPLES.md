# rulesrv 规则示例文档

## 规则结构说明

每个规则包含以下关键部分：

```json
{
  "id": "规则唯一标识",
  "name": "规则名称",
  "description": "规则描述",
  "conditions": {
    "operator": "AND|OR",
    "conditions": [...]
  },
  "actions": [...],
  "enabled": true,
  "priority": 1,
  "cooldown_seconds": 300
}
```

## 条件操作符

### 比较操作符
- `==` : 等于
- `!=` : 不等于
- `>` : 大于
- `>=` : 大于等于
- `<` : 小于
- `<=` : 小于等于
- `contains` : 包含（字符串）

### 逻辑操作符
- `AND` : 所有条件都必须满足
- `OR` : 至少一个条件满足

## 动作类型

### 1. 设备控制 (device_control)
```json
{
  "action_type": "device_control",
  "config": {
    "device_id": "设备ID",
    "channel": "控制通道",
    "point": "控制点",
    "value": 控制值
  }
}
```

### 2. 发布消息 (publish)
```json
{
  "action_type": "publish",
  "config": {
    "channel": "消息通道",
    "message": "消息内容"
  }
}
```

### 3. 设置值 (set_value)
```json
{
  "action_type": "set_value",
  "config": {
    "key": "Redis键",
    "value": "设置的值",
    "ttl": null
  }
}
```

### 4. 发送通知 (notify)
```json
{
  "action_type": "notify",
  "config": {
    "level": "critical|warning|info",
    "message": "通知消息",
    "recipients": ["email@example.com"]
  }
}
```

## 实用规则示例

### 1. 电池管理规则

#### 低电量启动发电机
```json
{
  "id": "battery_low_start_gen",
  "name": "低电量启动发电机",
  "description": "当电池电量低于阈值时自动启动发电机",
  "conditions": {
    "operator": "AND",
    "conditions": [
      {
        "source": "battery.soc",
        "operator": "<=",
        "value": 20.0,
        "description": "电池电量 <= 20%"
      },
      {
        "source": "generator.status",
        "operator": "==",
        "value": "stopped",
        "description": "发电机未运行"
      }
    ]
  },
  "actions": [
    {
      "action_type": "device_control",
      "config": {
        "device_id": "generator_001",
        "channel": "control",
        "point": "start",
        "value": true
      },
      "description": "启动发电机"
    },
    {
      "action_type": "notify",
      "config": {
        "level": "warning",
        "message": "电池电量低，已启动发电机",
        "recipients": ["operator@example.com"]
      }
    }
  ],
  "enabled": true,
  "priority": 1,
  "cooldown_seconds": 300
}
```

### 2. 电压监控规则

#### 三相电压异常检测
```json
{
  "id": "voltage_abnormal",
  "name": "电压异常检测",
  "description": "检测三相电压是否在正常范围",
  "conditions": {
    "operator": "OR",
    "conditions": [
      {
        "source": "comsrv:1001:T.1",
        "operator": "<",
        "value": 220.0,
        "description": "A相电压过低"
      },
      {
        "source": "comsrv:1001:T.1",
        "operator": ">",
        "value": 250.0,
        "description": "A相电压过高"
      }
    ]
  },
  "actions": [
    {
      "action_type": "publish",
      "config": {
        "channel": "ems:voltage:alert",
        "message": "VOLTAGE_ABNORMAL"
      }
    },
    {
      "action_type": "device_control",
      "config": {
        "device_id": "protection_001",
        "channel": "control",
        "point": "activate",
        "value": true
      },
      "description": "激活保护装置"
    }
  ],
  "enabled": true,
  "priority": 2,
  "cooldown_seconds": 60
}
```

### 3. 温度保护规则

#### 设备过热保护
```json
{
  "id": "temperature_protection",
  "name": "温度保护",
  "description": "设备温度过高时的保护措施",
  "conditions": {
    "operator": "OR",
    "conditions": [
      {
        "source": "transformer.temperature",
        "operator": ">",
        "value": 85.0,
        "description": "变压器过热"
      },
      {
        "source": "inverter.temperature",
        "operator": ">",
        "value": 70.0,
        "description": "逆变器过热"
      }
    ]
  },
  "actions": [
    {
      "action_type": "device_control",
      "config": {
        "device_id": "cooling_system",
        "channel": "control",
        "point": "max_cooling",
        "value": true
      },
      "description": "开启最大冷却"
    },
    {
      "action_type": "set_value",
      "config": {
        "key": "system.temperature_alert",
        "value": true,
        "ttl": 1800
      }
    },
    {
      "action_type": "notify",
      "config": {
        "level": "critical",
        "message": "设备温度超限，已启动紧急冷却",
        "recipients": ["safety@example.com", "operator@example.com"]
      }
    }
  ],
  "enabled": true,
  "priority": 0,
  "cooldown_seconds": 180
}
```

### 4. 负载管理规则

#### 峰值负载削减
```json
{
  "id": "peak_shaving",
  "name": "峰值削减",
  "description": "在用电高峰期自动削减非关键负载",
  "conditions": {
    "operator": "AND",
    "conditions": [
      {
        "source": "system.load_rate",
        "operator": ">",
        "value": 85.0,
        "description": "系统负载超过85%"
      },
      {
        "source": "time.is_peak_hour",
        "operator": "==",
        "value": true,
        "description": "处于用电高峰期"
      }
    ]
  },
  "actions": [
    {
      "action_type": "device_control",
      "config": {
        "device_id": "load_controller",
        "channel": "control",
        "point": "shed_level_1",
        "value": true
      },
      "description": "切除一级非关键负载"
    },
    {
      "action_type": "publish",
      "config": {
        "channel": "ems:load:management",
        "message": "PEAK_SHAVING_ACTIVE"
      }
    }
  ],
  "enabled": true,
  "priority": 3,
  "cooldown_seconds": 600
}
```

### 5. 故障响应规则

#### 通信中断处理
```json
{
  "id": "comm_failure_response",
  "name": "通信故障响应",
  "description": "设备通信中断时的应急处理",
  "conditions": {
    "operator": "AND",
    "conditions": [
      {
        "source": "device.comm_status",
        "operator": "==",
        "value": "offline",
        "description": "设备离线"
      },
      {
        "source": "device.offline_duration",
        "operator": ">",
        "value": 300,
        "description": "离线超过5分钟"
      }
    ]
  },
  "actions": [
    {
      "action_type": "device_control",
      "config": {
        "device_id": "backup_controller",
        "channel": "control",
        "point": "activate",
        "value": true
      },
      "description": "激活备用控制器"
    },
    {
      "action_type": "notify",
      "config": {
        "level": "warning",
        "message": "主控制器通信中断，已切换到备用控制器",
        "recipients": ["network@example.com"]
      }
    },
    {
      "action_type": "set_value",
      "config": {
        "key": "system.backup_mode",
        "value": true,
        "ttl": null
      }
    }
  ],
  "enabled": true,
  "priority": 1,
  "cooldown_seconds": 3600
}
```

## 高级规则示例

### 复合条件规则
```json
{
  "id": "complex_energy_optimization",
  "name": "综合能源优化",
  "description": "基于多种条件的能源优化策略",
  "conditions": {
    "operator": "AND",
    "conditions": [
      {
        "source": "battery.soc",
        "operator": ">",
        "value": 80.0,
        "description": "电池充足"
      },
      {
        "source": "grid.price",
        "operator": ">",
        "value": 1.2,
        "description": "电价高峰"
      },
      {
        "source": "solar.power",
        "operator": "<",
        "value": 1000,
        "description": "光伏发电不足"
      }
    ]
  },
  "actions": [
    {
      "action_type": "device_control",
      "config": {
        "device_id": "grid_tie_inverter",
        "channel": "control",
        "point": "mode",
        "value": "off_grid"
      },
      "description": "切换到离网模式"
    },
    {
      "action_type": "device_control",
      "config": {
        "device_id": "battery_inverter",
        "channel": "control",
        "point": "discharge_power",
        "value": 5000
      },
      "description": "电池放电5kW"
    },
    {
      "action_type": "publish",
      "config": {
        "channel": "ems:optimization",
        "message": "PEAK_PRICE_AVOIDANCE"
      }
    }
  ],
  "enabled": true,
  "priority": 2,
  "cooldown_seconds": 900
}
```

## 使用建议

### 1. 优先级设置
- 0: 紧急/安全相关规则
- 1-3: 重要操作规则
- 4-6: 常规管理规则
- 7-9: 优化/信息规则

### 2. 冷却期设置
- 紧急规则: 60-180秒
- 控制规则: 300-600秒
- 通知规则: 600-3600秒
- 维护规则: 3600-86400秒

### 3. 数据源命名
- 使用点号分隔层级: `system.subsystem.value`
- Hash字段使用: `hash_key.field`
- 保持命名一致性

### 4. 动作设计
- 关键操作使用设备控制
- 状态记录使用set_value
- 事件通知使用publish
- 人工干预使用notify

## 测试规则

在生产环境部署前，建议：

1. 使用测试模式验证规则逻辑
2. 检查数据源是否可用
3. 验证动作的副作用
4. 测试冷却期设置
5. 模拟异常情况