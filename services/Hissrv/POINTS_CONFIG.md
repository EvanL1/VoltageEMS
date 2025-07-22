# HisSrv 点位配置系统

## 概述

HisSrv 点位配置系统允许用户精细控制哪些点位数据会被保存到 InfluxDB。该系统支持多级配置规则和智能过滤器。

## 配置文件结构

点位配置文件默认位于 `config/points.yaml`，可以通过环境变量 `HISSRV_POINTS_CONFIG` 指定路径。

### 基本结构

```yaml
# 全局配置
enabled: true                    # 全局启用/禁用
default_policy: "allow_all"      # 默认策略

# 存储规则
rules:
  channels: []                   # 通道级别规则
  points: []                     # 点位级别规则

# 过滤器
filters: []                      # 智能过滤器
```

## 配置规则优先级

规则按以下优先级应用（从高到低）：

1. **点位级别规则** - 针对特定点位的规则
2. **通道级别规则** - 针对整个通道的规则
3. **默认策略** - 全局默认行为

## 详细配置说明

### 1. 全局配置

```yaml
enabled: true                    # 是否启用点位配置系统
default_policy: "allow_all"      # 默认策略: "allow_all" 或 "deny_all"
```

- `enabled`: 如果设置为 `false`，所有点位都不会被保存
- `default_policy`: 当没有匹配的规则时的默认行为

### 2. 通道级别规则

```yaml
rules:
  channels:
    - channel_id: 1001           # 通道ID
      enabled: true              # 是否启用该通道
      point_types: ["m", "s"]    # 允许的点位类型（可选）
      name: "主变电站"           # 通道名称（可选）
```

**点位类型说明**：
- `m` - 测量数据（Measurement/YC）
- `s` - 信号数据（Signal/YX）
- `c` - 控制数据（Control/YK）
- `a` - 调节数据（Adjustment/YT）

### 3. 点位级别规则

```yaml
rules:
  points:
    - channel_id: 1001           # 通道ID
      point_id: 10001            # 点位ID
      point_type: "m"            # 点位类型
      enabled: true              # 是否启用该点位
      name: "A相电压"           # 点位名称（可选）
```

### 4. 过滤器

#### 4.1 值范围过滤器

```yaml
filters:
  - type: "value_range"
    point_types: ["m"]           # 应用的点位类型
    min_value: -10000           # 最小值（可选）
    max_value: 10000            # 最大值（可选）
```

#### 4.2 时间间隔过滤器

```yaml
filters:
  - type: "time_interval"
    point_types: ["m", "s"]      # 应用的点位类型（可选）
    min_interval_seconds: 1      # 最小间隔秒数
```

#### 4.3 质量过滤器

```yaml
filters:
  - type: "quality"
    point_types: ["m"]           # 应用的点位类型（可选）
    min_quality: 192             # 最小质量值（可选）
```

## 配置示例

### 示例 1：基本配置

```yaml
enabled: true
default_policy: "allow_all"

rules:
  channels:
    - channel_id: 1001
      enabled: true
      point_types: ["m", "s"]
      name: "主变电站"
```

### 示例 2：复杂配置

```yaml
enabled: true
default_policy: "deny_all"

rules:
  channels:
    - channel_id: 1001
      enabled: true
      point_types: ["m", "s"]
      name: "主变电站"
    
    - channel_id: 1002
      enabled: false
      name: "测试通道"

  points:
    - channel_id: 1001
      point_id: 10001
      point_type: "m"
      enabled: true
      name: "A相电压"
    
    - channel_id: 1001
      point_id: 10002
      point_type: "m"
      enabled: false
      name: "故障点位"

filters:
  - type: "value_range"
    point_types: ["m"]
    min_value: -10000
    max_value: 10000
  
  - type: "time_interval"
    point_types: ["m", "s"]
    min_interval_seconds: 5
```

## API 接口

### 获取当前配置

```bash
curl -X GET http://localhost:8080/config/points
```

### 更新配置

```bash
curl -X PUT http://localhost:8080/config/points \
  -H "Content-Type: application/json" \
  -d @new_config.json
```

### 获取配置统计

```bash
curl -X GET http://localhost:8080/config/points/stats
```

### 重新加载配置

```bash
curl -X POST http://localhost:8080/config/points/reload
```

## 环境变量

- `HISSRV_POINTS_CONFIG`: 点位配置文件路径（默认：`config/points.yaml`）
- `HISSRV_CONFIG`: 主配置文件路径（默认：`config/hissrv.yaml`）

## 配置验证

系统会在加载配置时进行以下验证：

1. 通道ID和点位ID必须大于0
2. 点位类型必须是有效值（m、s、c、a）
3. 过滤器参数必须有效
4. 值范围过滤器的最小值必须小于最大值
5. 时间间隔必须大于0

## 性能考虑

- 点位规则检查是 O(1) 操作
- 过滤器按配置顺序应用
- 时间间隔过滤器使用内存状态跟踪
- 建议合理配置过滤器以平衡性能和功能

## 故障排除

### 常见问题

1. **配置文件加载失败**
   - 检查文件路径和权限
   - 验证 YAML 格式
   - 查看日志中的错误信息

2. **点位没有被保存**
   - 检查全局 `enabled` 设置
   - 验证通道和点位规则
   - 查看过滤器配置

3. **过滤器不工作**
   - 检查点位类型匹配
   - 验证过滤器参数
   - 查看处理统计信息

### 调试技巧

1. 使用 API 端点查看当前配置
2. 查看统计信息了解过滤效果
3. 调整日志级别获取更多信息
4. 使用简单配置逐步调试

## 最佳实践

1. **从简单配置开始**，逐步添加复杂规则
2. **合理使用默认策略**，减少配置复杂性
3. **定期检查统计信息**，优化过滤器配置
4. **使用描述性名称**，便于维护
5. **备份配置文件**，防止意外丢失
6. **分阶段部署**，验证配置效果