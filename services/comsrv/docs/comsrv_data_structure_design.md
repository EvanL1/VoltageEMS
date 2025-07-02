# comsrv 数据结构设计方案

## 核心设计原则

1. **实时点位数据使用 HashMap<u32, T>**：O(1) 查询性能
2. **多级索引结构**：支持按ID、名称、类型等多维度快速查询
3. **批量操作优化**：减少网络往返和锁竞争
4. **本地缓存层**：减少 Redis 访问频率
5. **使用合适的数据结构**：HashSet 替代 Vec 进行集合操作

## 数据结构设计

### 1. 点位管理器 (PointManager)

```rust
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::core::config::types::telemetry::TelemetryType;

pub struct OptimizedPointManager {
    // 主存储：使用 u32 作为 key，避免字符串转换开销
    points: Arc<RwLock<HashMap<u32, UniversalPointConfig>>>,
    
    // 实时数据缓存：快速访问最新点位值
    realtime_cache: Arc<RwLock<HashMap<u32, PointData>>>,
    
    // 按类型索引：使用 HashSet 提供 O(1) 查找
    points_by_type: Arc<RwLock<HashMap<TelemetryType, HashSet<u32>>>>,
    
    // 名称到ID映射：支持按名称快速查找
    name_to_id: Arc<RwLock<HashMap<String, u32>>>,
    
    // 权限分组：快速权限检查
    readonly_points: Arc<RwLock<HashSet<u32>>>,
    writable_points: Arc<RwLock<HashSet<u32>>>,
    
    // 启用的点位：避免遍历所有点位
    enabled_points: Arc<RwLock<HashSet<u32>>>,
    
    // 统计信息缓存
    stats: Arc<RwLock<PointManagerStats>>,
}

pub struct PointManagerStats {
    total_points: usize,
    points_by_type: HashMap<TelemetryType, usize>,
    last_update: std::time::Instant,
}
```

### 2. 协议工厂 (ProtocolFactory)

```rust
use dashmap::DashMap;
use ahash::RandomState;

pub struct OptimizedProtocolFactory {
    // 主通道存储：并发安全的 DashMap
    channels: DashMap<u16, ChannelEntry, RandomState>,
    
    // 协议类型索引：快速查找某协议的所有通道
    channels_by_protocol: DashMap<ProtocolType, HashSet<u16>, RandomState>,
    
    // 名称索引：支持按名称查找通道
    channels_by_name: DashMap<String, u16, RandomState>,
    
    // 活跃通道集合：避免遍历所有通道
    active_channels: Arc<RwLock<HashSet<u16>>>,
    
    // 统计信息缓存：避免实时计算
    cached_stats: Arc<RwLock<ChannelStats>>,
    stats_dirty: Arc<AtomicBool>,
    
    // 协议工厂注册表
    protocol_factories: DashMap<ProtocolType, Arc<dyn ProtocolClientFactory>, RandomState>,
}

pub struct ChannelStats {
    total_channels: usize,
    active_channels: usize,
    channels_by_protocol: HashMap<ProtocolType, usize>,
    last_update: std::time::Instant,
}
```

### 3. Redis 存储优化 (RedisStorage)

```rust
use redis::aio::ConnectionManager;
use std::time::{Duration, Instant};

pub struct OptimizedRedisStore {
    // 连接池管理
    connection_manager: ConnectionManager,
    
    // 本地缓存层：减少 Redis 访问
    local_cache: Arc<RwLock<HashMap<String, CachedValue>>>,
    cache_ttl: Duration,
    
    // 批量写入缓冲区
    write_buffer: Arc<RwLock<Vec<WriteOperation>>>,
    buffer_size_limit: usize,
    buffer_flush_interval: Duration,
    
    // 批量读取优化
    read_batch_size: usize,
    
    // SCAN 配置（替代危险的 KEYS 命令）
    scan_batch_size: usize,
}

struct CachedValue {
    value: serde_json::Value,
    expires_at: Instant,
    hit_count: u32,  // 用于 LRU 策略
}

enum WriteOperation {
    Set { key: String, value: serde_json::Value, ttl: Option<Duration> },
    Delete { key: String },
    Expire { key: String, ttl: Duration },
}
```

### 4. 命令管理器 (CommandManager)

```rust
use std::collections::BinaryHeap;
use std::cmp::Ordering;

pub struct OptimizedCommandManager {
    // 优先级命令队列
    command_queue: Arc<RwLock<BinaryHeap<PrioritizedCommand>>>,
    
    // 命令执行器池
    executor_pool: Arc<RwLock<Vec<CommandExecutor>>>,
    max_executors: usize,
    
    // 命令结果缓存（用于幂等性）
    result_cache: Arc<RwLock<HashMap<String, CommandResult>>>,
    result_cache_ttl: Duration,
    
    // 命令统计
    command_stats: Arc<RwLock<CommandStats>>,
    
    // Redis 后端
    redis_store: Arc<OptimizedRedisStore>,
}

#[derive(Eq, PartialEq)]
struct PrioritizedCommand {
    priority: CommandPriority,
    command: RemoteCommand,
    command_id: String,
    timestamp: Instant,
}

impl Ord for PrioritizedCommand {
    fn cmp(&self, other: &Self) -> Ordering {
        self.priority.cmp(&other.priority)
            .then_with(|| other.timestamp.cmp(&self.timestamp))
    }
}

#[derive(Eq, PartialEq, Ord, PartialOrd)]
enum CommandPriority {
    Critical = 3,
    High = 2,
    Normal = 1,
    Low = 0,
}
```

### 5. 协议映射管理 (ProtocolMappingManager)

```rust
pub struct ProtocolMappingManager {
    // 按协议类型组织映射
    mappings_by_protocol: Arc<RwLock<HashMap<ProtocolType, ProtocolMappings>>>,
    
    // 点位到协议映射的快速查找
    point_to_mapping: Arc<RwLock<HashMap<u32, MappingInfo>>>,
    
    // 地址到点位的反向映射（用于协议响应处理）
    address_to_points: Arc<RwLock<HashMap<AddressKey, Vec<u32>>>>,
}

struct ProtocolMappings {
    modbus_mappings: HashMap<u32, ModbusMapping>,
    can_mappings: HashMap<u32, CanMapping>,
    iec_mappings: HashMap<u32, IecMapping>,
}

struct MappingInfo {
    protocol_type: ProtocolType,
    mapping_data: serde_json::Value,
}

#[derive(Hash, Eq, PartialEq)]
struct AddressKey {
    protocol: ProtocolType,
    address: String,  // 协议特定的地址表示
}
```

## 性能优化关键点

### 1. 使用数值类型作为 Key
- 将 String ID 改为 u32，减少内存使用和比较开销
- 内存节省：约 30-40%
- 查询性能：提升 2-3 倍

### 2. 使用 HashSet 替代 Vec
- 集合操作从 O(n) 优化到 O(1)
- 特别适用于：权限检查、类型分组、启用状态检查

### 3. 多级索引结构
- 支持多维度查询而不需要遍历
- 空间换时间的经典优化

### 4. 缓存策略
- 本地缓存热点数据
- 统计信息缓存，避免实时计算
- LRU 缓存淘汰策略

### 5. 批量操作
- Redis 批量读写
- 减少网络往返次数
- 减少锁竞争

### 6. 并发优化
- 使用 DashMap 提供细粒度锁
- 读写分离的数据结构
- 无锁的统计信息更新

## 实施计划

### 第一阶段：迁移到新配置架构
1. 删除旧的 config_manager.rs 中的重复定义
2. 统一使用 config/types 中的新架构
3. 修复所有编译错误

### 第二阶段：优化点位管理器
1. 实现 OptimizedPointManager
2. 迁移现有代码使用新接口
3. 添加性能测试

### 第三阶段：优化协议工厂
1. 添加多级索引
2. 实现统计缓存
3. 优化查询接口

### 第四阶段：优化 Redis 存储
1. 实现本地缓存层
2. 添加批量操作
3. 使用 SCAN 替代 KEYS

### 第五阶段：优化命令管理
1. 实现优先级队列
2. 添加命令执行器池
3. 实现结果缓存

## 预期效果

1. **查询性能**：提升 5-10 倍
2. **内存使用**：减少 30-40%
3. **并发能力**：支持 10x 更高的并发
4. **响应延迟**：降低 50-70%
5. **Redis 负载**：减少 60-80%