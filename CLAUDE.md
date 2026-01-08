# CLAUDE.md

## 核心约束

单人开发运维项目，遵循 YAGNI 原则：简单优先、够用即可、禁止过度工程化。

**禁止**: 使用 `mod.rs` 组织 Rust 代码 | 硬编码 Redis 键字符串 | 编译时 SQLx 宏

## 常用命令

```bash
./scripts/quick-check.sh              # fmt + clippy + tests + frontend
monarch init all && monarch sync all  # 配置初始化并同步
monarch services start                # 启动服务
monarch services refresh --smart      # 智能刷新镜像
```

## 服务端口

| 服务 | 端口 | 服务 | 端口 |
|------|------|------|------|
| voltage-apps | 8080 | comsrv | 6001 |
| modsrv | 6002 | hissrv | 6004 |
| apigateway | 6005 | voltage-redis | 6379 |

**核心服务**: comsrv + modsrv 仅依赖 Redis

## 项目结构

```
apps/               libs/voltage-{model,routing,rtdb}/
services/{comsrv,modsrv}/   tools/monarch/
```

## 关键模式

```rust
// Redis 键（必须使用 KeySpaceConfig）
KeySpaceConfig::production().channel_key(1001, PointType::Telemetry)  // "comsrv:1001:T"

// SQLx（禁止编译时宏）
sqlx::query_as::<_, Row>("SELECT * FROM t WHERE id = ?").bind(id)
```

## 数据流

```
上行: Device → comsrv → Redis → route:c2m → inst:{id}:M
下行: modsrv → route:m2c → inst:{id}:A → comsrv TODO 队列
```
