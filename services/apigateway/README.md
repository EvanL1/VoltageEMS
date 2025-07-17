# VoltageEMS API Gateway

高性能的统一API网关，为VoltageEMS工业物联网能源管理系统提供智能数据访问和服务路由。

## 🚀 核心特性

### 混合数据访问架构
- **智能路由** - 根据数据类型自动选择最优访问策略
- **分层缓存** - 本地LRU缓存 + Redis缓存的二级架构  
- **HTTP回源** - 配置数据的智能降级和一致性保证
- **批量优化** - 并发批量操作提升性能

### 数据访问策略
- 🔥 **实时数据** (`RedisOnly`) - 毫秒级响应，直接Redis访问
- ⚡ **配置数据** (`RedisWithHttpFallback`) - 缓存优先，HTTP回源保证一致性
- 📊 **历史数据** (`InfluxDBQuery`) - 时间序列数据，InfluxDB直接查询
- 📈 **复杂查询** (`HttpOnly`) - 统计报表、分析计算

### 现代Web架构
- **axum框架** - 高性能异步Web服务器
- **JWT认证** - 安全的用户身份验证和授权
- **WebSocket实时推送** - 实时数据流和告警通知
- **CORS支持** - 完整的跨域资源共享配置

## 📋 支持的数据类型

### 实时数据 (Redis直接访问)
```
{channelID}:m:{pointID}    # 测量值 (遥测YC)
{channelID}:s:{pointID}    # 状态值 (遥信YX)  
{channelID}:c:{pointID}    # 控制状态 (遥控YK)
{channelID}:a:{pointID}    # 调节值 (遥调YT)
```

### 配置数据 (Redis缓存+HTTP回源)
```
cfg:channel:{channelID}    # 通道配置
cfg:module:{moduleName}    # 模块配置  
cfg:service:{serviceName} # 服务配置
model:def:{modelName}      # 设备模型定义
alarm:config:{ruleID}      # 告警规则配置
```

### 历史数据查询 (InfluxDB直接访问)
```
his:index:{channelID}:{date}  # 历史数据索引
his:query:{queryID}           # 查询结果缓存
his:stats:{channelID}:{date}  # 历史统计缓存
```

### 复杂查询 (HTTP服务访问)
```
stats:{type}:{id}         # 统计数据分析
report:{type}:{id}        # 报表生成
analytics:{type}:{id}     # 数据分析
```

## 🛠️ 快速开始

### 环境要求
- Rust 1.70+
- Redis 7.0+
- InfluxDB 2.x+ (历史数据存储)
- 后端服务 (comsrv, modsrv, hissrv, netsrv, alarmsrv, rulesrv)

### 本地开发
```bash
# 启动Redis
docker run -d --name redis-dev -p 6379:6379 redis:7-alpine

# 启动InfluxDB (历史数据存储)
docker run -d --name influxdb-dev -p 8086:8086 influxdb:2.7-alpine

# 开发模式运行
RUST_LOG=debug cargo run

# 指定配置文件
cargo run -- --config config/apigateway-test.yaml
```

### 生产部署
```bash
# 编译发布版本
cargo build --release

# 运行
./target/release/apigateway
```

## 📖 API文档

### 认证端点
```
POST /auth/login           # 用户登录
POST /auth/refresh         # 刷新Token
POST /auth/logout          # 用户登出
GET  /auth/me              # 获取当前用户信息
```

### 数据访问端点
```
GET  /api/channels                    # 获取通道列表
GET  /api/channels/{id}               # 获取通道详情
GET  /api/channels/{id}/telemetry     # 获取遥测数据
GET  /api/channels/{id}/signals       # 获取信号数据
POST /api/channels/{id}/control       # 发送控制命令
POST /api/channels/{id}/adjustment    # 发送调节命令
```

### 配置管理端点
```
GET    /api/configs                   # 获取配置列表
GET    /api/configs/{key}             # 获取单个配置
PUT    /api/configs/{key}             # 更新配置
DELETE /api/configs/{key}             # 删除配置
POST   /api/configs/sync/{service}    # 触发服务同步
GET    /api/configs/sync/status       # 获取同步状态
POST   /api/configs/cache/clear       # 清理缓存
```

### 告警管理端点
```
GET  /api/alarms                      # 获取告警列表
GET  /api/alarms/active               # 获取活动告警
POST /api/alarms/{id}/acknowledge     # 确认告警
```

### 历史数据端点 (InfluxDB查询)
```
GET  /api/historical                  # 历史数据查询
GET  /api/channels/{id}/points/{point_id}/history  # 点位历史数据
GET  /api/historical/range            # 时间范围查询
GET  /api/historical/aggregate        # 聚合查询
```

### 系统信息端点
```
GET  /api/system/info                 # 系统信息
GET  /api/device-models               # 设备模型列表
```

### 健康检查端点
```
GET  /health                          # 简单健康检查
GET  /health/check                    # 基础健康检查
GET  /health/detailed                 # 详细健康检查
```

### WebSocket实时数据
```
WS   /ws                              # WebSocket连接端点
```

### 服务代理端点
```
/api/comsrv/*     # 通信服务代理
/api/modsrv/*     # 模型服务代理  
/api/hissrv/*     # 历史服务代理
/api/netsrv/*     # 网络服务代理
/api/alarmsrv/*   # 告警服务代理
/api/rulesrv/*    # 规则服务代理
```

## ⚙️ 配置说明

### 主配置文件 (apigateway.yaml)
```yaml
server:
  host: "0.0.0.0"          # 绑定地址
  port: 8080               # 监听端口
  workers: 4               # 工作线程数

redis:
  url: "redis://localhost:6379"  # Redis连接URL
  pool_size: 10                  # 连接池大小
  timeout_seconds: 5             # 操作超时

services:                        # 后端服务配置
  comsrv:
    url: "http://localhost:8001"
    timeout_seconds: 30
  modsrv:
    url: "http://localhost:8002"
    timeout_seconds: 30
  # ... 其他服务

cors:                           # CORS配置
  allowed_origins:
    - "http://localhost:3000"
  allowed_methods:
    - "GET"
    - "POST"
    - "PUT"
    - "DELETE"
    - "OPTIONS"
  max_age: 3600

logging:                        # 日志配置
  level: "info"
  format: "json"
```

### Docker环境配置 (config/apigateway-test.yaml)
专为Docker容器环境优化的配置，使用容器服务名进行通信。

## 🧪 测试

### 单元测试
```bash
cargo test
```

### 集成测试
```bash
# 确保Redis运行
docker run -d --name redis-test -p 6379:6379 redis:7-alpine

# 运行集成测试
cargo test --test integration_test
```

### API测试
```bash
# 健康检查
curl http://localhost:8080/health

# 获取通道列表 (需要认证)
curl -H "Authorization: Bearer YOUR_TOKEN" \
     http://localhost:8080/api/channels

# WebSocket连接测试
wscat -c ws://localhost:8080/ws
```

## 🏗️ 架构设计

### 数据访问层架构
```
Frontend Request
       ↓
   API Gateway (axum)
       ↓
 DataAccessLayer (trait)
       ↓
 HybridDataAccess
    ↙    ↓    ↓    ↘
 Redis  Cache  InfluxDB  HTTP
   ↓      ↓       ↓       ↓
实时数据  配置缓存  历史数据  报表查询
```

### 存储架构
- **L1缓存**: 本地LRU缓存 (1000项，内存)
- **L2缓存**: Redis缓存 (分布式，TTL控制)
- **时序存储**: InfluxDB (历史数据，高性能时间序列)
- **业务存储**: HTTP服务 (配置数据，业务逻辑)

### 智能路由逻辑
1. 解析请求键前缀
2. 确定数据类型 (实时/配置/历史/复杂)
3. 选择访问策略 (Redis/InfluxDB/HTTP/混合)
4. 执行缓存策略
5. 返回响应

## 📊 性能特性

- **高并发**: 支持数千并发连接
- **低延迟**: 实时数据毫秒级响应
- **高可用**: 自动故障降级和重试
- **可扩展**: 水平扩展和负载均衡友好

## 🔧 开发工具

### 代码检查
```bash
cargo clippy --all-targets --all-features -- -D warnings
```

### 代码格式化
```bash
cargo fmt --all
```

### 性能分析
```bash
RUST_LOG=debug cargo run --release
```

## 📁 项目结构

```
src/
├── main.rs                 # 主程序入口
├── config.rs              # 配置管理
├── error.rs               # 错误定义
├── auth/                  # 认证模块
│   ├── jwt.rs            # JWT处理
│   ├── middleware.rs     # 认证中间件
│   └── mod.rs
├── data_access/           # 数据访问层
│   ├── mod.rs            # 接口定义
│   ├── hybrid.rs         # 混合访问实现
│   ├── cache.rs          # 缓存管理
│   └── sync.rs           # 配置同步
├── handlers/              # API处理器
│   ├── auth.rs           # 认证接口
│   ├── channels.rs       # 通道管理
│   ├── config.rs         # 配置管理
│   ├── data.rs           # 数据接口
│   ├── health.rs         # 健康检查
│   └── ...               # 服务代理
└── websocket/             # WebSocket模块
    ├── mod.rs
    ├── hub.rs            # 连接管理
    └── handlers/         # 消息处理
```

## 📚 相关文档

- [Redis键值设计规范](docs/redis-key-design.md)
- [修复日志](docs/fixlog/)
- [VoltageEMS架构文档](../../CLAUDE.md)

## 🤝 贡献指南

1. 遵循Rust代码规范
2. 编写测试覆盖新功能
3. 更新相关文档
4. 运行完整测试套件

## 📄 许可证

版权所有 © 2025 VoltageEMS团队