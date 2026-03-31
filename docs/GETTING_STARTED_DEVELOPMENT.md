# VoltageEMS 开发快速开始

本指南帮助新开发者在 30 分钟内搭建 VoltageEMS 开发环境并运行第一个测试。

## 目录

- [系统要求](#系统要求)
- [快速开始（5 分钟）](#快速开始5-分钟)
- [完整开发环境搭建](#完整开发环境搭建)
- [项目结构](#项目结构)
- [开发工作流](#开发工作流)
- [常见问题](#常见问题)

---

## 系统要求

### 必需软件

| 软件 | 版本要求 | 安装命令 |
|------|----------|----------|
| **Rust** | 1.75+ | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| **Docker** | 24.0+ | [下载 Docker Desktop](https://docs.docker.com/get-docker/) |
| **Node.js** | 18+ | `brew install node` 或 [下载](https://nodejs.org/) |
| **pnpm** | 8+ | `npm install -g pnpm` |

### 可选软件

| 软件 | 用途 | 安装命令 |
|------|------|----------|
| **redis-cli** | Redis 调试 | `brew install redis` |
| **sqlitebrowser** | SQLite 可视化 | `brew install --cask db-browser-for-sqlite` |
| **just** | 任务运行器 | `brew install just` |

### 验证安装

```bash
# 检查所有依赖
rustc --version    # rustc 1.75.0+
docker --version   # Docker version 24.0+
node --version     # v18.0.0+
pnpm --version     # 8.0.0+
```

---

## 快速开始（5 分钟）

```bash
# 1. 克隆项目
git clone https://github.com/EvanL1/VoltageEMS.git
cd VoltageEMS

# 2. 启动 Redis（唯一必需的外部依赖）
docker run -d --name voltage-redis -p 6379:6379 redis:7-alpine

# 3. 初始化数据库和配置
cargo build --release -p monarch
./target/release/monarch init
./target/release/monarch sync

# 4. 运行测试验证环境
cargo test --workspace

# 5. 启动服务
cargo run --release -p comsrv &
cargo run --release -p modsrv &

# 6. 验证服务
curl http://localhost:6001/health
curl http://localhost:6002/health
```

**成功标志：** 两个服务都返回 `{"status":"healthy"}`

---

## 完整开发环境搭建

### 步骤 1：环境配置

```bash
# 复制环境变量模板
cp .env.example .env

# 编辑 .env（通常默认值即可）
# 如需修改 Redis URL 或路径，编辑以下变量：
# VOLTAGE_REDIS_URL=redis://127.0.0.1:6379
# VOLTAGE_CONFIG_PATH=./config
# VOLTAGE_DATA_PATH=./data
```

### 步骤 2：启动基础服务

```bash
# 使用 Docker Compose 启动所有服务（推荐）
docker compose up -d

# 或者仅启动 Redis（最小依赖）
docker compose up -d voltage-redis
```

### 步骤 3：构建项目

```bash
# 构建所有包（首次需要较长时间）
cargo build --workspace

# 构建发布版本（性能更好）
cargo build --release --workspace
```

### 步骤 4：初始化数据

```bash
# 构建 Monarch CLI
cargo build --release -p monarch

# 初始化数据库 schema
./target/release/monarch init

# 同步配置到数据库
./target/release/monarch sync

# 验证配置
./target/release/monarch doctor
```

### 步骤 5：运行测试

```bash
# 运行所有测试
cargo test --workspace

# 运行特定包的测试
cargo test -p comsrv
cargo test -p modsrv
cargo test -p voltage-rtdb

# 运行集成测试
cargo test --test e2e_tests
```

### 步骤 6：启动前端

```bash
# 进入前端目录
cd apps

# 安装依赖
pnpm install

# 启动开发服务器
pnpm dev

# 访问 http://localhost:8080
```

---

## 项目结构

```
VoltageEMS/
├── apps/                    # 前端应用（Vue 3 + Element Plus + ECharts）
│   ├── src/
│   │   ├── views/          # 页面组件
│   │   ├── components/     # 通用组件
│   │   └── api/            # API 客户端
│   └── package.json
│
├── services/                # 后端服务
│   ├── comsrv/             # 通信服务 - 工业协议驱动、通道管理 (Rust)
│   │   ├── src/
│   │   │   ├── api/        # REST API 处理器
│   │   │   ├── core/       # 核心逻辑
│   │   │   └── protocols/  # 协议实现（10 种）
│   │   └── Cargo.toml
│   │
│   ├── modsrv/             # 模型服务 - 产品定义、设备实例、规则引擎 (Rust)
│   │   ├── src/
│   │   │   ├── api/        # REST API 处理器
│   │   │   └── rule_routes.rs  # 规则 API
│   │   └── Cargo.toml
│   │
├── hissrv/             # 历史数据服务 - TimescaleDB (Rust)
│   ├── apigateway/     # API 网关 (WebSocket, JWT, Rust)
│   ├── netsrv/         # 网络服务 (MQTT, Rust)
│   └── alarmsrv/       # 告警管理 (Rust)
│
├── libs/                    # 13 个共享 Rust 库
│   ├── voltage-core/       # 核心类型与编解码器（no_std）
│   ├── voltage-model/      # 数据模型、产品定义
│   ├── voltage-routing/    # 数据流路由
│   ├── voltage-rtdb/       # 实时数据库（Redis 抽象）
│   ├── voltage-rtdb-shm/   # 共享内存 RTDB（零拷贝）
│   ├── voltage-shm/        # 共享内存读写器
│   ├── voltage-infra/      # 基础设施（Redis、SQLite）
│   ├── voltage-calc/       # 表达式求值引擎
│   ├── voltage-rules/      # 规则引擎
│   ├── voltage-sim/        # 波形生成器
│   ├── voltage-schema-macro/ # SQL DDL 过程宏
│   ├── common/             # 服务引导与共享工具
│   └── errors/             # 统一错误类型
│
├── tools/
│   ├── monarch/            # CLI 配置与服务管理工具
│   └── simulator/          # Modbus TCP/RTU 从站模拟器
│
├── firmware/                # 嵌入式固件原型（ARM/STM32）
│
├── config/                  # 配置文件
│   ├── global.yaml         # 全局配置
│   ├── comsrv/             # 通信服务配置
│   │   ├── comsrv.yaml     # 通道定义
│   │   └── {channel_id}/   # 每通道点位和映射
│   └── modsrv/             # 模型服务配置
│       ├── instances.yaml  # 实例定义
│       └── rules/          # 规则文件
│
├── data/                    # 运行时数据
│   └── voltage.db          # SQLite 数据库
│
├── docker-compose.yml       # Docker 服务定义
├── Cargo.toml              # Rust workspace 配置
└── .env.example            # 环境变量模板
```

---

## 开发工作流

### 日常开发命令

```bash
# 代码检查（格式 + clippy + 测试）
./scripts/quick-check.sh

# 仅格式化
cargo fmt --all

# 仅 clippy 检查
cargo clippy --workspace --all-targets

# 监视模式开发（需要 cargo-watch）
cargo watch -x "check --workspace"
```

### 服务开发

```bash
# 开发模式运行 comsrv（自动重载）
cargo watch -x "run -p comsrv"

# 调试日志
RUST_LOG=debug cargo run -p comsrv

# 详细协议日志
RUST_LOG=comsrv::protocols=trace cargo run -p comsrv
```

### 配置更改流程

```bash
# 1. 编辑配置文件
vim config/comsrv/comsrv.yaml

# 2. 验证配置（不实际同步）
./target/release/monarch sync --dry-run

# 3. 同步到数据库
./target/release/monarch sync

# 4. 热加载服务
curl -X POST http://localhost:6001/api/channels/reload
```

### 数据库操作

```bash
# 查看数据库状态
./target/release/monarch status --detailed

# 导出配置备份
./target/release/monarch export --output backup/

# 重置数据库（谨慎！）
rm data/voltage.db
./target/release/monarch init
./target/release/monarch sync
```

### Git 工作流

```bash
# 创建功能分支
git checkout -b feature/my-feature develop

# 提交前检查
./scripts/quick-check.sh

# 提交
git add .
git commit -m "feat: add new feature"

# 推送并创建 PR
git push -u origin feature/my-feature
```

---

## 常见问题

### Q: Redis 连接失败

```
Error: Failed to connect to Redis at redis://localhost:6379
```

**解决方案：**
```bash
# 检查 Redis 是否运行
docker ps | grep redis

# 如果没有运行，启动它
docker run -d --name voltage-redis -p 6379:6379 redis:7-alpine

# 或使用 docker compose
docker compose up -d voltage-redis
```

### Q: 数据库初始化失败

```
Error: Failed to initialize database
```

**解决方案：**
```bash
# 确保 data 目录存在
mkdir -p data

# 检查权限
ls -la data/

# 重新初始化
./target/release/monarch init
```

### Q: 端口被占用

```
Error: Address already in use (os error 48)
```

**解决方案：**
```bash
# 查找占用端口的进程
lsof -i :6001
lsof -i :6002

# 终止进程
kill -9 <PID>

# 或更改端口（在 .env 中配置）
```

### Q: Cargo 构建缓慢

**解决方案：**
```bash
# 使用 mold 链接器（Linux）
cargo install mold
RUSTFLAGS="-C link-arg=-fuse-ld=mold" cargo build

# 使用增量编译
export CARGO_INCREMENTAL=1

# 使用 sccache
cargo install sccache
export RUSTC_WRAPPER=sccache
```

### Q: 测试失败但本地服务正常

**可能原因：** 测试使用 MemoryRtdb 而非 Redis

**解决方案：**
```bash
# 确保测试隔离
cargo test -- --test-threads=1

# 检查是否有环境依赖
unset VOLTAGE_REDIS_URL
cargo test
```

### Q: 前端无法连接后端

**解决方案：**
```bash
# 检查后端服务
curl http://localhost:6001/health
curl http://localhost:6002/health

# 检查 CORS 配置
# 确保后端允许 http://localhost:8080

# 检查前端 API 配置
cat apps/.env.local
# VITE_API_BASE_URL=http://localhost:6001
```

---

## 下一步

- 阅读 [API 参考文档](./API_REFERENCE.md) 了解所有 API
- 阅读 [Monarch CLI 指南](./MONARCH_CLI_GUIDE.md) 掌握管理工具
- 阅读 [配置格式指南](./CONFIG_FORMAT_GUIDE.md) 理解配置系统
- 查看 `CLAUDE.md` 了解代码规范和约束

---

## 联系支持

- **Issues**: https://github.com/EvanL1/VoltageEMS/issues
- **文档**: https://github.com/EvanL1/VoltageEMS/tree/main/docs
