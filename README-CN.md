# VoltageEMS

[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.90%2B-orange.svg)](https://www.rust-lang.org/)

[English](README.md) | [文档](docs/README.md)

基于 Rust 构建的工业物联网能源管理系统。多协议数据采集、共享内存实时处理、规则引擎执行，为工业能源场景提供全栈监控能力。

## 特性

- **多协议支持** — Modbus TCP/RTU、IEC 60870-5-104、OPC UA、MQTT、HTTP、DL/T 645、CAN、J1939、GPIO
- **零拷贝共享内存** — 通过 `/dev/shm` 实现服务间高性能数据通路，绕过序列化开销
- **规则引擎** — 可视化规则编辑（Vue Flow），支持实时执行、表达式求值和定时调度
- **时序数据集成** — InfluxDB 3.x 历史数据持久化与趋势分析
- **全栈可视化** — Vue.js 3 + ECharts 仪表盘，WebSocket 实时数据更新

## 架构

```
                        ┌─────────────────────────────────────────────┐
                        │              voltage-redis(:6379)           │
                        │            实时数据存储 + 消息路由            │
                        └──────┬──────────────────────┬───────────────┘
                               │                      │
  设备 ────────► comsrv(:6001) ┤                      ├─► modsrv(:6002)
   Modbus          通信服务    │    SHM + UDS         │   规则 / 计算
   IEC104          数据采集    ◄──────────────────────┘   设备实例
   OPC UA                     │
   MQTT/HTTP                  │
   DL645/CAN         apigateway(:6005) ──── apps(:8080)
   J1939/GPIO           API 网关            Vue.js 前端
                              │
                      hissrv(:6004) ◄── InfluxDB(:8181)
                      历史数据服务          时序数据库
                              │
                    alarmsrv(:6007)    netsrv(:6006)
                    告警管理            MQTT 网络通信
```

### 服务端口

| 服务 | 端口 | 语言 | 说明 |
|------|------|------|------|
| comsrv | 6001 | Rust | 通信服务 — 工业协议驱动、通道管理 |
| modsrv | 6002 | Rust | 模型服务 — 产品定义、设备实例、规则引擎 |
| hissrv | 6004 | Python | 历史数据服务 — InfluxDB 3.x 时序数据持久化 |
| apigateway | 6005 | Python | API 网关 — 统一 REST API、WebSocket、JWT 认证 |
| netsrv | 6006 | Python | 网络服务 — MQTT 代理集成 |
| alarmsrv | 6007 | Python | 告警服务 — 告警规则与通知 |
| apps | 8080 | Vue.js | 前端 — ECharts 仪表盘、Vue Flow 规则编辑器 |
| voltage-redis | 6379 | — | 实时数据存储与消息路由 |
| InfluxDB | 8181 | — | 时序数据库，历史数据存储 |

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

## 项目结构

```
VoltageEMS/
├── services/
│   ├── comsrv/              # 通信服务 (Rust)
│   ├── modsrv/              # 模型服务 + 规则 (Rust)
│   └── python-services/
│       ├── hissrv/          # 历史数据 (Python/FastAPI)
│       ├── apigateway/      # API 网关 (Python/FastAPI)
│       ├── netsrv/          # MQTT 网络通信 (Python/FastAPI)
│       └── alarmsrv/        # 告警管理 (Python/FastAPI)
├── libs/                    # 13 个共享 Rust 库
├── tools/
│   ├── monarch/             # CLI 配置与服务管理工具
│   └── simulator/           # Modbus TCP/RTU 从站模拟器
├── apps/                    # Vue.js 3 前端 (Element Plus + ECharts)
├── firmware/                # 嵌入式固件原型 (ARM/STM32)
├── config/                  # YAML/CSV 配置
└── docs/                    # 文档
```

## 库

### 核心

| 库名 | 说明 |
|------|------|
| voltage-core | 核心类型与编解码器 — 支持 `no_std`，可用于嵌入式固件 |
| voltage-model | 模型层 — 计算、产品定义、实例管理 |
| voltage-infra | 基础设施层 — Redis 和 SQLite 集成 |
| common | 服务引导、配置管理和共享工具 |
| errors | 统一错误类型 |

### 数据层

| 库名 | 说明 |
|------|------|
| voltage-rtdb | 实时数据库抽象 — 支持 Redis 和内存后端 |
| voltage-rtdb-shm | 共享内存 RTDB 实现 — 零拷贝数据共享 |
| voltage-shm | 平台无关的共享内存读写器 |
| voltage-routing | 数据流路由 — comsrv ↔ modsrv 消息路由 |

### 扩展

| 库名 | 说明 |
|------|------|
| voltage-calc | 表达式求值引擎，内置函数库 |
| voltage-rules | 规则引擎 — Vue Flow 规则解析、执行和调度 |
| voltage-sim | 波形生成器，用于设备仿真 |
| voltage-schema-macro | 过程宏 — 从 Rust 结构体自动生成 SQL DDL |

## 数据流

### 上行（设备 → 云端）

```
设备 → comsrv → Redis (route:c2m) → modsrv
                 通道数据              规则执行
                 "comsrv:{id}:T"      实例计算
```

### 下行（云端 → 设备）

```
主路径：modsrv → Redis (route:m2c) → SHM 写入 + UDS 通知 → comsrv → 设备
备份路径：modsrv → Redis (inst:{id}:A + TODO) → comsrv ShmPoller → 设备
```

主路径通过共享内存（`/dev/shm/voltage-rtdb.shm`）配合 Unix Domain Socket 通知实现最低延迟。备份路径通过 Redis 轮询保障可靠性。

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

## 开发

```bash
# 构建
cargo build --workspace

# 测试
cargo test --workspace

# 快速检查 (fmt + clippy + test + 前端)
./scripts/quick-check.sh
```

## 许可证

MIT License - 详见 [LICENSE](LICENSE)

## 链接

- 仓库: [https://github.com/EvanL1/VoltageEMS](https://github.com/EvanL1/VoltageEMS)
- 问题: [https://github.com/EvanL1/VoltageEMS/issues](https://github.com/EvanL1/VoltageEMS/issues)
