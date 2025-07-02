# 四遥（Four-Telemetry）设计说明

## 1. 四遥定义与区分

在当前的comsrv设计中，四遥通过`TelemetryType`枚举类型进行区分：

```rust
pub enum TelemetryType {
    /// 遥测 - Analog measurements (温度、压力、流量等)
    Telemetry,
    /// 遥信 - Digital status signals (开关状态、报警状态等)
    Signaling,
    /// 遥控 - Digital control commands (启动/停止、开/关等)
    Control,
    /// 遥调 - Analog regulation commands (设定值调整等)
    Setpoint,
}
```

## 2. 四遥特性对比

| 特性 | 遥测(Telemetry) | 遥信(Signaling) | 遥控(Control) | 遥调(Setpoint) |
|------|-----------------|-----------------|---------------|----------------|
| **数据类型** | 模拟量(float) | 数字量(bool) | 数字量(bool) | 模拟量(float) |
| **数据流向** | 设备→系统 | 设备→系统 | 系统→设备 | 系统→设备 |
| **可读性** | ✅ 可读 | ✅ 可读 | ❌ 不可读 | ❌ 不可读 |
| **可写性** | ❌ 不可写 | ❌ 不可写 | ✅ 可写 | ✅ 可写 |
| **典型应用** | 电压、电流、温度 | 开关状态、故障信号 | 启停控制、开关控制 | 频率设定、功率调节 |

## 3. 设计实现

### 3.1 点位配置结构

每个点位都包含`telemetry_type`字段来标识其四遥类型：

```rust
pub struct UniversalPointConfig {
    pub point_id: u32,
    pub name: Option<String>,
    pub telemetry_type: TelemetryType,  // 四遥类型标识
    pub data_type: String,               // 根据类型自动设置
    pub readable: bool,                  // 根据类型自动设置
    pub writable: bool,                  // 根据类型自动设置
    // ... 其他字段
}
```

### 3.2 自动属性推导

创建点位时，系统会根据四遥类型自动设置相关属性：

```rust
impl UniversalPointConfig {
    pub fn new(point_id: u32, name: &str, telemetry_type: TelemetryType) -> Self {
        let is_writable = matches!(telemetry_type, 
            TelemetryType::Control | TelemetryType::Setpoint);
        
        Self {
            // 根据是否为模拟量自动设置数据类型
            data_type: match telemetry_type.is_analog() {
                true => "float".to_string(),
                false => "bool".to_string(),
            },
            // 自动设置读写权限
            readable: telemetry_type.is_readable(),
            writable: is_writable,
            // ...
        }
    }
}
```

### 3.3 数据处理差异

不同类型的数据处理方式不同：

- **模拟量（遥测/遥调）**: 应用缩放和偏移
  ```rust
  pub fn process_value(&self, raw_value: f64) -> f64 {
      raw_value * self.scale + self.offset
  }
  ```

- **数字量（遥信/遥控）**: 应用反位逻辑
  ```rust
  pub fn process_digital_value(&self, source_data: bool) -> bool {
      if self.reverse == 1 {
          !source_data
      } else {
          source_data
      }
  }
  ```

## 4. 数据存储与索引

### 4.1 内存存储

在`OptimizedPointManager`中，点位按类型分组存储：

```rust
// 按类型分组的点位索引
points_by_type: Arc<RwLock<HashMap<TelemetryType, HashSet<u32>>>>,
```

### 4.2 Redis存储

在Redis中，通过类型索引快速查询：

- **点位数据**: `comsrv:demo:points:{point_id}`
- **类型索引**: `comsrv:demo:points:type:{TelemetryType}`

示例：
```
comsrv:demo:points:type:Telemetry → SET {1000, 1004, 1008, ...}
comsrv:demo:points:type:Signaling → SET {1001, 1005, 1009, ...}
comsrv:demo:points:type:Control → SET {1002, 1006, 1010, ...}
comsrv:demo:points:type:Setpoint → SET {1003, 1007, 1011, ...}
```

## 5. 查询接口

系统提供多种按四遥类型查询的接口：

```rust
// 获取某类型的所有点位
pub async fn get_points_by_type(&self, telemetry_type: &TelemetryType) -> Vec<u32>

// 获取某类型的启用点位
pub async fn get_enabled_points_by_type(&self, telemetry_type: &TelemetryType) -> Vec<u32>

// 获取某类型的实时数据
pub async fn get_point_data_by_type(&self, telemetry_type: &TelemetryType) -> Vec<PointData>
```

## 6. 协议映射

不同的工业协议对四遥的映射方式不同：

### Modbus协议映射
- **遥测**: 输入寄存器 (Function Code 04)
- **遥信**: 离散输入 (Function Code 02)
- **遥控**: 线圈 (Function Code 01/05/15)
- **遥调**: 保持寄存器 (Function Code 03/06/16)

### IEC 60870协议映射
- **遥测**: Type 9-14 (测量值)
- **遥信**: Type 1-4 (单点/双点信息)
- **遥控**: Type 45-47 (单/双命令)
- **遥调**: Type 48-50 (设定值命令)

## 7. 使用示例

### 创建不同类型的点位
```rust
// 遥测点 - 电压测量
let voltage = UniversalPointConfig::new(1001, "A相电压", TelemetryType::Telemetry);

// 遥信点 - 开关状态
let switch_status = UniversalPointConfig::new(2001, "断路器状态", TelemetryType::Signaling);

// 遥控点 - 开关控制
let switch_control = UniversalPointConfig::new(3001, "断路器控制", TelemetryType::Control);

// 遥调点 - 功率设定
let power_setpoint = UniversalPointConfig::new(4001, "功率设定", TelemetryType::Setpoint);
```

### 按类型查询
```rust
// 获取所有遥测点
let telemetry_points = manager.get_points_by_type(&TelemetryType::Telemetry).await;

// 获取所有可写点位（遥控+遥调）
let writable_points = manager.get_enabled_points_by_type(&TelemetryType::Control).await
    .into_iter()
    .chain(manager.get_enabled_points_by_type(&TelemetryType::Setpoint).await)
    .collect::<Vec<_>>();
```

## 8. 优势

1. **类型安全**: 编译时即可检查四遥类型的正确性
2. **自动推导**: 根据类型自动设置数据类型、读写权限等属性
3. **高效查询**: O(1)复杂度的类型查询
4. **协议无关**: 四遥抽象与具体协议解耦
5. **扩展性好**: 易于添加新的遥测类型或属性