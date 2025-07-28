# Comsrv 启动流程详解

## 概述

Comsrv是VoltageEMS系统中的工业协议网关服务，负责管理所有设备通信。本文档详细描述comsrv的启动流程，包括初始化步骤、组件加载顺序和错误处理机制。

## 启动流程图

 ┌──────────────┐
│   main()     │
└───────┬──────┘
        │
        ▼
┌──────────────────────┐
│ 1. 初始化日志系统    │
│    - tracing设置     │
│    - 日志级别配置    │
└───────┬──────────────┘
        │
        ▼
┌──────────────────────┐
│ 2. 加载配置文件        │
│    - default.yml     │
│    - channels.yml    │
│    - 环境变量覆盖      │
└───────┬──────────────┘
        │
        ▼
┌──────────────────────┐
│ 3. 初始化Redis连接     │
│    - 连接池创建        │
│    - 健康检查          │
│    - 失败重试          │
└───────┬──────────────┘
        │
        ▼
┌──────────────────────┐
│ 4. 初始化协议插件      │
│    - 注册插件类型      │
│    - ModbusTCP       │
│    - ModbusRTU       │
│    - IEC60870        │
└───────┬──────────────┘
        │
        ▼
┌──────────────────────┐
│ 5. 加载通道配置        │
│    - 解析channels     │
│    - 加载CSV映射表    │
│    - 验证配置完整性    │
└───────┬──────────────┘
        │
        ▼
┌──────────────────────┐
│ 6. 创建通道实例        │
│    - 按协议类型创建    │
│    - 初始化连接参数    │
│    - 设置批量读取      │
└───────┬──────────────┘
        │
        ▼
┌──────────────────────┐
│ 7. 启动API服务器     │
│    - HTTP服务(3000)  │
│    - 健康检查端点    │
│    - 管理接口        │
└───────┬──────────────┘
        │
        ▼
┌──────────────────────┐
│ 8. 启动通道任务      │
│    - 每通道一个task  │
│    - 定时批量读取    │
│    - 订阅控制命令    │
└───────┬──────────────┘
        │
        ▼
┌──────────────────────┐
│ 9. 进入主循环        │
│    - 监听信号        │
│    - 健康监控        │
│    - 优雅关闭        │
└──────────────────────┘

## 详细步骤说明

### 1. 初始化日志系统

**文件位置**: `src/main.rs`

```rust
// 初始化tracing
tracing_subscriber::fmt()
    .with_env_filter(EnvFilter::from_default_env())
    .init();
```

- 从环境变量 `RUST_LOG`读取日志级别
- 默认级别: `info`
- 支持按模块设置: `RUST_LOG=comsrv=debug,voltage_libs=info`

### 2. 加载配置文件

**文件位置**: `src/core/config/loaders.rs`

```rust
pub fn load_config() -> Result<Config> {
    let config = Figment::new()
        .merge(Yaml::file("config/default.yml"))
        .merge(Yaml::file("config/channels.yml"))
        .merge(Env::prefixed("COMSRV_"))
        .extract()?;
    Ok(config)
}
```

**配置优先级**（从低到高）:

1. `config/default.yml` - 默认配置
2. `config/channels.yml` - 通道配置
3. 环境变量 - `COMSRV_`前缀

**关键配置项**:

- `service.redis.url` - Redis连接地址
- `channels` - 通道列表配置
- `logging` - 日志配置

### 3. 初始化Redis连接

**文件位置**: `src/core/redis/client.rs`

```rust
// 创建Redis连接池
let redis_client = RedisClient::new(&config.redis.url)?;

// 健康检查
redis_client.ping().await?;
```

**错误处理**:

- 连接失败: 重试3次，间隔5秒
- 超时设置: 30秒
- 失败后: 程序退出，返回错误码1

### 4. 初始化协议插件

**文件位置**: `src/plugins/protocols/mod.rs`

```rust
// 注册所有协议插件
let mut plugin_registry = PluginRegistry::new();
plugin_registry.register("modbus_tcp", Box::new(ModbusTcpPlugin::new()));
plugin_registry.register("modbus_rtu", Box::new(ModbusRtuPlugin::new()));
```

**插件初始化**:

- 每个协议实现 `ProtocolPlugin` trait
- 注册到全局插件注册表
- 支持动态加载（未来特性）

### 5. 加载通道配置

**文件位置**: `src/core/config/channel_loader.rs`

```rust
for channel_config in &config.channels {
    // 加载四遥点表
    let measurement_table = load_csv(&channel_config.measurement_file)?;
    let signal_table = load_csv(&channel_config.signal_file)?;
  
    // 加载协议映射表
    let modbus_mapping = load_csv(&channel_config.modbus_mapping)?;
  
    // 验证点位ID唯一性
    validate_point_ids(&all_tables)?;
}
```

**CSV加载流程**:

1. 解析CSV文件头
2. 验证必需字段存在
3. 转换数据类型
4. 构建内存映射表

### 6. 创建通道实例

**文件位置**: `src/core/channel/manager.rs`

```rust
for channel_config in validated_channels {
    // 根据协议类型获取插件
    let plugin = plugin_registry.get(&channel_config.protocol)?;
  
    // 创建通道实例
    let channel = Channel::new(
        channel_config.id,
        channel_config.name,
        plugin,
        channel_config.parameters,
    );
  
    // 设置批量读取配置
    channel.set_batch_size(100);  // 每批100个点
    channel.set_read_interval(Duration::from_secs(1));
  
    channels.push(channel);
}
```

### 7. 启动API服务器

**文件位置**: `src/api/server.rs`

```rust
// 创建axum应用
let app = Router::new()
    .route("/health", get(health_check))
    .route("/channels", get(list_channels))
    .route("/channels/:id/status", get(channel_status))
    .with_state(app_state);

// 启动服务器
let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
axum::Server::bind(&addr)
    .serve(app.into_make_service())
    .await?;
```

**API端点**:

- `/health` - 健康检查
- `/channels` - 列出所有通道
- `/channels/:id/status` - 通道状态

### 8. 启动通道任务

**文件位置**: `src/core/channel/runner.rs`

```rust
for channel in channels {
    // 为每个通道启动独立任务
    tokio::spawn(async move {
        loop {
            // 批量读取数据
            let batch_data = channel.read_batch().await?;
          
            // 存储到Redis Hash
            for (point_id, value) in batch_data {
                redis_client.hset(
                    &format!("comsrv:{}:m", channel.id),
                    &point_id.to_string(),
                    &format!("{:.6}", value),
                ).await?;
            }
          
            // 发布到Pub/Sub
            redis_client.publish(
                &format!("comsrv:{}:m", channel.id),
                &batch_updates,
            ).await?;
          
            // 等待下次读取
            tokio::time::sleep(channel.read_interval).await;
        }
    });
}
```

**并发控制**:

- 每通道独立任务
- 批量操作减少Redis压力
- 错误隔离，单通道故障不影响其他

### 9. 进入主循环

**文件位置**: `src/main.rs`

```rust
// 设置信号处理
let mut sigterm = signal(SignalKind::terminate())?;
let mut sigint = signal(SignalKind::interrupt())?;

// 主循环
tokio::select! {
    _ = sigterm.recv() => {
        info!("Received SIGTERM, shutting down...");
    }
    _ = sigint.recv() => {
        info!("Received SIGINT, shutting down...");
    }
}

// 优雅关闭
shutdown_channels().await;
redis_client.close().await;
```

## 启动失败排查

### 常见启动错误

1. **配置文件缺失**

   ```
   Error: Failed to load config: config/default.yml not found
   ```

   解决: 确保配置文件存在于正确路径
2. **Redis连接失败**

   ```
   Error: Failed to connect to Redis: Connection refused
   ```

   解决: 检查Redis服务是否运行，地址是否正确
3. **端口占用**

   ```
   Error: Failed to bind to 0.0.0.0:3000: Address already in use
   ```

   解决: 检查端口占用或修改配置中的端口
4. **CSV文件格式错误**

   ```
   Error: Failed to parse CSV: missing field 'point_id'
   ```

   解决: 检查CSV文件格式，确保包含所有必需字段

### 日志级别调试

```bash
# 查看详细启动日志
RUST_LOG=debug cargo run

# 只看comsrv模块的调试日志
RUST_LOG=comsrv=debug cargo run

# 查看特定组件
RUST_LOG=comsrv::core::channel=trace cargo run
```

## 性能优化点

1. **批量读取优化**

   - 默认批量大小: 100个点
   - 可通过环境变量调整: `COMSRV_BATCH_SIZE=200`
2. **Redis Pipeline**

   - 批量写入使用pipeline
   - 减少网络往返次数
3. **连接池配置**

   - Redis连接池大小: 10
   - Modbus连接复用
4. **内存使用**

   - 点表使用HashMap存储，O(1)查询
   - 浮点数统一6位精度

## 监控指标

启动后可通过以下方式监控:

```bash
# 查看Redis中的数据
redis-cli hlen "comsrv:1001:m"

# 监控Pub/Sub消息
redis-cli psubscribe "comsrv:*"

# 查看健康状态
curl http://localhost:3000/health
```

## 总结

Comsrv的启动流程设计注重:

- **可靠性**: 每步都有错误处理和重试机制
- **性能**: 批量操作和连接复用
- **可观测性**: 详细的日志和监控指标
- **灵活性**: 插件架构支持扩展

理解这个启动流程有助于:

- 快速定位启动问题
- 优化配置参数
- 开发新的协议插件
- 集成到容器环境
