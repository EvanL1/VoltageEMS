# Comsrv 通信服务开发文档

## 项目概述

Comsrv (Communication Service) 是一个基于 Rust 开发的高性能工业通信服务，主要用于管理和处理多种工业通信协议的数据交换。该服务采用异步架构设计，支持高并发连接和实时数据处理。

### 核心特性

- **多协议支持**: Modbus TCP/RTU、IEC 60870-5-104 等工业通信协议
- **高性能架构**: 基于 Tokio 异步运行时，支持高并发处理
- **灵活配置**: YAML 配置文件，支持热重载
- **实时监控**: 内置指标收集和监控功能
- **RESTful API**: 完整的 HTTP API 接口
- **连接池管理**: 智能连接复用和管理
- **数据存储**: 支持 Redis 实时数据缓存

## 项目架构

### 整体架构图

```
┌─────────────────────────────────────────────────────────────┐
│                        Comsrv 架构                          │
├─────────────────────────────────────────────────────────────┤
│  HTTP API Layer                                            │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐          │
│  │   Health    │ │   Channel   │ │    Point    │          │
│  │    API      │ │     API     │ │     API     │          │
│  └─────────────┘ └─────────────┘ └─────────────┘          │
├─────────────────────────────────────────────────────────────┤
│  Core Service Layer                                        │
│  ┌─────────────────────────────────────────────────────────┐│
│  │              Protocol Factory                           ││
│  │  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐      ││
│  │  │ Modbus  │ │ Modbus  │ │ IEC104  │ │ Virtual │      ││
│  │  │   TCP   │ │   RTU   │ │         │ │         │      ││
│  │  └─────────┘ └─────────┘ └─────────┘ └─────────┘      ││
│  └─────────────────────────────────────────────────────────┘│
├─────────────────────────────────────────────────────────────┤
│  Infrastructure Layer                                      │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐          │
│  │   Config    │ │  Metrics    │ │   Storage   │          │
│  │  Manager    │ │   System    │ │    Layer    │          │
│  └─────────────┘ └─────────────┘ └─────────────┘          │
└─────────────────────────────────────────────────────────────┘
```

### 目录结构

```
services/comsrv/
├── src/
│   ├── main.rs                 # 应用程序入口
│   ├── api/                    # HTTP API 模块
│   │   ├── mod.rs
│   │   ├── handlers.rs         # API 处理函数
│   │   ├── models.rs           # API 数据模型
│   │   └── routes.rs           # 路由定义
│   ├── core/                   # 核心业务模块
│   │   ├── mod.rs
│   │   ├── config/             # 配置管理
│   │   ├── protocols/          # 通信协议实现
│   │   ├── protocol_factory.rs # 协议工厂
│   │   ├── metrics.rs          # 监控指标
│   │   ├── connection_pool.rs  # 连接池
│   │   └── storage/            # 数据存储
│   └── utils/                  # 工具模块
│       ├── mod.rs
│       ├── error.rs            # 错误处理
│       ├── logger.rs           # 日志系统
│       └── pool.rs             # 对象池
├── config/                     # 配置文件
├── docs/                       # 文档
└── Cargo.toml                 # 项目依赖
```

## 核心模块详解

### 1. Protocol Factory 模块

**文件位置**: `src/core/protocol_factory.rs`

Protocol Factory 是系统的核心模块，负责管理所有通信协议实例的生命周期。

#### 主要功能

- **协议实例创建**: 根据配置动态创建不同类型的协议客户端
- **通道管理**: 管理所有活跃的通信通道
- **并发控制**: 使用 `DashMap` 和 `Arc<RwLock>` 实现线程安全的并发访问
- **生命周期管理**: 支持通道的启动、停止、重启操作
- **资源清理**: 自动清理空闲和过期的连接

#### 核心数据结构

```rust
pub struct ProtocolFactory {
    /// 存储创建的通道，使用 DashMap 支持并发访问
    channels: DashMap<u16, Arc<RwLock<Box<dyn ComBase>>>, ahash::RandomState>,
    /// 通道元数据缓存
    channel_metadata: DashMap<u16, ChannelMetadata, ahash::RandomState>,
}

struct ChannelMetadata {
    pub name: String,
    pub protocol_type: ProtocolType,
    pub created_at: std::time::Instant,
    pub last_accessed: Arc<RwLock<std::time::Instant>>,
}
```

#### 关键方法

- `create_protocol()`: 创建协议实例
- `create_channel()`: 创建并注册通道
- `start_all_channels()`: 并行启动所有通道
- `stop_all_channels()`: 并行停止所有通道
- `get_channel()`: 获取指定通道
- `cleanup_channels()`: 清理空闲通道

### 2. 通信协议模块

**文件位置**: `src/core/protocols/`

#### 协议抽象接口 (ComBase)

所有通信协议都实现 `ComBase` trait，提供统一的接口：

```rust
pub trait ComBase: Send + Sync {
    // 基础操作
    async fn start(&mut self) -> Result<()>;
    async fn stop(&mut self) -> Result<()>;
    async fn is_running(&self) -> bool;
  
    // 数据操作
    async fn read_points(&self, point_ids: &[String]) -> Result<Vec<PointData>>;
    async fn write_point(&self, point_id: &str, value: &serde_json::Value) -> Result<()>;
    async fn get_all_points(&self) -> Result<Vec<PointData>>;
  
    // 状态查询
    async fn status(&self) -> ChannelStatus;
    fn name(&self) -> &str;
    fn protocol_type(&self) -> &str;
    fn get_parameters(&self) -> HashMap<String, String>;
}
```

#### 支持的协议

1. **Modbus TCP** (`src/core/protocols/modbus/tcp.rs`)

   - 基于 TCP 的 Modbus 通信
   - 支持线圈、离散输入、保持寄存器、输入寄存器
   - 连接池管理和自动重连
2. **Modbus RTU** (`src/core/protocols/modbus/rtu.rs`)

   - 基于串口的 Modbus 通信
   - 支持 CRC 校验
   - 超时和重试机制
3. **IEC 60870-5-104** (`src/core/protocols/iec60870/iec104.rs`)

   - 标准的电力系统通信协议
   - 支持 ASDU 数据格式
   - 序列号管理和确认机制

### 3. API 模块

**文件位置**: `src/api/`

#### API 架构

API 模块基于 Warp 框架构建，提供 RESTful 风格的接口：

```rust
// 主要的 API 端点
GET  /health                     # 健康检查
GET  /status                     # 服务状态
GET  /channels                   # 获取所有通道
GET  /channels/{id}              # 获取指定通道状态
POST /channels/{id}/control      # 控制通道操作
GET  /channels/{id}/points       # 获取通道所有点位
GET  /channels/{id}/points/{name} # 读取指定点位
PUT  /channels/{id}/points/{name} # 写入指定点位
```

#### 数据模型

API 使用标准化的数据模型进行数据交换：

```rust
#[derive(Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

#[derive(Serialize)]
pub struct ChannelStatus {
    pub id: String,
    pub name: String,
    pub protocol: String,
    pub connected: bool,
    pub last_response_time: f64,
    pub last_error: String,
    pub last_update_time: DateTime<Utc>,
    pub parameters: HashMap<String, serde_json::Value>,
}
```

### 4. 配置管理模块

**文件位置**: `src/core/config/`

#### 配置结构

系统使用 YAML 格式的配置文件，支持分层配置：

```yaml
service:
  name: "comsrv"
  version: "1.0.0"
  log_level: "info"

api:
  enabled: true
  address: "0.0.0.0:3000"
  cors_enabled: true

metrics:
  enabled: true
  address: "0.0.0.0:9090"

channels:
  - id: 1
    name: "PLC-001"
    protocol: "modbus_tcp"
    parameters:
      host: "192.168.1.100"
      port: 502
      unit_id: 1
    register_mappings:
      - name: "temperature"
        register_type: "input_register"
        address: 1000
        data_type: "float32"
```

#### 配置管理器

```rust
pub struct ConfigManager {
    config: Arc<RwLock<Config>>,
    config_path: String,
}

impl ConfigManager {
    pub fn from_file(path: &str) -> Result<Self>;
    pub fn reload_config(&mut self) -> Result<bool>;
    pub fn get_channels(&self) -> Vec<ChannelConfig>;
    pub fn get_api_config(&self) -> &ApiConfig;
}
```

### 5. 监控和指标模块

**文件位置**: `src/core/metrics.rs`

#### 指标收集

系统集成 Prometheus 指标收集，监控关键性能指标：

- **通道状态指标**: 连接状态、响应时间、错误率
- **数据吞吐指标**: 读写操作数量、数据包统计
- **系统性能指标**: 内存使用、CPU 使用率
- **业务指标**: 活跃连接数、协议分布

```rust
pub struct Metrics {
    // 通道相关指标
    channel_status: GaugeVec,
    channel_response_time: HistogramVec,
    channel_errors: CounterVec,
  
    // 数据操作指标
    data_operations: CounterVec,
    data_bytes: CounterVec,
  
    // 系统指标
    active_connections: Gauge,
    uptime: Gauge,
}
```

## 开发流程

### 1. 环境准备

#### 系统要求

- Rust 1.70+
- 操作系统: Linux/macOS/Windows
- 内存: 最少 2GB
- 磁盘: 最少 1GB 可用空间

#### 依赖安装

```bash
# 安装 Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 克隆项目
git clone <repository-url>
cd services/comsrv

# 构建项目
cargo build --release
```

### 2. 添加新协议支持

#### 步骤 1: 创建协议实现

在 `src/core/protocols/` 下创建新的协议模块：

```rust
// src/core/protocols/your_protocol/mod.rs
use crate::core::protocols::common::ComBase;
use crate::utils::Result;

pub struct YourProtocolClient {
    config: ChannelConfig,
    // 其他字段
}

#[async_trait]
impl ComBase for YourProtocolClient {
    async fn start(&mut self) -> Result<()> {
        // 实现启动逻辑
    }
  
    async fn stop(&mut self) -> Result<()> {
        // 实现停止逻辑
    }
  
    // 实现其他必要方法
}
```

#### 步骤 2: 注册协议类型

在 `src/core/config/config_manager.rs` 中添加协议类型：

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ProtocolType {
    ModbusTcp,
    ModbusRtu,
    Iec104,
    YourProtocol, // 添加新协议
    Virtual,
}
```

#### 步骤 3: 更新协议工厂

在 `src/core/protocol_factory.rs` 中添加创建逻辑：

```rust
pub fn create_protocol(&self, config: ChannelConfig) -> Result<Box<dyn ComBase>> {
    match config.protocol {
        ProtocolType::YourProtocol => self.create_your_protocol(config),
        // 其他协议...
    }
}

fn create_your_protocol(&self, config: ChannelConfig) -> Result<Box<dyn ComBase>> {
    let client = YourProtocolClient::new(config);
    Ok(Box::new(client))
}
```

### 3. 添加新 API 端点

#### 步骤 1: 定义数据模型

在 `src/api/models.rs` 中添加新的数据结构：

```rust
#[derive(Serialize, Deserialize)]
pub struct YourApiRequest {
    pub field1: String,
    pub field2: i32,
}

#[derive(Serialize)]
pub struct YourApiResponse {
    pub result: String,
    pub timestamp: DateTime<Utc>,
}
```

#### 步骤 2: 实现处理函数

在 `src/api/handlers.rs` 中添加处理逻辑：

```rust
pub async fn your_handler(
    request: YourApiRequest,
    protocol_factory: Arc<RwLock<ProtocolFactory>>,
) -> Result<impl Reply, Rejection> {
    // 实现业务逻辑
    let response = YourApiResponse {
        result: "success".to_string(),
        timestamp: Utc::now(),
    };
  
    Ok(warp::reply::json(&ApiResponse::success(response)))
}
```

#### 步骤 3: 注册路由

在 `src/api/routes.rs` 中添加路由：

```rust
pub fn api_routes(
    factory: Arc<RwLock<ProtocolFactory>>,
    start_time: Arc<DateTime<Utc>>,
) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
    let your_route = warp::path!("your" / "endpoint")
        .and(warp::post())
        .and(warp::body::json())
        .and(with_factory(factory.clone()))
        .and_then(handlers::your_handler);
  
    // 组合所有路由
    your_route.or(other_routes)
}
```

### 4. 测试开发

#### 单元测试

```rust
#[cfg(test)]
mod tests {
    use super::*;
  
    #[tokio::test]
    async fn test_your_protocol() {
        let config = ChannelConfig {
            id: 1,
            name: "test".to_string(),
            protocol: ProtocolType::YourProtocol,
            parameters: HashMap::new(),
        };
      
        let client = YourProtocolClient::new(config);
        assert!(client.start().await.is_ok());
    }
}
```

#### 集成测试

```bash
# 运行所有测试
cargo test

# 运行特定测试
cargo test test_your_protocol

# 运行集成测试
cargo test --test integration
```

## API 文档

### 认证

目前系统不使用认证机制，所有 API 端点都是开放的。生产环境建议添加适当的认证和授权机制。

### 通用响应格式

所有 API 响应都使用统一的格式：

```json
{
  "success": true,
  "data": {},
  "error": null
}
```

### 端点详情

#### 1. 健康检查

```http
GET /health
```

**响应示例:**

```json
{
  "success": true,
  "data": {
    "status": "OK",
    "uptime": 3600,
    "memory_usage": 0,
    "cpu_usage": 0.0
  },
  "error": null
}
```

#### 2. 服务状态

```http
GET /status
```

**响应示例:**

```json
{
  "success": true,
  "data": {
    "name": "ComsrvRust",
    "version": "0.1.0",
    "uptime": 3600,
    "start_time": "2024-01-01T00:00:00Z",
    "channels": 5,
    "active_channels": 3
  },
  "error": null
}
```

#### 3. 获取所有通道

```http
GET /channels
```

**响应示例:**

```json
{
  "success": true,
  "data": [
    {
      "id": "1",
      "name": "PLC-001",
      "protocol": "modbus_tcp",
      "connected": true,
      "last_response_time": 15.5,
      "last_error": "",
      "last_update_time": "2024-01-01T12:00:00Z",
      "parameters": {
        "host": "192.168.1.100",
        "port": 502
      }
    }
  ],
  "error": null
}
```

#### 4. 控制通道

```http
POST /channels/{id}/control
Content-Type: application/json

{
  "operation": "start"
}
```

**操作类型:**

- `start`: 启动通道
- `stop`: 停止通道
- `restart`: 重启通道

#### 5. 读取点位数据

```http
GET /channels/{id}/points/{name}
```

**响应示例:**

```json
{
  "success": true,
  "data": {
    "name": "temperature",
    "value": 25.6,
    "quality": true,
    "timestamp": "2024-01-01T12:00:00Z"
  },
  "error": null
}
```

#### 6. 写入点位数据

```http
PUT /channels/{id}/points/{name}
Content-Type: application/json

{
  "value": 30.0
}
```

## 配置说明

### 主配置文件

配置文件位于 `config/comsrv.yaml`，包含以下主要部分：

#### 服务配置

```yaml
service:
  name: "comsrv"                    # 服务名称
  version: "1.0.0"                  # 服务版本
  log_level: "info"                 # 日志级别: trace/debug/info/warn/error
  log_file: "logs/comsrv.log"       # 日志文件路径
```

#### API 配置

```yaml
api:
  enabled: true                     # 是否启用 API 服务
  address: "0.0.0.0:3000"          # API 监听地址
  cors_enabled: true                # 是否启用 CORS
  request_timeout: 30               # 请求超时时间(秒)
```

#### 指标配置

```yaml
metrics:
  enabled: true                     # 是否启用指标收集
  address: "0.0.0.0:9090"          # 指标服务地址
  collection_interval: 10          # 指标收集间隔(秒)
```

#### 通道配置

```yaml
channels:
  - id: 1                          # 通道唯一标识
    name: "PLC-001"                # 通道名称
    protocol: "modbus_tcp"         # 协议类型
    enabled: true                  # 是否启用
    parameters:                    # 协议参数
      host: "192.168.1.100"
      port: 502
      unit_id: 1
      timeout: 5000
      retry_count: 3
    register_mappings:             # 寄存器映射
      - name: "temperature"
        register_type: "input_register"
        address: 1000
        data_type: "float32"
        scale: 0.1
        offset: 0
```

#### Redis 配置 (可选)

```yaml
redis:
  enabled: true                    # 是否启用 Redis
  host: "127.0.0.1"               # Redis 主机
  port: 6379                      # Redis 端口
  database: 0                     # 数据库编号
  password: ""                    # 密码(可选)
  pool_size: 10                   # 连接池大小
```

## 部署指南

### 1. 编译部署

#### 发布构建

```bash
# 优化构建
cargo build --release

# 可执行文件位于
./target/release/comsrv
```

#### 交叉编译

```bash
# 安装交叉编译工具
rustup target add x86_64-unknown-linux-gnu

# 交叉编译
cargo build --release --target x86_64-unknown-linux-gnu
```

### 2. Docker 部署

#### Dockerfile

```dockerfile
FROM rust:1.70 as builder
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bullseye-slim
WORKDIR /app
COPY --from=builder /app/target/release/comsrv .
COPY --from=builder /app/config ./config
EXPOSE 3000 9090
CMD ["./comsrv"]
```

#### Docker Compose

```yaml
version: '3.8'
services:
  comsrv:
    build: .
    ports:
      - "3000:3000"   # API 端口
      - "9090:9090"   # 指标端口
    volumes:
      - ./config:/app/config
      - ./logs:/app/logs
    environment:
      - RUST_LOG=info
      - CONFIG_FILE=config/comsrv.yaml
    restart: unless-stopped
  
  redis:
    image: redis:7-alpine
    ports:
      - "6379:6379"
    volumes:
      - redis_data:/data
    restart: unless-stopped

volumes:
  redis_data:
```

### 3. 系统服务

#### Systemd 服务文件

```ini
# /etc/systemd/system/comsrv.service
[Unit]
Description=Communication Service
After=network.target

[Service]
Type=simple
User=comsrv
Group=comsrv
WorkingDirectory=/opt/comsrv
ExecStart=/opt/comsrv/bin/comsrv
Environment=CONFIG_FILE=/opt/comsrv/config/comsrv.yaml
Environment=LOG_DIR=/opt/comsrv/logs
Restart=always
RestartSec=10

[Install]
WantedBy=multi-user.target
```

#### 管理命令

```bash
# 启用服务
sudo systemctl enable comsrv

# 启动服务
sudo systemctl start comsrv

# 查看状态
sudo systemctl status comsrv

# 查看日志
sudo journalctl -u comsrv -f
```

### 4. 监控部署

#### Prometheus 配置

```yaml
# prometheus.yml
scrape_configs:
  - job_name: 'comsrv'
    static_configs:
      - targets: ['localhost:9090']
    scrape_interval: 15s
    metrics_path: /metrics
```

#### Grafana 仪表板

推荐监控指标：

- 通道连接状态
- 数据读写速率
- 响应时间分布
- 错误率统计
- 系统资源使用

## 性能优化

### 1. 内存优化

- 使用对象池减少内存分配
- 配置合理的连接池大小
- 定期清理空闲连接

### 2. 并发优化

- 使用 `DashMap` 实现无锁并发访问
- 合理设置 Tokio 运行时参数
- 避免长时间持有锁

### 3. 网络优化

- 启用 TCP keepalive
- 配置合理的超时参数
- 使用连接复用

### 4. 配置建议

```yaml
# 高性能配置示例
service:
  worker_threads: 8              # 工作线程数
  max_blocking_threads: 512      # 最大阻塞线程数

api:
  max_connections: 1000          # 最大连接数
  connection_timeout: 30         # 连接超时

channels:
  max_concurrent_requests: 100   # 最大并发请求
  connection_pool_size: 20       # 连接池大小
```

## 故障排除

### 常见问题

1. **配置文件格式错误**

   - 检查 YAML 语法
   - 验证字段名称和类型
2. **通道连接失败**

   - 检查网络连通性
   - 验证协议参数
   - 查看防火墙设置
3. **性能问题**

   - 监控 CPU 和内存使用
   - 检查连接池配置
   - 分析慢查询日志

### 调试命令

```bash
# 查看详细日志
RUST_LOG=debug ./comsrv

# 性能分析
cargo flamegraph --bin comsrv

# 内存使用分析
valgrind --tool=massif ./target/release/comsrv
```

## 开发规范

### 代码规范

- 使用 `rustfmt` 格式化代码
- 遵循 Rust 命名约定
- 编写完整的文档注释
- 实现必要的错误处理

### 提交规范

```
feat(modsrv): add new protocol support
fix(api): resolve connection timeout issue
docs(readme): update installation guide
test(core): add unit tests for factory
```

### 版本管理

- 使用语义化版本号
- 维护 CHANGELOG.md
- 标记重要版本

## 总结

Comsrv 是一个功能完整、性能优异的工业通信服务。通过模块化设计和异步架构，它能够有效处理多种工业协议的数据交换需求。本文档涵盖了从开发到部署的完整流程，为开发者提供了详细的指导。

如有问题或建议，请通过 Issue 或 Pull Request 与我们交流。
