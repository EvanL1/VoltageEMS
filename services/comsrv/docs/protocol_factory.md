# Protocol Factory - 集成协议工厂

## 概述

协议工厂是 VoltageEMS 通信服务的核心组件，提供了一个统一的接口来创建、管理和配置不同类型的通信协议客户端。它采用高性能的设计，支持并发访问、动态协议注册和配置验证。

## 主要特性

### 🚀 高性能设计
- 使用 `DashMap` 实现无锁并发访问
- 支持并行协议实例创建
- 优化的内存使用和缓存机制
- 异步操作支持

### 🔧 可扩展架构
- 基于 trait 的协议工厂模式
- 支持动态协议注册
- 内置协议和自定义协议支持
- 配置验证和模式生成

### 📊 完整的生命周期管理
- 通道创建和销毁
- 批量启动和停止
- 空闲通道清理
- 统计信息收集

### ✅ 内置协议支持
- **Modbus TCP**: 标准 Modbus over TCP/IP 通信
- **IEC 60870-5-104**: 电力系统通信标准
- **扩展性**: 支持添加 Modbus RTU、CAN、IEC 61850 等

## 架构设计

```
ProtocolFactory
├── 协议工厂注册表 (DashMap<ProtocolType, Factory>)
├── 通道实例管理 (DashMap<u16, Channel>)
├── 通道元数据缓存 (DashMap<u16, Metadata>)
└── 配置验证和模式生成
```

### 核心组件

1. **ProtocolClientFactory Trait**
   - 定义协议工厂接口
   - 支持配置验证和默认配置
   - 提供 JSON Schema 生成

2. **ProtocolFactory 主类**
   - 管理协议工厂注册
   - 处理通道生命周期
   - 提供高级操作接口

3. **内置协议工厂**
   - ModbusTcpFactory
   - Iec104Factory
   - 可扩展的自定义工厂

## 使用指南

### 基本使用

```rust
use comsrv::core::protocol_factory::create_default_factory;

// 创建默认工厂（包含所有内置协议）
let factory = create_default_factory();

// 查看支持的协议
let protocols = factory.supported_protocols();
println!("支持的协议: {:?}", protocols);
```

### 创建通信通道

```rust
use comsrv::core::config::config_manager::{ChannelConfig, ProtocolType, ChannelParameters};
use std::collections::HashMap;

// 创建 Modbus TCP 配置
let mut parameters = HashMap::new();
parameters.insert("address".to_string(), serde_yaml::Value::String("192.168.1.100".to_string()));
parameters.insert("port".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(502)));
parameters.insert("timeout".to_string(), serde_yaml::Value::Number(serde_yaml::Number::from(5000)));

let config = ChannelConfig {
    id: 1,
    name: "PLC通信通道".to_string(),
    description: "连接到主PLC的Modbus TCP通道".to_string(),
    protocol: ProtocolType::ModbusTcp,
    parameters: ChannelParameters::Generic(parameters),
};

// 验证配置
factory.validate_config(&config)?;

// 创建通道
factory.create_channel(config)?;
```

### 批量操作

```rust
// 并行创建多个协议实例
let configs = vec![config1, config2, config3];
let results = factory.create_protocols_parallel(configs).await;

// 启动所有通道
factory.start_all_channels().await?;

// 停止所有通道
factory.stop_all_channels().await?;
```

### 通道管理

```rust
// 获取通道统计信息
let stats = factory.get_channel_stats();
println!("总通道数: {}", stats.total_channels);
println!("协议分布: {:?}", stats.protocol_counts);

// 访问特定通道
if let Some(channel) = factory.get_channel(1).await {
    let mut ch = channel.write().await;
    // 使用通道进行通信
}

// 清理空闲通道
let idle_time = std::time::Duration::from_minutes(5);
factory.cleanup_channels(idle_time).await;
```

## 自定义协议支持

### 实现自定义协议工厂

```rust
use comsrv::core::protocol_factory::ProtocolClientFactory;
use async_trait::async_trait;

struct MyCustomFactory;

impl ProtocolClientFactory for MyCustomFactory {
    fn protocol_type(&self) -> ProtocolType {
        ProtocolType::Virtual // 或自定义类型
    }
    
    fn create_client(&self, config: ChannelConfig) -> Result<Box<dyn ComBase>> {
        // 创建自定义协议客户端
        let client = MyCustomClient::new(config);
        Ok(Box::new(client))
    }
    
    fn validate_config(&self, config: &ChannelConfig) -> Result<()> {
        // 自定义配置验证逻辑
        Ok(())
    }
    
    fn default_config(&self) -> ChannelConfig {
        // 返回默认配置
    }
    
    fn config_schema(&self) -> serde_json::Value {
        // 返回 JSON Schema
    }
}
```

### 注册自定义协议

```rust
let factory = ProtocolFactory::new();
factory.register_protocol_factory(Arc::new(MyCustomFactory));
```

## 配置参数

### Modbus TCP 配置

| 参数 | 类型 | 必需 | 默认值 | 描述 |
|------|------|------|--------|------|
| address | string | ✅ | - | 目标设备IP地址 |
| port | integer | ❌ | 502 | TCP端口号 |
| timeout | integer | ❌ | 5000 | 通信超时时间(ms) |

### IEC 104 配置

| 参数 | 类型 | 必需 | 默认值 | 描述 |
|------|------|------|--------|------|
| address | string | ✅ | - | 目标设备IP地址 |
| port | integer | ❌ | 2404 | TCP端口号 |
| timeout | integer | ❌ | 5000 | 通信超时时间(ms) |

## 性能优化

### 并发设计
- 使用 `DashMap` 替代 `Mutex<HashMap>` 实现无锁并发
- `Arc<RwLock<_>>` 保证通道访问的线程安全
- 异步操作避免阻塞

### 内存管理
- 元数据缓存减少重复计算
- 惰性清理机制避免内存泄漏
- 智能指针管理生命周期

### 网络优化
- 连接池复用
- 批量操作减少系统调用
- 超时机制防止资源浪费

## 监控和诊断

### 统计信息
```rust
let stats = factory.get_channel_stats();
println!("运行状态:");
println!("  总通道数: {}", stats.total_channels);
println!("  运行中通道: {}", stats.running_channels);
println!("  协议分布: {:?}", stats.protocol_counts);
```

### 日志记录
工厂使用 `tracing` 框架记录重要事件：
- 通道创建和销毁
- 协议工厂注册
- 错误和警告信息

### 错误处理
所有操作都返回详细的错误信息：
- `ConfigError`: 配置相关错误
- `ProtocolNotSupported`: 不支持的协议类型
- `InvalidParameter`: 参数验证失败

## 最佳实践

### 1. 配置验证
始终在创建通道前验证配置：
```rust
factory.validate_config(&config)?;
```

### 2. 资源清理
定期清理空闲通道：
```rust
// 每小时清理一次
tokio::spawn(async move {
    let mut interval = tokio::time::interval(Duration::from_secs(3600));
    loop {
        interval.tick().await;
        factory.cleanup_channels(Duration::from_secs(300)).await;
    }
});
```

### 3. 错误恢复
实现重试机制处理临时故障：
```rust
for attempt in 1..=3 {
    match factory.create_channel(config.clone()) {
        Ok(_) => break,
        Err(e) if attempt < 3 => {
            tracing::warn!("创建通道失败，重试 {}/3: {}", attempt, e);
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        Err(e) => return Err(e),
    }
}
```

### 4. 性能监控
定期收集性能指标：
```rust
let stats = factory.get_channel_stats();
metrics::gauge!("channels.total", stats.total_channels as f64);
metrics::gauge!("channels.running", stats.running_channels as f64);
```

## 扩展计划

### 即将支持的协议
- **Modbus RTU**: 串口通信支持
- **CAN Bus**: 工业总线通信
- **IEC 61850**: 智能变电站通信
- **OPC UA**: 工业4.0标准协议

### 功能增强
- 动态配置热加载
- 协议转换和桥接
- 高可用和故障转移
- 分布式部署支持

## 示例代码

完整的使用示例请参考：
- [基本使用示例](../examples/protocol_factory_usage.rs)
- [性能测试](../benches/protocol_benchmarks.rs)
- [集成测试](../tests/integration_tests.rs)

## 故障排除

### 常见问题

**Q: 创建通道时出现"Protocol type not supported"错误**
A: 检查协议类型是否正确，使用 `factory.supported_protocols()` 查看支持的协议。

**Q: 配置验证失败**
A: 使用 `factory.get_config_schema()` 获取配置模式，确保所有必需参数都已提供。

**Q: 通道启动失败**
A: 检查网络连接、防火墙设置和目标设备状态。查看日志获取详细错误信息。

### 调试技巧

1. 启用详细日志：
```bash
RUST_LOG=comsrv=debug cargo run
```

2. 使用配置模式验证：
```rust
if let Some(schema) = factory.get_config_schema(&protocol_type) {
    println!("配置模式: {}", serde_json::to_string_pretty(&schema)?);
}
```

3. 检查通道状态：
```rust
let all_channels = factory.get_all_channels();
for (id, channel) in all_channels {
    let ch = channel.read().await;
    println!("通道 {}: {:?}", id, ch.is_running());
}
``` 