# VoltageEMS ComsRv 配置迁移指南

## 概述

本指南详细说明如何将 comsrv 从遗留配置系统迁移到现代化分布式配置系统。迁移过程包括配置重构、缓存集成和配置服务连接。

## 迁移前准备

### 1. 环境检查

```bash
# 检查当前配置文件
ls -la config/
ls -la services/comsrv/config/

# 检查 Rust 版本和依赖
cargo --version
cargo check
```

### 2. 备份现有配置

```bash
# 创建配置备份
mkdir -p config/backup
cp -r config/* config/backup/
cp -r services/comsrv/config/* config/backup/comsrv/

# 记录当前配置版本
echo "$(date): Starting config migration" >> config/migration.log
```

### 3. 验证依赖

确保以下依赖已添加到 `Cargo.toml`：

```toml
# HTTP 客户端
reqwest = { version = "0.11", features = ["json"] }

# WebSocket 客户端
tokio-tungstenite = "0.20"
futures-util = "0.3"

# UUID 生成
uuid = { version = "1.0", features = ["v4", "serde"] }
```

## 迁移步骤

### 步骤 1：渐进式启用新配置系统

#### 1.1 环境变量配置

```bash
# 设置环境变量启用现代配置管理
export VOLTAGE_CONFIG_MODERN=true

# 可选：配置服务 URL（如果可用）
export VOLTAGE_CONFIG_SERVICE_URL=http://config-framework:8080

# 配置服务名称
export VOLTAGE_CONFIG_SERVICE_NAME=comsrv
```

#### 1.2 更新代码调用

将原有的配置管理器调用：

```rust
// 旧代码
use comsrv::core::config::ConfigManager;

let config_manager = ConfigManager::from_file("config/comsrv.yaml")?;
let app_config = config_manager.app_config()?;
```

替换为：

```rust
// 新代码
use comsrv::core::config::{ConfigManagerFactory, ConfigManagerTrait};

let config_manager = ConfigManagerFactory::create(Some("config/comsrv.yaml")).await?;
let app_config = config_manager.app_config().await?;
```

### 步骤 2：本地缓存集成

#### 2.1 启用本地缓存

```rust
use comsrv::core::config::{ModernConfigManager, ConfigManagerConfig, CacheConfig};
use std::time::Duration;

let config = ConfigManagerConfig {
    enable_local_cache: true,
    cache_config: CacheConfig {
        max_entries: 1000,
        default_ttl: Duration::from_secs(3600),
        enable_persistence: true,
        persistence_dir: Some("cache/config".to_string()),
        ..Default::default()
    },
    local_config_path: Some("config/comsrv.yaml".to_string()),
    ..Default::default()
};

let manager = ModernConfigManager::new(config).await?;
```

#### 2.2 验证缓存功能

```rust
// 获取配置统计
let stats = manager.stats().await?;
println!("缓存统计: {:?}", stats.cache_stats);

// 健康检查
let health = manager.health_check().await?;
println!("缓存健康: {}", health.local_cache_healthy);
```

### 步骤 3：配置服务集成

#### 3.1 配置客户端连接

```rust
let config = ConfigManagerConfig {
    config_service_url: Some("http://config-framework:8080".to_string()),
    enable_config_service: true,
    enable_local_cache: true,
    service_name: "comsrv".to_string(),
    auth_token: Some("your-auth-token".to_string()), // 可选
    sync_interval: Duration::from_secs(300), // 5分钟同步
    ..Default::default()
};

let manager = ModernConfigManager::new(config).await?;
```

#### 3.2 配置热更新

现代配置管理器会自动处理配置热更新：

```rust
// 配置变更会自动通过 WebSocket 推送
// 无需手动处理，系统会自动更新本地缓存

// 可以手动触发同步
manager.reload().await?;
```

### 步骤 4：配置迁移工具

#### 4.1 运行配置迁移

```rust
use comsrv::core::config::{ConfigMigrationManager, MigrationConfig, MigrationStrategy};

let migration_config = MigrationConfig {
    source_config_path: "config/legacy".into(),
    target_config_path: "config/modern".into(),
    backup_config_path: "config/backup".into(),
    strategy: MigrationStrategy::Complete,
    enable_validation: true,
    create_backup: true,
    preserve_source: true,
    ..Default::default()
};

let migration_manager = ConfigMigrationManager::new(migration_config);
let result = migration_manager.migrate().await?;

println!("迁移结果: 成功={}, 失败={}", result.migrated_count, result.failed_count);
```

#### 4.2 验证迁移结果

```bash
# 检查迁移后的配置文件
ls -la config/modern/
cat config/modern/app_config.yaml

# 检查备份文件
ls -la config/backup/
```

### 步骤 5：故障转移测试

#### 5.1 测试降级模式

```rust
// 模拟配置服务不可用
manager.switch_to_degraded_mode().await?;

// 验证仍能获取配置
let config = manager.get_config_with_fallback::<AppConfig>("app_config").await?;
assert!(config.service.name == "comsrv");

// 检查运行模式
assert_eq!(manager.mode().await, ConfigManagerMode::Degraded);
```

#### 5.2 测试服务恢复

```rust
// 尝试恢复服务
let recovered = manager.try_recover().await?;
if recovered {
    println!("配置服务已恢复");
    assert_eq!(manager.mode().await, ConfigManagerMode::ServiceFirst);
}
```

## 配置文件格式

### 现代配置管理器配置

创建 `config/config_manager.yaml`：

```yaml
# 配置管理器设置
config_service_url: "http://config-framework:8080"
service_name: "comsrv"
enable_config_service: true
enable_local_cache: true

# 本地缓存配置
cache_config:
  max_entries: 1000
  default_ttl_seconds: 3600
  max_memory_usage: 104857600  # 100MB
  eviction_policy: "LRU"
  cleanup_interval_seconds: 300
  enable_persistence: true
  persistence_dir: "cache/config"

# 故障转移配置
fallback_config:
  enabled: true
  max_retries: 3
  retry_interval_seconds: 10
  health_check_timeout_seconds: 5
  degraded_mode:
    allow_degraded: true
    degraded_refresh_interval_seconds: 60
    degraded_alert_threshold_seconds: 300

# 本地配置路径
local_config_path: "config/comsrv.yaml"

# 同步间隔
sync_interval_seconds: 300
```

### 环境变量配置

创建 `.env` 文件：

```bash
# 配置管理
VOLTAGE_CONFIG_MODERN=true
VOLTAGE_CONFIG_SERVICE_URL=http://config-framework:8080
VOLTAGE_CONFIG_SERVICE_NAME=comsrv
VOLTAGE_CONFIG_AUTH_TOKEN=your-auth-token

# 缓存设置
VOLTAGE_CONFIG_CACHE_MAX_ENTRIES=1000
VOLTAGE_CONFIG_CACHE_TTL_SECONDS=3600
VOLTAGE_CONFIG_CACHE_ENABLE_PERSISTENCE=true

# 故障转移
VOLTAGE_CONFIG_FALLBACK_ENABLED=true
VOLTAGE_CONFIG_DEGRADED_MODE=true
```

## 验证和测试

### 1. 单元测试

```bash
# 运行配置模块测试
cargo test config::tests

# 运行迁移测试
cargo test migration::tests

# 运行缓存测试
cargo test cache::tests
```

### 2. 集成测试

```bash
# 运行完整的配置系统测试
cargo test --test config_integration

# 测试故障转移
cargo test --test failover_test
```

### 3. 性能测试

```bash
# 配置缓存性能测试
cargo bench cache_performance

# 配置加载性能测试
cargo bench config_load_performance
```

## 性能优化建议

### 1. 缓存配置优化

```rust
let cache_config = CacheConfig {
    max_entries: 5000,           // 根据内存调整
    default_ttl: Duration::from_secs(1800), // 30分钟
    max_memory_usage: 50 * 1024 * 1024,     // 50MB
    eviction_policy: EvictionPolicy::LRU,
    cleanup_interval: Duration::from_secs(300),
    enable_persistence: true,
    ..Default::default()
};
```

### 2. 网络配置优化

```rust
let config = ConfigManagerConfig {
    sync_interval: Duration::from_secs(600), // 10分钟同步
    fallback_config: FallbackConfig {
        max_retries: 5,
        retry_interval: Duration::from_secs(5),
        health_check_timeout: Duration::from_secs(3),
        ..Default::default()
    },
    ..Default::default()
};
```

### 3. 批处理优化

```rust
// 批量配置更新
let operations = vec![
    ConfigOperation { /* ... */ },
    ConfigOperation { /* ... */ },
];

let results = config_client.batch_update(operations).await?;
```

## 故障排除

### 常见问题

#### 1. 配置服务连接失败

```
错误: Failed to connect to config service: Connection refused
解决: 检查配置服务是否运行，网络连通性
```

**解决方案：**
```bash
# 检查配置服务状态
curl -I http://config-framework:8080/api/v1/health

# 检查网络连通性
ping config-framework

# 查看防火墙设置
sudo ufw status
```

#### 2. 缓存写入失败

```
错误: Failed to write cache file: Permission denied
解决: 检查缓存目录权限
```

**解决方案：**
```bash
# 创建缓存目录并设置权限
mkdir -p cache/config
chmod 755 cache/config
chown $USER:$USER cache/config
```

#### 3. 配置验证失败

```
错误: Configuration validation failed: Missing required field 'service.name'
解决: 检查配置文件格式和必需字段
```

**解决方案：**
```bash
# 使用 YAML 验证工具
yamllint config/comsrv.yaml

# 检查配置文件结构
yq eval '.' config/comsrv.yaml
```

### 日志调试

#### 1. 启用详细日志

```bash
export RUST_LOG=debug
export VOLTAGE_LOG_LEVEL=debug
```

#### 2. 日志文件配置

```rust
// 在 main.rs 中配置日志
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

tracing_subscriber::registry()
    .with(tracing_subscriber::EnvFilter::new("debug"))
    .with(tracing_subscriber::fmt::layer())
    .init();
```

#### 3. 查看配置管理器日志

```bash
# 查看实时日志
tail -f logs/comsrv.log | grep -i config

# 搜索错误日志
grep -i error logs/comsrv.log | grep -i config
```

## 回滚策略

### 1. 快速回滚

如果迁移出现问题，可以快速回滚：

```bash
# 禁用现代配置管理
export VOLTAGE_CONFIG_MODERN=false

# 恢复备份配置
cp -r config/backup/* config/

# 重启服务
systemctl restart comsrv
```

### 2. 使用迁移工具回滚

```rust
// 使用配置迁移管理器回滚
let backup_path = PathBuf::from("config/backup/config_backup_20250701_120000.yaml");
migration_manager.rollback(&backup_path).await?;
```

### 3. 手动回滚检查

```bash
# 检查服务状态
systemctl status comsrv

# 验证配置加载
cargo run --bin comsrv -- --dry-run

# 检查日志
journalctl -u comsrv -n 50
```

## 迁移完成检查

### 1. 功能验证

- [ ] 配置正常加载
- [ ] 缓存功能正常
- [ ] 配置热更新工作
- [ ] 故障转移正常
- [ ] 性能满足要求

### 2. 性能验证

- [ ] 配置加载时间 < 1秒
- [ ] 内存使用 < 100MB
- [ ] 缓存命中率 > 90%
- [ ] 网络延迟 < 100ms

### 3. 稳定性验证

- [ ] 运行24小时无异常
- [ ] 配置服务重启后自动恢复
- [ ] 网络中断后自动重连
- [ ] 高并发访问稳定

## 总结

通过以上步骤，comsrv 服务已成功从遗留配置系统迁移到现代化分布式配置系统。新系统提供了以下优势：

1. **分布式配置管理**：支持集中配置和分发
2. **高性能缓存**：多层缓存提升访问速度
3. **故障容错**：自动故障转移和降级机制
4. **热更新支持**：实时配置变更推送
5. **向后兼容**：平滑迁移，不破坏现有功能

迁移完成后，建议：

1. 持续监控系统性能和稳定性
2. 定期备份配置数据
3. 根据使用情况调优缓存参数
4. 建立配置变更流程和审批机制

如有问题，请参考本指南的故障排除部分或查看系统日志进行诊断。