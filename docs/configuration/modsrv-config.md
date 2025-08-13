# ModSrv 配置指南

## 概述

ModSrv（模型服务）是 VoltageEMS 的数据建模服务，负责将 ComSrv 采集的原始数据映射到逻辑模型，提供面向对象的数据访问接口。支持模板化配置，便于批量创建相同类型的设备模型。

## 配置文件结构

```
config/
├── modsrv.yaml      # 服务主配置
└── models.yaml      # 模型定义文件
```

## 服务配置 (modsrv.yaml)

### 基本配置

```yaml
service:
  name: modsrv           # 服务名称
  port: 6001            # 服务端口

redis:
  url: redis://redis:6379    # Redis 连接URL

model_config:
  sync_interval_ms: 1000     # 数据同步间隔（毫秒）
  enable_caching: true       # 启用缓存
```

### 环境变量覆盖

支持通过环境变量覆盖配置：

```bash
# 覆盖服务端口
MODSRV_SERVICE_PORT=6001

# 覆盖 Redis 连接
MODSRV_REDIS_URL=redis://localhost:6379

# 覆盖同步间隔
MODSRV_MODEL_CONFIG_SYNC_INTERVAL_MS=500
```

## 模型定义 (models.yaml)

### 文件结构

```yaml
# 模型模板定义
templates:
  - id: "template_id"
    name: "模板名称"
    description: "模板描述"
    data_points:
      # 数据点定义
    actions:
      # 动作定义

# 模型实例
models:
  - id: "model_id"
    name: "模型名称"
    template: "template_id"
    mapping:
      # 映射配置

# 全局设置
settings:
  auto_reverse_mapping: true
  enable_value_notifications: true
  cache_ttl: 3600
```

### 模板定义

模板是可重用的模型定义，包含数据点和动作的结构：

```yaml
templates:
  - id: "transformer_standard"
    name: "标准变压器模板"
    description: "三相变压器的标准数据模型"
    data_points:
      oil_temp:                  # 数据点标识
        base_id: 1              # 基础点号
        unit: "℃"               # 单位
        description: "油温"      # 描述
      voltage_a:
        base_id: 2
        unit: "V"
        description: "A相电压"
      current_a:
        base_id: 3
        unit: "A"
        description: "A相电流"
      power_active:
        base_id: 8
        unit: "kW"
        description: "有功功率"
    actions:
      breaker_control:           # 动作标识
        base_id: 1
        description: "断路器控制"
```

### 模型实例

基于模板创建具体的设备模型实例：

```yaml
models:
  - id: "transformer_01"
    name: "1号主变"
    template: "transformer_standard"    # 引用模板
    description: "1号主变压器"
    mapping:
      channel: 1001                     # 对应 comsrv 的通道ID
      # 数据点映射 - 映射到 comsrv 的 point_id
      data:
        oil_temp: 1                     # 模板的 oil_temp 映射到通道的点 1
        voltage_a: 2                    # 模板的 voltage_a 映射到通道的点 2
        current_a: 3
        voltage_b: 4
        current_b: 5
        voltage_c: 6
        current_c: 7
        power_active: 8
        power_reactive: 9
        power_factor: 10
      # 动作映射
      action:
        breaker_control: 1              # 控制点映射
    metadata:                           # 扩展元数据
      location: "配电室A"
      capacity: "1000kVA"
      manufacturer: "ABB"
      install_date: "2020-01-01"
```

### 映射关系

#### 数据流向

```
ComSrv (Channel/Point) → ModSrv (Model/Property) → Application
```

1. ComSrv 从设备采集原始数据，存储在 `comsrv:{channel_id}:T`
2. ModSrv 读取原始数据，映射到模型属性
3. 应用通过模型 API 访问数据，无需了解底层细节

#### 映射类型

**一对一映射**：
```yaml
data:
  oil_temp: 1    # 模型属性 oil_temp 对应通道点 1
```

**计算映射**（计划支持）：
```yaml
data:
  total_power:
    expression: "power_a + power_b + power_c"
```

**聚合映射**（计划支持）：
```yaml
data:
  avg_current:
    aggregate: "avg"
    sources: [3, 5, 7]   # A、B、C 相电流平均值
```

## 高级配置

### 模型分组

支持对模型进行逻辑分组：

```yaml
models:
  - id: "transformer_01"
    name: "1号主变"
    template: "transformer_standard"
    group: "main_transformers"    # 所属分组
    tags: ["critical", "monitored"]   # 标签
```

### 继承和扩展

模型可以扩展模板定义：

```yaml
models:
  - id: "transformer_special"
    template: "transformer_standard"
    # 扩展额外的数据点
    extend_data:
      winding_temp:
        point_id: 31
        unit: "℃"
        description: "绕组温度"
```

### 告警配置

为模型配置告警规则：

```yaml
models:
  - id: "transformer_01"
    template: "transformer_standard"
    alarms:
      high_temp:
        property: "oil_temp"
        condition: "> 85"
        severity: "warning"
        message: "变压器油温过高"
      over_current:
        property: "current_a"
        condition: "> 1000"
        severity: "critical"
        message: "A相电流过载"
```

### 访问控制

配置模型的访问权限：

```yaml
models:
  - id: "transformer_01"
    template: "transformer_standard"
    access:
      read: ["operator", "viewer"]
      write: ["operator", "admin"]
      control: ["operator"]
```

## Redis 数据结构

ModSrv 在 Redis 中维护以下数据结构：

```
# 模型实例数据
modsrv:model:{model_id}:data     # Hash - 当前数据
modsrv:model:{model_id}:meta     # Hash - 元数据
modsrv:model:{model_id}:status   # String - 状态

# 模型索引
modsrv:models:index              # Set - 所有模型ID
modsrv:models:by_template        # Hash - 按模板分组
modsrv:models:by_group           # Hash - 按组分组
```

## API 访问

### 获取模型列表

```http
GET /api/models
```

响应：
```json
{
  "models": [
    {
      "id": "transformer_01",
      "name": "1号主变",
      "template": "transformer_standard",
      "status": "online"
    }
  ]
}
```

### 获取模型数据

```http
GET /api/models/{model_id}/data
```

响应：
```json
{
  "model_id": "transformer_01",
  "timestamp": "2025-08-13T10:00:00Z",
  "data": {
    "oil_temp": 75.5,
    "voltage_a": 10500,
    "current_a": 850.5,
    "power_active": 8500
  }
}
```

### 执行模型动作

```http
POST /api/models/{model_id}/actions/{action_id}
```

请求体：
```json
{
  "value": 1,
  "reason": "维护操作"
}
```

## 性能优化

### 缓存策略

```yaml
settings:
  cache_ttl: 3600              # 缓存生存时间（秒）
  cache_size: 1000             # 最大缓存条目数
  cache_strategy: "lru"        # 缓存淘汰策略
```

### 批量同步

```yaml
model_config:
  batch_sync: true             # 启用批量同步
  batch_size: 100              # 批量大小
  sync_interval_ms: 1000       # 同步间隔
```

## 故障排除

### 常见问题

1. **模型数据不更新**
   - 检查 channel 映射是否正确
   - 确认 ComSrv 正在写入数据
   - 查看 ModSrv 日志

2. **Redis 连接失败**
   - 验证 Redis URL 配置
   - 检查网络连接
   - 确认 Redis 服务状态

3. **模板未找到**
   - 确认模板 ID 正确
   - 检查 models.yaml 语法
   - 重启服务加载配置

### 调试模式

```bash
# 启用调试日志
RUST_LOG=debug,modsrv=trace ./modsrv

# 验证配置文件
./modsrv --validate
```

## 最佳实践

1. **使用模板**：相同类型设备使用统一模板，便于维护
2. **合理命名**：使用有意义的 ID 和名称
3. **适度缓存**：根据数据更新频率调整缓存时间
4. **监控性能**：定期检查同步延迟和内存使用
5. **版本管理**：对 models.yaml 进行版本控制

## 配置示例

### 最简配置

```yaml
# modsrv.yaml
service:
  port: 6001
redis:
  url: redis://localhost:6379

# models.yaml
models:
  - id: "device_01"
    name: "测试设备"
    mapping:
      channel: 1001
      data:
        value1: 1
        value2: 2
```

### 完整配置

参见 `config-examples/full/` 目录中的完整示例。

## 相关文档

- [ComSrv 配置指南](./comsrv-config.md)
- [API 文档](../api/rest-api.md)
- [数据模型](../api/data-models.md)