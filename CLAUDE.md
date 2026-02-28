# CLAUDE.md

## 核心约束

单人开发运维项目，遵循 YAGNI 原则：简单优先、够用即可、禁止过度工程化。

**禁止**: 使用 `mod.rs` 组织 Rust 代码 | 硬编码 Redis 键字符串 | 编译时 SQLx 宏

**禁止过度工程化**: 始终从最简方案开始，优先扩展现有代码。未经明确要求，禁止引入新 service、新框架、新抽象层。方案设计时默认推荐最小侵入选项。

## 代码简化

- **目标**: 单文件 <800 行，超过则拆分模块或提取 helper
- **多文件任务**: 优先完成所有文件再打磨单个；接近 rate limit 时保存进度转下一个
- **验证**: 每个文件改完 `cargo check`，全部完成 `cargo test`，报告前后行数
- 常用手法: 提取重复 match/error-handling 为 helper | 宏处理重复测试代码 | `.expect()` → `Result` + `// SAFETY:` 注释 | generics/traits 去重 adapter | 拆分 God Object（config/client/logging/poll）

## 构建验证

- 修改后必须 `cargo check` + `cargo test`
- cargo lock 被占用时重试最多 3 次（间隔 2s）
- **并行 agent**: 只关注自己修改文件的编译错误，其他文件错误忽略并标注

## 常用命令

```bash
./scripts/quick-check.sh              # fmt + clippy + tests + frontend
monarch init && monarch sync  # 配置初始化并同步
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
下行: modsrv → route:m2c → SHM写入 + UDS通知 → comsrv ShmListener → Device
      (备份: Redis inst:{id}:A + TODO → comsrv ShmPoller 轮询)
```
