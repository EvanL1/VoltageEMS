# ComSrv v2.0 迁移指南

## 1. 概述

本指南帮助您从 ComSrv v1.x 迁移到 v2.0。主要变更包括：
- 移除 Transport 抽象层
- 实现自动重连机制
- 重构配置结构
- 简化代码组织

## 2. 重大变更

### 2.1 架构变更

#### Transport 层移除
```rust
// v1.x - 使用 Transport 抽象
let transport = TransportFactory::create(TransportType::Tcp, config)?;
let protocol = ModbusProtocol::new(transport);

// v2.0 - 协议直接管理连接
let protocol = ModbusProtocol::new(channel_config, connection_params)?;
```

#### 影响范围
- 所有自定义协议插件需要更新
- Transport 相关的配置参数移到协议配置中
- 测试代码需要调整 mock 方式

### 2.2 配置变更

#### 配置文件分离
```
# v1.x - 单一配置文件
config/
└── comsrv.yaml  # 包含所有配置

# v2.0 - 分层配置
config/
├── comsrv.yaml       # 主配置
├── protocols/        # 协议配置
└── channels/         # 点表配置
```

#### 配置结构简化
```yaml
# v1.x
channels:
  - id: 1001
    protocol: "modbus_tcp"
    parameters:
      host: "192.168.1.100"
      port: 502
      timeout_ms: 3000
      # ... 大量协议参数

# v2.0
channels:
  - id: 1001
    name: "电表通道"
    protocol: "modbus_tcp"
    # 协议参数移到 protocols/modbus_tcp.yaml
```

### 2.3 API 变更

#### ComBase Trait
```rust
// v1.x
#[async_trait]
pub trait ComBase {
    async fn set_transport(&mut self, transport: Box<dyn Transport>);
    // ...
}

// v2.0
#[async_trait]
pub trait ComBase {
    // 移除 set_transport 方法
    // 新增重连相关方法
    async fn on_connection_lost(&mut self) -> Result<()>;
    async fn get_connection_state(&self) -> ConnectionState;
}
```

## 3. 迁移步骤

### 3.1 准备阶段

1. **备份现有配置和代码**
   ```bash
   cp -r config config.bak
   git checkout -b migration-v2
   ```

2. **检查依赖兼容性**
   ```bash
   cargo tree | grep -E "comsrv|voltage-libs"
   ```

3. **运行现有测试确保稳定**
   ```bash
   cargo test --workspace
   ```

### 3.2 配置迁移

#### 步骤 1：拆分配置文件

```bash
# 创建新的配置目录结构
mkdir -p config/protocols config/channels

# 使用迁移工具（如果可用）
comsrv-migrate --input config/comsrv.yaml --output config/

# 或手动拆分配置
```

#### 步骤 2：更新主配置

```yaml
# config/comsrv.yaml
service:
  name: "comsrv"
  version: "2.0.0"
  # 添加重连配置
  reconnect:
    max_attempts: 3
    initial_delay: "1s"
    max_delay: "60s"

channels:
  # 简化通道配置，移除 parameters
  - id: 1001
    name: "南区电表"
    protocol: "modbus_tcp"
```

#### 步骤 3：创建协议配置

```yaml
# config/protocols/modbus_tcp.yaml
modbus_tcp:
  channels:
    1001:
      host: "192.168.1.100"
      port: 502
      timeout_ms: 3000
      # 从原 parameters 中迁移
```

### 3.3 代码迁移

#### 步骤 1：更新协议插件

对于自定义协议插件：

```rust
// 1. 移除 Transport 依赖
// use crate::core::transport::{Transport, TransportConfig};

// 2. 直接实现连接管理
pub struct MyProtocol {
    connection: Option<TcpStream>,  // 或其他连接类型
    reconnect_helper: Option<ReconnectHelper>,
    // ...
}

// 3. 实现连接方法
impl MyProtocol {
    async fn connect(&mut self) -> Result<()> {
        // 直接连接逻辑
        let stream = TcpStream::connect(&self.config.address).await?;
        self.connection = Some(stream);
        Ok(())
    }
}
```

#### 步骤 2：更新测试代码

```rust
// v1.x - 使用 MockTransport
#[cfg(test)]
mod tests {
    use crate::core::transport::mock::MockTransport;
    
    #[test]
    fn test_protocol() {
        let transport = MockTransport::new();
        let protocol = MyProtocol::new(Box::new(transport));
        // ...
    }
}

// v2.0 - 直接 mock 连接
#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_protocol() {
        let mut protocol = MyProtocol::new_with_mock_connection();
        // ...
    }
}
```

#### 步骤 3：集成重连机制

```rust
use crate::service::reconnect::{ReconnectHelper, ReconnectPolicy};

impl MyProtocol {
    pub fn new(config: ChannelConfig) -> Result<Self> {
        // 创建重连助手
        let reconnect_helper = if let Some(reconnect_config) = &config.reconnect {
            Some(ReconnectHelper::new(ReconnectPolicy::from(reconnect_config)))
        } else {
            None
        };
        
        Ok(Self {
            reconnect_helper,
            // ...
        })
    }
    
    async fn ensure_connected(&mut self) -> Result<()> {
        if self.is_connected() {
            return Ok(());
        }
        
        if let Some(helper) = &mut self.reconnect_helper {
            helper.execute_reconnect(|| self.connect()).await
        } else {
            self.connect().await
        }
    }
}
```

### 3.4 构建和测试

#### 步骤 1：清理和构建

```bash
# 清理旧的构建产物
cargo clean

# 检查编译
cargo check --workspace

# 修复编译错误后构建
cargo build --workspace
```

#### 步骤 2：运行测试

```bash
# 单元测试
cargo test --workspace

# 集成测试
cargo test --workspace --features integration-tests

# 特定协议测试
cargo test -p comsrv --test modbus_integration
```

#### 步骤 3：验证功能

1. **基本功能测试**
   - 启动服务
   - 检查 API 响应
   - 验证数据采集

2. **重连测试**
   - 模拟网络中断
   - 验证自动重连
   - 检查数据恢复

3. **性能测试**
   - 对比 v1.x 性能
   - 检查内存使用
   - 验证 CPU 占用

## 4. 常见问题

### 4.1 编译错误

#### Q: Transport 相关类型找不到
```
error[E0433]: failed to resolve: could not find `transport` in `core`
```

**A**: Transport 层已移除，需要：
1. 删除 `use crate::core::transport::*` 语句
2. 直接使用具体的连接类型（如 `TcpStream`）

#### Q: ComBase trait 方法不匹配
```
error[E0046]: not all trait items implemented
```

**A**: ComBase trait 已更新，需要：
1. 移除 `set_transport` 实现
2. 实现新的连接状态方法

### 4.2 运行时问题

#### Q: 配置加载失败
```
Failed to load configuration: Channel parameters not found
```

**A**: 配置格式已变更：
1. 检查是否已拆分配置文件
2. 确保协议配置文件存在
3. 验证配置路径正确

#### Q: 连接无法自动恢复
```
Connection lost and not recovering
```

**A**: 需要配置重连策略：
1. 在主配置中添加 `reconnect` 部分
2. 或在通道级别配置重连参数

### 4.3 性能问题

#### Q: 内存使用增加
**A**: 检查以下方面：
1. 重连助手是否正确释放资源
2. 连接池大小是否合适
3. 是否有内存泄漏

## 5. 回滚方案

如果迁移出现严重问题，可以回滚：

```bash
# 1. 切换到备份分支
git checkout v1.x-stable

# 2. 恢复配置
cp -r config.bak/* config/

# 3. 重新部署
cargo build --release
./deploy.sh
```

## 6. 迁移检查清单

- [ ] 备份现有系统
- [ ] 更新依赖版本
- [ ] 迁移配置文件
- [ ] 更新自定义协议插件
- [ ] 修复编译错误
- [ ] 通过所有测试
- [ ] 验证基本功能
- [ ] 测试重连机制
- [ ] 性能基准测试
- [ ] 更新部署脚本
- [ ] 更新监控配置
- [ ] 编写回滚计划

## 7. 获取帮助

### 文档资源
- [架构概览](./architecture-overview.md)
- [重连机制设计](./reconnect-design.md)
- [配置重构方案](./config-refactor.md)

### 技术支持
- GitHub Issues: https://github.com/voltageems/comsrv/issues
- 技术论坛: https://forum.voltageems.com

### 示例代码
- 迁移示例: `examples/migration/`
- 新协议模板: `examples/protocol-template/`

## 8. 时间规划建议

### 小型部署（1-5个通道）
- 准备阶段：1天
- 配置迁移：1天
- 代码更新：1-2天
- 测试验证：1天
- **总计**：4-5天

### 中型部署（5-20个通道）
- 准备阶段：2天
- 配置迁移：2天
- 代码更新：3-4天
- 测试验证：2-3天
- **总计**：9-11天

### 大型部署（20+个通道）
- 准备阶段：3天
- 配置迁移：3-4天
- 代码更新：5-7天
- 测试验证：5-7天
- 分阶段上线：3-5天
- **总计**：19-26天

## 9. 迁移后的优化

### 9.1 利用新特性
1. **配置重连策略**优化系统稳定性
2. **简化配置结构**提高维护效率
3. **移除冗余抽象**提升性能

### 9.2 最佳实践
1. 为关键设备配置无限重试
2. 使用分层配置管理复杂系统
3. 定期检查重连统计信息

### 9.3 持续改进
1. 监控系统指标
2. 收集用户反馈
3. 逐步优化配置

## 10. 总结

ComSrv v2.0 通过简化架构和增强可靠性，提供了更好的用户体验。虽然迁移需要一定的工作量，但长期来看将显著降低维护成本并提高系统稳定性。建议在非生产环境充分测试后再进行生产环境迁移。