# modsrv - 设备模型计算服务

## 概述

modsrv 是 VoltageEMS 的核心计算引擎，负责执行基于 DAG（有向无环图）的实时计算模型和设备物模型管理。它从 Redis 读取 comsrv 采集的实时数据，执行各种计算任务，并将结果存储回 Redis 供其他服务使用。

## 主要特性

- **DAG 计算引擎**: 支持复杂的数据流计算和依赖管理
- **物模型系统**: 完整的设备建模、实例管理和计算框架
- **实时数据处理**: 订阅 Redis 数据变化，自动触发计算
- **高性能存储**: 使用 Redis Hash 结构，计算结果不含时间戳
- **内置函数库**: sum、avg、min、max、scale 等常用计算函数
- **性能基准测试**: 包含完整的 benchmark 套件

## 快速开始

### 运行服务

```bash
cd services/modsrv
cargo run
```

### 运行性能测试

```bash
# 完整基准测试
cargo bench

# 快速模式
cargo bench -- --quick
```

### 配置文件

主配置文件位于 `config/default.yml`：

```yaml
service:
  name: "modsrv"
  redis:
    url: "redis://localhost:6379"
  logging:
    level: "info"
    file: "logs/modsrv.log"

models:
  config_path: "./config/models"
  update_interval: 1000  # 毫秒
```

## Redis 数据结构

### Hash 存储格式

```
键: modsrv:{modelname}:{type}
字段: {field_name}
值: "{value:.6f}"

示例:
modsrv:power_meter:measurement → {
    "total_power": "1200.500000",
    "power_factor": "0.950000",
    "efficiency": "0.890000"
}

modsrv:power_meter:control → {
    "enable": "1.000000",
    "setpoint": "1000.000000"
}
```

### 数据格式特点

- **无时间戳**: modsrv 存储的计算结果仅包含值，不包含时间戳
- **标准精度**: 所有浮点数值使用 6 位小数精度
- **按模型组织**: 使用模型名称作为命名空间

## 物模型系统

### 核心组件

1. **DeviceModel**: 设备模型定义
   - 属性（Properties）
   - 遥测（Telemetry）
   - 命令（Commands）
   - 事件（Events）
   - 计算（Calculations）

2. **InstanceManager**: 实例管理
   - 创建、更新、删除实例
   - 实例状态管理
   - 数据持久化

3. **CalculationEngine**: 计算引擎
   - 内置函数支持
   - 自定义计算逻辑
   - 依赖图解析

4. **DataFlowProcessor**: 数据流处理
   - Redis 订阅管理
   - 自动计算触发
   - 结果发布

### 使用示例

```rust
// 创建设备实例
let instance_id = device_system.create_instance(
    "power_meter_v1",
    "meter_001".to_string(),
    "主电表".to_string(),
    initial_properties,
).await?;

// 获取遥测数据
let voltage = device_system.get_telemetry(&instance_id, "voltage_a").await?;

// 执行命令
device_system.execute_command(&instance_id, "reset_counter", params).await?;
```

## DAG 计算模型

### 模型定义

使用 YAML 定义计算模型：

```yaml
id: "power_calculation"
name: "功率计算模型"
inputs:
  - name: "voltage"
    source: "comsrv:1001:m:10001"
  - name: "current"
    source: "comsrv:1001:m:10002"

calculations:
  - id: "apparent_power"
    function: "multiply"
    inputs: ["voltage", "current"]
  
  - id: "real_power"
    function: "scale"
    inputs: ["apparent_power"]
    params:
      factor: 0.95

outputs:
  - name: "total_power"
    target: "modsrv:power_meter:measurement"
    field: "total_power"
    source: "real_power"
```

### 内置函数

- `sum(inputs)` - 求和
- `avg(inputs)` - 平均值
- `min(inputs)` - 最小值
- `max(inputs)` - 最大值
- `scale(input, factor)` - 缩放
- `multiply(a, b)` - 乘法
- `divide(a, b)` - 除法

## 开发指南

### 添加新的计算函数

```rust
// 在 calculation_engine.rs 中注册新函数
pub fn register_custom_functions(engine: &mut CalculationEngine) {
    engine.register_function("custom_calc", |inputs, params| {
        // 实现计算逻辑
        let result = inputs[0] * params.get("factor").unwrap_or(&1.0);
        Ok(StandardFloat::new(result))
    });
}
```

### 订阅数据更新

```rust
// 订阅 comsrv 数据变化
let subscription = DataSubscription {
    patterns: vec![
        "comsrv:1001:m:*".to_string(),
        "comsrv:1002:s:*".to_string(),
    ],
    handler: Box::new(move |update| {
        // 处理数据更新
        trigger_calculation(update).await
    }),
};
```

## 性能优化

- **批量计算**: 收集多个输入变化后批量执行
- **缓存机制**: 缓存中间计算结果
- **并行处理**: 无依赖的计算并行执行
- **内存池**: 复用计算缓冲区

## 监控指标

通过 `/metrics` 端点暴露 Prometheus 指标：

- `modsrv_calculations_total` - 计算执行总数
- `modsrv_calculation_duration_seconds` - 计算耗时
- `modsrv_model_instances_active` - 活跃模型实例数
- `modsrv_redis_operations_total` - Redis 操作计数

## 环境变量

- `RUST_LOG` - 日志级别
- `REDIS_URL` - Redis 连接地址
- `MODSRV_PORT` - API 服务端口（默认 8082）

## 相关文档

- [架构设计](docs/architecture.md)
- [设备模型系统](docs/device-model.md)
- [Redis 接口](docs/redis-interface.md)