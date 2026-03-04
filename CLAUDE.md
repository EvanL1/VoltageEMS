# CLAUDE.md

## 核心约束

单人项目，YAGNI 原则。**禁止**: `mod.rs` | 硬编码 Redis 键 | 编译时 SQLx 宏 | 过度工程化

## 常用命令

```bash
./scripts/quick-check.sh              # fmt + clippy + tests + frontend
monarch init && monarch sync           # 配置初始化并同步
monarch services start                 # 启动服务
monarch services refresh --smart       # 智能刷新镜像
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
KeySpaceConfig::production().channel_key(1001, PointType::Telemetry)  // Redis 键
sqlx::query_as::<_, Row>("SELECT * FROM t WHERE id = ?").bind(id)     // SQLx (禁编译时宏)
```

## 数据流

```
上行: Device → comsrv → Redis → route:c2m → inst:{id}:M
下行: modsrv → route:m2c → SHM+UDS → comsrv ShmListener → Device
```
