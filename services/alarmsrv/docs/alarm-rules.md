# 告警规则配置指南

## 概述

alarmsrv 提供灵活的告警规则配置系统，支持阈值告警、模式匹配、复合条件等多种告警触发机制。本文档详细介绍如何配置和管理告警规则。

## 规则配置文件

### 配置文件位置

```
config/
├── default.yml          # 主配置文件
├── thresholds.yml       # 阈值规则
├── patterns.yml         # 模式匹配规则
└── escalations.yml      # 升级规则
```

### 主配置文件

```yaml
# config/default.yml
alarm:
  rules:
    enabled: true
    config_path: "./config"
    reload_interval: 300  # 秒
  
  defaults:
    debounce_seconds: 60  # 防抖时间
    cooldown_minutes: 10  # 冷却时间
    max_alarms_per_point: 5  # 每个点位最大告警数
```

## 阈值规则

### 基本阈值配置

```yaml
# config/thresholds.yml
thresholds:
  # 温度监控
  - id: "temp_high_warning"
    name: "高温预警"
    description: "温度超过预警阈值"
    point_pattern: "comsrv:*:m"
    point_filter:
      field_name: "temperature"
    conditions:
      - operator: ">"
        value: 35.0
        level: "Warning"
        
  - id: "temp_critical"
    name: "高温严重告警"
    description: "温度达到危险水平"
    point_pattern: "comsrv:*:m"
    point_filter:
      field_name: "temperature"
    conditions:
      - operator: ">"
        value: 45.0
        level: "Critical"
        
  # 电压监控
  - id: "voltage_low"
    name: "电压过低"
    description: "电压低于正常范围"
    point_pattern: "comsrv:*:m"
    point_filter:
      field_name: "voltage"
    conditions:
      - operator: "<"
        value: 180.0
        level: "Major"
        
  - id: "voltage_high"
    name: "电压过高"
    description: "电压高于正常范围"
    point_pattern: "comsrv:*:m"
    point_filter:
      field_name: "voltage"
    conditions:
      - operator: ">"
        value: 250.0
        level: "Major"
```

### 复合条件规则

```yaml
# 复合条件示例
- id: "power_anomaly"
  name: "功率异常"
  description: "功率因数异常且功率过高"
  point_pattern: "modsrv:*:measurement"
  composite_conditions:
    logic: "AND"  # AND 或 OR
    conditions:
      - field: "power_factor"
        operator: "<"
        value: 0.85
      - field: "total_power"
        operator: ">"
        value: 1000.0
  alarm_level: "Major"
```

### 动态阈值

```yaml
# 基于历史数据的动态阈值
- id: "dynamic_current"
  name: "电流异常波动"
  description: "电流偏离历史平均值"
  point_pattern: "comsrv:*:m"
  point_filter:
    field_name: "current"
  dynamic_threshold:
    type: "deviation"  # deviation, percentage, stddev
    reference: "historical_avg"
    window: "1h"
    deviation: 20.0  # 偏离20%触发
    level: "Warning"
```

## 模式匹配规则

### 状态变化检测

```yaml
# config/patterns.yml
patterns:
  # 设备离线检测
  - id: "device_offline"
    name: "设备离线"
    description: "检测设备通信中断"
    pattern_type: "state_change"
    monitor_pattern: "comsrv:*:s"
    conditions:
      from_value: 1  # 在线
      to_value: 0    # 离线
      duration: 30    # 持续30秒
    alarm_level: "Major"
    
  # 频繁状态切换
  - id: "frequent_switching"
    name: "频繁切换"
    description: "开关状态频繁变化"
    pattern_type: "frequency"
    monitor_pattern: "comsrv:*:s"
    conditions:
      changes_count: 10
      time_window: "5m"
    alarm_level: "Warning"
```

### 趋势检测

```yaml
# 趋势分析规则
- id: "temperature_rising"
  name: "温度持续上升"
  description: "检测温度上升趋势"
  pattern_type: "trend"
  monitor_pattern: "comsrv:*:m"
  point_filter:
    field_name: "temperature"
  trend_config:
    direction: "increasing"
    min_duration: "10m"
    min_change: 5.0
  alarm_level: "Warning"
```

## 告警分类规则

### 自动分类配置

```yaml
# 分类规则配置
classification_rules:
  Environmental:
    keywords:
      - "温度"
      - "temperature"
      - "湿度"
      - "humidity"
      - "环境"
    patterns:
      - "temp"
      - "°C"
      - "°F"
      - "%RH"
    priority: 1.0
    
  Power:
    keywords:
      - "电压"
      - "voltage"
      - "电流"
      - "current"
      - "功率"
      - "power"
    patterns:
      - "V"
      - "A"
      - "kW"
      - "kVA"
    priority: 1.0
    
  Communication:
    keywords:
      - "通信"
      - "连接"
      - "离线"
      - "超时"
    patterns:
      - "timeout"
      - "offline"
      - "disconnect"
    priority: 0.9
```

## 告警升级规则

### 基本升级规则

```yaml
# config/escalations.yml
escalation_rules:
  # 预警升级到次要
  - id: "warning_to_minor"
    name: "预警自动升级"
    from_level: "Warning"
    to_level: "Minor"
    conditions:
      - type: "time_based"
        after_minutes: 60
      - type: "status"
        status: "not_acknowledged"
        
  # 次要升级到主要
  - id: "minor_to_major"
    name: "次要告警升级"
    from_level: "Minor"
    to_level: "Major"
    conditions:
      - type: "time_based"
        after_minutes: 30
      - type: "status"
        status: "not_acknowledged"
        
  # 主要升级到严重
  - id: "major_to_critical"
    name: "主要告警升级"
    from_level: "Major"
    to_level: "Critical"
    conditions:
      - type: "time_based"
        after_minutes: 15
      - type: "status"
        status: "not_resolved"
```

### 条件升级

```yaml
# 基于其他告警的升级
- id: "cascade_escalation"
  name: "级联升级"
  from_level: "Minor"
  to_level: "Major"
  conditions:
    - type: "related_alarms"
      count: 3
      category: "same"
      time_window: "10m"
```

## 告警抑制规则

### 防抖配置

```yaml
debounce_rules:
  # 通用防抖
  - id: "general_debounce"
    name: "通用防抖"
    point_pattern: "*"
    debounce_seconds: 60
    
  # 特定点位防抖
  - id: "critical_point_debounce"
    name: "关键点位防抖"
    point_pattern: "comsrv:1001:m:10001"
    debounce_seconds: 180
```

### 告警抑制

```yaml
suppression_rules:
  # 维护期间抑制
  - id: "maintenance_suppression"
    name: "维护期抑制"
    enabled: false  # 需要时启用
    suppress_pattern: "comsrv:2001:*"
    start_time: "2025-07-23T22:00:00Z"
    end_time: "2025-07-24T06:00:00Z"
    
  # 关联抑制
  - id: "parent_child_suppression"
    name: "父子告警抑制"
    parent_pattern: "comsrv:1001:s:20001"  # 主开关
    child_patterns:
      - "comsrv:1001:m:*"  # 相关测量点
    condition: "parent_alarm_active"
```

## 规则优先级

### 优先级配置

```yaml
rule_priorities:
  # 规则执行优先级（数字越小优先级越高）
  - rule_id: "temp_critical"
    priority: 1
    
  - rule_id: "voltage_low"
    priority: 2
    
  - rule_id: "device_offline"
    priority: 3
    
  - rule_id: "temp_high_warning"
    priority: 10
```

## 动态规则管理

### API 配置

```bash
# 获取当前规则
GET /config/rules

# 添加新规则
POST /config/rules
Content-Type: application/json
{
  "id": "new_rule",
  "type": "threshold",
  "config": { ... }
}

# 更新规则
PUT /config/rules/{rule_id}

# 删除规则
DELETE /config/rules/{rule_id}

# 启用/禁用规则
PATCH /config/rules/{rule_id}/enable
PATCH /config/rules/{rule_id}/disable
```

### 规则验证

```rust
// 规则验证示例
pub fn validate_rule(rule: &AlarmRule) -> Result<()> {
    // 检查必填字段
    if rule.id.is_empty() {
        return Err(Error::InvalidRule("Rule ID is required"));
    }
    
    // 验证阈值
    if let Some(threshold) = &rule.threshold {
        if threshold.value.is_nan() || threshold.value.is_infinite() {
            return Err(Error::InvalidRule("Invalid threshold value"));
        }
    }
    
    // 验证模式
    if let Some(pattern) = &rule.point_pattern {
        validate_redis_pattern(pattern)?;
    }
    
    Ok(())
}
```

## 最佳实践

### 1. 规则命名

- 使用描述性的 ID：`temp_high_critical` 而非 `rule1`
- 包含清晰的名称和描述
- 使用一致的命名约定

### 2. 阈值设置

- 设置合理的阈值范围
- 考虑使用多级阈值（预警→严重）
- 定期审查和调整阈值

### 3. 防抖和抑制

- 为易波动的数据设置防抖
- 在维护期间启用告警抑制
- 避免告警风暴

### 4. 性能考虑

- 限制模式匹配的范围
- 合理设置规则优先级
- 定期清理过期规则

## 示例场景

### 场景 1：温度监控

```yaml
# 完整的温度监控规则集
temperature_monitoring:
  rules:
    - id: "temp_normal_to_warning"
      threshold: 35.0
      level: "Warning"
      debounce: 60
      
    - id: "temp_warning_to_major"
      threshold: 40.0
      level: "Major"
      debounce: 30
      
    - id: "temp_emergency"
      threshold: 45.0
      level: "Critical"
      debounce: 10
      auto_escalate: false  # 不再升级
      
  escalations:
    - from: "Warning"
      to: "Major"
      after_minutes: 30
      
    - from: "Major"
      to: "Critical"
      after_minutes: 15
```

### 场景 2：设备状态监控

```yaml
# 设备在线状态监控
device_monitoring:
  rules:
    - id: "device_offline_detection"
      pattern_type: "state_change"
      from: 1
      to: 0
      duration: 30
      level: "Major"
      
    - id: "device_flapping"
      pattern_type: "frequency"
      changes: 5
      window: "5m"
      level: "Warning"
      
  suppressions:
    - parent: "main_power_status"
      children: ["device_*_status"]
      reason: "主电源故障时抑制设备告警"
```