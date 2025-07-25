# ModSrv v2.0 数据结构文档

## 概述

本文档详细描述了ModSrv v2.0中使用的核心数据结构，包括模型定义、映射配置、Redis数据格式和API接口数据结构。

## 核心数据结构

### 1. 模型数据结构

#### Model (模型)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Model {
    /// 模型唯一标识符
    pub id: String,
    /// 模型显示名称
    pub name: String,
    /// 模型描述
    pub description: String,
    /// 监视点配置 (点位名称 -> 配置)
    pub monitoring_config: HashMap<String, PointConfig>,
    /// 控制点配置 (点位名称 -> 配置)
    pub control_config: HashMap<String, PointConfig>,
}
```

#### PointConfig (点位配置)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointConfig {
    /// 点位描述
    pub description: String,
    /// 单位 (可选)
    pub unit: Option<String>,
}
```

**示例**:
```json
{
  "id": "power_meter_demo",
  "name": "演示电表模型",
  "description": "用于演示的简单电表监控模型v2.0",
  "monitoring_config": {
    "voltage_a": {
      "description": "A相电压",
      "unit": "V"
    },
    "current_a": {
      "description": "A相电流",
      "unit": "A"
    }
  },
  "control_config": {
    "main_switch": {
      "description": "主开关",
      "unit": null
    }
  }
}
```

### 2. 映射数据结构

#### PointMapping (点位映射)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PointMapping {
    /// 通道ID (对应ComsRv通道)
    pub channel: u16,
    /// 点位ID
    pub point: u32,
    /// 点位类型: "m"(测量), "s"(信号), "c"(控制), "a"(调节)
    #[serde(rename = "type")]
    pub point_type: String,
}
```

#### ModelMappingConfig (模型映射配置)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMappingConfig {
    /// 监视点映射 (逻辑名称 -> 物理地址)
    pub monitoring: HashMap<String, PointMapping>,
    /// 控制点映射 (逻辑名称 -> 物理地址)
    pub control: HashMap<String, PointMapping>,
}
```

**映射示例**:
```json
{
  "monitoring": {
    "voltage_a": {
      "channel": 1001,
      "point": 10001,
      "type": "m"
    }
  },
  "control": {
    "main_switch": {
      "channel": 1001,
      "point": 20001,
      "type": "c"
    }
  }
}
```

### 3. 运行时数据结构

#### PointValue (点位值)

```rust
#[derive(Debug, Clone)]
pub struct PointValue {
    /// 数值 (使用6位小数精度)
    value: StandardFloat,
    /// 时间戳 (可选)
    timestamp: Option<i64>,
}
```

#### StandardFloat (标准化浮点数)

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StandardFloat(f64);

impl StandardFloat {
    /// 创建标准化浮点数 (6位小数精度)
    pub fn new(value: f64) -> Self {
        Self((value * 1_000_000.0).round() / 1_000_000.0)
    }
    
    /// 获取数值
    pub fn value(&self) -> f64 {
        self.0
    }
    
    /// 格式化为字符串 (6位小数)
    pub fn to_string(&self) -> String {
        format!("{:.6}", self.0)
    }
}
```

## Redis数据格式规范

### 1. 数据存储格式 (Hash)

ModSrv遵循VoltageEMS Redis v3.2数据结构规范：

```
键格式: comsrv:{channelID}:{type}
值格式: Hash {pointID: value}
```

**类型映射**:
- `m`: 测量数据 (Measurement)
- `s`: 信号数据 (Signal) 
- `c`: 控制数据 (Control)
- `a`: 调节数据 (Adjustment)

**示例**:
```redis
# 通道1001的测量数据
HGETALL comsrv:1001:m
1) "10001"      # 点位ID
2) "220.123456" # 电压值 (6位小数)
3) "10002"
4) "221.567890"
5) "10003"
6) "219.876543"

# 通道1001的控制数据
HGETALL comsrv:1001:c
1) "20001"      # 控制点位ID
2) "0.000000"   # 开关状态
3) "20002"
4) "100.000000" # 功率限制值
```

### 2. 发布订阅格式 (Pub/Sub)

#### 数据更新通知

```
通道: comsrv:{channelID}:{type}
消息: {pointID}:{value:.6f}
```

**示例**:
```redis
# 订阅通道1001的测量数据更新
SUBSCRIBE comsrv:1001:m

# 接收消息
Message: 10001:220.123456  # 点位10001更新为220.123456
Message: 10002:221.567890  # 点位10002更新为221.567890
```

#### 控制命令发布

```
通道: cmd:{channelID}:control  (控制命令)
通道: cmd:{channelID}:adjust   (调节命令)
消息: {pointID}:{value:.6f}
```

**示例**:
```redis
# 发布控制命令到通道1001
PUBLISH cmd:1001:control "20001:1.000000"  # 开启开关
PUBLISH cmd:1001:adjust "20002:150.000000" # 设置功率限制
```

### 3. 配置数据格式

```
键格式: cfg:{channelID}:{type}:{pointID}
值格式: JSON配置数据
```

## API数据结构

### 1. REST API响应结构

#### 健康检查响应

```rust
#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,    // "ok" | "error"
    pub version: String,   // "2.0.0"
    pub service: String,   // "modsrv"
}
```

**示例**:
```json
{
  "status": "ok",
  "version": "2.0.0",
  "service": "modsrv"
}
```

#### 模型列表响应

```rust
#[derive(Serialize)]
pub struct ModelListResponse {
    pub models: Vec<ModelSummary>,
    pub total: usize,
}

#[derive(Serialize)]
pub struct ModelSummary {
    pub id: String,
    pub name: String,
    pub description: String,
    pub monitoring_count: usize,  // 监视点数量
    pub control_count: usize,     // 控制点数量
}
```

**示例**:
```json
{
  "models": [
    {
      "id": "power_meter_demo",
      "name": "演示电表模型",
      "description": "用于演示的简单电表监控模型v2.0",
      "monitoring_count": 8,
      "control_count": 2
    }
  ],
  "total": 1
}
```

#### 模型详情响应

```rust
#[derive(Serialize)]
pub struct ModelFullResponse {
    pub id: String,
    pub name: String,
    pub description: String,
    pub monitoring: HashMap<String, PointWithValue>,
    pub control: HashMap<String, PointConfig>,
}

#[derive(Serialize)]
pub struct PointWithValue {
    pub config: PointConfig,
    pub value: Option<f64>,     // 当前值
    pub timestamp: Option<i64>, // 时间戳
}
```

**示例**:
```json
{
  "id": "power_meter_demo",
  "name": "演示电表模型",
  "description": "用于演示的简单电表监控模型v2.0",
  "monitoring": {
    "voltage_a": {
      "config": {
        "description": "A相电压",
        "unit": "V"
      },
      "value": 220.123456,
      "timestamp": 1672531200
    }
  },
  "control": {
    "main_switch": {
      "description": "主开关",
      "unit": null
    }
  }
}
```

#### 控制命令请求/响应

```rust
#[derive(Deserialize)]
pub struct ControlRequest {
    pub value: f64,
}

#[derive(Serialize)]
pub struct ControlResponse {
    pub success: bool,
    pub message: String,
    pub timestamp: i64,
}
```

**请求示例**:
```json
{
  "value": 1.0
}
```

**响应示例**:
```json
{
  "success": true,
  "message": "控制命令执行成功: power_meter_demo:main_switch = 1.0",
  "timestamp": 1672531200
}
```

### 2. WebSocket消息格式

#### 订阅消息

```json
{
  "type": "subscribe",
  "model_id": "power_meter_demo"
}
```

#### 数据推送消息

```json
{
  "type": "data_update",
  "model_id": "power_meter_demo",
  "timestamp": 1672531200,
  "data": {
    "voltage_a": 220.123456,
    "voltage_b": 221.567890,
    "current_a": 45.678901
  }
}
```

#### 错误消息

```json
{
  "type": "error",
  "code": "INVALID_MODEL",
  "message": "模型不存在: invalid_model_id"
}
```

## 配置文件数据结构

### 1. 主配置文件 (`config.yml`)

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub service_name: String,
    pub version: String,
    pub redis: RedisConfig,
    pub log: LogConfig,
    pub api: ApiConfig,
    pub models: Vec<ModelConfig>,
    pub update_interval_ms: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RedisConfig {
    pub url: String,
    pub key_prefix: String,
    pub connection_timeout_ms: u64,
    pub retry_attempts: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ApiConfig {
    pub host: String,
    pub port: u16,
    pub timeout_seconds: u64,
}
```

### 2. 模型配置来源

#### v2.0统一配置方式

ModSrv v2.0中，所有模型定义都在主配置文件中，不使用独立的JSON模型文件：

```yaml
# 主配置文件: test-configs/config.yml
models:
  - id: "power_meter_demo"
    name: "演示电表模型"
    description: "用于演示的简单电表监控模型v2.0"
    monitoring:
      voltage_a:
        description: "A相电压"
        unit: "V"
    control:
      main_switch:
        description: "主开关"
```

#### 设备模板文件

```yaml
# 模板文件: templates/devices/power_meter_template.yml
id: "${device_id}"
name: "${device_name}"
description: "${device_description}"
enabled: true

monitoring:
  voltage_a:
    description: "A相电压"
    unit: "V"

control:
  main_switch:
    description: "主开关"

# 变量定义
template_variables:
  device_id: "设备唯一标识符"
  device_name: "设备显示名称"
  device_description: "设备详细描述"
```

## 数据类型约定

### 1. 标识符规范

- **模型ID**: 字母数字下划线，如 `power_meter_demo`
- **点位名称**: 字母数字下划线，如 `voltage_a`
- **通道ID**: 16位无符号整数 (1-65535)
- **点位ID**: 32位无符号整数 (1-4294967295)

### 2. 数值精度规范

- **浮点数值**: 统一使用6位小数精度
- **时间戳**: Unix时间戳 (秒)
- **单位**: 国际单位制 (SI) 或常用工业单位

### 3. 字符编码

- **配置文件**: UTF-8编码
- **API响应**: UTF-8 JSON
- **Redis存储**: UTF-8字符串

## 数据验证规则

### 1. 配置验证

- 模型ID唯一性检查
- 必需字段完整性验证
- 数据类型正确性验证
- 引用完整性检查

### 2. 映射验证

- 映射文件与模型对应性
- 通道和点位ID有效性
- 点位类型正确性
- 无重复映射检查

### 3. 运行时验证

- 输入参数范围检查
- 控制值有效性验证
- 权限访问控制
- 并发操作安全性

## 性能考虑

### 1. 内存优化

- 使用`Arc<T>`共享不可变数据
- 使用`HashMap`提供O(1)查找
- 惰性加载减少内存占用
- 及时释放不用的数据

### 2. 序列化优化

- 使用`serde`高效序列化
- 避免不必要的数据拷贝
- 批量操作减少系统调用
- 压缩传输减少网络开销

### 3. 缓存策略

- 热点数据内存缓存
- 配置数据启动时加载
- 映射关系预计算
- TTL自动过期管理

## 兼容性考虑

### 1. 向后兼容

- 配置文件版本标记
- API版本控制
- 数据格式平滑升级
- 迁移工具支持

### 2. 跨版本兼容

- 可选字段向后兼容
- 默认值合理设置
- 废弃功能优雅降级
- 清晰的变更日志