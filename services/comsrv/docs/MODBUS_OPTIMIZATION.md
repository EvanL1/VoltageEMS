# Modbus 性能优化指南

## 概述

本指南提供了详细的Modbus通信性能优化策略和最佳实践，帮助您在大规模部署中获得最佳性能。

## 性能基准

在标准测试环境下的性能指标：

| 场景 | 点位数 | 未优化 | 优化后 | 提升 |
|------|--------|--------|--------|------|
| 单点顺序读取 | 100 | 50 req/s | 50 req/s | 1x |
| 批量读取 | 100 | 50 req/s | 150 req/s | 3x |
| 并发读取 | 1000 | 100 req/s | 800 req/s | 8x |
| 轮询引擎 | 1500 | 80 req/s | 600 req/s | 7.5x |

## 优化策略

### 1. 批量读取优化

#### 原理

将多个连续或接近的寄存器读取合并为单个请求。

```
未优化：
  Read(40001) → Response
  Read(40002) → Response  
  Read(40003) → Response
  总计：3次网络往返

优化后：
  Read(40001-40003) → Response
  总计：1次网络往返
```

#### 配置方法

```yaml
polling:
  enable_batch_reading: true
  max_batch_size: 125      # Modbus标准限制
  
batch_config:
  max_gap: 10             # 允许的最大地址间隔
  max_batch_size: 50      # 自定义批量大小
  merge_function_codes: false
```

#### 实现代码

```rust
// 手动批量读取
let values = client.read_holding_registers(
    slave_id, 
    start_address, 
    count
).await?;

// 使用批量优化配置
let batch_config = ModbusBatchConfig {
    max_gap: 10,
    max_batch_size: 50,
    merge_function_codes: false,
    device_limits: HashMap::new(),
};
```

### 2. 并发优化

#### 并发级别控制

```rust
// 全局并发限制
let engine = ModbusProtocolEngine::with_concurrency_limit(10);

// 从站级别并发控制
slave_configs:
  1:
    max_concurrent_requests: 1  # 串行访问
  2:
    max_concurrent_requests: 5  # 5个并发
```

#### 异步批量操作

```rust
// 并发读取多个从站
let futures: Vec<_> = slaves.iter()
    .map(|&slave_id| {
        let client = client.clone();
        async move {
            client.read_holding_registers(slave_id, 40001, 10).await
        }
    })
    .collect();

let results = futures::future::join_all(futures).await;
```

### 3. 缓存优化

#### 缓存策略

```rust
pub struct CacheConfig {
    pub max_size: usize,       // 最大缓存条目
    pub ttl: Duration,         // 过期时间
    pub enable_cache: bool,    // 启用开关
}

// 应用场景
// - 静态配置数据：TTL = 1小时
// - 缓变数据：TTL = 10秒
// - 实时数据：TTL = 1秒或禁用缓存
```

#### 智能缓存

```rust
// 基于数据类型的缓存策略
match telemetry_type {
    TelemetryType::Config => Duration::from_secs(3600),    // 1小时
    TelemetryType::Status => Duration::from_secs(10),      // 10秒
    TelemetryType::Realtime => Duration::from_millis(500), // 500ms
    _ => Duration::from_secs(5),                           // 默认5秒
}
```

### 4. 网络优化

#### TCP优化

```rust
// TCP_NODELAY - 禁用Nagle算法
socket.set_nodelay(true)?;

// Keep-alive设置
socket.set_keepalive(Some(Duration::from_secs(60)))?;

// 缓冲区大小
socket.set_send_buffer_size(65536)?;
socket.set_recv_buffer_size(65536)?;
```

#### 连接池

```rust
pub struct ConnectionPool {
    connections: HashMap<String, Vec<Connection>>,
    max_connections_per_host: usize,
    idle_timeout: Duration,
}

// 使用连接池减少连接开销
let conn = pool.get_connection(&host).await?;
```

### 5. 协议优化

#### 功能码选择

| 操作类型 | 推荐功能码 | 原因 |
|---------|-----------|------|
| 读多个寄存器 | FC03/FC04 | 支持批量 |
| 读多个线圈 | FC01/FC02 | 支持批量 |
| 写多个寄存器 | FC16 | 一次写入多个 |
| 写多个线圈 | FC15 | 一次写入多个 |

#### 数据打包

```rust
// 使用32位数据减少请求次数
// 未优化：2个16位寄存器分别读取
let high = read_register(40001).await?;
let low = read_register(40002).await?;
let value = (high << 16) | low;

// 优化：一次读取2个寄存器
let regs = read_registers(40001, 2).await?;
let value = (regs[0] << 16) | regs[1];
```

### 6. 轮询优化

#### 分级轮询

```yaml
# 根据数据重要性设置不同轮询频率
polling_groups:
  critical:      # 关键数据
    interval_ms: 100
    points: [1001, 1002, 1003]
  
  normal:        # 普通数据
    interval_ms: 1000
    points: [2001, 2002, 2003]
  
  slow:          # 缓变数据
    interval_ms: 10000
    points: [3001, 3002, 3003]
```

#### 动态轮询

```rust
// 根据数据变化率调整轮询频率
if value_changed > threshold {
    // 数据变化大，增加轮询频率
    interval = interval / 2;
} else {
    // 数据稳定，降低轮询频率
    interval = (interval * 1.5).min(max_interval);
}
```

## 性能监控

### 关键指标

```rust
pub struct PerformanceMetrics {
    pub requests_per_second: f64,
    pub average_response_time: Duration,
    pub cache_hit_rate: f64,
    pub error_rate: f64,
    pub concurrent_connections: usize,
    pub batch_efficiency: f64,  // 批量请求占比
}
```

### 监控实现

```rust
// Prometheus指标
lazy_static! {
    static ref REQUEST_COUNTER: IntCounter = register_int_counter!(
        "modbus_requests_total",
        "Total number of Modbus requests"
    ).unwrap();
    
    static ref RESPONSE_TIME: Histogram = register_histogram!(
        "modbus_response_time_seconds",
        "Modbus response time distribution"
    ).unwrap();
}

// 使用示例
REQUEST_COUNTER.inc();
let timer = RESPONSE_TIME.start_timer();
let result = modbus_operation().await;
timer.observe_duration();
```

## 常见性能问题

### 1. 高延迟问题

**症状**：响应时间超过100ms

**解决方案**：
- 检查网络延迟：`ping <modbus_server>`
- 减少请求大小
- 启用批量读取
- 使用并发请求

### 2. 低吞吐量

**症状**：请求速率低于预期

**解决方案**：
- 增加并发数
- 优化批量大小
- 检查CPU使用率
- 使用连接池

### 3. 内存占用高

**症状**：内存持续增长

**解决方案**：
- 限制缓存大小
- 设置合理的TTL
- 检查内存泄漏
- 使用内存分析工具

### 4. 连接不稳定

**症状**：频繁断线重连

**解决方案**：
- 启用keep-alive
- 增加超时时间
- 实现重连机制
- 检查网络质量

## 优化检查清单

- [ ] 启用批量读取
- [ ] 配置合适的批量大小
- [ ] 设置合理的轮询间隔
- [ ] 使用并发请求
- [ ] 启用数据缓存
- [ ] 配置连接池
- [ ] 启用TCP优化
- [ ] 实现错误重试
- [ ] 监控性能指标
- [ ] 定期分析瓶颈

## 性能测试工具

### 1. 内置压力测试

```bash
# 运行1000点压力测试
cargo run --example stress_test_1000_points

# 运行批量优化测试
cargo run --example batch_optimization_test
```

### 2. 性能分析

```bash
# CPU分析
cargo build --release
perf record --call-graph=dwarf target/release/comsrv
perf report

# 内存分析
valgrind --leak-check=full target/release/comsrv
```

### 3. 实时监控

```bash
# 监控Redis操作
redis-cli monitor | grep point:

# 监控网络流量
tcpdump -i any -n port 502

# 系统资源监控
htop
iotop
```

## 最佳实践总结

1. **始终使用批量操作**：将相邻的寄存器合并读取
2. **合理设置并发数**：根据设备能力调整
3. **分级轮询策略**：不同数据采用不同频率
4. **启用缓存机制**：减少重复请求
5. **监控关键指标**：及时发现性能问题
6. **定期优化调整**：根据实际负载调整参数
7. **错误处理优化**：避免错误级联
8. **资源限制保护**：防止资源耗尽

## 参考配置

### 小型系统（<100点）

```yaml
polling:
  default_interval_ms: 1000
  enable_batch_reading: true
  max_batch_size: 50
  
concurrency:
  max_concurrent_requests: 5
  
cache:
  enabled: true
  ttl_seconds: 10
```

### 中型系统（100-1000点）

```yaml
polling:
  default_interval_ms: 500
  enable_batch_reading: true
  max_batch_size: 100
  
concurrency:
  max_concurrent_requests: 10
  
cache:
  enabled: true
  ttl_seconds: 5
  max_entries: 1000
```

### 大型系统（>1000点）

```yaml
polling:
  enable_batch_reading: true
  max_batch_size: 125
  
polling_groups:
  critical:
    interval_ms: 100
  normal:
    interval_ms: 1000
  slow:
    interval_ms: 10000
    
concurrency:
  max_concurrent_requests: 20
  
cache:
  enabled: true
  ttl_seconds: 2
  max_entries: 10000
  
connection_pool:
  max_connections: 50
  idle_timeout: 300
```