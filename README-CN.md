# VoltageEMS - 工业物联网能源管理系统

[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org/)
[![Docker](https://img.shields.io/badge/docker-ready-blue.svg)](https://www.docker.com/)

[English Version](README.md)

VoltageEMS 是一个基于 Rust 微服务架构构建的高性能工业物联网能源管理系统。它为工业能源管理场景提供实时数据采集、处理和监控能力。

## 🚀 核心特性

- **高性能架构**: 使用 Rust 构建，确保最佳性能和内存安全
- **集中式配置**: 所有 SQL 查询、Redis 键、表名在 voltage-config 库中统一维护
- **混合架构设计**: Rust 服务处理 I/O，Redis Lua 函数处理业务逻辑
- **实时数据流**: 通过 Redis 映射自动从设备路由数据到模型
- **多协议支持**: Modbus TCP/RTU、Virtual、gRPC，支持插件扩展
- **基于模型的系统**: 实例化数据建模，支持产品层次结构
- **零轮询设计**: 使用 Redis Lua 函数实现事件驱动数据流
- **RESTful API**: 所有服务提供标准 HTTP/JSON 接口
- **Docker 就绪**: 完全容器化部署，支持 docker-compose
- **CLI 工具集**: 提供完善的命令行工具进行服务管理
- **配置管理工具**: Monarch 工具实现 YAML/CSV 与 SQLite 双向同步

## 🏗️ 系统架构

### 整体架构
```
                ┌─────────────┐
                │    设备      │ (Modbus, Virtual, gRPC)
                └──────┬──────┘
                       │
                ┌──────▼──────┐
                │   客户端     │ （开发环境直接访问服务）
                └──────┬──────┘
                       │
       ┌───────────────┴───────────────────────────┐
       │                                           │
       ▼                                           ▼
                                      ┌──────────────────┐
                                      │    微服务集群      │
                                      │                  │
                                      │ comsrv(:6001)    │ ← 通信服务
                                      │ modsrv(:6002)    │ ← 模型服务
                                      │ rulesrv(:6003)   │ ← 规则引擎
                                      └──────────────────┘
                                                 │
                                                 ▼
                                    ┌───────────────────────────────┐
                                    │ Redis(:6379)                  │
                                    └───────────────────────────────┘
```

### 数据流架构
```
设备 → comsrv → Redis哈希 → Lua函数 → modsrv实例
      (插件)   (comsrv:ch:T) (自动路由)  (实时)
                              ↓
                        route:c2m 路由表
                       (通道→实例映射)
```

### 关键数据流程
1. **设备数据采集**: comsrv 通过协议插件采集设备数据
2. **数据存储**: 数据存储到 Redis Hash (如 `comsrv:1001:T`)
3. **自动路由**: `comsrv_batch_update` Lua 函数自动查找映射并路由数据
4. **实例更新**: 数据实时更新到 modsrv 实例哈希 (键 `modsrv:pv_inverter_01:M`，字段为点号如 `1`)

## 📦 服务说明

| 服务 | 端口 | 功能描述 |
|------|------|----------|
| **comsrv** | 6001 | 通信服务 - 处理工业协议和数据采集 |
| **modsrv** | 6002 | 模型服务 - 管理数据模型和计算引擎 |
| **rulesrv** | 6003 | 规则引擎 - 执行业务规则 |
| **redis**  | 6379 | 内存存储与 Lua 函数运行时 |

提示：默认提供的 docker-compose 仅运行 comsrv/modsrv/rulesrv 与 Redis。

## 🛠️ 技术栈

- **编程语言**: Rust 1.75+
- **Web 框架**: Axum 0.8
- **数据库**: Redis 8+, InfluxDB 2.x
- **容器技术**: Docker, Docker Compose
- **消息格式**: JSON, Protocol Buffers
- **构建工具**: Cargo

## 🚦 快速开始

### 环境要求

- Rust 1.75+ ([安装 Rust](https://rustup.rs/))
- Docker & Docker Compose
- Redis 8+ (开发环境)

### 开发环境设置

1. 克隆仓库:
```bash
git clone https://github.com/your-org/VoltageEMS.git
cd VoltageEMS
```

2. 启动开发环境:
```bash
./scripts/dev.sh
```

3. 加载 Redis Lua 函数（数据流的关键）:
```bash
cd scripts/redis-functions && ./load_functions.sh
```

4. 运行特定服务:
```bash
RUST_LOG=debug cargo run --bin comsrv
```

### Docker 部署

1. 构建并启动所有服务:
```bash
# 构建 Docker 镜像
docker build -t voltageems:latest .

# 启动所有服务（会自动构建）
docker compose up -d

# 检查服务状态
docker-compose ps
```

2. 验证数据流:
```bash
# 查看日志
docker compose logs -f comsrv modsrv

# 测试数据流
docker exec voltageems-redis redis-cli FCALL comsrv_batch_update 0 "1001" "T" '{"1":100}'

# 检查映射后的数据（运行态使用哈希存储）
docker exec voltageems-redis redis-cli HGET "modsrv:pv_inverter_01:M" "1"
```

## 📝 配置说明
> 运行期配置来源：服务主要从 SQLite 配置库读取；`config/` 下的 YAML 用于通过 Monarch 工具生成/同步配置。

### 配置优先级
- **优先级顺序（从高到低）**：
  1. **配置文件（YAML/SQLite）** - 明确配置时具有最高优先级
  2. **环境变量** - 配置文件未指定或使用默认值时的后备选项
  3. **程序默认值** - 两者都未配置时的内置默认值

- **实现说明**：
  - 服务会检查配置值是否为非默认值，然后才会回退到环境变量
  - 例如：SQLite 中 `port=6001`（默认值）时，ENV 仍可覆盖；但 `port=7001`（非默认值）时，配置文件优先
  - 本地运行：自动加载当前目录下的 `.env` 文件（若存在）
  - 容器运行：使用 docker-compose 注入的环境变量

### 服务配置细节

- comsrv（通信服务）
  - 监听地址/端口优先级：
    - CLI `--bind-address` > 配置文件 > `SERVICE_HOST` 和 `SERVICE_PORT` > 默认 `0.0.0.0:6001`
  - Redis 地址：
    - 配置文件 `redis.url`（非默认值）> `REDIS_URL` > 默认 `redis://127.0.0.1:6379`
  - 其它常见变量：
    - `RUST_LOG` 控制日志级别（如 `info,comsrv=debug`）
    - `CSV_BASE_PATH` / `CONFIG_BASE_PATH` / `SQLITE_DB_PATH` 由底层组件使用（影响文件路径/存储），非 main 入口统一管理
  - .env：自动加载（仅文件存在时生效）

- modsrv（模型服务）
  - 端口：
    - SQLite `service_config.port`（非默认值）> `MODSRV_PORT` > 默认 `6002`
  - Redis 地址：
    - SQLite `service_config.redis_url`（非默认值）> `REDIS_URL` > 默认 `redis://127.0.0.1:6379`
  - SQLite 配置库路径：
    - `VOLTAGE_DB_PATH`（默认 `data/voltage.db`）- 所有服务共享的统一数据库
      - 表为空时，`MODSRV_ALLOW_EMPTY=true` 允许继续启动（用于开发/冷启动）
  - 其它：
    - `MODSRV_PRODUCTS_DIR`（默认 `config/modsrv/products`）
    - `MODSRV_INSTANCES_DIR`（默认 `config/modsrv/instances`）
    - `RUST_LOG` 控制日志级别
  - .env：自动加载（仅文件存在时生效）

- rulesrv（规则服务）
  - 端口：
    - SQLite `service_config.port`（非默认值）> `RULESRV_PORT` > `SERVICE_PORT` > 默认 `6003`
  - Redis 地址：
    - SQLite `service_config.redis_url`（非默认值）> `REDIS_URL` > 默认 `redis://127.0.0.1:6379`
  - SQLite 配置库路径：
    - `VOLTAGE_DB_PATH`（默认 `data/voltage.db`）- 所有服务共享的统一数据库
      - 若不存在或不可用，设置 `RULESRV_ALLOW_EMPTY=true` 可继续启动（使用默认配置），并可不加载规则。
  - 其它：
    - `RUST_LOG` 控制日志级别
  - .env：自动加载（仅文件存在时生效）

> **已弃用的环境变量**（不再使用）：
> - `COMSRV_DB_PATH`, `MODSRV_DB_PATH`, `RULESRV_DB_PATH` - 已被统一的 `VOLTAGE_DB_PATH` 替代
> - `RULESRV_RULES_DB_PATH` - 已合并到统一数据库中

> 说明：docker-compose 中的 `SERVICE_PORT` 现在对 comsrv 与 rulesrv 生效；modsrv 仍推荐使用 `MODSRV_PORT` 覆盖端口。

### 服务配置 (YAML)
```yaml
# config/comsrv/comsrv.yaml
channels:
  - id: 1001
    name: "光伏逆变器通道"
    protocol: "modbus_tcp"
    parameters:
      host: "192.168.1.100"
      port: 502
      timeout_secs: 5
      polling_interval_ms: 1000

  - id: 1002
    name: "储能变流器通道"
    protocol: "modbus_tcp"
    parameters:
      host: "192.168.1.101"
      port: 502
      polling_interval_ms: 1000
```

### 通道数据结构
```
config/comsrv/
├── comsrv.yaml                     # 通道定义
├── {channel_id}/                    # 例如：1001
│   ├── telemetry.csv               # T类型点定义
│   ├── signal.csv                  # S类型点定义
│   ├── control.csv                 # C类型点定义
│   ├── adjustment.csv              # A类型点定义
│   └── mapping/
│       ├── telemetry_mapping.csv   # T点的协议映射
│       ├── signal_mapping.csv      # S点的协议映射
│       ├── control_mapping.csv     # C点的协议映射
│       └── adjustment_mapping.csv  # A点的协议映射
```

### 点定义示例 (CSV)
```csv
# config/comsrv/1001/telemetry.csv
point_id,signal_name,scale,offset,unit,reverse,data_type
1,DC_Voltage,0.1,0,V,false,float32
2,DC_Current,0.01,0,A,false,float32
```

### 协议映射示例 (CSV)
```csv
# config/comsrv/1001/mapping/telemetry_mapping.csv
point_id,slave_id,function_code,register_address,data_type,byte_order
1,1,3,0,float32,ABCD
2,1,3,2,float32,ABCD
```

### 实例配置 (YAML)
```yaml
# config/modsrv/instances.yaml
instances:
  pv_inverter_01:
    product_name: pv_inverter
    config:
      rated_power: 100.0
      efficiency: 0.98
```

### 通道-实例映射 (CSV)
```csv
# config/modsrv/instances/pv_inverter_01/channel_mappings.csv
channel_id,channel_type,channel_point_id,instance_type,instance_point_id,description
1001,T,1,M,1,直流电压输入
1001,T,2,M,2,直流电流输入
```

## 🔧 开发指南

### 项目结构
```
VoltageEMS/
├── services/             # 微服务
│   ├── comsrv/          # 通信服务 (复杂架构，含插件)
│   ├── modsrv/          # 模型服务 (单文件架构)
│   ├── rulesrv/         # 规则引擎 (单文件架构)
├── tools/               # CLI 工具
│   ├── modsrv-cli/      # 模型管理工具
│   ├── comsrv-cli/      # 通信管理工具
│   └── ...
├── libs/                # 共享库 (voltage_libs)
├── scripts/             # 工具脚本
│   ├── redis-functions/ # Redis Lua 函数
│   ├── dev.sh          # 开发环境脚本
│   └── quick-check.sh  # 代码检查脚本
├── config/             # 配置文件
│   ├── comsrv/         # 通信服务配置
│   └── modsrv/         # 模型服务配置
│       └── instances/  # 实例映射配置
└── docker-compose.yml  # 服务编排
```

### 维护脚本

```bash
# 清理废弃的 Redis meta 结构
# 在迁移到点级别时间戳和原始值存储后使用
./scripts/cleanup-meta-structure.sh

# 该脚本会删除已被以下结构替代的旧 comsrv:{channel}:meta 键：
# - comsrv:{channel}:{type}:ts 用于点级别时间戳
# - comsrv:{channel}:{type}:raw 用于原始值
```

### 数据结构（ComSrv）
- 键前缀与类型（三层结构设计）
  - `comsrv:{channel}:{type}`（Hash）
    - 缩放后的工程值；field=`{point_id}`，value=`{string}`（格式化为6位小数）
    - `{type}` ∈ `T`(遥测/测量), `S`(信号/状态), `C`(遥控), `A`(设定值)
  - `comsrv:{channel}:{type}:ts`（Hash）
    - 点级别时间戳；field=`{point_id}`，value=`{unix_timestamp}`（毫秒级Unix时间戳）
    - 每个点独立记录更新时间，支持异步更新
  - `comsrv:{channel}:{type}:raw`（Hash，可选）
    - 缩放前的原始值；field=`{point_id}`，value=`{string}`
    - 保留原始采集数据，便于调试和审计
  - `comsrv:{channel}:{C|A}:TODO`（List，FIFO）
    - 待执行命令队列（RPUSH 入队，BLPOP 消费）
    - 元素 JSON 包含：`command_id`、`channel_id`、`command_type`（C/A）、`point_id`、`value`、`timestamp`、`source`（可选 `priority`）

- 数据流说明
  - 采集写入：调用 `comsrv_batch_update(channel, T|S, updates_json, [raw_values_json])`
    - 批量 `HSET comsrv:{channel}:{T|S}` → 工程值
    - 批量 `HSET comsrv:{channel}:{T|S}:ts` → 时间戳（点级别）
    - 批量 `HSET comsrv:{channel}:{T|S}:raw` → 原始值（如果提供）
    - 通过映射 `route:c2m` 批量路由到 ModSrv 对应实例键
  - 查询：`GET /api/channels/{channel}/{type}/{point_id}`
    - 通过 Rust REST 接口返回包含工程值、点级别时间戳和原始值的 JSON
  - 命令下发：HTTP `POST /api/channels/{channel_id}/points/{point_id}/{control|adjustment}` 或服务内部推送
    - `HSET comsrv:{channel}:{C|A}`（记录最新状态）→ `RPUSH comsrv:{channel}:{C|A}:TODO`（队列）
    - 协议层消费 BLPOP 执行到设备

- 映射索引（由 ModSrv 维护，ComSrv 路由时使用）
  - `route:c2m`（Hash）：`comsrv:{channel}:{type}:{point}` → `modsrv:{instance_name}:{M|A}:{point}`
  - `route:m2c`（Hash）：`modsrv:{instance_name}:{M|A}:{point}` → `comsrv:{channel}:{C|A}:{point}`

- 示例
  - 点表：`HSET comsrv:1001:T "1" "230.5"`
  - 命令入队：`RPUSH comsrv:1001:A:TODO '{"point_id":7,"value":12.3,"timestamp":...}'`

### 数据结构（ModSrv）
- 映射索引（运行期路由的唯一事实来源）
  - `route:c2m`（Hash）：`comsrv:{channel}:{type}:{point}` → `modsrv:{instance_name}:{M|A}:{point}`
  - `route:m2c`（Hash）：`modsrv:{instance_name}:{M|A}:{point}` → `comsrv:{channel}:{C|A}:{point}`

- 实例目录（管理/展示）
  - `instance:index`（Set）：全部实例名称
- `instance:{instance_name}:info`（Hash）：`id`、`product_name`、`properties`(JSON)、`created_at`、`updated_at`
  - `instance:{instance_name}:parameters`（Hash）：运行参数（k→v）
  - `instance:{instance_name}:mappings`（Hash，可选）：`M:{pid}`/`A:{pid}` → Redis键（用于展示）

- 产品目录（只读缓存）
  - `modsrv:products`（Set）：产品ID
  - `modsrv:product:{pid}`（Hash）：`definition`(JSON)、`updated_at`
  - `modsrv:product:{pid}:measurements|actions|properties`（Hash）：点/属性定义（id/name 等JSON）

- 实例运行态
  - `modsrv:{instance_name}:M`（Hash）：测量点表，field=`{point_id}`，value=`{string}`
  - `modsrv:{instance_name}:A`（Hash）：动作点当前目标值（可视化）
  - `modsrv:{instance_name}:status`（Hash）：`state`、`last_update`、`health`、`errors`
  - `modsrv:{instance_name}:config`（Hash）：由 properties 初始化的配置缓存
  - 统计：`modsrv:stats:routed`（Hash）：按 `channel_id` 累积路由计数（诊断用途）

- 动作下发（实例语义 → 设备命令）
  - 入口：ModSrv API（`modsrv_execute_action`）或 RuleSrv 实例动作
  - 路径：写 `modsrv:{instance_name}:A`（可视化）→ 查 `route:m2c` → `RPUSH comsrv:{channel}:{C|A}:TODO`

- 示例
  - 实例动作（通过函数）：`FCALL modsrv_execute_action 0 "pv_inv_01" '{"action_id":"7","value":1}'`

### 数据结构（RuleSrv）
- 规则定义存储在 SQLite `rules` 表中，字段包含 `id`、`name`、`description`、`flow_json`、`enabled`、`priority` 及时间戳。
- 规则增删改查与启停通过 REST 接口 `/api/rules/*` 完成，已不再提供 `FCALL rulesrv_*`。
- 运行时条件/字段引用继续沿用 ModSrv 语法：`{instance}.{M|A}.{point}`，并支持 `SUM/AVG/MAX/MIN/COUNT(...)` 聚合。



### 构建命令
```bash
# 检查编译
cargo check --workspace

# 构建所有服务
cargo build --workspace

# 发布版本构建
cargo build --release --workspace

# 运行测试
cargo test --workspace

# 代码格式化
cargo fmt --all

# 代码检查
cargo clippy --all-targets --all-features -- -D warnings
```

### Redis Lua 函数

系统包含 7 个 Lua 函数库：
- **comsrv**: 数据采集、批量更新、命令路由
- **modsrv**: 点位映射、实例数据路由
- **alarmsrv**: 告警检查和管理
- **rulesrv**: 规则执行引擎
- **hissrv**: 历史数据操作
- **core**: 通用工具函数
- **services**: 跨服务工具

重要函数：
- `comsrv_batch_update`: 批量更新数据并自动路由到 modsrv
- `modsrv_route_data`: 路由数据到模型实例
- `modsrv_load_mappings`: 加载通道到实例的映射关系

## 🎯 核心功能与优化

### 实时数据流 (2025-09)
- **自动数据路由**: comsrv_batch_update Lua 函数自动路由数据到 modsrv
- **基于实例的建模**: 从 instances.yaml 加载有意义的实例名称
- **零轮询架构**: 通过 Redis 映射实现事件驱动的数据流
- **通道到实例映射**: 基于 CSV 配置的灵活数据路由

### 性能优化
- **混合处理**: Rust 处理 I/O，Redis Lua 处理业务逻辑（微秒级延迟）
- **单文件服务**: modsrv、alarmsrv、rulesrv 采用简化架构
- **直接 Redis 操作**: 消除不必要的抽象层
- **优化的 Docker 构建**: 统一镜像包含所有服务（体积减少约 20%）

## 🔍 监控与调试

### 日志查看
```bash
# 查看服务日志
docker-compose logs -f comsrv modsrv

# 启用调试级别
RUST_LOG=debug,comsrv=trace cargo run --bin comsrv
```

### Redis 监控
```bash
# 监控 Redis 活动
redis-cli MONITOR | grep comsrv

# 检查数据
redis-cli HGETALL "comsrv:1001:T"

# 查看映射
redis-cli HGETALL "route:c2m"

# 检查实例数据（运行态为哈希）
redis-cli HGET "modsrv:pv_inverter_01:M" "1"
```

## 🔑 关键设计决策

1. **简单优先**: 避免过度设计，尽可能使用单文件服务
2. **性能至上**: 将热路径逻辑委托给 Redis Lua 函数
3. **配置分层**: 基础设施使用环境变量，业务逻辑使用 YAML
4. **映射驱动**: 通过 CSV 文件定义灵活的数据映射关系
5. **实时性保证**: 事件驱动架构，无轮询延迟

## 🤝 贡献指南

1. Fork 本仓库
2. 创建功能分支 (`git checkout -b feature/amazing-feature`)
3. 提交更改 (`git commit -m 'Add some amazing feature'`)
4. 推送到分支 (`git push origin feature/amazing-feature`)
5. 创建 Pull Request

## 📄 许可证

本项目采用 MIT 许可证 - 详见 [LICENSE](LICENSE) 文件

## 🙏 致谢

- 使用 [Rust](https://www.rust-lang.org/) 构建
- Web 框架: [Axum](https://github.com/tokio-rs/axum)
- 内存数据库: [Redis](https://redis.io/)
- 时序数据库: [InfluxDB](https://www.influxdata.com/)

## 📞 联系方式

- 项目地址: [https://github.com/your-org/VoltageEMS](https://github.com/your-org/VoltageEMS)
- 问题反馈: [https://github.com/your-org/VoltageEMS/issues](https://github.com/your-org/VoltageEMS/issues)
