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
  common          — 共享 bootstrap、logging、test_utils/schema
  errors          — 统一 VoltageError + ErrorCategory → HTTP status 映射
  voltage-model   — PointType、KeySpaceConfig、产品常量（编译时）
  voltage-routing — RoutingCache、set_action_point
  voltage-rtdb    — RedisRtdb
  voltage-rtdb-shm — 统一 SHM（UnifiedWriter/Reader）、UDS notifier、bitmap、snapshot
  voltage-rules   — 规则引擎：parser → scheduler → executor
  voltage-calc    — 公式求值、CalcEngine
  voltage-core    — no_std 核心类型（固件共用）
  voltage-shm     — 平台抽象 SHM（含 embedded RawPtrShm）
services/
  comsrv, modsrv, apigateway, hissrv, netsrv, alarmsrv
tools/
  monarch（CLI 管理工具）, simulator
```

## 关键模式

```rust
KeySpaceConfig::production().channel_key(1001, PointType::Telemetry)  // Redis 键
sqlx::query_as::<_, Row>("SELECT * FROM t WHERE id = ?").bind(id)     // SQLx（禁编译时宏）
```

## 数据流

```
上行: Device → comsrv → SHM(T/S slots) + Redis → route:c2m → inst:{id}:M
下行: modsrv → SHM(C/A slots) write + UDS notify → comsrv ShmCommandListener → Device
     （路由配置来自 route:m2c 表，运行时数据不经 Redis）
```

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

`errors` crate: `VoltageError` → `ErrorCategory` → HTTP status。
每服务可扩展（如 `ModSrvError::DispatchDegraded` → 502）。
API 响应统一格式：`{ success, data, error: { code, message, details }, meta }`

## 测试

- `common::test_utils::schema` — 共享 DDL 常量
- `noop_dispatch()` → `Arc<NoopDispatch>`（无 SHM 的 InstanceManager 测试）
- 集成测试需 Redis，用 `tempfile::TempDir` 做 SQLite
