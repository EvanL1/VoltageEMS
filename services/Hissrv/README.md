# HisSrv - Historical Data Service

HisSrv 是一个独立的、可配置的历史数据服务，专为 VoltageEMS 系统设计。它通过 Redis 订阅/发布机制与其他服务通信，并支持多种存储后端。

## 🚀 特性

- **独立服务**: 完全独立运行，通过 Redis 与其他服务通信
- **多存储后端**: 支持 InfluxDB、Redis、PostgreSQL、MongoDB
- **可配置路由**: 基于模式匹配的数据路由和过滤
- **REST API**: 完整的 OpenAPI 3.0 规范 API
- **实时监控**: 内置指标收集和健康检查
- **结构化日志**: 支持 JSON 和文本格式的结构化日志
- **异步架构**: 基于 Tokio 的高性能异步处理

## 📋 系统要求

- Rust 1.70+
- Redis 服务器
- InfluxDB (可选)
- PostgreSQL (可选)
- MongoDB (可选)

## 🛠️ 安装和启动

### 快速启动

```bash
# 克隆项目
git clone <repo-url>
cd services/Hissrv

# 使用启动脚本 (推荐)
./start.sh
```

### 手动启动

```bash
# 构建项目
cargo build --release

# 创建配置文件 (参考 hissrv.yaml)
cp hissrv.yaml.example hissrv.yaml

# 启动服务
./target/release/hissrv-rust --config hissrv.yaml
```

## ⚙️ 配置

HisSrv 使用 YAML 格式的配置文件。主要配置项：

### 服务配置
```yaml
service:
  name: "hissrv"
  version: "0.2.0"
  port: 8080
  host: "0.0.0.0"
```

### Redis 配置
```yaml
redis:
  connection:
    host: "127.0.0.1"
    port: 6379
    password: ""
    database: 0
  subscription:
    channels:
      - "data:*"
      - "events:*"
```

### 存储后端配置
```yaml
storage:
  default: "influxdb"
  backends:
    influxdb:
      enabled: true
      url: "http://localhost:8086"
      database: "hissrv_data"
      retention_days: 30
```

### 数据过滤规则
```yaml
data:
  filters:
    default_policy: "store"
    rules:
      - pattern: "temp:*"
        action: "store"
        storage: "influxdb"
      - pattern: "log:*"
        action: "ignore"
```

## 🔌 API 接口

服务启动后，API 文档可通过以下地址访问：

- **Swagger UI**: http://localhost:8080/api/v1/swagger-ui
- **健康检查**: http://localhost:8080/api/v1/health
- **指标监控**: http://localhost:8080/api/v1/admin/statistics

### 主要 API 端点

#### 历史数据查询
| 方法 | 路径 | 描述 |
|------|------|------|
| GET | `/api/v1/history/query` | 查询历史数据点 |
| GET | `/api/v1/history/sources` | 获取数据源列表 |
| GET | `/api/v1/history/sources/{id}` | 获取数据源详情 |
| GET | `/api/v1/history/statistics` | 获取时间序列统计 |

#### 数据导出
| 方法 | 路径 | 描述 |
|------|------|------|
| POST | `/api/v1/history/export` | 创建导出任务 |
| GET | `/api/v1/history/export/{job_id}` | 获取导出状态 |

#### 管理监控
| 方法 | 路径 | 描述 |
|------|------|------|
| GET | `/api/v1/health` | 健康检查 |
| GET | `/api/v1/admin/storage-stats` | 存储统计 |
| GET | `/api/v1/admin/config` | 配置信息 |

## 🔄 数据流

```
其他服务 → Redis Pub/Sub → HisSrv → 存储后端
                                ↓
                           REST API ← 客户端查询
```

1. **数据接收**: 通过 Redis 订阅其他服务发布的数据
2. **数据处理**: 应用过滤规则和转换逻辑
3. **数据存储**: 根据配置路由到相应的存储后端
4. **数据查询**: 通过 REST API 提供数据查询服务

## 📊 监控和日志

### 监控指标

- 处理消息总数和速率
- API 请求统计
- 存储后端状态
- 系统资源使用情况

### 日志配置

```yaml
logging:
  level: "info"          # 日志级别
  format: "json"         # 格式: json/text
  file: "logs/hissrv.log"
```

## 🧪 开发和测试

### 构建

```bash
# 开发构建
cargo build

# 发布构建
cargo build --release

# 运行测试
cargo test
```

### 配置检查

```bash
# 检查配置文件语法
./target/release/hissrv-rust --config hissrv.yaml --help
```

## 🐛 故障排除

### 常见问题

1. **Redis 连接失败**
   - 检查 Redis 服务是否运行
   - 验证连接配置 (host, port, password)

2. **InfluxDB 连接失败**
   - 确认 InfluxDB 服务状态
   - 检查数据库是否存在

3. **API 无法访问**
   - 检查端口是否被占用
   - 验证防火墙设置

### 日志查看

```bash
# 实时日志
tail -f logs/hissrv.log

# 错误日志过滤
grep "ERROR" logs/hissrv.log
```

## 🚧 架构设计

### 模块结构

```
src/
├── main.rs           # 主程序入口
├── config.rs         # 配置管理
├── error.rs          # 错误定义
├── storage/          # 存储后端
│   ├── mod.rs
│   ├── influxdb_storage.rs
│   └── redis_storage.rs
├── pubsub/           # 消息处理
│   └── mod.rs
├── api/              # REST API
│   ├── mod.rs
│   ├── handlers.rs
│   └── models.rs
├── monitoring/       # 监控指标
│   └── mod.rs
└── logging/          # 日志系统
    └── mod.rs
```

### 设计原则

- **模块化**: 每个组件都是独立的模块
- **可配置**: 所有行为都可以通过配置文件控制
- **异步优先**: 使用 Tokio 实现高并发处理
- **类型安全**: 利用 Rust 的类型系统确保安全性

## 📝 版本历史

- **v0.2.0**: 重构为独立服务，添加 REST API 和监控
- **v0.1.0**: 初始版本，基本的 Redis 到 InfluxDB 数据传输

## 🤝 贡献

欢迎提交 Issue 和 Pull Request！

## 📄 许可证

[待定]