# ModSrv v2.0 架构文档

## 概述

ModSrv (Model Service) v2.0 是VoltageEMS工业物联网系统中的设备模型管理服务，负责设备模型定义、实时数据处理和控制命令执行。本文档详细描述了ModSrv的系统架构、设计原则和实现细节。

## 系统架构

### 整体架构图

```
┌────────────────────────────────────────────────────────┐
│                    前端应用层                            │
│          Web UI | Mobile App | SCADA                   │
└─────────────────┬──────────────────────────────────────┘
                  │ HTTP/WebSocket
┌─────────────────┴──────────────────────────────────────┐
│                       ModSrv                           │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐     │
│  │ API Gateway │  │ Model Mgr   │  │ WebSocket   │     │
│  │   (Axum)    │  │ (Core Logic)│  │  (Real-time)│     │
│  └─────┬───────┘  └─────┬───────┘  └─────┬───────┘     │
│        │                │                │             │
│  ┌─────┴────────────────┴────────────────┴──────┐      │
│  │            Mapping Manager                   │      │
│  │        (Logic ↔ Physical Address)            │      │
│  └─────────────────┬────────────────────────────┘      │
└────────────────────┼───────────────────────────────────┘
                     │ Redis Pub/Sub & KV
┌────────────────────┴───────────────────────────────────┐
│                    Redis v3.2                          │
│  Hash: comsrv:{channel}:{type} → {point: value}        │
│  Pub/Sub: comsrv:{channel}:{type}                      │
│  Control: cmd:{channel}:control, cmd:{channel}:adjust  │
└────────────────────┬───────────────────────────────────┘
                     │
┌────────────────────┴────────────────────────────────────┐
│                   ComsRv                                │
│            工业协议通信服务                                │
│    Modbus | IEC60870 | CAN | GPIO | Serial              │
└─────────────────────────────────────────────────────────┘
```

### ModSrv v2.0 内部架构

```
┌─────────────────────────────────────────────────────────┐
│                    ModSrv Service                       │
├─────────────────────────────────────────────────────────┤
│                   API Layer                             │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐     │
│  │    REST     │  │  WebSocket  │  │   Health    │     │
│  │  Endpoints  │  │   Handler   │  │   Check     │     │
│  └─────┬───────┘  └─────┬───────┘  └─────┬───────┘     │
├───────┼─────────────────┼─────────────────┼─────────────┤
│       │                 │                 │             │
│                  Business Logic Layer                   │
│  ┌─────┴───┐  ┌─────────┴─────────┐  ┌────┴─────┐     │
│  │ Model   │  │   Data Stream     │  │ Control  │     │
│  │ Manager │  │   Processor       │  │ Executor │     │
│  └─────┬───┘  └─────────┬─────────┘  └────┬─────┘     │
├───────┼─────────────────┼──────────────────┼───────────┤
│       │                 │                  │           │
│                  Data Access Layer                     │
│  ┌─────┴───┐  ┌─────────┴─────────┐  ┌────┴─────┐     │
│  │ Mapping │  │   Redis Client    │  │ Template │     │
│  │ Manager │  │   (Async/Sync)    │  │ Engine   │     │
│  └─────────┘  └───────────────────┘  └──────────┘     │
├─────────────────────────────────────────────────────────┤
│                 Infrastructure Layer                    │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐     │
│  │ Config Mgr  │  │ Log System  │  │ Error Handler│    │
│  │ (Figment)   │  │ (Tracing)   │  │  (Anyhow)   │     │
│  └─────────────┘  └─────────────┘  └─────────────┘     │
└─────────────────────────────────────────────────────────┘
```

## 设计原则

### 1. 分层架构 (Layered Architecture)

- **API层**: 处理HTTP请求和WebSocket连接
- **业务逻辑层**: 实现核心业务功能
- **数据访问层**: 管理数据存储和访问
- **基础设施层**: 提供通用服务支持

### 2. 监视与控制分离 (Monitoring-Control Separation)

```rust
// v2.0简化模型结构
pub struct Model {
    pub id: String,
    pub name: String,
    pub description: String,
    pub monitoring_config: HashMap<String, PointConfig>,  // 监视点配置
    pub control_config: HashMap<String, PointConfig>,     // 控制点配置
}
```

- **Monitoring**: 只读数据，实时更新
- **Control**: 写操作，需要权限验证

### 3. 映射抽象 (Mapping Abstraction)

逻辑名称与物理地址分离：
```
逻辑模型: voltage_a, main_switch
    ↓ (Mapping Layer)
物理地址: channel:1001, point:10001
```

### 4. 事件驱动 (Event-Driven)

- Redis Pub/Sub订阅数据更新
- WebSocket推送实时数据变化
- 异步处理提高响应性能

## 核心组件详解

### 1. Model Manager (模型管理器)

```rust
pub struct ModelManager {
    models: Arc<RwLock<HashMap<String, Model>>>,
    redis_client: Arc<Mutex<RedisClient>>,
    mappings: Arc<RwLock<MappingManager>>,
}
```

**职责**:
- 模型定义加载和管理
- 遥测数据读取和缓存
- 控制命令执行
- 数据订阅和分发

**关键方法**:
- `load_models_from_config()`: 从配置加载模型
- `get_monitoring_value()`: 获取监视数据
- `execute_control()`: 执行控制命令
- `subscribe_data_updates()`: 订阅数据更新

### 2. Mapping Manager (映射管理器)

```rust
pub struct MappingManager {
    mappings: HashMap<String, ModelMappingConfig>,
}

pub struct PointMapping {
    pub channel: u16,    // 通道ID
    pub point: u32,      // 点位ID
    pub point_type: String, // 类型: m/s/c/a
}
```

**职责**:
- 逻辑点位名称到物理地址的双向映射
- 映射配置的加载和验证
- 点位类型管理

### 3. API Server (API服务器)

基于Axum框架的REST API服务：

```rust
pub struct ApiServer {
    model_manager: Arc<ModelManager>,
    ws_manager: Arc<WsConnectionManager>,
    config: Config,
}
```

**端点设计**:
- `GET /health` - 健康检查
- `GET /models` - 模型列表
- `GET /models/{id}` - 模型详情
- `GET /models/{id}/config` - 模型配置
- `GET /models/{id}/values` - 实时数据
- `POST /models/{id}/control/{name}` - 控制命令
- `WS /ws/models/{id}/values` - WebSocket实时推送

### 4. WebSocket Manager (WebSocket管理器)

```rust
pub struct WsConnectionManager {
    connections: Arc<Mutex<HashMap<String, Vec<WsConnection>>>>,
    model_manager: Arc<ModelManager>,
}
```

**功能**:
- WebSocket连接生命周期管理
- 实时数据推送
- 订阅管理（按模型分组）
- 连接统计和监控

## 数据流架构

### 1. 监视数据流 (Monitoring Data Flow)

```
ComsRv → Redis Hash → ModSrv → WebSocket → Frontend
  │         │           │          │
  │         │           │          └─ 实时推送
  │         │           └─ REST API查询
  │         └─ comsrv:{channel}:{type}
  └─ Pub/Sub通知: {point}:{value}
```

**详细流程**:
1. ComsRv写入数据到Redis Hash: `comsrv:1001:m`
2. ComsRv发布更新通知: channel `comsrv:1001:m`, message `10001:220.5`
3. ModSrv订阅更新通知，解析点位数据
4. ModSrv通过映射找到对应的逻辑点位名称
5. ModSrv推送数据到WebSocket客户端
6. 前端通过REST API查询最新数据

### 2. 控制数据流 (Control Data Flow)

```
Frontend → REST API → ModSrv → Redis Pub/Sub → ComsRv → Device
   │          │         │           │            │        │
   │          │         │           │            │        └─ 物理控制
   │          │         │           │            └─ 协议转换
   │          │         │           └─ cmd:{channel}:control
   │          │         └─ 映射转换 + 权限验证
   │          └─ POST /models/{id}/control/{name}
   └─ JSON: {"value": 1.0}
```

**详细流程**:
1. 前端发送控制命令: `POST /models/power_meter/control/main_switch`
2. ModSrv验证模型和控制点存在性
3. ModSrv通过映射获取物理地址: channel=1001, point=20001
4. ModSrv发布控制命令到Redis: `cmd:1001:control`
5. ComsRv接收控制命令，转换为设备协议
6. 设备执行控制操作

## 性能特性

### 1. 并发处理

- **Tokio异步运行时**: 支持高并发请求处理
- **Arc + RwLock**: 读写分离，支持多读单写
- **连接池**: Redis连接复用
- **WebSocket并发**: 支持大量并发WebSocket连接

### 2. 缓存策略

- **内存缓存**: 模型配置和映射配置
- **Redis缓存**: 实时数据和历史数据
- **懒加载**: 按需加载模型数据
- **TTL管理**: 自动过期失效数据

### 3. 容错机制

- **重试机制**: Redis连接失败自动重试
- **熔断器**: 防止雪崩效应
- **降级策略**: 部分功能失效时的降级处理
- **健康检查**: 定期检查服务状态

## 扩展性设计

### 1. 水平扩展

- **无状态设计**: 服务实例无状态，支持水平扩展
- **负载均衡**: 支持多实例部署
- **分片策略**: 按模型ID分片处理

### 2. 模块化扩展

- **插件架构**: 支持自定义数据处理插件
- **协议扩展**: 支持新的通信协议
- **模板系统**: 支持设备模型模板化

### 3. 监控和运维

- **指标收集**: Prometheus兼容的指标
- **链路追踪**: 支持分布式追踪
- **日志聚合**: 结构化日志输出
- **告警机制**: 异常情况自动告警

## 安全考虑

### 1. 访问控制

- **API认证**: 支持JWT令牌认证
- **权限验证**: 基于角色的访问控制
- **控制权限**: 控制命令需要特殊权限

### 2. 数据安全

- **数据加密**: 敏感数据传输加密
- **审计日志**: 控制操作审计记录
- **输入验证**: 严格的输入参数验证

### 3. 网络安全

- **内网隔离**: Docker内部网络隔离
- **防火墙**: 端口访问控制
- **TLS支持**: HTTPS/WSS安全连接

## 部署架构

### 1. 容器化部署

```yaml
services:
  modsrv:
    image: modsrv:v2.0
    environment:
      - REDIS_URL=redis://redis:6379
      - CONFIG_FILE=/config/config.yml
    volumes:
      - ./config:/config:ro
      - ./logs:/logs
    networks:
      - voltage-ems-network
```

### 2. 服务发现

- **DNS解析**: 基于Docker网络的服务发现
- **健康检查**: 容器健康状态监控
- **负载均衡**: 支持多实例负载均衡

### 3. 数据持久化

- **配置持久化**: 配置文件外部挂载
- **日志持久化**: 日志目录外部挂载
- **Redis数据**: Redis数据卷持久化

## 技术栈

- **核心语言**: Rust 1.88+
- **Web框架**: Axum 0.8.4
- **异步运行时**: Tokio 1.35
- **数据库**: Redis 8.0
- **配置管理**: Figment + Serde
- **日志系统**: Tracing + Tracing-subscriber
- **错误处理**: Anyhow + Thiserror
- **容器化**: Docker + Docker Compose
- **文档**: Markdown + API文档

## 版本演进

### v1.0 → v2.0 重大变更

1. **架构简化**: 从四分类(遥测/遥信/遥控/遥调)简化为二分类(监视/控制)
2. **映射系统**: 引入独立的映射管理器
3. **WebSocket支持**: 新增实时数据推送
4. **性能优化**: 异步处理和缓存优化
5. **容器化**: 完整的Docker化部署方案
