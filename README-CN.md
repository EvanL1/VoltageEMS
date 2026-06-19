# VoltageEMS

[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.90%2B-orange.svg)](https://www.rust-lang.org/)
[![Version](https://img.shields.io/badge/version-0.4.0-yellow.svg)](CHANGELOG.md)
[![Status](https://img.shields.io/badge/status-beta-orange.svg)](CHANGELOG.md)

[English](README.md) | [文档](docs/README.md) | [更新日志](CHANGELOG.md)

> **v0.4.0 — 亚毫秒事件平面（2026-05-29）。** 事件驱动共享内存路径（PointWatch）将关键控制延迟压至 ~1.5 ms — 生产环境 Cortex-A55 端到端 P50 ≈ 206 µs。详见 [CHANGELOG](CHANGELOG.md)。

基于 Rust 构建的工业物联网能源管理系统。多协议数据采集、共享内存实时处理、规则引擎执行，为工业能源场景提供全栈监控能力。

## 特性

- **多协议支持** — Modbus TCP/RTU、IEC 60870-5-104、IEC 61850（MMS）、OPC UA、MQTT、HTTP、DL/T 645、CAN/J1939、GPIO、BLE、Zigbee、Matter、Voltage-485、Virtual（14 种协议）
- **零拷贝共享内存** — 通过 `/dev/shm` 实现服务间高性能数据通路，绕过序列化开销
- **规则引擎** — 可视化规则编辑（Vue Flow），支持实时执行、表达式求值和定时调度
- **可插拔时序存储** — 运行时可配置后端（PostgreSQL / TimescaleDB），历史数据持久化
- **全栈可视化** — Vue.js 3 + ECharts 仪表盘，WebSocket 实时数据更新

## 架构

```
  设备 ─────► comsrv(:6001) ──── SHM（热路径，~10ns/点）
  14 种协议    通信 & 采集        │
                                ├── ShmRedisSync（100ms 异步）
                                │         │
                                │   voltage-redis(:6379)
                     SHM + UDS  │   数据镜像 + 路由
                ◄───────────────┤         │
                                │   ┌─────┴─────────────────┐
                          modsrv(:6002)                      │
                          规则 / 计算            apigateway(:6005) ── apps(:8080)
                          设备实例                  API 网关         Vue.js 前端
                                                       │
                                               hissrv(:6004) ◄── PostgreSQL/TimescaleDB
                                               历史数据服务
                                                       │
                                             alarmsrv(:6007)    netsrv(:6006)
                                             告警管理            MQTT 网络通信
```

### 服务端口

| 服务 | 端口 | 语言 | 说明 |
|------|------|------|------|
| comsrv | 6001 | Rust | 通信服务 — 工业协议驱动、通道管理 |
| modsrv | 6002 | Rust | 模型服务 — 产品定义、设备实例、规则引擎 |
| hissrv | 6004 | Rust | 历史数据服务 — 可插拔后端（PostgreSQL / TimescaleDB） |
| apigateway | 6005 | Rust | API 网关 — 统一 REST API、WebSocket、JWT 认证 |
| netsrv | 6006 | Rust | 网络服务 — MQTT 代理集成 |
| alarmsrv | 6007 | Rust | 告警服务 — 告警规则与通知 |
| apps | 8080 | Vue.js | 前端 — ECharts 仪表盘、Vue Flow 规则编辑器 |
| voltage-redis | 6379 | — | 数据镜像（从 SHM 异步同步）与路由表 |
| TimescaleDB | 5432 | — | 时序数据库，历史数据存储（可选，运行时配置） |

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
│   ├── hissrv/              # 历史数据 (Rust)
│   ├── apigateway/          # API 网关 (Rust)
│   ├── netsrv/              # MQTT 网络通信 (Rust)
│   └── alarmsrv/            # 告警管理 (Rust)
├── libs/                    # 14 个共享 Rust 库
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
| voltage-model | 模型层 — PointType、KeySpaceConfig、编译期产品常量 |
| voltage-config | 跨平台配置 schema — comsrv/modsrv/monarch 共用，可在 Windows 构建 |
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
设备 → comsrv → SHM（set_direct，~10ns/点）
              → ShmRedisSync（100ms 异步）→ Redis pipeline → 下游服务
```

### 下行（云端 → 设备）

```
主路径：modsrv → SHM + UDS 通知 → comsrv ShmCommandListener → 设备
```

主路径通过共享内存（路径经 `VOLTAGE_SHM_PATH` 环境变量解析，Docker 下默认 `/shm/rtdb/voltage-rtdb.shm`）配合 Unix Domain Socket 通知实现最低延迟。若 comsrv 重启，UDS 会以指数退避（1–5 秒）自动重连。

## Monarch CLI

Monarch 是 VoltageEMS 的统一管理工具 — 配置管理、实时监控、远程运维。

### 安装

```bash
# 一键安装 (Linux / macOS / WSL)
curl -fsSL https://raw.githubusercontent.com/EvanL1/VoltageEMS/develop/tools/monarch/install.sh | bash

# 源码编译
cargo install --path tools/monarch
```

### 配置管理

```bash
monarch init                   # 初始化数据库
monarch sync                   # 同步 YAML/CSV 配置到 SQLite
monarch sync --dry-run         # 预览变更
monarch export                 # 从数据库导出配置
monarch status                 # 查看配置状态
```

### 服务操作

```bash
monarch channels list          # 列出所有通道
monarch channels status 1      # 通道运行状态
monarch models instances list  # 列出设备实例
monarch rules list             # 列出业务规则
monarch templates list         # 列出通道模板
monarch templates snapshot 1   # 从通道快照创建模板
monarch templates apply 1 2    # 将模板应用到通道
```

### 远程管理

```bash
# 指定远端机器（所有服务端口自动解析）
monarch --host 192.168.30.21 channels list
monarch --host 192.168.30.21 models instances list
monarch --host 192.168.30.21 rules list
monarch --host 192.168.30.21 logs level all debug
```

### 交互式 TUI 仪表盘

```bash
monarch top                              # 本地监控
monarch --host 192.168.30.21 top         # 远端监控

# 导航:
#   ←→ / Tab    切换视图 (Channels / Instances / Rules)
#   ↑↓ / j/k    在列表内导航
#   Enter        钻入详情（点位数据、实时值、路由信息）
#   Esc          返回上级
#   z            切换隐藏零值
#   r            强制刷新
#   q            退出
```

### JSON 输出（AI Agent 和脚本集成）

```bash
# 所有命令支持 --json 结构化输出
monarch --json channels list
monarch --json --host 192.168.30.21 rtdb scan "inst:*"

# 或设置环境变量
export MONARCH_JSON=1
monarch channels list          # 自动输出 JSON
```

### 基础设施

```bash
monarch services start         # 启动 Docker 服务
monarch services status        # 检查服务状态
monarch doctor                 # 全链路健康检查
monarch logs level all debug   # 动态调整日志级别
monarch rtdb scan "inst:*"     # 直接 Redis 操作
monarch shm top                # 本地共享内存 TUI
```

### 环境变量

| 变量 | 说明 | 默认值 |
|------|------|--------|
| `VOLTAGE_REDIS_URL` | Redis 连接 | `redis://localhost:6379` |
| `VOLTAGE_COMSRV_URL` | Comsrv 地址 | `http://localhost:6001` |
| `VOLTAGE_MODSRV_URL` | Modsrv 地址 | `http://localhost:6002` |
| `VOLTAGE_CONFIG_PATH` | 配置目录 | 自动检测 |
| `VOLTAGE_DATA_PATH` | SQLite 数据目录 | 自动检测 |
| `VOLTAGE_SHM_PATH` | SHM 文件路径 | `/shm/rtdb/voltage-rtdb.shm` |
| `MONARCH_JSON` | 强制 JSON 输出 | — |

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

MIT OR Apache-2.0 — 详见 [LICENSE](LICENSE)

## 链接

- 仓库: [https://github.com/EvanL1/VoltageEMS](https://github.com/EvanL1/VoltageEMS)
- 问题: [https://github.com/EvanL1/VoltageEMS/issues](https://github.com/EvanL1/VoltageEMS/issues)
