# ComSrv 整体架构设计

## 1. 概述

ComSrv（Communication Service）是 VoltageEMS 系统的核心通信服务，负责与各种工业设备进行数据交换。本文档描述了 v2.0 版本的架构设计，主要变更包括移除 Transport 层、实现统一的重连机制，以及简化配置结构。

## 2. 设计理念

### 2.1 核心原则

1. **简洁性优于灵活性**
   - 移除不必要的抽象层（Transport）
   - 每个协议直接管理自己的物理连接

2. **协议独立性**
   - 每个协议插件是独立的模块
   - 协议特定的连接管理逻辑内置于插件中

3. **可靠性优先**
   - 统一的重连机制
   - 优雅的错误处理和降级

4. **配置简化**
   - 四遥配置与协议配置分离
   - 层次化的配置结构

### 2.2 架构演进

#### v1.0 架构（旧）
```
Protocol Layer → Transport Layer → Physical Layer
```

#### v2.0 架构（新）
```
Protocol Plugin → Physical Connection (内置)
```

## 3. 系统架构

### 3.1 整体架构图

```
┌─────────────────────────────────────────────────────────────┐
│                         ComSrv                              │
├─────────────────────────────────────────────────────────────┤
│                      API Layer                              │
│                  (REST API - Axum)                          │
├─────────────────────────────────────────────────────────────┤
│                    Service Layer                            │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐          │
│  │  Lifecycle  │ │  Reconnect  │ │ Maintenance │          │
│  │  Manager    │ │   Helper    │ │    Tasks    │          │
│  └─────────────┘ └─────────────┘ └─────────────┘          │
├─────────────────────────────────────────────────────────────┤
│                     Core Layer                              │
│  ┌─────────────────────┐ ┌─────────────────────┐          │
│  │      ComBase        │ │   Config Manager    │          │
│  │  (Protocol Trait)   │ │  (YAML + CSV)       │          │
│  └─────────────────────┘ └─────────────────────┘          │
├─────────────────────────────────────────────────────────────┤
│                   Plugin Layer                              │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐     │
│  │ Modbus   │ │ Modbus   │ │  IEC104  │ │   CAN    │     │
│  │   TCP    │ │   RTU    │ │          │ │          │     │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘     │
├─────────────────────────────────────────────────────────────┤
│                   Storage Layer                             │
│                  (Redis Client)                             │
└─────────────────────────────────────────────────────────────┘
```

### 3.2 模块职责

#### 3.2.1 API Layer
- **职责**：提供 RESTful API 接口
- **技术**：Axum web 框架
- **功能**：
  - 通道管理（启动/停止/状态查询）
  - 数据读写
  - 健康检查

#### 3.2.2 Service Layer
- **Lifecycle Manager**：服务生命周期管理
  - 服务启动/停止
  - 通道初始化
  - 优雅关闭
  
- **Reconnect Helper**：通用重连逻辑
  - 指数退避算法
  - 最大重试次数控制
  - 重连状态跟踪
  
- **Maintenance Tasks**：后台维护任务
  - 资源清理
  - 统计信息收集
  - 健康监控

#### 3.2.3 Core Layer
- **ComBase Trait**：协议抽象接口
  - 定义所有协议必须实现的方法
  - 四遥数据模型（遥测、遥信、遥控、遥调）
  
- **Config Manager**：配置管理
  - YAML 主配置加载
  - CSV 点表解析
  - 配置验证

#### 3.2.4 Plugin Layer
- **协议插件**：具体协议实现
  - 每个插件独立管理连接
  - 内置协议特定的重连逻辑
  - 支持批量读写优化

#### 3.2.5 Storage Layer
- **Redis Client**：数据存储
  - Hash 结构存储实时数据
  - Pub/Sub 事件通知
  - 命令队列

## 4. 数据流

### 4.1 数据采集流程

```
设备 → 协议插件 → ComBase → Storage → Redis
                     ↓
                  API Layer → 客户端
```

### 4.2 控制命令流程

```
客户端 → API Layer → ComBase → 协议插件 → 设备
           ↑
      Redis Pub/Sub
```

## 5. 协议插件架构

### 5.1 插件结构

```
protocol_plugin/
├── mod.rs          # 模块入口
├── plugin.rs       # ProtocolPlugin trait 实现
├── protocol.rs     # ComBase trait 实现
├── connection.rs   # 连接管理（TCP/Serial/etc）
├── types.rs        # 协议特定类型
└── config.rs       # 协议特定配置
```

### 5.2 Modbus 插件示例

当前 Modbus 插件已经很好地实现了双模式支持：

```rust
pub enum ModbusConnection {
    Tcp(TcpStream),
    Rtu(SerialStream),
}

// 两个独立的插件入口
pub struct ModbusTcpPlugin;
pub struct ModbusRtuPlugin;

// 共享的核心逻辑
pub struct ModbusCore {
    // 协议处理逻辑
}

pub struct ModbusProtocol {
    core: Arc<Mutex<ModbusCore>>,
    connection_manager: Arc<ModbusConnectionManager>,
    // 实现 ComBase trait
}
```

## 6. 配置架构

### 6.1 配置分层

```
主配置 (comsrv.yaml)
  ├── 服务配置
  ├── 通道列表（仅ID、名称、协议类型）
  └── 四遥表配置路径

协议配置 (protocols/*.yaml)
  └── 协议特定参数（连接参数、超时等）

点表配置 (CSV文件)
  ├── measurement.csv
  ├── signal.csv
  ├── control.csv
  └── adjustment.csv
```

### 6.2 配置示例

```yaml
# comsrv.yaml
service:
  name: "comsrv"
  api:
    port: 8001
  reconnect:
    max_attempts: 3
    initial_delay: 1s

channels:
  - id: 1001
    name: "电表通道"
    protocol: "modbus_tcp"
    table_config:
      route: "channels/1001"
      files:
        measurement: "measurement.csv"
        signal: "signal.csv"
```

## 7. 重连机制

### 7.1 设计目标

- 自动恢复连接
- 避免频繁重试造成的资源浪费
- 提供可配置的重试策略

### 7.2 实现方式

1. **Service 层提供通用 ReconnectHelper**
2. **协议插件可选集成**
3. **支持全局和通道级配置覆盖**

详见 [reconnect-design.md](./reconnect-design.md)

## 8. 优势对比

### 8.1 移除 Transport 层的优势

| 方面 | v1.0 (有Transport层) | v2.0 (无Transport层) |
|------|---------------------|---------------------|
| 代码复杂度 | 高（多层抽象） | 低（直接实现） |
| 维护成本 | 高（需要维护统一接口） | 低（各协议独立） |
| 性能 | 有额外开销 | 无额外开销 |
| 灵活性 | 理论上更灵活 | 实际够用 |

### 8.2 实际案例

- **Modbus TCP/RTU**：已在同一插件中实现双模式
- **IEC104**：只需要 TCP，不需要抽象
- **CAN**：只需要 CAN 硬件，不需要抽象

## 9. 扩展性考虑

### 9.1 添加新协议

1. 创建新的协议插件目录
2. 实现 `ProtocolPlugin` trait
3. 实现 `ComBase` trait
4. 注册到插件管理器

### 9.2 协议变体处理

- 方案1：独立插件（如 ModbusTcp 和 ModbusRtu）
- 方案2：单一插件多模式（如当前 Modbus 实现）

## 10. 总结

新架构通过移除不必要的 Transport 抽象层，使系统更加简洁和易于维护。每个协议插件直接管理自己的物理连接，既保持了足够的灵活性，又避免了过度设计。统一的重连机制和简化的配置结构进一步提升了系统的可靠性和易用性。