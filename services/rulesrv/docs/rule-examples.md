# Rulesrv 规则示例

本文档提供各种场景下的规则配置示例。

## 目录

1. [温度监控规则](#温度监控规则)
2. [电力监控规则](#电力监控规则)
3. [设备状态规则](#设备状态规则)
4. [组合条件规则](#组合条件规则)
5. [时间相关规则](#时间相关规则)
6. [分级告警规则](#分级告警规则)
7. [控制联动规则](#控制联动规则)
8. [统计分析规则](#统计分析规则)

## 温度监控规则

### 1. 简单温度阈值告警

```json
{
  "id": "temp_high_warning",
  "name": "高温预警",
  "description": "温度超过80°C时触发预警",
  "enabled": true,
  "condition": "temperature > 80",
  "actions": [
    {
      "type": "publish",
      "channel": "alarm:temperature:warning",
      "message": "温度预警：当前温度超过80°C"
    }
  ],
  "priority": 20
}
```

### 2. 分级温度告警

```json
{
  "id": "temp_critical",
  "name": "高温严重告警",
  "description": "温度超过95°C时触发严重告警",
  "enabled": true,
  "condition": "temperature > 95",
  "actions": [
    {
      "type": "publish",
      "channel": "alarm:temperature:critical",
      "message": "严重告警：温度超过95°C，请立即处理！"
    },
    {
      "type": "control",
      "channel_id": 1001,
      "point_type": "control",
      "point_id": 10001,
      "value": 0,
      "description": "关闭加热器"
    }
  ],
  "priority": 100
}
```

### 3. 低温告警

```json
{
  "id": "temp_low_warning",
  "name": "低温告警",
  "description": "温度低于10°C时触发告警",
  "enabled": true,
  "condition": "temperature < 10",
  "actions": [
    {
      "type": "publish",
      "channel": "alarm:temperature:low",
      "message": "低温告警：当前温度低于10°C"
    }
  ],
  "priority": 15
}
```

## 电力监控规则

### 1. 电压异常告警

```json
{
  "id": "voltage_abnormal",
  "name": "电压异常告警",
  "description": "电压偏离正常范围时告警",
  "enabled": true,
  "condition": "voltage_a > 250",
  "actions": [
    {
      "type": "publish",
      "channel": "alarm:power:voltage_high",
      "message": "A相电压过高，当前值超过250V"
    }
  ],
  "priority": 30
}
```

### 2. 电流过载告警

```json
{
  "id": "current_overload",
  "name": "电流过载告警",
  "description": "电流超过额定值时告警",
  "enabled": true,
  "condition": "current_total > 100",
  "actions": [
    {
      "type": "publish",
      "channel": "alarm:power:overload",
      "message": "电流过载：总电流超过100A"
    },
    {
      "type": "notification",
      "method": "webhook",
      "url": "https://api.company.com/alerts/power",
      "data": {
        "type": "current_overload",
        "severity": "high"
      }
    }
  ],
  "priority": 50
}
```

### 3. 功率因数告警

```json
{
  "id": "power_factor_low",
  "name": "功率因数过低",
  "description": "功率因数低于0.9时告警",
  "enabled": true,
  "condition": "power_factor < 0.9",
  "actions": [
    {
      "type": "publish",
      "channel": "alarm:power:pf_low",
      "message": "功率因数过低，请检查无功补偿设备"
    }
  ],
  "priority": 25
}
```

## 设备状态规则

### 1. 设备停机告警

```json
{
  "id": "device_stopped",
  "name": "设备停机告警",
  "description": "设备运行状态变为停止时告警",
  "enabled": true,
  "condition": "running == 0",
  "actions": [
    {
      "type": "publish",
      "channel": "alarm:device:stopped",
      "message": "设备已停机"
    }
  ],
  "priority": 40
}
```

### 2. 设备故障告警

```json
{
  "id": "device_fault",
  "name": "设备故障告警",
  "description": "设备出现故障代码时告警",
  "enabled": true,
  "condition": "fault_code != 0",
  "actions": [
    {
      "type": "publish",
      "channel": "alarm:device:fault",
      "message": "设备故障，请检查故障代码"
    },
    {
      "type": "control",
      "channel_id": 1002,
      "point_type": "control",
      "point_id": 20001,
      "value": 0,
      "description": "停止设备运行"
    }
  ],
  "priority": 90
}
```

### 3. 设备效率低下

```json
{
  "id": "device_low_efficiency",
  "name": "设备效率低下",
  "description": "设备运行效率低于70%时告警",
  "enabled": true,
  "condition": "efficiency < 70",
  "actions": [
    {
      "type": "publish",
      "channel": "alarm:device:efficiency",
      "message": "设备运行效率低于70%，请进行维护"
    }
  ],
  "priority": 20
}
```

## 组合条件规则

### 1. 温度和压力组合告警

```json
{
  "id": "temp_pressure_alarm",
  "name": "温度压力组合告警",
  "description": "温度高且压力高时触发",
  "enabled": true,
  "condition": "temperature > 80 && pressure > 5",
  "actions": [
    {
      "type": "publish",
      "channel": "alarm:system:critical",
      "message": "危险：温度和压力同时超标"
    },
    {
      "type": "control",
      "channel_id": 1001,
      "point_type": "control", 
      "point_id": 10002,
      "value": 1,
      "description": "打开安全阀"
    }
  ],
  "priority": 95
}
```

### 2. 多参数异常检测

```json
{
  "id": "multi_param_abnormal",
  "name": "多参数异常",
  "description": "多个参数同时异常",
  "enabled": true,
  "condition": "temperature > 70 || vibration > 10 || noise_level > 85",
  "actions": [
    {
      "type": "publish",
      "channel": "alarm:system:abnormal",
      "message": "系统异常：多个参数超出正常范围"
    }
  ],
  "priority": 60
}
```

## 时间相关规则

### 1. 运行时间超限

```json
{
  "id": "runtime_exceed",
  "name": "运行时间超限",
  "description": "连续运行时间超过8小时",
  "enabled": true,
  "condition": "runtime_hours > 8",
  "actions": [
    {
      "type": "publish",
      "channel": "alarm:maintenance:runtime",
      "message": "设备连续运行超过8小时，建议停机检查"
    }
  ],
  "priority": 30
}
```

### 2. 维护周期提醒

```json
{
  "id": "maintenance_reminder",
  "name": "维护提醒",
  "description": "距离上次维护超过30天",
  "enabled": true,
  "condition": "days_since_maintenance > 30",
  "actions": [
    {
      "type": "publish",
      "channel": "notification:maintenance:due",
      "message": "设备维护提醒：已超过30天未维护"
    }
  ],
  "priority": 10
}
```

## 分级告警规则

### 1. 三级温度告警体系

```json
[
  {
    "id": "temp_level_1",
    "name": "温度一级告警",
    "condition": "temperature > 70",
    "actions": [{
      "type": "publish",
      "channel": "alarm:temp:info",
      "message": "提示：温度超过70°C"
    }],
    "priority": 10
  },
  {
    "id": "temp_level_2",
    "name": "温度二级告警",
    "condition": "temperature > 85",
    "actions": [{
      "type": "publish",
      "channel": "alarm:temp:warning",
      "message": "警告：温度超过85°C"
    }],
    "priority": 50
  },
  {
    "id": "temp_level_3",
    "name": "温度三级告警",
    "condition": "temperature > 95",
    "actions": [
      {
        "type": "publish",
        "channel": "alarm:temp:critical",
        "message": "严重：温度超过95°C，立即处理！"
      },
      {
        "type": "control",
        "channel_id": 1001,
        "point_type": "control",
        "point_id": 10001,
        "value": 0
      }
    ],
    "priority": 100
  }
]
```

## 控制联动规则

### 1. 温控联动

```json
{
  "id": "temp_control_linkage",
  "name": "温度控制联动",
  "description": "根据温度自动控制冷却系统",
  "enabled": true,
  "condition": "temperature > 75",
  "actions": [
    {
      "type": "control",
      "channel_id": 1001,
      "point_type": "control",
      "point_id": 30001,
      "value": 1,
      "description": "启动冷却风扇"
    },
    {
      "type": "publish",
      "channel": "log:control:cooling",
      "message": "温度超过75°C，已自动启动冷却系统"
    }
  ],
  "priority": 70
}
```

### 2. 压力保护联动

```json
{
  "id": "pressure_protection",
  "name": "压力保护联动",
  "description": "压力过高时自动释放",
  "enabled": true,
  "condition": "pressure > 8",
  "actions": [
    {
      "type": "control",
      "channel_id": 1002,
      "point_type": "control",
      "point_id": 40001,
      "value": 1,
      "description": "打开泄压阀"
    },
    {
      "type": "control",
      "channel_id": 1002,
      "point_type": "control",
      "point_id": 40002,
      "value": 0,
      "description": "停止加压泵"
    },
    {
      "type": "publish",
      "channel": "alarm:pressure:protection",
      "message": "压力保护动作：已打开泄压阀"
    }
  ],
  "priority": 95
}
```

## 统计分析规则

### 1. 能耗异常检测

```json
{
  "id": "energy_abnormal",
  "name": "能耗异常",
  "description": "当前能耗超过历史平均值20%",
  "enabled": true,
  "condition": "energy_consumption > energy_avg * 1.2",
  "actions": [
    {
      "type": "publish",
      "channel": "alarm:energy:abnormal",
      "message": "能耗异常：当前能耗超出平均值20%"
    }
  ],
  "priority": 35
}
```

### 2. 生产效率告警

```json
{
  "id": "production_efficiency",
  "name": "生产效率低下",
  "description": "生产效率低于目标值",
  "enabled": true,
  "condition": "production_rate < target_rate * 0.9",
  "actions": [
    {
      "type": "publish",
      "channel": "notification:production:efficiency",
      "message": "生产效率低于目标值90%"
    }
  ],
  "priority": 25
}
```

## 高级规则示例

### 1. 智能预测性维护

```json
{
  "id": "predictive_maintenance",
  "name": "预测性维护",
  "description": "基于振动和温度趋势预测维护需求",
  "enabled": true,
  "condition": "vibration_trend > 1.5 && temperature_trend > 1.2",
  "actions": [
    {
      "type": "publish",
      "channel": "maintenance:predictive:alert",
      "message": "预测性维护提醒：设备可能在未来7天内需要维护"
    },
    {
      "type": "notification",
      "method": "webhook",
      "url": "https://maintenance.company.com/api/schedule",
      "data": {
        "device_id": "{{device_id}}",
        "prediction_days": 7,
        "vibration_trend": "{{vibration_trend}}",
        "temperature_trend": "{{temperature_trend}}"
      }
    }
  ],
  "priority": 40
}
```

### 2. 智能负载均衡

```json
{
  "id": "load_balance",
  "name": "智能负载均衡",
  "description": "根据负载自动调整设备运行",
  "enabled": true,
  "condition": "load_percentage > 85",
  "actions": [
    {
      "type": "control",
      "channel_id": 2001,
      "point_type": "control",
      "point_id": 50001,
      "value": 1,
      "description": "启动备用设备"
    },
    {
      "type": "publish",
      "channel": "system:load:balanced",
      "message": "负载均衡：已启动备用设备分担负载"
    }
  ],
  "priority": 75
}
```

## 规则管理最佳实践

### 1. 规则命名规范
- 使用清晰的ID：`{系统}_{参数}_{级别}`
- 例如：`temp_high_warning`, `power_voltage_critical`

### 2. 优先级设置
- 0-20：信息提示
- 21-40：一般告警
- 41-60：重要告警
- 61-80：严重告警
- 81-100：紧急告警

### 3. 动作设计
- 低优先级：仅记录日志
- 中优先级：发送通知
- 高优先级：自动控制
- 紧急情况：多重动作

### 4. 测试建议
- 先在测试环境验证
- 使用模拟数据测试
- 逐步提升规则复杂度
- 记录规则触发历史