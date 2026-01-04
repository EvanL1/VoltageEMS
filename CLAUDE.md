# CLAUDE.md

## 核心约束

单人开发运维项目，遵循 YAGNI 原则：简单优先、够用即可、禁止过度工程化。

**禁止**: 使用 `mod.rs` 组织 Rust 代码 | 硬编码 Redis 键字符串 | 编译时 SQLx 宏

## 常用命令

```bash
# 开发
./scripts/quick-check.sh              # fmt + clippy + unit tests
cargo test --lib                      # 单元测试

# 配置管理
monarch init all && monarch sync all  # 初始化并同步
monarch status --detailed             # 检查状态

# 服务管理
monarch services start                # 启动服务
monarch services refresh --smart      # 智能刷新镜像
monarch logs level all debug          # 动态切换日志级别
```

## 服务端口

| 服务 | 端口 | 说明 |
|------|------|------|
| comsrv | 6001 | Rust 通讯服务（Modbus/CAN/虚拟协议） |
| modsrv | 6002 | Rust 模型计算 + 规则引擎 |
| hissrv | 6004 | Python 历史数据（InfluxDB） |
| apigateway | 6005 | Python API 网关 |
| voltage-redis | 6379 | Redis |

**核心服务**: comsrv + modsrv 仅依赖 Redis，不依赖 InfluxDB

## 项目结构

```
libs/
  voltage-model/     # 核心域模型（KeySpaceConfig、PointType）- 类型唯一来源
  voltage-routing/   # M2C 路由
  voltage-rtdb/      # Redis 抽象层
services/
  comsrv/           # 通讯服务
  modsrv/           # 模型服务
tools/monarch/      # 配置 CLI
```

## 关键模式

```rust
// Redis 键生成（必须使用 KeySpaceConfig）
let config = KeySpaceConfig::production();
config.channel_key(1001, PointType::Telemetry)  // => "comsrv:1001:T"
config.instance_measurement_key(5)               // => "inst:5:M"

// SQLx 运行时查询（禁止编译时宏）
sqlx::query_as::<_, Row>("SELECT * FROM t WHERE id = ?").bind(id)
```

## 数据流

```
上行: Device → comsrv → Redis → route:c2m → inst:{id}:M
下行: modsrv → route:m2c → inst:{id}:A → comsrv TODO 队列
```

## 扩展文档

- 数据库结构: `docs/DATABASE_STRUCTURE_CN.md`
- 运维日志: `docs/operations-log.md`
