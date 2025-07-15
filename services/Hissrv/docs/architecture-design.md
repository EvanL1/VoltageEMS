# HisSrv 架构设计文档

## 1. 概述

HisSrv（Historical Service）是 VoltageEMS 系统中负责历史数据管理的核心服务。它作为实时数据到历史存储的桥梁，提供高性能的批量写入、智能的数据保留策略和灵活的查询接口。

### 1.1 设计目标

- **高吞吐量**: 支持每秒 10 万+ 数据点的写入
- **低延迟**: 数据从 Redis 到 InfluxDB 的延迟 < 1 秒
- **高可靠性**: 数据零丢失，支持故障恢复
- **智能管理**: 自动数据生命周期管理和存储优化
- **易扩展**: 支持多种存储后端和灵活的配置

### 1.2 核心特性

- 批量写入优化
- 扁平化存储适配
- 多级数据保留策略
- 动态配置管理
- 数据生命周期管理

## 2. 系统架构

### 2.1 整体架构图

```
┌─────────────────────────────────────────────────────────────────┐
│                         HisSrv Service                          │
├─────────────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌──────────────┐  ┌────────────────────┐    │
│  │   Redis     │  │    REST      │  │   Config Center    │    │
│  │ Subscriber  │  │    API       │  │    Integration     │    │
│  └──────┬──────┘  └──────┬───────┘  └─────────┬──────────┘    │
│         │                 │                     │               │
│  ┌──────▼─────────────────▼────────────────────▼──────────┐    │
│  │                  Write Buffer Layer                     │    │
│  │  ┌─────────┐  ┌──────────┐  ┌───────────────────┐    │    │
│  │  │ Channel │  │  Batch   │  │   Write-Ahead     │    │    │
│  │  │ Buffer  │  │ Manager  │  │      Log          │    │    │
│  │  └─────────┘  └──────────┘  └───────────────────┘    │    │
│  └─────────────────────┬───────────────────────────────────┘    │
│                        │                                         │
│  ┌─────────────────────▼───────────────────────────────────┐    │
│  │                 Processing Layer                         │    │
│  │  ┌───────────┐  ┌─────────────┐  ┌───────────────┐    │    │
│  │  │   Flat    │  │  InfluxDB   │  │   Parallel    │    │    │
│  │  │   Key     │  │   Mapper    │  │  Processor    │    │    │
│  │  │  Parser   │  │             │  │               │    │    │
│  │  └───────────┘  └─────────────┘  └───────────────┘    │    │
│  └─────────────────────┬───────────────────────────────────┘    │
│                        │                                         │
│  ┌─────────────────────▼───────────────────────────────────┐    │
│  │                  Storage Layer                           │    │
│  │  ┌───────────┐  ┌─────────────┐  ┌───────────────┐    │    │
│  │  │ InfluxDB  │  │    Redis    │  │   RocksDB     │    │    │
│  │  │ Storage   │  │   Cache     │  │  Persistent   │    │    │
│  │  └───────────┘  └─────────────┘  └───────────────┘    │    │
│  └──────────────────────────────────────────────────────────┘    │
│                                                                  │
│  ┌──────────────────────────────────────────────────────────┐    │
│  │                 Management Layer                         │    │
│  │  ┌───────────┐  ┌─────────────┐  ┌───────────────┐    │    │
│  │  │ Retention │  │   Config    │  │  Lifecycle    │    │    │
│  │  │  Policy   │  │  Manager    │  │   Manager     │    │    │
│  │  └───────────┘  └─────────────┘  └───────────────┘    │    │
│  └──────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────┘
```

### 2.2 组件说明

#### 2.2.1 数据输入层
- **Redis Subscriber**: 订阅实时数据更新，支持模式匹配订阅
- **REST API**: 提供 HTTP 接口用于数据查询和管理
- **Config Center Integration**: 与配置中心集成，支持动态配置

#### 2.2.2 缓冲层
- **Channel Buffer**: 按通道分离的写入缓冲
- **Batch Manager**: 批量聚合和触发管理
- **Write-Ahead Log**: 数据持久化保证

#### 2.2.3 处理层
- **Flat Key Parser**: 解析扁平化存储键格式
- **InfluxDB Mapper**: 数据格式转换和映射
- **Parallel Processor**: 并行数据处理

#### 2.2.4 存储层
- **InfluxDB Storage**: 主要的时序数据存储
- **Redis Cache**: 热数据缓存
- **RocksDB Persistent**: 故障恢复持久化

#### 2.2.5 管理层
- **Retention Policy Manager**: 数据保留策略管理
- **Config Manager**: 配置管理和热更新
- **Lifecycle Manager**: 数据生命周期管理

## 3. 核心模块设计

### 3.1 批量写入优化

#### 3.1.1 写入缓冲设计

```rust
pub struct WriteBufferConfig {
    // 缓冲区大小配置
    channel_buffer_size: usize,      // 每个通道的缓冲大小
    global_buffer_size: usize,       // 全局缓冲大小
    
    // 批量配置
    min_batch_size: usize,           // 最小批量大小
    max_batch_size: usize,           // 最大批量大小
    batch_timeout: Duration,         // 批量超时时间
    
    // 性能配置
    worker_threads: usize,           // 工作线程数
    max_concurrent_batches: usize,   // 最大并发批次
}
```

#### 3.1.2 触发机制

批量写入触发采用混合机制：

1. **大小触发**: 当缓冲区数据量达到阈值
2. **时间触发**: 定期刷新缓冲区
3. **内存触发**: 内存压力过大时主动刷新
4. **优先级触发**: 高优先级数据立即处理

### 3.2 扁平化存储适配

#### 3.2.1 键格式定义

```
{channel_id}:{type}:{point_id}

示例：
1001:m:10001  # 通道1001的测量点10001
1001:s:20001  # 通道1001的信号点20001
1001:c:30001  # 通道1001的控制点30001
1001:a:40001  # 通道1001的调节点40001
```

#### 3.2.2 InfluxDB 表设计

```sql
-- 测量值表
CREATE TABLE measurements (
    time TIMESTAMP,
    channel_id TAG,
    point_id TAG,
    value FIELD,
    quality FIELD
)

-- 信号值表
CREATE TABLE signals (
    time TIMESTAMP,
    channel_id TAG,
    point_id TAG,
    value FIELD,
    quality FIELD
)

-- 控制值表
CREATE TABLE controls (
    time TIMESTAMP,
    channel_id TAG,
    point_id TAG,
    value FIELD,
    status FIELD
)

-- 调节值表
CREATE TABLE adjustments (
    time TIMESTAMP,
    channel_id TAG,
    point_id TAG,
    value FIELD,
    target FIELD
)
```

### 3.3 数据保留策略

#### 3.3.1 分级保留

```yaml
retention_policies:
  - name: "raw_data"
    duration: "7d"
    replication: 1
    conditions:
      - all_data: true
      
  - name: "1min_avg"
    duration: "30d"
    replication: 1
    conditions:
      - downsample_from: "raw_data"
      - interval: "1m"
      - functions: ["mean", "max", "min"]
      
  - name: "1hour_avg"
    duration: "365d"
    replication: 1
    conditions:
      - downsample_from: "1min_avg"
      - interval: "1h"
      - functions: ["mean", "max", "min"]
```

#### 3.3.2 智能清理

- 基于访问频率的热度评分
- 基于数据重要性的保留优先级
- 基于存储空间的动态调整

### 3.4 配置管理

#### 3.4.1 配置层次

```yaml
# 全局配置
global:
  service_name: "hissrv"
  version: "1.0.0"

# 运行时配置
runtime:
  batch_size: 1000
  flush_interval: 10s
  worker_threads: 8

# 通道配置
channels:
  - id: 1001
    retention_days: 30
    batch_size: 500
    priority: high
    
  - id: 1002
    retention_days: 7
    batch_size: 1000
    priority: normal
```

#### 3.4.2 动态更新

- 支持不停机配置更新
- 配置版本管理和回滚
- 配置变更审计日志

## 4. 性能优化

### 4.1 写入优化

1. **批量聚合**: 减少网络往返和写入次数
2. **并行处理**: 多线程并发处理不同通道
3. **内存池**: 复用对象减少 GC 压力
4. **零拷贝**: 使用高效的序列化方案

### 4.2 查询优化

1. **索引优化**: 合理设计 InfluxDB 索引
2. **缓存策略**: Redis 缓存热点数据
3. **分区查询**: 按时间和通道分区
4. **预聚合**: 提前计算常用统计值

### 4.3 存储优化

1. **压缩算法**: 使用高效的时序数据压缩
2. **分层存储**: 热温冷数据分离
3. **自动归档**: 老数据自动归档到对象存储
4. **空间回收**: 定期清理过期数据

## 5. 可靠性设计

### 5.1 数据保证

1. **WAL 日志**: 写前日志保证数据不丢失
2. **重试机制**: 失败自动重试
3. **死信队列**: 处理失败数据隔离
4. **数据校验**: CRC 校验保证数据完整性

### 5.2 故障恢复

1. **断点续传**: 支持从故障点恢复
2. **数据重放**: 从 WAL 重放丢失数据
3. **健康检查**: 定期检查服务状态
4. **自动降级**: 故障时自动降级服务

## 6. 监控和运维

### 6.1 监控指标

```rust
pub struct Metrics {
    // 写入指标
    write_throughput: Counter,       // 写入吞吐量
    write_latency: Histogram,        // 写入延迟
    write_errors: Counter,           // 写入错误数
    
    // 查询指标
    query_throughput: Counter,       // 查询吞吐量
    query_latency: Histogram,        // 查询延迟
    query_errors: Counter,           // 查询错误数
    
    // 系统指标
    buffer_usage: Gauge,             // 缓冲区使用率
    storage_usage: Gauge,            // 存储使用率
    connection_pool: Gauge,          // 连接池状态
}
```

### 6.2 运维接口

1. **健康检查**: `/health` 端点
2. **指标导出**: `/metrics` Prometheus 格式
3. **配置查看**: `/config` 当前配置
4. **状态查看**: `/status` 服务状态

## 7. 扩展性设计

### 7.1 存储后端扩展

```rust
pub trait StorageBackend: Send + Sync {
    async fn connect(&mut self) -> Result<()>;
    async fn write_batch(&mut self, points: &[DataPoint]) -> Result<()>;
    async fn query(&self, filter: &QueryFilter) -> Result<Vec<DataPoint>>;
    async fn delete(&mut self, filter: &DeleteFilter) -> Result<u64>;
}
```

### 7.2 插件系统

- 支持自定义数据处理插件
- 支持自定义存储后端
- 支持自定义保留策略

## 8. 部署架构

### 8.1 单机部署

```yaml
version: '3.8'
services:
  hissrv:
    image: voltageems/hissrv:latest
    ports:
      - "8080:8080"
    environment:
      - REDIS_URL=redis://redis:6379
      - INFLUXDB_URL=http://influxdb:8086
    depends_on:
      - redis
      - influxdb
      
  redis:
    image: redis:7-alpine
    
  influxdb:
    image: influxdb:2.7
    environment:
      - INFLUXDB_DB=voltageems
```

### 8.2 集群部署

- 支持水平扩展
- 负载均衡策略
- 数据分片方案

## 9. 未来规划

1. **机器学习集成**: 异常检测和预测
2. **联邦查询**: 跨多个存储后端查询
3. **流式处理**: 实时数据流处理
4. **边缘计算**: 支持边缘节点部署

## 10. 总结

HisSrv 作为 VoltageEMS 的历史数据服务，通过批量写入优化、智能保留策略和灵活的架构设计，实现了高性能、高可靠的历史数据管理。其模块化的设计保证了良好的扩展性和维护性，能够满足工业物联网场景下的各种需求。