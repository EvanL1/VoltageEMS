# VoltageEMS - 工业物联网能源管理系统

[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.90%2B-orange.svg)](https://www.rust-lang.org/)
[![Docker](https://img.shields.io/badge/docker-ready-blue.svg)](https://www.docker.com/)

[English Version](README.md)

VoltageEMS 是一个基于 Rust 微服务架构构建的高性能工业物联网能源管理系统。它为工业能源管理场景提供实时数据采集、处理和监控能力。

## 🚀 核心特性

- **高性能架构**: 使用 Rust 构建，确保最佳性能和内存安全
- **集中式配置**: 所有数据结构和模型定义在 voltage-model 库中统一维护
- **微服务架构**: Rust 服务配合 Redis 实现实时数据处理
- **实时数据流**: 通过 Redis 映射自动从设备路由数据到模型
- **多协议支持**: Modbus TCP/RTU、Virtual、gRPC，支持插件扩展
- **基于模型的系统**: 实例化数据建模，支持产品层次结构
- **事件驱动设计**: 通过 Redis 路由实现实时数据流
- **RESTful API**: 所有服务提供标准 HTTP/JSON 接口
- **Docker 就绪**: 完全容器化部署，支持 docker-compose
- **CLI 工具集**: 提供完善的命令行工具进行服务管理
- **配置管理工具**: Monarch 工具实现 YAML/CSV 与 SQLite 双向同步

## 🏗️ 系统架构

### 整体架构
```
                ┌─────────────┐
                │    设备      │ (Modbus, Virtual, gRPC, CAN)
                └──────┬──────┘
                       │
       ┌───────────────┴───────────────────────────┐
       ▼                                           ▼
┌──────────────────┐                      ┌──────────────────┐
│    微服务集群      │                      │   前端应用        │
│                  │                      │   (Vue.js)       │
│ comsrv(:6001)    │ ← 通信服务            └──────────────────┘
│ modsrv(:6002)    │ ← 模型服务 + 规则引擎
└──────────────────┘
         │
         ▼
┌───────────────────────────────┐
│ Redis(:6379)                  │
└───────────────────────────────┘
```

### 数据流架构
```
上行（设备 → 模型）:
  设备 → comsrv → Redis Hash → route:c2m → inst:{id}:M

下行（控制 → 设备）:
  1. 查询 route:m2c 找到目标通道
  2. 写入 inst:{id}:A Hash（状态）
  3. 推送到 comsrv TODO 队列（触发）
```

## 📦 服务说明

| 服务 | 端口 | 功能描述 |
|------|------|----------|
| **comsrv** | 6001 | 通信服务 - 处理工业协议和数据采集 |
| **modsrv** | 6002 | 模型服务 - 管理数据模型、计算引擎和规则引擎 |
| **redis**  | 6379 | 内存数据存储 |

提示：docker-compose 运行 comsrv、modsrv（含规则引擎）与 Redis。规则引擎已集成到 modsrv（端口 6002）。

## 🛠️ 技术栈

- **编程语言**: Rust 1.90+
- **Web 框架**: Axum 0.8
- **数据库**: Redis 8+, InfluxDB 3.x
- **容器技术**: Docker, Docker Compose
- **消息格式**: JSON, Protocol Buffers
- **构建工具**: Cargo

## 🚦 快速开始

### 环境要求

- Rust 1.90+ ([安装 Rust](https://rustup.rs/))
- Docker & Docker Compose
- Redis 8+ (开发环境)

### 开发环境设置

1. 克隆仓库:
```bash
git clone https://github.com/your-org/VoltageEMS.git
cd VoltageEMS
```

2. 初始化配置:
```bash
cargo build --release -p monarch
./target/release/monarch init all && ./target/release/monarch sync all
```

3. 运行特定服务:
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

2. 验证服务:
```bash
# 查看日志
docker compose logs -f comsrv modsrv

# 检查服务健康
curl http://localhost:6001/health  # comsrv
curl http://localhost:6002/health  # modsrv（含规则引擎）

# 检查实例数据
docker exec voltageems-redis redis-cli HGETALL "inst:1:M"
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

- 规则引擎（集成在 modsrv）
  - 端口：6002（与 modsrv 共用端口）
  - 配置：与 modsrv 共享配置
  - API：`/api/rules/*` 用于规则管理

> **已弃用的环境变量**（不再使用）：
> - `COMSRV_DB_PATH`, `MODSRV_DB_PATH`, `RULES_DB_PATH` - 已被统一的 `VOLTAGE_DB_PATH` 替代

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
├── libs/
│   ├── voltage-model/       # 数据结构定义（权威来源）
│   ├── voltage-routing/     # M2C 路由共享库
│   ├── voltage-rtdb/        # Redis 抽象层
│   ├── voltage-rules/       # 规则引擎库
│   └── common/              # 通用工具
├── services/
│   ├── comsrv/              # 通信服务
│   └── modsrv/              # 模型服务 + 规则引擎
├── tools/monarch/           # 配置管理 CLI (YAML/CSV → SQLite)
├── apps/                    # Vue.js 前端
├── config/                  # YAML/CSV 配置源
├── scripts/                 # 运维脚本
└── docker-compose.yml
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
  - 采集写入：通过 Rust `RoutingCache` 批量更新
    - 批量 `HSET comsrv:{channel}:{T|S}` → 工程值
    - 批量 `HSET comsrv:{channel}:{T|S}:ts` → 时间戳（点级别）
    - 通过映射 `route:c2m` 路由到 ModSrv（应用层路由）
  - 查询：`GET /api/channels/{channel}/{type}/{point_id}`
    - 返回包含工程值和时间戳的 JSON
  - 命令下发：HTTP `POST /api/channels/{channel_id}/points/{point_id}/{control|adjustment}`
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
  - 入口：ModSrv API 或规则引擎动作
  - 路径：写 `inst:{id}:A` → 查 `route:m2c` → `RPUSH comsrv:{channel}:{C|A}:TODO`

- 示例
  - `POST /api/instances/1/action {"action_id": 7, "value": 1}`

### 数据结构（规则引擎 - 集成在 modsrv）
- 规则定义存储在 SQLite `rules` 表中，字段包含 `id`、`name`、`description`、`flow_json`、`enabled`、`priority` 及时间戳。
- 规则增删改查与启停通过 6002 端口 REST 接口 `/api/rules/*` 完成。
- 运行时字段引用使用 ModSrv 语法：`{instance}.{M|A}.{point}`，支持 `SUM/AVG/MAX/MIN/COUNT(...)` 聚合。



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

## 🎯 核心功能与优化

### 实时数据流
- **应用层路由**: Rust `RoutingCache` 实现 C2M/M2C 路由
- **基于实例的建模**: 从 SQLite 加载有意义的实例名称
- **事件驱动架构**: 通过 Redis 映射实现实时数据流
- **通道到实例映射**: 基于 CSV 配置通过 Monarch 同步

### 性能优化
- **纯 Rust 处理**: 所有路由在 Rust 中完成，性能一致
- **服务整合**: modsrv 包含规则引擎（单一部署）
- **DashMap 路由缓存**: 内存路由，Redis 作为数据源
- **优化的 Docker 构建**: 统一镜像包含所有服务

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

1. **简单优先**: 避免过度设计，服务整合减少运维复杂度
2. **性能至上**: 应用层路由，DashMap 内存缓存
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
