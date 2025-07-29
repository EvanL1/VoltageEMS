# ModSrv - 设备模型计算引擎

ModSrv是VoltageEMS系统的核心计算引擎，负责执行基于DAG的实时数据计算和设备模型管理。

## 特性

- **DAG计算引擎** - 支持复杂的数据流计算图
- **设备模型系统** - 统一的设备抽象和管理
- **实时数据处理** - 毫秒级计算延迟
- **内置函数库** - sum、avg、min、max、scale等
- **Redis集成** - 高性能数据存储和发布

## 架构

```
Redis Hash (comsrv数据) → ModSrv计算引擎 → Redis Hash (计算结果)
                            ↓
                        告警触发 → AlarmSrv
                        规则触发 → RuleSrv
```

## 快速开始

### 环境要求

- Rust 1.88+
- Redis 7.0+

### 运行服务

```bash
# 开发模式
cargo run -p modsrv

# 生产模式
cargo run --release -p modsrv

# 指定日志级别
RUST_LOG=modsrv=debug cargo run -p modsrv
```

### 配置文件

```yaml
# services/modsrv/config/default.yml
service_name: "modsrv"
version: "2.0.0"

redis:
  url: "redis://localhost:6379"
  key_prefix: "modsrv:"

api:
  host: "0.0.0.0"
  port: 8092

models:
  - id: "power_meter_demo"
    name: "演示电表模型"
    description: "用于演示的简单电表监控模型"
    monitoring:
      voltage_a:
        description: "A相电压"
        unit: "V"
      current_a:
        description: "A相电流"
        unit: "A"
      power:
        description: "有功功率"
        unit: "kW"
    control:
      main_switch:
        description: "主开关"
      power_limit:
        description: "功率限制设定"
        unit: "kW"
```

### 点位映射

```json
// services/modsrv/mappings/power_meter_demo.json
{
  "monitoring": {
    "voltage_a": {
      "channel": 1001,
      "point": 1,      // 注意：点位ID从1开始
      "type": "m"
    },
    "current_a": {
      "channel": 1001,
      "point": 2,
      "type": "m"
    }
  },
  "control": {
    "main_switch": {
      "channel": 1001,
      "point": 1,      // 控制点也从1开始
      "type": "c"
    }
  }
}
```

## API接口

### 健康检查

```bash
curl http://localhost:8092/health
```

### 获取模型列表

```bash
curl http://localhost:8092/models
```

### 获取模型数据

```bash
curl http://localhost:8092/models/power_meter_demo
```

### 发送控制命令

```bash
curl -X POST http://localhost:8092/models/power_meter_demo/control/main_switch \
  -H "Content-Type: application/json" \
  -d '{"value": 1}'
```

## DAG计算示例

```rust
// 内部计算逻辑示例
let dag = DAGBuilder::new()
    .add_node("voltage", Source::Redis("comsrv:1001:m", "1"))
    .add_node("current", Source::Redis("comsrv:1001:m", "2"))
    .add_node("power", Function::Multiply(vec!["voltage", "current"]))
    .add_node("scaled_power", Function::Scale("power", 0.001))  // W转kW
    .build();

// 执行计算
let results = dag.execute().await?;
```

## 数据流

1. **数据输入**: 从Redis Hash读取 `comsrv:{channelID}:{type}`
2. **计算处理**: 执行DAG定义的计算流程
3. **结果存储**: 写入 `modsrv:{modelname}:measurement`
4. **事件发布**: 发布到 `modsrv:{modelname}:update`

## 监控和调试

### 查看Redis数据

```bash
# 查看输入数据
redis-cli hgetall "comsrv:1001:m"

# 查看计算结果
redis-cli hgetall "modsrv:power_meter_demo:measurement"

# 监控数据更新
redis-cli subscribe "modsrv:power_meter_demo:update"
```

### 日志监控

```bash
# 查看服务日志
tail -f logs/modsrv.log

# 调试模式
RUST_LOG=modsrv=trace cargo run
```

## 性能优化

- **批量读取**: 使用HGETALL减少Redis往返
- **计算缓存**: 避免重复计算相同的节点
- **并行处理**: 独立的计算分支并行执行
- **连接池**: Redis连接复用

## 开发指南

### 添加新函数

```rust
// 在Function枚举中添加
pub enum Function {
    // ...
    MyNewFunction(Vec<String>),  // 输入参数列表
}

// 实现计算逻辑
impl Function {
    pub fn execute(&self, inputs: &HashMap<String, f64>) -> Result<f64> {
        match self {
            Function::MyNewFunction(params) => {
                // 实现函数逻辑
            }
        }
    }
}
```

### 测试

```bash
# 运行单元测试
cargo test -p modsrv

# 运行特定测试
cargo test -p modsrv test_dag_calculation

# 运行集成测试
cargo test -p modsrv --test integration
```

## 故障排查

### 常见问题

1. **无数据输出**
   - 检查Redis连接
   - 验证点位映射配置
   - 确认comsrv数据存在

2. **计算错误**
   - 检查DAG定义是否有循环依赖
   - 验证输入数据格式
   - 查看错误日志

3. **性能问题**
   - 监控Redis操作延迟
   - 检查计算图复杂度
   - 优化批量操作

## 配置参考

### 环境变量

```bash
RUST_LOG=modsrv=info      # 日志级别
REDIS_URL=redis://localhost:6379
MODSRV_PORT=8092
```

### 高级配置

```yaml
# 计算引擎配置
compute:
  max_dag_depth: 10        # DAG最大深度
  cache_ttl: 60            # 缓存时间(秒)
  batch_size: 100          # 批处理大小
  
# 性能调优
performance:
  worker_threads: 4        # 工作线程数
  queue_size: 1000        # 任务队列大小
```

## 许可证

MIT License