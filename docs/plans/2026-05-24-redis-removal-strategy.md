# Redis Removal Strategy

**Status**: Analysis / Decision Record
**Date**: 2026-05-24
**Author**: Evan

## TL;DR

Redis 在 VoltageEMS 早期承担"实时数据库"角色；SHM 落地后被降级为
**二级慢速视图**。`ShmRedisSync` 每 100ms 把 SHM 内容镜像到 Redis，纯粹是
为了让 4 个外围服务（apigateway/hissrv/alarmsrv/netsrv）能跨进程读到当前值。

由于 SHM seqlock 天然支持 **多读单写**（MRSW），且
`monarch shm` 已经覆盖了 redis-cli 的所有调试用途，**Redis 现在没有任何
"非它不可"的角色**。但去掉 Redis 不是零成本，需要分阶段迁移。

本文档：

1. 审计每个服务对 Redis 的实际依赖
2. 区分"易迁移"、"需要换设计"、"运维改动"三类影响
3. 提出渐进式三步迁移计划
4. 列出尚未决策的开放问题

---

## 1. 现状：Redis 在做什么

### 1.1 写入方

| 写入方 | Key | 类型 | 用途 |
|---|---|---|---|
| comsrv (ShmRedisSync) | `comsrv:{ch}:T` / `:S` | Hash | SHM → Redis mirror（telemetry / status） |
| comsrv (ShmRedisSync) | `inst:{id}:M` | Hash | C2M 路由结果 mirror |
| comsrv (channel_task) | `comsrv:online` | Hash | **channel 在线状态广播**（非 mirror） |
| modsrv | `inst:{id}:A` | Hash | Action 当前值 mirror |
| 规则引擎 | `rule:{id}:exec` | Hash + TTL | 规则执行审计（24h TTL） |
| 路由配置 | `route:m2c` 等 | Hash | 路由表缓存 |

### 1.2 读取方（4 个外围服务）

| 服务 | 用法 | 调用点 |
|---|---|---|
| **apigateway** | `hash_get` / `hash_get_all` 读 instance 当前值 | `ws.rs:281,290,401,439` |
| **hissrv** | `scan_match("inst:*:M")` + `hash_get_all` 批量采样 | `collector.rs:28,44` |
| **alarmsrv** | `hash_get` 读阈值比对的当前值 | `monitor.rs:96` |
| **netsrv** | `scan_match` + `hash_get` 上报 MQTT | `forwarder.rs:120,139`, `mqtt.rs:296,312` |

### 1.3 Redis 已经**没在用**的能力

- ❌ Pub/Sub（事件广播）
- ❌ Streams（持久化事件流）
- ❌ Sorted Sets（带排序集合）
- ❌ Lua scripts（原子多步操作）
- ❌ HyperLogLog / Bitmaps / Geo / Time Series

**结论**：只用到 Hash + KV + TTL + SCAN，是 Redis 全部能力的 ~10%。

---

## 2. 为什么 Redis 是"架构层死代码"

### 2.1 历史角色 vs 今天角色

```
2024 (早期):                       2026 (今天):
┌──────────┐                       ┌──────────┐
│  Redis   │ ← 真相源              │   SHM    │ ← 真相源
└────┬─────┘                       └────┬─────┘
     │                                  │
     ↓                                  ↓
[所有服务读写]                       [comsrv/modsrv 直接读写]
                                        │
                                  ShmRedisSync (100ms)
                                        ↓
                                   ┌──────────┐
                                   │  Redis   │ ← 二级视图
                                   └────┬─────┘
                                        ↓
                                  [apigateway/hissrv/...]
```

### 2.2 "用 Redis 监控 SHM"理由已破产

`monarch shm` 工具已覆盖：

| 用途 | redis-cli | `monarch shm` |
|---|---|---|
| 看 instance 当前值 | `HGET inst:1:M 5` | `get inst:1:M:5` |
| 实时盯一个点 | `--watch` | `watch inst:1:M:5` |
| 全局 dashboard | RedisInsight | `top` (TUI) |
| SHM 总览 | n/a | `info` |

而且 `monarch shm` 读的是 **SHM 真相源**，不是 100ms 延迟的 mirror。

### 2.3 多 reader 技术可行性

- **算法层**：seqlock 设计本身就是 MRSW，`test_seqlock_multi_reader_stress`
  已经验证 4 reader + 1 writer 工作正常
- **OS 层**：POSIX `mmap(MAP_SHARED)` 对 reader 数量无限制
- **现状证明**：modsrv 已经是跨进程 SHM reader，证明模式 work

---

## 3. 去掉 Redis 的成本分析

### A 类：直接搬（80% 用例，每处 5-10 行）

| 服务 | 调用点数 | 难度 |
|---|---|---|
| apigateway WS | 4 | 🟢 直接换 `UnifiedReader.read_slot()` |
| alarmsrv monitor | 1 | 🟢 同上 |
| hissrv collector | 1 | 🟢 同上（配合 B1） |
| netsrv mqtt | 2 | 🟢 同上 |

工作量估计：**1 天**

### B 类：需要换设计

#### B1. `scan_match("inst:*:M")` — 用 Redis 来"发现 instance"

**当前**：hissrv/netsrv 用 SCAN 模式匹配发现所有 instance。

**问题**：SHM 没有 KEYS pattern API（设计上就不该有 — 固定 slot 数组）。

**解决方案**：从 SQLite `instances` 表 enumerate（**这本来就是 instance
是否存在的真相源**）。

```rust
// Before
let keys = rtdb.scan_match("inst:*:M").await?;

// After
let instance_ids: Vec<u32> = sqlx::query_scalar(
    "SELECT instance_id FROM instances"
).fetch_all(&pool).await?;
```

**附带收益**：消除了 Redis 作为"发现真相"的隐式角色，更符合
"SQLite = 配置真相源，SHM = 当前值真相源"的设计原则。

工作量估计：**1 天**

#### B2. ⚠️ `comsrv:online` channel 健康状态广播（**最棘手**）

**当前**：
- comsrv 写 `publish_channel_online(channel_id, true/false)` 到 Redis hash
- modsrv 读 `ChannelHealthCache.refresh()` 拉这个 hash，**用来 fail-closed C/A 写入**

**这不是 mirror，是 Redis 在承担的真实工作** — 跨服务的轻量级状态广播。

**候选方案**：

1. **SHM header 加 channel_state bitmap**
   - 每个 channel 1 bit，N channels = N/8 bytes
   - comsrv 单写、modsrv 多读，符合 MRSW 模型
   - 最 elegant，但要扩展 SHM header 协议
2. **HTTP 查询** `comsrv /health/channel/:id`
   - 增加耦合 + 延迟（每次 dispatch 查一次？还是缓存？）
   - 不推荐
3. **UDS 反向广播**（comsrv → modsrv pub/sub）
   - 类似 M2C notification 的镜像
   - 需要新的 IPC 协议

**推荐**：方案 1（SHM header bitmap）。它把"channel 在线状态"放到已有的
跨进程共享通道里，不引入新依赖。

工作量估计：**2-3 天**（包含 SHM header 协议升级 + 测试）

### C 类：部署/运维改动

| 项目 | 工作量 |
|---|---|
| `/dev/shm/` 容器挂载（4 个新服务加 volume） | 🟢 小（docker-compose YAML） |
| 抽出 `SharedShmReader` helper（含 inode watcher） | 🟡 中（避免每服务都抄一份） |
| 启动顺序依赖（4 个新 reader 都要 `wait_for_dependency(comsrv)`） | 🟢 小（已有 utility） |
| 容器 user/uid 权限对齐 | 🟢 小 |

工作量估计：**0.5 天**

### 总工作量

| 阶段 | 时间 |
|---|---|
| A 类（直接搬） | 1 天 |
| B1（scan_match → SQLite） | 1 天 |
| B2（channel_online 重设计） | 2-3 天 |
| C 类（部署） | 0.5 天 |
| 测试 + 集成验证 | 1-2 天 |
| **总计** | **~1 周** |

---

## 4. 渐进式迁移计划

### Step 1：多 reader 支持（保留 Redis）

**目标**：让 4 个外围服务**多一条 SHM 读取路径**，可选启用。Redis 保持
正常工作不动。

**做什么**：
- A 类的 4 个服务各加一个 SHM reader 模块（开关由 feature flag 或
  config 控制）
- C 类的部署改动
- 加一个 `SharedShmReader` helper crate（或 module）

**好处**：
- 实测多 reader 稳定性
- 任何一个服务出问题可以一键回滚到 Redis 路径
- 验证容器 `/dev/shm/` 挂载在生产无副作用

**风险**：极低（Redis 路径还在，新路径只是 opt-in）

### Step 2：消除 scan_match（解耦"发现真相"）

**目标**：B1 — 把所有 `scan_match("inst:*:M")` 等替换为 SQLite enumerate。

**做什么**：
- hissrv/netsrv/modsrv 的 4 个 `scan_match` 调用点全部改成 SQLite query
- 加 `enumerate_instance_ids() -> Vec<u32>` helper 到 common crate

**好处**：
- 单独就有架构清晰收益（即使不去 Redis 也应该做）
- 不依赖 Step 1 完成

**风险**：低（纯重构，SQLite query 已经在 hot path 上）

### Step 3：删除 Redis（最后一步）

**前提**：Step 1 + Step 2 已上线稳定

**做什么**：
- B2：实现 channel_online SHM bitmap 广播
- 关闭 `ShmRedisSync`
- 4 个外围服务的 Redis 路径删除
- voltage-redis 容器下线
- 移除所有 `voltage_rtdb::RedisRtdb` 依赖

**好处**：
- 砍掉 ~400 行 ShmRedisSync 代码
- 一个进程下线（运维负担减半）
- 架构变成"SHM = 实时真相，SQLite = 配置真相"的双源清晰模型

**风险**：中（需要充分集成测试，特别是 channel_online 的边界场景）

---

## 5. 开放问题（决策时机：开始 Step 3 之前）

1. **B2 选哪个方案？**
   - SHM header bitmap（推荐）vs HTTP 查询 vs UDS 反向 pub
2. **`rule:{id}:exec` 的 24h TTL 怎么办？**
   - SQLite 表 + cron 清理？还是引入轻量级时间序列方案？
3. **未来如果加分布式特性怎么办？**
   - 比如多节点 EMS 集群 — 那时是否需要 Redis 回归？
   - 但当前是单机部署，YAGNI
4. **Pub/Sub 类需求是否会出现？**
   - 比如告警事件流广播给多个订阅者
   - 如果会，那么 Step 3 之前要先解决（或保留 Redis 仅用于 Pub/Sub）

---

## 6. 不推荐做的事

- ❌ **直接动手 Step 3** —— 跳过验证步骤风险太高
- ❌ **保留 Redis 做"调试窗口"** —— `monarch shm` 已经更好
- ❌ **新加 Redis 功能**（Streams、Sorted Sets 等）—— 应该往删除方向走，不是加深依赖
- ❌ **同时改造 4 个服务** —— 一次一个，验证一个

---

## 附录：相关代码位置

- SHM 实现：`libs/voltage-rtdb-shm/`
- Seqlock：`libs/voltage-rtdb-shm/src/core/slot.rs::try_load_consistent`
- 多 reader 验证：`test_seqlock_multi_reader_stress`
- ShmRedisSync：`services/comsrv/src/store/shm_redis_sync.rs`
- `comsrv:online` 写入：`services/comsrv/src/core/channels/channel_task.rs`
- `comsrv:online` 读取：`services/modsrv/src/infra/channel_health.rs`
- SHM 调试工具：`tools/monarch/src/shm.rs`、`tools/monarch/src/shm_dashboard.rs`
