# modsrv 存储层设计方案

基于 comsrv 的优化经验，为 modsrv 设计了一个专门的数据结构方案，实现监视值读取和控制命令写入的分离处理。

## 1. 设计原则

- **扁平化键值结构**：采用类似 comsrv 的扁平化设计，避免复杂的嵌套
- **读写分离**：监视值只读（从 comsrv），控制命令只写（发送给 comsrv）
- **批量操作优化**：支持批量读写，减少 Redis 往返次数
- **类型安全**：使用强类型枚举区分不同的数据类型

## 2. 数据结构设计

### 2.1 监视值（Monitor Values）存储结构

监视值用于读取来自 comsrv 的实时数据和存储模型计算结果。

#### 键格式设计

```
# 监视值键格式
mod:{model_id}:{type}:{point_id}

# 类型缩写：
- mv:m  (monitor value: measurement) - 来自comsrv的测量值
- mv:s  (monitor value: signal) - 来自comsrv的信号值  
- mo    (model output) - 模型输出值
- mi    (model intermediate) - 中间计算值

# 示例：
mod:power_calc_001:mv:m:10001  # 模型power_calc_001监视的测量点10001
mod:power_calc_001:mo:25678    # 模型输出值（使用字段名哈希作为ID）
```

#### 值格式设计

```rust
// 格式：value:timestamp:quality:source
// 示例：235.5:1704067200000:100:power_calc_001

pub struct MonitorValue {
    pub value: f64,        // 数值
    pub timestamp: i64,    // 时间戳（毫秒）
    pub quality: u8,       // 数据质量（0-100）
    pub source: String,    // 数据源标识
}
```

### 2.2 控制命令（Control Commands）存储结构

控制命令用于向 comsrv 发送控制指令。

#### 命令存储格式

```
# 控制命令键格式
cmd:{command_id}  # 使用Hash存储命令详情

# 命令列表键（按模型）
cmd:list:{model_id}  # List类型，存储命令ID列表

# 发布通道格式（供comsrv订阅）
cmd:{channel_id}:{type}
- cc:c  (control command: control) - 遥控命令
- cc:a  (control command: adjust) - 遥调命令
```

#### 命令数据结构

```rust
pub struct ControlCommand {
    pub id: String,                    // 命令ID (UUID)
    pub channel_id: u16,               // 目标通道
    pub point_id: u32,                 // 点位ID
    pub command_type: ControlType,     // 命令类型
    pub value: f64,                    // 命令值
    pub status: CommandStatus,         // 命令状态
    pub created_at: i64,               // 创建时间
    pub updated_at: i64,               // 更新时间
    pub message: Option<String>,       // 执行消息
    pub source_model: String,          // 来源模型
}
```

### 2.3 只读数据访问

modsrv 可以直接读取 comsrv 的数据，使用 comsrv 的键格式：

```
# comsrv 键格式（只读）
{channel_id}:{type}:{point_id}

# 类型：
- m: 测量值 (YC)
- s: 信号值 (YX)
- c: 控制值 (YK) 
- a: 调节值 (YT)

# 值格式：value:timestamp
```

## 3. 核心接口设计

### 3.1 存储接口 (ModelStorage)

```rust
pub struct ModelStorage {
    conn: ConnectionManager,
}

impl ModelStorage {
    // 监视值操作
    async fn get_monitor_value(...) -> Result<Option<MonitorValue>>;
    async fn get_monitor_values(...) -> Result<Vec<Option<MonitorValue>>>;
    async fn set_monitor_value(...) -> Result<()>;
    async fn set_monitor_values(...) -> Result<()>;
    
    // 控制命令操作
    async fn create_control_command(...) -> Result<()>;
    async fn get_control_command(...) -> Result<Option<ControlCommand>>;
    async fn update_command_status(...) -> Result<()>;
    async fn get_model_commands(...) -> Result<Vec<ControlCommand>>;
    
    // comsrv数据读取（只读）
    async fn read_comsrv_point(...) -> Result<Option<(f64, i64)>>;
    async fn read_comsrv_points(...) -> Result<Vec<Option<(f64, i64)>>>;
}
```

### 3.2 监视管理器 (MonitorManager)

提供高级监视功能：

```rust
pub struct MonitorManager {
    storage: ModelStorage,
}

impl MonitorManager {
    // 批量读取模型输入
    async fn read_model_inputs(&mut self, 
        input_mappings: &[(u16, &str, u32, String)]
    ) -> Result<HashMap<String, f64>>;
    
    // 批量写入模型输出
    async fn write_model_outputs(&mut self,
        model_id: &str,
        outputs: HashMap<String, f64>
    ) -> Result<()>;
    
    // 中间值管理
    async fn write_intermediate_value(...) -> Result<()>;
    async fn read_intermediate_values(...) -> Result<HashMap<String, Option<MonitorValue>>>;
}
```

### 3.3 控制管理器 (ControlManager)

提供控制命令管理：

```rust
pub struct ControlManager {
    storage: ModelStorage,
    timeout_duration: Duration,
}

impl ControlManager {
    // 发送控制命令
    async fn send_remote_control(...) -> Result<String>;
    async fn send_remote_adjust(...) -> Result<String>;
    
    // 命令状态管理
    async fn get_command_status(...) -> Result<CommandStatus>;
    async fn wait_for_completion(...) -> Result<CommandStatus>;
    async fn cancel_command(...) -> Result<()>;
    
    // 批量操作
    async fn send_batch_commands(...) -> Result<Vec<String>>;
    
    // 条件检查
    async fn check_command_conditions(...) -> bool;
}
```

## 4. 使用示例

```rust
// 创建管理器
let mut monitor_mgr = MonitorManager::from_env().await?;
let mut control_mgr = ControlManager::from_env().await?;

// 1. 读取输入数据
let input_mappings = vec![
    (1001, "m", 10001, "voltage".to_string()),
    (1001, "m", 10002, "current".to_string()),
];
let inputs = monitor_mgr.read_model_inputs(&input_mappings).await?;

// 2. 执行计算
let mut outputs = HashMap::new();
outputs.insert("power".to_string(), inputs["voltage"] * inputs["current"]);

// 3. 保存输出
monitor_mgr.write_model_outputs("model_001", outputs).await?;

// 4. 发送控制命令
if outputs["power"] > 1000.0 {
    let cmd_id = control_mgr.send_remote_control(
        1001, 40001, false, "model_001".to_string()
    ).await?;
    
    // 等待执行完成
    let status = control_mgr.wait_for_completion(&cmd_id).await?;
}
```

## 5. 优化特性

1. **批量操作**：使用 Redis Pipeline 减少网络往返
2. **类型安全**：使用枚举确保类型正确性
3. **过期管理**：命令自动过期，避免数据堆积
4. **发布订阅**：控制命令通过 pub/sub 实时通知
5. **性能监控**：记录批量操作耗时

## 6. 与 comsrv 的协调

1. **数据流向**：
   - comsrv → modsrv：通过直接读取 Redis 键获取实时数据
   - modsrv → comsrv：通过发布控制命令到指定通道

2. **键命名空间**：
   - comsrv：`{channel_id}:{type}:{point_id}`
   - modsrv：`mod:` 和 `cmd:` 前缀，避免冲突

3. **命令反馈**：
   - comsrv 执行命令后通过特定通道发布反馈
   - modsrv 订阅反馈通道更新命令状态