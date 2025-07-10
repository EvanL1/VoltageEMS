# VoltageEMS 配置管理架构建议

## 概述

基于AWS配置管理最佳实践和VoltageEMS当前架构，本文档提出一个统一的配置管理方案。

## 当前状态分析

### 现有问题
1. **配置分散**：每个服务独立管理配置文件（YAML/TOML）
2. **缺乏版本控制**：配置更改没有审计追踪
3. **无动态更新**：配置更改需要重启服务
4. **环境管理复杂**：开发/测试/生产环境配置管理困难

### 现有优势
- 已有 `voltage-config` 框架提供基础抽象
- 支持多种配置格式（YAML、TOML、JSON、环境变量）
- 有 SQLite 配置提供者的初步实现

## 建议架构

### 1. 分层配置管理模式

参考AWS Systems Manager Parameter Store的层次化设计：

```
/voltage-ems/
├── global/                    # 全局配置
│   ├── redis/                # Redis连接配置
│   ├── logging/              # 日志配置
│   └── monitoring/           # 监控配置
├── services/                 # 服务级配置
│   ├── comsrv/
│   │   ├── common/          # 服务通用配置
│   │   └── channels/        # 通道特定配置
│   ├── modsrv/
│   └── ...
└── environments/            # 环境特定配置
    ├── dev/
    ├── test/
    └── prod/
```

### 2. 配置服务架构

#### 选项 A：中央配置服务（推荐）

```
┌─────────────────────────────────────────────────────┐
│                 Config Service                       │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐ │
│  │   HTTP API  │  │  gRPC API   │  │   WebSocket │ │
│  └─────────────┘  └─────────────┘  └─────────────┘ │
│  ┌─────────────────────────────────────────────────┐│
│  │           Configuration Engine                   ││
│  │  - Version Control                               ││
│  │  - Validation                                    ││
│  │  - Access Control                                ││
│  └─────────────────────────────────────────────────┘│
│  ┌──────────┐  ┌──────────┐  ┌───────────────────┐ │
│  │  SQLite  │  │  Redis   │  │  File System      │ │
│  └──────────┘  └──────────┘  └───────────────────┘ │
└─────────────────────────────────────────────────────┘
```

**优点**：
- 集中管理，易于审计
- 支持动态配置更新
- 统一的访问控制
- 配置版本化和回滚

**缺点**：
- 引入单点故障
- 增加网络延迟
- 需要高可用部署

#### 选项 B：分布式配置（当前模式优化）

保持当前每个服务独立配置，但增强功能：
- 使用 Redis 作为配置变更通知中心
- 本地缓存 + 远程同步
- 配置文件版本控制（Git）

### 3. 实施方案（推荐中央配置服务）

#### Phase 1：基础设施（2周）
1. 创建独立的 `configsrv` 服务
2. 实现基础 API（REST + gRPC）
3. 集成现有 `voltage-config` 框架
4. 实现 SQLite 存储后端

#### Phase 2：核心功能（3周）
1. 配置版本控制
2. 配置验证框架
3. 访问控制（基于服务身份）
4. 配置变更通知（WebSocket/Redis PubSub）

#### Phase 3：高级功能（2周）
1. 配置模板和继承
2. 环境管理
3. 配置加密（敏感信息）
4. 审计日志

#### Phase 4：服务迁移（3周）
1. 创建迁移工具
2. 逐个服务迁移
3. 保持向后兼容

## 技术细节

### API 设计

```rust
// 配置获取
GET /api/v1/config/{service}/{key}
GET /api/v1/config/{service}  // 获取服务所有配置

// 配置更新
PUT /api/v1/config/{service}/{key}
{
    "value": "...",
    "version": "1.0.0",
    "metadata": {
        "updated_by": "admin",
        "reason": "Update Redis connection"
    }
}

// 配置订阅
WS /api/v1/config/subscribe/{service}
```

### 客户端SDK

```rust
use voltage_config_client::{ConfigClient, ConfigUpdate};

// 初始化
let client = ConfigClient::new("http://configsrv:8080")
    .with_service("comsrv")
    .with_cache(true)
    .build()?;

// 获取配置
let redis_config: RedisConfig = client.get("redis").await?;

// 监听变更
let mut updates = client.subscribe().await?;
while let Some(update) = updates.next().await {
    match update {
        ConfigUpdate::Changed(key, value) => {
            // 处理配置更新
        }
    }
}
```

### 配置优先级

1. 环境变量（最高优先级）
2. 命令行参数
3. 配置服务
4. 本地配置文件（最低优先级）

## 与现有系统集成

### 1. voltage-common 扩展

```rust
// 新增配置客户端模块
pub mod config_client {
    pub struct ConfigClient { ... }
    pub trait ConfigSource { ... }
}
```

### 2. 服务适配

```rust
// 服务启动时
let config = if env::var("USE_CONFIG_SERVICE").is_ok() {
    ConfigClient::load::<ServiceConfig>().await?
} else {
    // 回退到本地配置
    load_local_config()?
};
```

## 安全考虑

1. **认证**：服务间使用 mTLS 或 JWT
2. **授权**：基于服务身份的访问控制
3. **加密**：敏感配置端到端加密
4. **审计**：所有配置变更记录

## 监控和运维

1. **健康检查**：配置服务健康状态
2. **指标**：配置获取延迟、缓存命中率
3. **告警**：配置服务不可用、异常变更
4. **备份**：定期备份配置数据库

## 迁移计划

### 阶段1：并行运行
- 新配置服务与现有配置并存
- 服务可选择使用新系统

### 阶段2：逐步迁移
- 从非关键服务开始
- 监控和验证
- 收集反馈

### 阶段3：完全切换
- 所有服务使用新配置系统
- 废弃旧配置文件

## 决策建议

基于VoltageEMS的规模和需求，建议：

1. **短期（1-2个月）**：
   - 修复现有 config-framework 编译问题
   - 继续使用增强的本地配置模式
   - 添加配置验证和热重载支持

2. **中期（3-6个月）**：
   - 实施中央配置服务
   - 保持向后兼容
   - 逐步迁移服务

3. **长期（6个月+）**：
   - 完全迁移到中央配置
   - 实现高级功能（A/B测试、灰度发布）
   - 与CI/CD集成

## 参考实现

- **AWS Systems Manager Parameter Store**: 层次化、加密、版本控制
- **AWS AppConfig**: 验证、部署策略、监控
- **Consul**: 服务发现 + 配置管理
- **etcd**: 分布式键值存储
- **Spring Cloud Config**: 中央配置服务

## 总结

建议采用**中央配置服务**方案，因为：
1. 统一管理，降低运维复杂度
2. 支持动态配置，无需重启服务
3. 完善的审计和版本控制
4. 符合微服务最佳实践

但实施应循序渐进，确保系统稳定性。