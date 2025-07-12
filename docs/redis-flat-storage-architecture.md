# VoltageEMS Redis 扁平化存储架构

## 概述

VoltageEMS 采用扁平化的 Redis 键值对存储架构，优化了高频实时数据的读写性能。该架构专门针对工业物联网场景设计，支持大规模点位数据的实时采集和跨服务查询。

## 核心设计原则

1. **极简主义**：最少的抽象层，直接的数据访问
2. **高性能**：单点查询 O(1)，无需二次哈希
3. **可扩展**：支持百万级点位，易于 Redis Cluster 分片
4. **独立更新**：每个点独立存储，无并发冲突

## 数据结构设计

### 键命名规范

```
实时数据：{channelID}:{type}:{pointID}
配置数据：cfg:{channelID}:{type}:{pointID}
```

### 四遥类型映射

| 类型        | 缩写 | 说明             | 原始名称 |
| ----------- | ---- | ---------------- | -------- |
| Measurement | m    | 遥测（模拟量）   | YC       |
| Signal      | s    | 遥信（数字量）   | YX       |
| Control     | c    | 遥控（控制命令） | YK       |
| Adjustment  | a    | 遥调（设定值）   | YT       |

### 存储示例

```redis
# 实时数据
1001:m:10001 -> "25.6:1704956400"      # 温度测量值
1001:m:10002 -> "380.5:1704956401"     # 电压值
1001:s:20001 -> "1:1704956402"         # 开关状态
1001:c:30001 -> "0:1704956403"         # 控制命令
1001:a:40001 -> "50.0:1704956404"      # 功率设定值

# 配置数据
cfg:1001:m:10001 -> {"name":"温度传感器1","unit":"°C","scale":0.1,"offset":0,"address":"1:3:100"}
cfg:1001:s:20001 -> {"name":"主开关","unit":"","scale":1.0,"offset":0,"address":"1:1:0"}
```

## 数据流架构

```
┌─────────────┐     扁平化写入        ┌─────────────┐     单点查询       ┌─────────────┐
│   设备层     │ ──────────────────> │    Redis    │ <───────────────  │   应用层    │
│ (采集驱动)   │                      │  (K-V存储)  │                   │ (Web/API)   │
└─────────────┘                     └─────────────┘                   └─────────────┘
       │                                   ↑ ↓                                │
       │                                   ↑ ↓                                │
       v                                   ↑ ↓                                v
┌─────────────┐                     ┌─────────────┐                   ┌─────────────┐
│   comsrv    │ ──────────────────> │   modsrv    │ <───────────────  │   hissrv    │
│ (协议转换)  │    批量Pipeline      │ (计算引擎)  │    时序查询       │ (历史存储)  │
└─────────────┘                     └─────────────┘                   └─────────────┘
```

## 性能特性

### 写入性能

- 单点更新：< 0.5ms
- 批量更新（1000点）：< 5ms（使用 Pipeline）
- 并发写入：支持多客户端并发，无锁设计

### 查询性能

- 单点查询：< 0.5ms
- 批量查询（100点）：< 2ms（使用 MGET）
- 通道扫描：使用 SCAN 避免阻塞

### 内存效率

- 每个点位：~100 字节（含键名和元数据）
- 10000 点：~1MB
- 100万点：~100MB

## 实现细节

### 存储层接口（Rust）

```rust
pub struct RedisStorage {
    conn: ConnectionManager,
}

impl RedisStorage {
    // 单点操作
    pub async fn set_point(channel_id: u16, point_type: &str, point_id: u32, value: f64);
    pub async fn get_point(channel_id: u16, point_type: &str, point_id: u32) -> Option<(f64, i64)>;
  
    // 批量操作
    pub async fn set_points(&[PointUpdate]);  // 使用 Pipeline
    pub async fn get_points(&[PointKey]) -> Vec<Option<(f64, i64)>>;  // 使用 MGET
  
    // 配置管理
    pub async fn set_config(channel_id: u16, point_type: &str, point_id: u32, config: &PointConfig);
    pub async fn get_config(channel_id: u16, point_type: &str, point_id: u32) -> Option<PointConfig>;
}
```

### 数据类型定义

```rust
// 极简的点位值
pub struct PointValue {
    pub value: f64,
    pub timestamp: i64,
}

// 点位配置
pub struct PointConfig {
    pub name: String,
    pub unit: String,
    pub scale: f64,
    pub offset: f64,
    pub address: String,
}

// 批量更新
pub struct PointUpdate {
    pub channel_id: u16,
    pub point_type: &'static str,
    pub point_id: u32,
    pub value: f64,
}
```

## 使用场景

### 1. 实时数据采集（comsrv）

```rust
// 协议数据到达后
storage.set_point(1001, TYPE_MEASUREMENT, 10001, 25.6).await?;
```

### 2. 计算服务查询（modsrv）

```rust
// 获取计算所需的输入
let temp = storage.get_point(1001, TYPE_MEASUREMENT, 10001).await?;
```

### 3. 批量数据同步（hissrv）

```rust
// 批量获取历史归档
let keys = vec![
    PointKey { channel_id: 1001, point_type: TYPE_MEASUREMENT, point_id: 10001 },
    PointKey { channel_id: 1001, point_type: TYPE_MEASUREMENT, point_id: 10002 },
];
let values = storage.get_points(&keys).await?;
```

## 扩展性设计

### 水平扩展

- 使用 Redis Cluster，按 channel_id 自动分片
- 支持读写分离，从节点处理查询

### 垂直扩展

- 使用 Pipeline 减少网络往返
- 批量操作优化
- 连接池管理

### 数据分层

```
热数据：Redis（最近5分钟）
温数据：Redis（最近1小时，可选压缩）
冷数据：InfluxDB（历史数据）
```

## 监控指标

### 性能指标

```redis
# 写入性能
perf:write:qps -> "15000"         # 每秒写入点数
perf:write:latency:p99 -> "2.5"   # P99延迟(ms)

# 查询性能  
perf:read:qps -> "50000"          # 每秒查询点数
perf:read:latency:p99 -> "1.2"    # P99延迟(ms)
```

### 数据质量

```redis
# 数据完整性
quality:missing:1001 -> "3"       # 通道1001缺失点数
quality:stale:1001 -> "5"         # 通道1001过期点数
```

## 最佳实践

1. **批量操作**：尽可能使用 Pipeline 和 MGET/MSET
2. **连接复用**：使用连接池，避免频繁建立连接
3. **错误处理**：实现重试机制和降级策略
4. **监控告警**：监控延迟和错误率
5. **定期清理**：清理过期数据，控制内存使用

## 迁移指南

从旧的 Hash 结构迁移到扁平化结构：

1. **双写阶段**：新旧结构同时写入
2. **验证阶段**：对比数据一致性
3. **切换阶段**：逐步切换读取到新结构
4. **清理阶段**：删除旧数据

## 故障处理

### Redis 不可用

- 本地缓存最近数据
- 降级到只读模式
- 自动重连机制

### 网络分区

- 客户端侧缓存
- 批量重传机制
- 数据去重

## 总结

扁平化存储架构通过简化数据结构、优化访问路径，为 VoltageEMS 提供了高性能、可扩展的实时数据存储方案。该架构特别适合工业物联网场景下的高频数据采集和跨服务查询需求。
