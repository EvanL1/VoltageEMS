# VoltageEMS - 工业物联网能源管理系统

[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)
[![Docker](https://img.shields.io/badge/docker-ready-blue.svg)](https://www.docker.com/)

[English Version](README.md)

VoltageEMS 是一个基于 Rust 微服务架构的高性能工业物联网能源管理系统。它为工业能源管理场景提供实时数据采集、处理和监控能力。

## 🚀 特性

- **高性能**：使用 Rust 构建，实现最佳性能和内存安全
- **微服务架构**：模块化设计，服务独立部署
- **多协议支持**：支持 Modbus TCP/RTU、虚拟协议，以及可扩展的插件系统
- **实时处理**：低延迟的数据采集和处理
- **基于 Redis 的存储**：快速的内存数据存储，支持持久化
- **RESTful API**：所有服务提供标准的 HTTP/JSON 接口
- **Docker 就绪**：完全容器化部署
- **Nginx 集成**：统一入口点，反向代理

## 🏗️ 架构

```
                ┌─────────────┐
                │    客户端    │
                └──────┬──────┘
                       │
                ┌──────▼──────┐
                │ Nginx (:80) │ ← 统一入口点
                └──────┬──────┘
                       │
       ┌───────────────┴───────────────────────────┐
       │                                           │
       ▼                                           ▼
┌─────────────┐                         ┌──────────────────┐
│  API 网关   │                         │     微服务       │
│   (:6005)   │                         │                  │
│  (最小化)   │                         │ comsrv(:6000)    │
└─────────────┘                         │ modsrv(:6001)    │
                                        │ alarmsrv(:6002)  │
                                        │ rulesrv(:6003)   │
                                        │ hissrv(:6004)    │
                                        │ netsrv(:6006)    │
                                        └──────────────────┘
                                                 │
                                                 ▼
                                    ┌─────────────────────────┐
                                    │   Redis(:6379) & 存储   │
                                    └─────────────────────────┘
```

## 📦 服务说明

| 服务 | 端口 | 描述 |
|------|------|------|
| **nginx** | 80/443 | 反向代理和负载均衡器 |
| **comsrv** | 6000 | 通信服务 - 处理工业协议 |
| **modsrv** | 6001 | 模型服务 - 管理数据模型和计算 |
| **alarmsrv** | 6002 | 告警服务 - 监控和管理告警 |
| **rulesrv** | 6003 | 规则引擎 - 执行业务规则 |
| **hissrv** | 6004 | 历史服务 - 存储时序数据 |
| **apigateway** | 6005 | API 网关 - 最小化代理服务 |
| **netsrv** | 6006 | 网络服务 - 处理外部通信 |

## 🛠️ 技术栈

- **编程语言**：Rust 1.75+
- **Web 框架**：Axum
- **数据库**：Redis 8+、InfluxDB 2.x
- **容器**：Docker、Docker Compose
- **消息格式**：JSON、Protocol Buffers
- **构建工具**：Cargo

## 🚦 快速开始

### 前置要求

- Rust 1.75+ ([安装 Rust](https://rustup.rs/))
- Docker & Docker Compose
- Redis 8+（开发环境）

### 开发环境设置

1. 克隆仓库：
```bash
git clone https://github.com/your-org/VoltageEMS.git
cd VoltageEMS
```

2. 启动开发环境：
```bash
./scripts/dev.sh
```

3. 运行特定服务：
```bash
RUST_LOG=debug cargo run --bin comsrv
```

### Docker 部署

1. 构建所有镜像：
```bash
./scripts/build.sh release
```

2. 启动所有服务：
```bash
docker-compose up -d
```

3. 检查服务状态：
```bash
docker-compose ps
```

## 📝 配置

每个服务都有自己的 YAML 格式配置文件：

```yaml
# 示例：services/comsrv/config/comsrv.yaml
service:
  name: "comsrv"
  host: "0.0.0.0"
  port: 6000

redis:
  url: "redis://localhost:6379"
  
channels:
  - id: 1
    name: "modbus_channel_1"
    protocol: "modbus"
    enabled: true
```

## 🔧 开发

### 项目结构

```
VoltageEMS/
├── services/           # 微服务
│   ├── comsrv/        # 通信服务
│   ├── modsrv/        # 模型服务
│   ├── alarmsrv/      # 告警服务
│   ├── rulesrv/       # 规则引擎
│   ├── hissrv/        # 历史服务
│   └── apigateway/    # API 网关
├── libs/              # 共享库
├── scripts/           # 工具脚本
│   └── redis-functions/  # Redis Lua 函数
├── config/            # 配置文件
└── docker/            # Docker 相关文件
```

### 构建

```bash
# 检查编译
cargo check --workspace

# 构建所有服务
cargo build --workspace

# 运行测试
cargo test --workspace

# 格式化代码
cargo fmt --all

# 运行 clippy
cargo clippy --all-targets --all-features -- -D warnings
```

### 测试

```bash
# 运行所有测试
./scripts/test.sh

# 运行特定服务测试
cargo test -p comsrv

# 带输出运行
cargo test -- --nocapture
```

## 📊 API 文档

所有服务都提供 RESTful API。以下是一些常用端点：

### 健康检查
```bash
GET /health
```

### 通信服务 (comsrv)
```bash
# 获取所有通道
GET /api/channels

# 获取通道状态
GET /api/channels/{id}/status

# 读取数据点
GET /api/channels/{id}/read/{point_id}
```

### 模型服务 (modsrv)
```bash
# 应用模型
POST /api/models/apply
{
  "model_id": "energy_calc",
  "inputs": {...}
}
```

## 🔍 监控

### 日志
```bash
# 查看服务日志
docker logs -f voltageems-comsrv

# 使用调试级别
RUST_LOG=debug cargo run --bin comsrv
```

### Redis 监控
```bash
# 监控 Redis 活动
redis-cli monitor | grep comsrv

# 检查数据
redis-cli hgetall "comsrv:1001:T"
```

## 🤝 贡献

1. Fork 本仓库
2. 创建您的特性分支 (`git checkout -b feature/amazing-feature`)
3. 提交您的更改 (`git commit -m 'Add some amazing feature'`)
4. 推送到分支 (`git push origin feature/amazing-feature`)
5. 开启一个 Pull Request

## 📄 许可证

本项目基于 MIT 许可证 - 查看 [LICENSE](LICENSE) 文件了解详情。

## 🙏 致谢

- 使用 [Rust](https://www.rust-lang.org/) 构建
- Web 框架：[Axum](https://github.com/tokio-rs/axum)
- 内存数据库：[Redis](https://redis.io/)
- 时序数据库：[InfluxDB](https://www.influxdata.com/)

## 📞 联系方式

- 项目链接：[https://github.com/your-org/VoltageEMS](https://github.com/your-org/VoltageEMS)
- 问题反馈：[https://github.com/your-org/VoltageEMS/issues](https://github.com/your-org/VoltageEMS/issues)