# Redis 扁平化存储架构

## 概述

VoltageEMS 采用扁平化的 Redis 键值对存储架构，摒弃了传统的 Hash 嵌套结构，实现了极简、高效的数据存储方案。该架构专门针对工业物联网的高频实时数据场景优化，支持百万级点位的并发读写。

## 设计理念

### 核心原则
1. **扁平化存储**：每个数据点独立存储，无嵌套结构
2. **直接寻址**：通过键名直接定位数据，O(1) 复杂度
3. **原子操作**：单点更新原子性，无并发冲突
4. **极简设计**：最少的抽象层，最直接的访问路径

### 性能优势
- **读取性能**：单点查询 < 0.5ms
- **写入性能**：批量写入 10,000+ points/s
- **内存效率**：每个点位约 100 字节
- **扩展性**：天然支持 Redis Cluster 分片

## 键命名规范

### 基础格式
```
{channel_id}:{type}:{point_id}
```

### 类型映射表

| 四遥类型 | 类型标识 | 说明         | 示例键名        |
|---------|---------|--------------|----------------|
| 遥测(YC) | m       | Measurement  | 1001:m:10001   |
| 遥信(YX) | s       | Signal       | 1001:s:20001   |
| 遥控(YK) | c       | Control      | 1001:c:30001   |
| 遥调(YT) | a       | Adjustment   | 1001:a:40001   |

### 配置数据键
```
cfg:{channel_id}:{type}:{point_id}
```

## 数据格式

### 实时数据值
```
格式：{value}:{timestamp}
示例：25.6:1704956400
```

### 配置数据（JSON）
```json
{
  "name": "主变压器温度",
  "unit": "°C",
  "scale": 0.1,
  "offset": 0,
  "address": "1:3:100",
  "description": "A相绕组温度"
}
```

## 存储实现

### Rust 存储接口
```rust
/// 极简的 Redis 存储实现
pub struct RedisStorage {
    conn: ConnectionManager,
}

impl RedisStorage {
    /// 设置单个点位值
    pub async fn set_point(
        &mut self,
        channel_id: u16,
        point_type: &str,
        point_id: u32,
        value: f64,
    ) -> Result<()>;

    /// 批量设置（Pipeline）
    pub async fn set_points(&mut self, updates: &[PointUpdate]) -> Result<()>;

    /// 获取单个点位值
    pub async fn get_point(
        &mut self,
        channel_id: u16,
        point_type: &str,
        point_id: u32,
    ) -> Result<Option<(f64, i64)>>;

    /// 批量获取（MGET）
    pub async fn get_points(&mut self, keys: &[PointKey]) -> Result<Vec<Option<(f64, i64)>>>;
}
```

### 批量操作优化

#### Pipeline 写入
```rust
let mut pipe = Pipeline::new();
for update in updates {
    let key = format!("{}:{}:{}", update.channel_id, update.point_type, update.point_id);
    let value = format!("{}:{}", update.value, timestamp);
    pipe.set(&key, &value);
}
pipe.query_async(&mut conn).await?;
```

#### MGET 批量读取
```rust
let keys: Vec<String> = points.iter()
    .map(|p| format!("{}:{}:{}", p.channel_id, p.point_type, p.point_id))
    .collect();
let values: Vec<Option<String>> = conn.mget(&keys).await?;
```

## 数据分布策略

### Redis Cluster 分片
- 按 channel_id 自然分片
- 同一通道的数据倾向于在同一节点
- 支持按需增加节点扩容

### 键空间分析
```
通道 1001：约 1000 个点
├── 1001:m:* (遥测点 500 个)
├── 1001:s:* (遥信点 300 个)
├── 1001:c:* (遥控点 100 个)
└── 1001:a:* (遥调点 100 个)
```

## 性能基准测试

### 测试环境
- Redis 7.0 单节点
- 8 核 16GB 内存
- 1000 并发连接

### 测试结果
| 操作类型        | 延迟(P50) | 延迟(P99) | 吞吐量       |
|----------------|-----------|-----------|--------------|
| 单点写入        | 0.3ms     | 0.8ms     | 50,000 ops/s |
| 批量写入(1000)  | 3.2ms     | 5.1ms     | 15,000 ops/s |
| 单点读取        | 0.2ms     | 0.5ms     | 80,000 ops/s |
| 批量读取(100)   | 1.1ms     | 2.3ms     | 30,000 ops/s |

## 内存占用分析

### 单点内存开销
```
键名：~30 字节 (例如 "1001:m:10001")
值：~20 字节 (例如 "380.5:1704956400")
Redis开销：~50 字节
总计：~100 字节/点
```

### 容量规划
| 点位数量  | 内存占用 | 建议配置    |
|----------|---------|------------|
| 10,000   | ~1 MB   | 1GB 内存   |
| 100,000  | ~10 MB  | 2GB 内存   |
| 1,000,000| ~100 MB | 4GB 内存   |

## 数据过期策略

### TTL 设置
```rust
// 实时数据：7天过期
conn.setex(&key, 604800, &value).await?;

// 配置数据：永不过期
conn.set(&config_key, &config_value).await?;
```

### 清理策略
- 使用 Redis 的惰性删除
- 定期任务清理过期数据
- 监控内存使用率

## 监控指标

### Redis 监控
```bash
# 键空间统计
redis-cli --scan --pattern "1001:*" | wc -l

# 内存使用
redis-cli info memory

# 命令统计
redis-cli info commandstats
```

### 应用层监控
- 读写延迟直方图
- 批量操作大小分布
- 错误率和重试次数

## 最佳实践

### 1. 批量操作
- 写入使用 Pipeline，建议批次 1000-5000
- 读取使用 MGET，建议批次 100-500

### 2. 连接池管理
```rust
// 使用连接池
let manager = ConnectionManager::new(client).await?;
```

### 3. 错误处理
- 实现指数退避重试
- 记录详细错误日志
- 监控连接池状态

### 4. 键命名规范
- 保持键名简短
- 使用数字 ID 而非字符串
- 避免特殊字符

## 迁移方案

### 从 Hash 结构迁移
```python
# 迁移脚本示例
for channel_id in channels:
    hash_data = redis.hgetall(f"channel:{channel_id}")
    for field, value in hash_data.items():
        point_type, point_id = parse_field(field)
        new_key = f"{channel_id}:{point_type}:{point_id}"
        redis.set(new_key, value)
```

### 数据一致性保证
1. 双写过渡期
2. 数据校验
3. 逐步切换
4. 回滚方案

## 故障恢复

### Redis 故障
- 主从切换自动进行
- 应用层实现重连逻辑
- 本地缓存临时数据

### 数据恢复
- 从 InfluxDB 恢复历史数据
- 从设备重新采集实时数据
- 配置数据从备份恢复