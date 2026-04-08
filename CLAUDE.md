# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## 核心约束

单人项目，YAGNI 原则。**禁止**: `mod.rs` | 硬编码 Redis 键 | 编译时 SQLx 宏 | 过度工程化

## 常用命令

```bash
# 日常开发
./scripts/quick-check.sh                  # fmt + clippy + tests + frontend
cargo test -p comsrv --lib                # 单 crate 测试（最快反馈）
cargo test -p modsrv --test test_shm_dispatch  # 单个集成测试

# 构建部署
./scripts/build-installer.sh -s rust      # ARM64 Rust 服务打包 → release/*.run
scp release/MonarchEdge-arm64-*.run root@192.168.30.21:/tmp/
ssh root@192.168.30.21 '/tmp/MonarchEdge-arm64-*.run'

# 配置管理
monarch init && monarch sync              # 配置初始化并同步到 SQLite
monarch services start/stop/refresh       # 服务管理
```

## 服务端口

| 服务 | 端口 | 服务 | 端口 |
|------|------|------|------|
| voltage-apps | 8080 | comsrv | 6001 |
| modsrv | 6002 | hissrv | 6004 |
| apigateway | 6005 | netsrv | 6006 |
| alarmsrv | 6007 | voltage-redis | 6379 |

## 项目结构

```
libs/
  common          — 共享 bootstrap、logging、test_utils/schema、dependency（启动依赖检查）
  errors          — 统一 VoltageError + ErrorCategory → HTTP status 映射
  voltage-model   — PointType、KeySpaceConfig、产品常量（编译时）
  voltage-routing — RoutingCache、set_action_point
  voltage-rtdb    — Rtdb trait + RedisRtdb + MemoryRtdb
  voltage-rtdb-shm — 统一 SHM（UnifiedWriter/Reader）、UDS notifier、bitmap、snapshot
  voltage-rules   — 规则引擎：parser → scheduler → executor
  voltage-calc    — 公式求值、CalcEngine
  voltage-core    — no_std 核心类型（固件共用）
  voltage-shm     — 平台抽象 SHM（含 embedded RawPtrShm）
  voltage-infra   — Redis/SQLite 连接池封装
services/
  comsrv, modsrv, apigateway, hissrv, netsrv, alarmsrv
tools/
  monarch（CLI 管理工具）, simulator
workspace-hack/ — cargo-hakari 生成，统一 feature flags（勿手动编辑）
```

## 服务间通信

| 路径 | 机制 | 延迟 |
|------|------|------|
| comsrv → all（数据） | SHM 直写 + 后台异步 Redis 同步 | <1ms（SHM），~100ms（Redis） |
| comsrv → modsrv（读数） | SHM mmap 零拷贝 | <1ms |
| modsrv → comsrv（M2C 命令） | SHM write + UDS notify | ~1–2ms |
| alarmsrv → apigateway/netsrv | HTTP POST (reqwest) | ~5ms |
| netsrv → cloud | MQTT | network |
| apigateway → browsers | WebSocket | network |
| all ↔ SQLite | sqlx (in-process) | local |

**启动顺序**: comsrv 必须先于 modsrv 启动（comsrv 创建 SHM + routing_hash，modsrv 打开时验证）。modsrv 使用 `common::dependency::wait_for_dependency()` 等待 comsrv health。

## Rtdb trait 设计边界

`Rtdb` trait（`voltage-rtdb/src/traits.rs`）使用 AFIT，**不是 object-safe**，只能泛型 `<R: Rtdb>` 使用。

- **comsrv/modsrv** 的核心结构体（`ChannelManager<R>`、`InstanceManager<R>`）是泛型的 → 单元测试用 `MemoryRtdb` 不需要 Redis
- **apigateway/hissrv/netsrv/alarmsrv** 直接持有 `Arc<RedisRtdb>` → 这些服务 Redis 交互简单，不值得泛型化
- **MemoryRtdb 是纯测试替身**，不是 SHM 的抽象。SHM（定长 PointSlot 数组 + seqlock）和 Rtdb（KV/Hash/List/Set）数据模型不兼容
- **不要尝试**将 Rtdb trait 向其他服务传播或用于 SHM 抽象

## 协议扩展

comsrv 协议通过 `ChannelRuntime` trait（object-safe，`#[async_trait]`）+ 编译时 feature gates：
1. 在 `services/comsrv/src/protocols/adapters/` 加适配器模块
2. 实现 `ChannelRuntime` trait
3. 在 `protocols/gateway/factory.rs` 加 `#[cfg(feature = "...")]` 分支
4. 在 `services/comsrv/Cargo.toml` 声明 feature

当前 13 个协议：Modbus、IEC 104、OPC UA、MQTT、HTTP、DL/T 645、CAN/J1939、GPIO、BLE、Zigbee、Matter、Voltage-485、Virtual。

## 关键模式

```rust
KeySpaceConfig::production().channel_key(1001, PointType::Telemetry)  // Redis 键
sqlx::query_as::<_, Row>("SELECT * FROM t WHERE id = ?").bind(id)     // SQLx（禁编译时宏）
```

## 数据流

```
上行: Device → comsrv → SHM(T/S slots) [热路径, ~10ns/点]
                      → ShmRedisSync (100ms) → Redis pipeline → comsrv:{ch}:{T|S} + inst:{id}:M
下行: modsrv → SHM(C/A slots) write + UDS notify → comsrv ShmCommandListener → Device
     （路由配置来自 route:m2c 表，运行时数据不经 Redis）
```

**ShmRedisSync**: 后台任务扫描 SHM 脏槽（seq 变化检测），pipeline 批量写 Redis。
包含 C2M 路由（channel → inst:{id}:M）、24h TTL 刷新（~60s 一次）、routing reload 自动 reset。
`ReverseSlotIndex`（slot → channel/point 反向映射）支撑脏槽到 Redis key 的还原。

## SHM 架构

**文件**: `/shm/rtdb/voltage-rtdb.shm`（Docker tmpfs），`UnifiedHeader(64B) + PointSlot[N](32B each)`

**写者所有权**: comsrv 拥有 T/S 槽，modsrv 拥有 C/A 槽。**永远不要交叉写入。**

**关键 header 字段**:
- `routing_hash` — comsrv 和 modsrv 必须匹配，否则 modsrv 拒绝打开
- `writer_generation` — comsrv 每次 create/reconfigure 递增，modsrv dispatch 时检测不一致

**M2C 通知**: `ShmNotifier` → UDS(`/tmp/voltage-m2c.sock`) → `ShmCommandListener`
- 48 字节 `ShmNotification`，`producer_id + seq` 去重
- UDS 失败自动重连（指数退避 1–5s），无轮询降级

**Seqlock**: `load_consistent()` 返回 `Option`（重试耗尽返回 None，不返回撕裂数据）

## 规则引擎

**双列存储（关键不变量）**:
- `flow_json` — 前端 Vue Flow 完整 JSON（含 UI 布局）
- `nodes_json` — 紧凑执行拓扑（`RuleFlow { start_node, nodes }`）
- **两列必须同步更新**：API PUT 调用 `extract_rule_flow()`，`monarch sync` 同样调用

**执行流**: Scheduler(100ms tick) → Executor → RTDB write + SHM C/A write + UDS notify
**执行结果**: 写入 Redis `rule:{id}:exec`（24h TTL），WebSocket 直接推送

## 配置流

```
config/*.yaml → monarch sync → SQLite(voltage.db) → 服务启动时加载
```

服务不直接读 YAML，所有配置经 `monarch sync` 写入 SQLite 后生效。

## 错误处理

`errors` crate: `VoltageError`（~35 variants）→ `ErrorCategory`（16 variants）→ HTTP status。
`VoltageErrorTrait` 定义统一接口（`error_code()`、`is_retryable()`、`http_status()`）。

- **comsrv**: `ComSrvError`（15 variants）实现 `VoltageErrorTrait`，有完整错误链
- **modsrv/hissrv/netsrv/alarmsrv**: 用 `anyhow::Result`（内部服务，不直接面向前端）
- **apigateway**: 面向前端，通过 `VoltageError` 映射 HTTP status

API 响应统一格式：`{ success, data, error: { code, message, details }, meta }`

## 测试

- `common::test_utils::schema` — 共享 DDL 常量（`init_comsrv_schema()`、`init_modsrv_schema()`、`init_rules_schema()`）
- `create_test_rtdb()` → `Arc<MemoryRtdb>`（单元测试不需要 Redis）
- `noop_dispatch()` → `Arc<NoopDispatch>`（无 SHM 的 InstanceManager 测试）
- 集成测试需 Redis，用 `tempfile::TempDir` 做 SQLite
- hissrv `StorageBackend` trait 有 4 个实现（Null/Pg/Timescale/Influx），支持运行时热切换

## workspace-hack (cargo-hakari)

`workspace-hack/` 由 cargo-hakari 生成，统一所有 crate 的 feature flags 以提升编译缓存命中率。

```bash
cargo hakari generate          # 依赖变更后重新生成
cargo hakari manage-deps       # 新增 crate 后同步依赖
cargo hakari verify            # CI 中验证一致性（已集成到 quality-check job）
```

**不要手动编辑** `workspace-hack/Cargo.toml`。
