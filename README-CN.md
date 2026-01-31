# VoltageEMS

[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.90%2B-orange.svg)](https://www.rust-lang.org/)

[English](README.md) | [文档](docs/README.md)

基于 Rust 构建的工业物联网能源管理系统。为工业能源场景提供实时数据采集、处理和监控能力。

## 架构

```
设备 (Modbus/Virtual/gRPC) → comsrv(:6001) → Redis(:6379) → modsrv(:6002)
```

| 服务 | 端口 | 说明 |
|------|------|------|
| comsrv | 6001 | 通信服务 - 工业协议 |
| modsrv | 6002 | 模型服务 + 规则引擎 |
| Redis | 6379 | 实时数据存储 |

## 快速开始

### 环境要求

- Rust 1.90+ | Docker & Docker Compose | Redis 8+

### 开发环境

```bash
# 克隆
git clone https://github.com/EvanL1/VoltageEMS.git
cd VoltageEMS

# 构建 Monarch CLI
cargo build --release -p monarch

# 初始化并同步配置
./target/release/monarch init
./target/release/monarch sync

# 启动服务
./target/release/monarch services start

# 检查系统状态
./target/release/monarch doctor
```

### Docker 部署

```bash
docker compose up -d
docker compose ps
```

## Monarch CLI

Monarch 是 VoltageEMS 的配置管理工具。

```bash
# 初始化数据库
monarch init

# 同步 YAML/CSV 配置到 SQLite
monarch sync
monarch sync comsrv    # 同步特定服务
monarch sync --dry-run # 预览变更

# 服务管理
monarch services start
monarch services stop
monarch services status

# 系统健康检查
monarch doctor

# 通道管理
monarch channels list
monarch channels status 1001

# 帮助
monarch --help
monarch <command> --help
```

### 环境变量

| 变量 | 说明 | 默认值 |
|------|------|--------|
| `VOLTAGE_REDIS_URL` | Redis 连接 | `redis://localhost:6379` |
| `VOLTAGE_COMSRV_URL` | Comsrv 地址 | `http://localhost:6001` |
| `VOLTAGE_MODSRV_URL` | Modsrv 地址 | `http://localhost:6002` |
| `VOLTAGE_CONFIG_PATH` | 配置目录 | 自动检测 |
| `VOLTAGE_DATA_PATH` | 数据目录 | 自动检测 |

## 项目结构

```
VoltageEMS/
├── services/comsrv/     # 通信服务
├── services/modsrv/     # 模型服务 + 规则
├── tools/monarch/       # CLI 工具
├── libs/                # 共享库
├── apps/                # Vue.js 前端
├── config/              # YAML/CSV 配置
└── docs/                # 文档
```

## 开发

```bash
# 构建
cargo build --workspace

# 测试
cargo test --workspace

# 快速检查 (fmt + clippy + test)
./scripts/quick-check.sh
```

## 许可证

MIT License - 详见 [LICENSE](LICENSE)

## 链接

- 仓库: [https://github.com/EvanL1/VoltageEMS](https://github.com/EvanL1/VoltageEMS)
- 问题: [https://github.com/EvanL1/VoltageEMS/issues](https://github.com/EvanL1/VoltageEMS/issues)
