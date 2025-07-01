# ComsRV 通信服务功能说明

## 概述

ComsRV（Communication Service）是VoltageEMS系统的核心通信服务，负责处理与各种工业设备和系统的通信协议，提供统一的数据接口和管理功能。该服务采用现代化的Rust架构，支持多协议、高并发、可扩展的工业通信需求。

## 核心功能

### 1. 多协议支持

#### 支持的协议类型

- **Modbus TCP** - 基于TCP/IP的Modbus通信协议
- **Modbus RTU** - 基于串口的Modbus RTU协议
- **IEC 60870-5-104** - 电力系统远动通信协议（规划中）
- **CAN总线** - 车辆和工业自动化CAN通信（规划中）
- **Virtual Protocol** - 虚拟协议用于测试和仿真

#### 协议工厂模式

- 采用插件化设计，支持运行时协议注册和卸载
- 统一的协议接口 `ComBase`，实现协议无关的上层逻辑
- 支持协议热插拔和动态配置更新

### 2. 统一数据访问接口 ⭐ 新增功能

#### ComBase Trait 统一接口

- **接口标准化**：所有协议使用统一的点表数据访问接口
- **UniversalPointManager集成**：深度集成统一点表管理器，提供缓存和按类型查询
- **四遥类型查询**：支持按遥测(YC)、遥信(YX)、遥控(YK)、遥调(YT)类型精确查询
- **向后兼容**：现有协议无需修改，可选择性迁移到新的统一接口

#### 新增接口方法

```rust
#[async_trait]
pub trait ComBase: Send + Sync {
    /// Get the universal point manager if available
    async fn get_point_manager(&self) -> Option<UniversalPointManager>;
  
    /// Get points by telemetry type using unified point manager
    async fn get_points_by_telemetry_type(&self, telemetry_type: &TelemetryType) -> Vec<PointData>;
  
    /// Get all point configurations using unified point manager
    async fn get_all_point_configs(&self) -> Vec<UniversalPointConfig>;
  
    /// Get enabled points by telemetry type
    async fn get_enabled_points_by_type(&self, telemetry_type: &TelemetryType) -> Vec<String>;
}
```

#### 使用示例

```rust
// 创建带统一点表管理的协议实例
let protocol = ComBaseImpl::new_with_point_manager("Modbus Client", "modbus_tcp", config);

// 加载点表配置
let point_configs = vec![
    UniversalPointConfig::new(1001, "Temperature", TelemetryType::Telemetry),
    UniversalPointConfig::new(2001, "Pump Control", TelemetryType::Control),
];
protocol.load_point_configs(point_configs).await?;

// 统一访问接口
let all_points = protocol.get_all_points().await;
let telemetry_points = protocol.get_points_by_telemetry_type(&TelemetryType::Telemetry).await;
let enabled_controls = protocol.get_enabled_points_by_type(&TelemetryType::Control).await;
```

#### 架构优势

- ✅ **架构统一**：消除各协议重复实现，提高系统一致性
- ✅ **复杂度降低**：协议实现专注于协议逻辑，不需要关心点表管理细节
- ✅ **代码复用**：统一的点表管理逻辑，减少重复代码
- ✅ **扩展性强**：新协议可轻松集成统一的点表管理功能
- ✅ **易于维护**：集中化的点表操作，便于统一优化和维护

### 3. 配置管理

#### 分离式四遥点表架构

```
config/
├── test_points/
│   └── ChannelName/
│       ├── telemetry.csv      # 遥测点表（YC）
│       ├── signal.csv         # 遥信点表（YX）
│       ├── adjustment.csv     # 遥调点表（YT）
│       ├── control.csv        # 遥控点表（YK）
│       ├── mapping_telemetry.csv    # 遥测协议映射
│       ├── mapping_signal.csv       # 遥信协议映射
│       ├── mapping_adjustment.csv   # 遥调协议映射
│       └── mapping_control.csv      # 遥控协议映射
└── main_config.yaml           # 主配置文件
```

#### 配置特性

- YAML格式主配置文件，支持环境变量和模板替换
- CSV格式点表文件，便于批量编辑和导入导出
- 支持配置热重载和实时验证
- 分层配置结构：服务级 -> 通道级 -> 点位级

### 4. 通道管理

#### 通道生命周期

- **创建** - 根据配置动态创建通信通道
- **启动** - 建立连接并开始数据采集
- **监控** - 实时监控通道状态和通信质量
- **停止** - 优雅关闭连接和清理资源
- **更新** - 支持运行时配置更新

#### 并发处理

- 基于Tokio异步运行时，支持高并发通道管理
- 使用DashMap实现无锁并发访问
- 独立的通道任务，故障隔离

### 5. 通道级别独立日志

#### 日志功能

- 每个通道拥有独立的日志目录和文件
- 支持不同日志级别：DEBUG, INFO, WARN, ERROR
- JSON格式日志输出，便于日志分析和监控
- 本地时间戳格式：`YYYY-MM-DD HH:MM:SS.ssssss`

#### 日志配置

```yaml
channels:
  - id: 1
    name: "TestChannel"
    logging:
      enabled: true
      level: "debug"
      log_dir: "logs/{channel_name}"
      max_file_size: 5242880  # 5MB
      max_files: 3
      retention_days: 7
```

#### 日志文件结构

```
logs/
└── ChannelName/
    ├── channel_1.log       # 主日志文件
    ├── channel_1_debug.log # 调试日志文件
    └── ...
```

### 6. REST API服务

#### API端点

- `GET /api/health` - 服务健康检查
- `GET /api/status` - 服务状态信息
- `GET /api/channels` - 获取通道列表
- `GET /api/channels/{id}/points/{type}/{name}` - 读取点位数据
- `POST /api/channels/{id}/points/{type}/{name}` - 写入点位数据
- `GET /api-docs/openapi.json` - OpenAPI规范文档

#### 特性

- 基于Axum框架的高性能HTTP服务
- 支持CORS跨域访问
- JSON格式数据交换
- 集成Swagger UI文档

### 7. 数据存储

#### Redis存储

- 支持Redis集群和单机模式
- 数据持久化和跨服务共享
- 配置热切换，支持存储策略动态调整

```yaml
redis:
  enabled: true
  connection_type: "Tcp"
  address: "127.0.0.1:6379"
  db: 0
```

### 8. 监控和诊断

#### 性能监控

- 实时通道统计信息
- 通信成功率和延迟监控
- 内存使用情况跟踪
- 错误计数和分类

#### 健康检查

- 通道连接状态监控
- 协议层错误检测
- 自动重连机制
- 优雅降级处理

## 架构设计

### 1. 分层架构

```
┌─────────────────────────────────────┐
│           REST API Layer            │  ← HTTP/JSON接口
├─────────────────────────────────────┤
│         Service Layer               │  ← 业务逻辑层
├─────────────────────────────────────┤
│         Protocol Factory            │  ← 协议工厂
├─────────────────────────────────────┤
│    Protocol Implementation Layer    │  ← 具体协议实现
├─────────────────────────────────────┤
│         Storage Layer               │  ← 存储抽象层
└─────────────────────────────────────┘
```

### 2. 核心组件

#### ConfigManager

- 配置文件解析和验证
- 环境变量替换
- 配置热重载支持

#### ProtocolFactory

- 协议注册和管理
- 通道创建和生命周期管理
- 并发安全的通道访问

#### ComBase Trait

```rust
#[async_trait]
pub trait ComBase: Send + Sync {
    async fn start(&mut self) -> Result<()>;
    async fn stop(&mut self) -> Result<()>;
    async fn is_running(&self) -> bool;
    async fn get_all_points(&self) -> Vec<PointData>;
    // ... 更多方法
}
```

### 3. 数据流

```
设备 ←→ Protocol Layer ←→ ProtocolFactory ←→ API Layer ←→ 客户端
                ↓
              Storage Layer (Memory/Redis)
                ↓
              Logging System
```

## 使用指南

### 1. 服务启动

```bash
# 基本启动
./comsrv --config config/comsrv.yaml

# 指定日志级别
./comsrv --config config/comsrv.yaml --log-level debug

# 超级测试模式
./comsrv --config config/comsrv.yaml --super-test --duration 300
```

### 2. 配置示例

#### 主配置文件

```yaml
service:
  name: "comsrv-production"
  description: "Production Communication Service"
  
  api:
    enabled: true
    bind_address: "0.0.0.0:8080"
  
  redis:
    enabled: true
    address: "redis://localhost:6379"
  
  logging:
    level: "info"
    console: true

channels:
  - id: 1
    name: "PLC_Tank_Farm"
    protocol: "modbus_tcp"
    parameters:
      host: "192.168.1.100"
      port: 502
      slave_id: 1
    table_config:
      four_telemetry_route: "config/points/tank_farm"
      protocol_mapping_route: "config/mappings/tank_farm"
```

### 3. API使用

#### 读取遥测数据

```bash
curl http://localhost:8080/api/channels/1/points/telemetry/tank1_level
```

#### 写入遥调数据

```bash
curl -X POST http://localhost:8080/api/channels/1/points/adjustment/setpoint \
  -H "Content-Type: application/json" \
  -d '{"value": 75.5}'
```

## 特性优势

### 1. 高性能

- 基于Rust零开销抽象
- 异步I/O和并发处理
- 内存安全和线程安全

### 2. 可扩展性

- 插件化协议架构
- 水平扩展支持
- 模块化组件设计

### 3. 可靠性

- 优雅错误处理
- 自动重连机制
- 故障隔离设计

### 4. 可维护性

- 清晰的代码结构
- 完整的日志记录
- 标准化配置格式

### 5. 工业级特性

- 分离式四遥架构
- 支持电力行业标准
- 实时性保证

## 部署和运维

### 1. 容器化部署

```dockerfile
FROM rust:alpine
COPY target/release/comsrv /usr/local/bin/
COPY config/ /app/config/
WORKDIR /app
CMD ["comsrv", "--config", "config/comsrv.yaml"]
```

### 2. 监控指标

- 通道连接状态
- 数据采集频率
- 通信错误率
- 系统资源使用率

### 3. 故障排查

- 检查通道日志文件
- 验证配置文件语法
- 测试网络连接
- 查看API健康检查

## 版本信息

- **当前版本**: 0.1.0
- **Rust版本**: 1.70+
- **最低依赖**: tokio 1.0, serde 1.0
- **协议支持**: Modbus TCP/RTU, Virtual Protocol

## 路线图

### 近期计划

- [ ] IEC 60870-5-104协议支持
- [ ] CAN总线协议支持
- [ ] 配置文件加密
- [ ] 高可用部署支持

### 长期规划

- [ ] OPC UA协议支持
- [ ] 边缘计算集成
- [ ] 云原生部署
- [ ] 机器学习数据分析

---

*本文档描述ComsRV v0.1.0的功能特性，如有疑问请参考源码或联系开发团队。*
