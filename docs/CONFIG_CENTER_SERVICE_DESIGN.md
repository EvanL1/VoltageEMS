# 配置中心服务设计文档

## 1. 服务概述

配置中心服务（config-center）是 VoltageEMS 的核心基础服务之一，负责管理所有微服务的配置信息，提供配置的存储、查询、更新和版本管理功能。

### 1.1 设计目标

- **集中管理**：所有服务配置集中存储和管理
- **动态更新**：支持配置热更新，无需重启服务
- **多环境支持**：开发、测试、生产环境配置隔离
- **版本控制**：配置变更历史追踪和回滚
- **高可用性**：支持集群部署，故障自动切换
- **安全性**：敏感配置加密，访问控制

## 2. 系统架构

```
┌─────────────────────────────────────────────────────────┐
│                    客户端层                              │
├─────────────────┬─────────────────┬────────────────────┤
│    ModSrv       │    ComSrv       │   其他服务...      │
│  ConfigLoader   │  ConfigLoader   │  ConfigLoader      │
└────────┬────────┴────────┬────────┴────────┬───────────┘
         │                 │                 │
         └─────────────────┼─────────────────┘
                           │ HTTP/gRPC
┌──────────────────────────▼──────────────────────────────┐
│                   配置中心服务                           │
├─────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐   │
│  │   API层     │  │  业务逻辑层  │  │   存储层    │   │
│  │             │  │             │  │             │   │
│  │ - REST API  │  │ - 配置管理   │  │ - Redis     │   │
│  │ - gRPC API  │  │ - 版本控制   │  │ - 文件系统   │   │
│  │ - WebSocket │  │ - 权限控制   │  │ - etcd      │   │
│  └─────────────┘  └─────────────┘  └─────────────┘   │
└─────────────────────────────────────────────────────────┘
```

## 3. 数据模型

### 3.1 配置实体

```rust
#[derive(Serialize, Deserialize)]
struct ServiceConfig {
    // 配置标识
    id: String,                    // 唯一ID
    service_name: String,          // 服务名称
    environment: String,           // 环境：dev/test/prod
    version: u32,                  // 版本号
    
    // 配置内容
    config_data: serde_json::Value, // 配置JSON数据
    schema: Option<String>,         // 配置schema验证
    
    // 元数据
    created_at: DateTime<Utc>,     // 创建时间
    updated_at: DateTime<Utc>,     // 更新时间
    created_by: String,            // 创建者
    updated_by: String,            // 更新者
    
    // 状态
    status: ConfigStatus,          // active/draft/archived
    checksum: String,              // 配置校验和
}

#[derive(Serialize, Deserialize)]
enum ConfigStatus {
    Active,    // 生效中
    Draft,     // 草稿
    Archived,  // 已归档
}
```

### 3.2 配置历史

```rust
#[derive(Serialize, Deserialize)]
struct ConfigHistory {
    id: String,
    config_id: String,
    version: u32,
    config_data: serde_json::Value,
    change_type: ChangeType,
    change_description: String,
    changed_by: String,
    changed_at: DateTime<Utc>,
}

#[derive(Serialize, Deserialize)]
enum ChangeType {
    Create,
    Update,
    Delete,
    Rollback,
}
```

## 4. API 设计

### 4.1 RESTful API

#### 获取配置
```
GET /api/v1/config/{service_name}
GET /api/v1/config/{service_name}?env={environment}
GET /api/v1/config/{service_name}?version={version}

Response:
{
    "service": {
        "name": "modsrv",
        "version": "1.0.0",
        "description": "Model Service"
    },
    "redis": {
        "url": "redis://localhost:6379"
    },
    // ... 其他配置
}
```

#### 创建/更新配置
```
PUT /api/v1/config/{service_name}
Content-Type: application/json
Authorization: Bearer {token}

{
    "environment": "prod",
    "config_data": {
        // 完整配置
    },
    "change_description": "Update Redis connection"
}
```

#### 获取配置历史
```
GET /api/v1/config/{service_name}/history
GET /api/v1/config/{service_name}/history?limit=10&offset=0
```

#### 回滚配置
```
POST /api/v1/config/{service_name}/rollback
{
    "version": 5,
    "reason": "Rollback due to connection issues"
}
```

### 4.2 WebSocket API (配置推送)

```javascript
// 客户端订阅
ws.send({
    "action": "subscribe",
    "service": "modsrv",
    "environment": "prod"
});

// 服务端推送
{
    "event": "config_updated",
    "service": "modsrv",
    "environment": "prod",
    "version": 10,
    "config_data": { /* 新配置 */ }
}
```

## 5. 核心功能

### 5.1 配置管理

- **CRUD操作**：创建、读取、更新、删除配置
- **批量操作**：批量导入/导出配置
- **配置验证**：基于 Schema 的配置验证
- **配置模板**：预定义配置模板

### 5.2 版本控制

- **版本追踪**：每次修改生成新版本
- **差异比较**：版本间差异对比
- **快速回滚**：一键回滚到指定版本
- **分支管理**：支持配置分支（未来）

### 5.3 环境管理

- **环境隔离**：不同环境配置完全隔离
- **环境复制**：快速复制配置到新环境
- **环境对比**：跨环境配置差异分析

### 5.4 权限控制

- **基于角色**：管理员、开发者、只读用户
- **服务级权限**：控制对特定服务的访问
- **操作审计**：所有操作记录审计日志

### 5.5 配置推送

- **实时推送**：配置变更实时通知客户端
- **增量更新**：只推送变更部分
- **推送确认**：客户端确认机制

## 6. 存储设计

### 6.1 Redis 存储结构

```
# 当前配置
config:current:{service}:{env} -> JSON配置数据

# 配置版本
config:version:{service}:{env}:{version} -> JSON配置数据

# 配置元数据
config:meta:{service}:{env} -> {
    "current_version": 10,
    "created_at": "2025-01-04T10:00:00Z",
    "updated_at": "2025-01-04T12:00:00Z"
}

# 配置历史
config:history:{service}:{env} -> List<ConfigHistory>

# 服务注册
config:services -> Set<service_name>
```

### 6.2 文件系统备份

```
/data/config-center/
├── backup/
│   ├── daily/
│   ├── weekly/
│   └── monthly/
├── configs/
│   ├── modsrv/
│   │   ├── dev.yaml
│   │   ├── test.yaml
│   │   └── prod.yaml
│   └── comsrv/
└── schemas/
```

## 7. 安全设计

### 7.1 认证授权

- **JWT Token**：API 访问认证
- **API Key**：服务间认证
- **IP 白名单**：网络层访问控制

### 7.2 配置加密

- **敏感字段加密**：密码、密钥等敏感信息
- **传输加密**：HTTPS/TLS 加密传输
- **存储加密**：可选的存储层加密

### 7.3 审计日志

```rust
#[derive(Serialize, Deserialize)]
struct AuditLog {
    id: String,
    timestamp: DateTime<Utc>,
    user: String,
    action: String,
    service: String,
    environment: String,
    details: serde_json::Value,
    ip_address: String,
    result: String,
}
```

## 8. 高可用设计

### 8.1 集群部署

- **主从模式**：一主多从，读写分离
- **负载均衡**：多实例负载均衡
- **故障转移**：自动故障检测和切换

### 8.2 缓存策略

- **本地缓存**：热点配置本地缓存
- **分布式缓存**：Redis 缓存层
- **缓存更新**：主动失效机制

### 8.3 容灾备份

- **定期备份**：自动备份到文件系统
- **异地备份**：支持 S3 等云存储
- **快速恢复**：一键恢复机制

## 9. 监控告警

### 9.1 监控指标

- **性能指标**：QPS、响应时间、错误率
- **业务指标**：配置更新频率、活跃服务数
- **系统指标**：CPU、内存、磁盘使用

### 9.2 告警规则

- **服务异常**：服务长时间未更新配置
- **配置异常**：配置验证失败、回滚频繁
- **系统异常**：存储故障、网络异常

## 10. 实施计划

### Phase 1: 基础功能（1-2周）
- [ ] 基本 CRUD API
- [ ] Redis 存储实现
- [ ] 简单权限控制

### Phase 2: 高级功能（2-3周）
- [ ] 版本控制和回滚
- [ ] WebSocket 推送
- [ ] 配置验证

### Phase 3: 企业特性（3-4周）
- [ ] 集群支持
- [ ] 完整权限系统
- [ ] 审计日志

### Phase 4: 优化完善（持续）
- [ ] 性能优化
- [ ] UI 管理界面
- [ ] 更多存储后端

## 11. 技术选型

### 11.1 开发语言和框架
- **语言**：Rust
- **Web框架**：Actix-web
- **异步运行时**：Tokio

### 11.2 存储
- **主存储**：Redis
- **备份存储**：文件系统 + S3
- **可选**：etcd、Consul

### 11.3 通信协议
- **REST API**：主要接口
- **gRPC**：高性能场景
- **WebSocket**：实时推送

### 11.4 部署
- **容器化**：Docker
- **编排**：Kubernetes
- **网关**：Traefik/Nginx

## 12. 示例配置

### 12.1 配置中心自身配置

```yaml
# config-center.yaml
service:
  name: config-center
  version: "1.0.0"
  instance_id: "config-center-01"

api:
  host: "0.0.0.0"
  port: 8080
  
redis:
  url: "redis://localhost:6379"
  prefix: "config:"
  
storage:
  primary: redis
  backup:
    enabled: true
    path: "/data/config-center/backup"
    interval: "1h"
    
security:
  jwt_secret: "${JWT_SECRET}"
  encryption_key: "${ENCRYPTION_KEY}"
  
monitoring:
  metrics_port: 9090
  health_check_port: 8081
```

### 12.2 客户端集成示例

```rust
// 客户端配置加载
let config = ConfigCenterClient::new("http://config-center:8080")
    .with_auth_token(token)
    .get_config("modsrv", "prod")
    .await?;

// 订阅配置更新
let mut subscription = client
    .subscribe("modsrv", "prod")
    .await?;

while let Some(update) = subscription.next().await {
    println!("Config updated: version {}", update.version);
    // 重新加载配置
}
```