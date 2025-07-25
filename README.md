# VoltageEMS - 工业物联网能源管理系统

高性能的微服务架构工业物联网能源管理系统，基于Rust构建，支持多种工业协议和实时数据处理。

## 🚀 核心特性

### 微服务架构
- **API Gateway** - 统一API网关，智能数据路由和缓存
- **comsrv** - 工业协议网关（Modbus、CAN、IEC60870）
- **modsrv** - 计算引擎，基于DAG的实时计算和物模型系统
- **hissrv** - 历史数据服务，时序数据管理
- **netsrv** - 云端网关，数据转发和同步
- **alarmsrv** - 告警管理，实时告警检测和处理
- **rulesrv** - 规则引擎，自动化控制逻辑

### 数据存储架构
- **Redis** - 高性能实时数据存储和消息总线
- **InfluxDB** - 时序数据库，历史数据存储和查询
- **Hash结构存储** - O(1)性能，支持百万级点位
- **标准化数据格式** - 6位小数精度，统一数据类型

### 现代前端应用
- **Web Frontend** - Vue 3响应式Web界面
- **Desktop App** - Tauri跨平台桌面应用
- **Config UI** - 系统配置管理界面

## 🏗️ 架构设计

### 系统架构
```
┌─────────────────────────────────────────────────────────────────────────┐
│                           前端应用层                                      │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐   │
│  │   Web UI    │  │  Mobile App │  │   HMI/SCADA │  │  Third Party│   │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘   │
└─────────┼─────────────────┼─────────────────┼─────────────────┼─────────┘
          └─────────────────┴─────────────────┴─────────────────┘
                                    │
                         ┌──────────┴──────────┐
                         │    API Gateway      │
                         │   (RESTful/WS)      │
                         └──────────┬──────────┘
                                    │
     ┌──────────────────────────────┼──────────────────────────────┐
     │                         Redis 消息总线                        │
     │  ┌────────────────────────────────────────────────────────┐ │
     │  │  Pub/Sub Channels  │  Key-Value Storage  │  Streams   │ │
     │  └────────────────────────────────────────────────────────┘ │
     └─────┬───────────┬───────────┬───────────┬───────────┬──────┘
           │           │           │           │           │
    ┌──────┴──────┐ ┌──┴──┐ ┌──┴───┐ ┌─────┴─────┐ ┌──┴──┐ ┌──────┴──────┐
    │   comsrv    │ │modsrv│ │rulesrv│ │  hissrv   │ │netsrv│ │  alarmsrv   │
    │ (通信服务)   │ │(模型) │ │(规则) │ │ (历史存储)  │ │(云端)│ │  (告警服务)  │
    └──────┬──────┘ └─────┘ └──────┘ └───────────┘ └─────┘ └─────────────┘
           │
    ┌──────┴───────────────────────────────────────┐
    │              工业设备层                        │
    │  ┌─────────┐ ┌─────────┐ ┌─────────┐       │
    │  │ Modbus  │ │IEC60870 │ │   CAN   │  ...  │
    │  └─────────┘ └─────────┘ └─────────┘       │
    └─────────────────────────────────────────────┘
```

### 数据流架构
- **实时遥测**: 设备 → comsrv → Redis → modsrv/hissrv/alarmsrv → 前端
- **控制命令**: 前端 → API Gateway → Redis → comsrv → 设备
- **告警事件**: Redis → alarmsrv → 告警队列 → 通知系统

### 智能数据访问层
- 🔥 **实时数据** - 毫秒级Redis直接访问
- ⚡ **配置数据** - Redis缓存+HTTP回源
- 📊 **历史数据** - InfluxDB时序查询
- 📈 **复杂查询** - HTTP服务计算

## 🛠️ 快速开始

### 环境要求
- **Rust** 1.88+
- **Redis** 7.0+ (推荐Redis 8)
- **InfluxDB** 2.x+
- **Node.js** 18+ (前端开发)
- **Docker** (可选，用于部署)

### 本地开发

#### 1. 启动基础设施
```bash
# 启动Redis
docker run -d --name redis-dev -p 6379:6379 redis:7-alpine

# 启动InfluxDB
docker run -d --name influxdb-dev -p 8086:8086 influxdb:2.7-alpine
```

#### 2. 编译和运行服务
```bash
# 工作区级别操作
cargo check --workspace          # 检查编译（推荐）
cargo build --workspace          # 编译所有服务
cargo test --workspace           # 运行所有测试
cargo clippy --all-targets --all-features -- -D warnings  # 代码检查

# 单独服务操作
cd services/apigateway
cargo run                        # 启动API Gateway

cd services/comsrv
cargo run                        # 启动通信服务

# 使用Docker测试环境
cd services/modsrv
docker-compose -f docker-compose.test.yml up -d
```

#### 3. 启动前端应用
```bash
# Web前端
cd apps/web-frontend
npm install && npm run serve

# 桌面应用
cd apps/tauri-desktop
npm install && npm run tauri:dev

# 配置界面
cd apps/config-ui
npm install && npm run dev
```

### 生产部署
```bash
# 构建发布版本
cargo build --release --workspace

# 使用Docker Compose
docker-compose up -d
```

## 📖 核心服务

### API Gateway (Port 8080)
统一API网关，提供智能数据路由和缓存：

**主要功能：**
- 混合数据访问架构（Redis+InfluxDB+HTTP）
- JWT认证和授权
- WebSocket实时数据推送
- 自动降级和故障转移

**关键端点：**
```
GET  /api/channels           # 获取通道列表
GET  /api/historical         # 历史数据查询（InfluxDB）
WS   /ws                     # WebSocket实时数据
POST /auth/login             # 用户认证
```

### comsrv (Port 8001)
工业协议网关，支持多种工业通信协议：

**支持协议：**
- Modbus TCP/RTU
- IEC 60870-5-104
- CAN Bus
- 自定义协议插件

**数据类型：**
- 遥测(YC) - 模拟量测量
- 遥信(YX) - 数字量状态  
- 遥控(YK) - 控制命令
- 遥调(YT) - 模拟量调节

### hissrv (Port 8003)
历史数据服务，负责时序数据存储和查询：

**核心功能：**
- Redis实时数据→InfluxDB时序存储
- 批量写入优化
- 数据保留策略管理
- 历史数据查询API

### modsrv (Port 8002)
计算引擎，实现业务逻辑和数据处理：

**主要特性：**
- DAG计算工作流
- 物模型系统（DeviceModel）
- 实时计算触发
- 设备实例管理
- 内置计算函数（sum、avg、min、max、scale）

## 🗄️ 数据存储设计

### Redis键值规范 (v3.2)
```
# Hash存储结构
comsrv:{channelID}:{type}   # type: m(测量), s(信号), c(控制), a(调节)
modsrv:{modelname}:{type}   # type: measurement, control
alarm:{alarmID}             # 告警主数据
rulesrv:rule:{ruleID}       # 规则定义

# 数据格式标准
- 浮点数值：6位小数精度 (例: "25.123456")
- 点位访问：Hash字段O(1)查询
- 批量操作：支持HGETALL/HMGET

# Pub/Sub通道
comsrv:{channelID}:{type}   # 消息格式: "{pointID}:{value:.6f}"
modsrv:{modelname}:{type}   # 计算结果通知
cmd:{channelID}:control     # 控制命令通道
```

### InfluxDB时序存储
```sql
-- 测量表结构
measurement,channel_id=1001,point_id=10001,type=YC value=123.45,quality=0
```

## 🔧 技术栈

### 后端服务
- **语言**: Rust (Edition 2021)
- **Web框架**: axum (统一使用)
- **异步运行时**: tokio
- **序列化**: serde, serde_json
- **存储**: Redis 7.0+, InfluxDB 2.x
- **通信**: TCP, Serial, CAN, WebSocket
- **配置**: YAML, CSV, Figment

### 前端应用
- **框架**: Vue 3, TypeScript
- **UI库**: Element Plus, TailwindCSS
- **状态管理**: Pinia
- **图表**: ECharts
- **桌面**: Tauri

### 开发工具
- **构建**: Cargo, Vite
- **测试**: cargo test, pytest
- **容器**: Docker, Docker Compose
- **CI/CD**: GitHub Actions
- **代码质量**: cargo fmt, cargo clippy

## 📁 项目结构

```
VoltageEMS/
├── services/              # 微服务
│   ├── apigateway/       # API网关 (axum)
│   ├── comsrv/           # 通信服务
│   ├── modsrv/           # 计算服务
│   ├── hissrv/           # 历史服务
│   ├── netsrv/           # 网络服务
│   ├── alarmsrv/         # 告警服务
│   └── rulesrv/          # 规则服务
├── apps/                 # 前端应用
│   ├── web-frontend/     # Web界面
│   ├── tauri-desktop/    # 桌面应用
│   └── config-ui/        # 配置界面
├── libs/                 # 共享库
│   └── voltage-libs/     # 通用库（types, error, redis）
├── docs/                 # 文档
│   ├── architecture/     # 架构文档
│   └── fixlog/          # 修复日志
├── config/              # 配置文件
└── scripts/             # 部署脚本
```

## 📊 性能特点

- **高并发**: 支持10,000+并发连接
- **低延迟**: 实时数据毫秒级响应（< 100ms）
- **高吞吐**: 单服务10,000+ points/s写入性能
- **高可用**: 自动故障降级和重试
- **可扩展**: 水平扩展和负载均衡友好
- **内存优化**: Hash结构存储，节省30%+内存
- **键数量优化**: 从百万键减少到千级键

## 🤝 开发指南

### 代码规范
```bash
cargo fmt --all                    # 代码格式化
cargo clippy --all-targets --all-features -- -D warnings  # 代码检查
cargo test --workspace             # 运行测试
cargo check --workspace            # 快速编译检查
```

### 开发流程
1. 从 `develop` 分支创建 worktree
2. 开发并测试功能
3. 更新 `docs/fixlog/fixlog_{date}.md`
4. 提交PR到 `develop` 分支
5. Code Review后合并

### 配置管理
- 使用YAML配置文件（基于Figment）
- 支持环境变量覆盖
- CSV文件管理点位映射
- 分层配置合并

### 日志和监控
- 结构化日志输出（tracing）
- 服务级和通道级日志
- Redis性能监控
- 服务健康检查端点

## 📄 相关文档

- [CLAUDE.md](CLAUDE.md) - 开发指南和架构说明
- [系统架构](docs/architecture/system-architecture.md) - 详细架构文档
- [Redis数据结构](docs/architecture/redis-data-structures.md) - v3.2规范
- [数据流架构](docs/architecture/data-flow-architecture.md) - 数据流设计
- [修复日志](docs/fixlog/) - 开发变更记录

## 📝 许可证

版权所有 © 2025 VoltageEMS团队