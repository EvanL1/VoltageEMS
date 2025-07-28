# 重连机制设计

## 1. 概述

本文档详细描述 ComSrv v2.0 的重连机制设计。重连机制旨在提高系统的可靠性，在网络中断或设备故障时自动恢复连接。

## 2. 设计目标

1. **自动化**：无需人工干预即可恢复连接
2. **可配置**：支持灵活的重试策略配置
3. **资源友好**：避免频繁重试造成的资源浪费
4. **可观测**：提供重连状态和统计信息
5. **协议无关**：可被任何协议插件使用

## 3. 架构设计

### 3.1 组件关系

```
┌─────────────────────────────────────────┐
│           协议插件                      │
│  ┌─────────────────────────────────┐   │
│  │     ConnectionManager            │   │
│  │  ┌─────────────────────────┐    │   │
│  │  │    ReconnectHelper      │    │   │
│  │  │  - 重试策略             │    │   │
│  │  │  - 状态管理             │    │   │
│  │  └─────────────────────────┘    │   │
│  └─────────────────────────────────┘   │
└─────────────────────────────────────────┘
```

### 3.2 核心组件

#### 3.2.1 ReconnectHelper

```rust
/// 通用重连助手
pub struct ReconnectHelper {
    /// 重连策略
    policy: ReconnectPolicy,
    /// 当前状态
    state: ReconnectState,
    /// 统计信息
    stats: ReconnectStats,
}

/// 重连策略
pub struct ReconnectPolicy {
    /// 最大重试次数（0 表示无限）
    max_attempts: u32,
    /// 初始延迟
    initial_delay: Duration,
    /// 最大延迟
    max_delay: Duration,
    /// 退避倍数
    backoff_multiplier: f64,
    /// 是否添加抖动
    jitter: bool,
}

/// 重连状态
pub struct ReconnectState {
    /// 当前重试次数
    current_attempt: u32,
    /// 上次重试时间
    last_attempt: Option<Instant>,
    /// 下次重试时间
    next_attempt: Option<Instant>,
    /// 连接状态
    connection_state: ConnectionState,
}

/// 连接状态
pub enum ConnectionState {
    /// 已连接
    Connected,
    /// 断开连接
    Disconnected,
    /// 正在重连
    Reconnecting,
    /// 重连失败（达到最大次数）
    Failed,
}
```

#### 3.2.2 重连统计

```rust
/// 重连统计信息
pub struct ReconnectStats {
    /// 总重连次数
    total_attempts: u64,
    /// 成功重连次数
    successful_reconnects: u64,
    /// 失败重连次数
    failed_reconnects: u64,
    /// 最后成功连接时间
    last_connected: Option<Instant>,
    /// 最长连接保持时间
    longest_connection_duration: Duration,
    /// 平均重连时间
    average_reconnect_time: Duration,
}
```

## 4. 重连算法

### 4.1 指数退避算法

```rust
impl ReconnectHelper {
    /// 计算下次重试延迟
    pub fn calculate_next_delay(&self) -> Duration {
        let base_delay = self.policy.initial_delay;
        let multiplier = self.policy.backoff_multiplier;
        let attempt = self.state.current_attempt as u32;
        
        // 指数退避：delay = initial_delay * (multiplier ^ attempt)
        let mut delay = base_delay.mul_f64(multiplier.powi(attempt));
        
        // 限制最大延迟
        if delay > self.policy.max_delay {
            delay = self.policy.max_delay;
        }
        
        // 添加抖动（±25%）
        if self.policy.jitter {
            let jitter_range = delay.as_millis() as f64 * 0.25;
            let jitter = rand::thread_rng().gen_range(-jitter_range..jitter_range);
            delay = Duration::from_millis((delay.as_millis() as f64 + jitter) as u64);
        }
        
        delay
    }
}
```

### 4.2 重连流程

```rust
impl ReconnectHelper {
    /// 执行重连
    pub async fn execute_reconnect<F, Fut, E>(
        &mut self,
        connect_fn: F,
    ) -> Result<(), ReconnectError>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = Result<(), E>>,
        E: Into<ReconnectError>,
    {
        // 检查是否已达到最大重试次数
        if self.policy.max_attempts > 0 
            && self.state.current_attempt >= self.policy.max_attempts {
            self.state.connection_state = ConnectionState::Failed;
            return Err(ReconnectError::MaxAttemptsExceeded);
        }
        
        // 更新状态
        self.state.connection_state = ConnectionState::Reconnecting;
        self.state.current_attempt += 1;
        self.stats.total_attempts += 1;
        
        // 计算延迟
        let delay = self.calculate_next_delay();
        
        // 等待延迟
        tokio::time::sleep(delay).await;
        
        // 尝试连接
        let start_time = Instant::now();
        match connect_fn().await {
            Ok(()) => {
                // 连接成功
                self.state.connection_state = ConnectionState::Connected;
                self.state.current_attempt = 0;
                self.state.last_attempt = Some(Instant::now());
                self.stats.successful_reconnects += 1;
                
                // 更新统计
                let reconnect_time = start_time.elapsed();
                self.update_average_reconnect_time(reconnect_time);
                
                Ok(())
            }
            Err(e) => {
                // 连接失败
                self.stats.failed_reconnects += 1;
                Err(e.into())
            }
        }
    }
}
```

## 5. 集成方式

### 5.1 在协议插件中使用

```rust
// 以 Modbus 为例
impl ModbusConnectionManager {
    reconnect_helper: Option<ReconnectHelper>,
    
    /// 确保连接可用
    pub async fn ensure_connected(&mut self) -> Result<()> {
        // 如果已连接，直接返回
        if self.is_connected().await {
            return Ok(());
        }
        
        // 使用重连助手
        if let Some(helper) = &mut self.reconnect_helper {
            helper.execute_reconnect(|| self.connect()).await
                .map_err(|e| ComSrvError::ConnectionError(e.to_string()))
        } else {
            // 没有配置重连，直接连接
            self.connect().await
        }
    }
    
    /// 处理连接错误
    pub async fn handle_connection_error(&mut self, error: &Error) -> Result<()> {
        warn!("Connection error: {}", error);
        
        // 标记为断开
        self.mark_disconnected().await;
        
        // 尝试重连
        self.ensure_connected().await
    }
}
```

### 5.2 在协议操作中自动重连

```rust
impl ModbusProtocol {
    /// 读取数据（带自动重连）
    pub async fn read_with_reconnect(&mut self, request: ReadRequest) -> Result<Response> {
        // 最多尝试 2 次（首次 + 1 次重连）
        for attempt in 0..2 {
            // 确保连接
            self.connection_manager.ensure_connected().await?;
            
            // 尝试读取
            match self.do_read(request).await {
                Ok(response) => return Ok(response),
                Err(e) if is_connection_error(&e) && attempt == 0 => {
                    // 连接错误且是首次尝试，触发重连
                    self.connection_manager.handle_connection_error(&e).await?;
                    continue;
                }
                Err(e) => return Err(e),
            }
        }
        
        Err(ComSrvError::OperationFailed("Read failed after reconnect".into()))
    }
}
```

## 6. 配置方式

### 6.1 全局配置

```yaml
# comsrv.yaml
service:
  reconnect:
    # 最大重试次数（0=无限）
    max_attempts: 5
    # 初始延迟
    initial_delay: 1s
    # 最大延迟
    max_delay: 60s
    # 退避倍数
    backoff_multiplier: 2.0
    # 是否启用抖动
    jitter: true
```

### 6.2 通道级配置

```yaml
channels:
  - id: 1001
    name: "关键设备"
    protocol: "modbus_tcp"
    # 覆盖全局重连配置
    reconnect:
      max_attempts: 0  # 无限重试
      initial_delay: 500ms
      max_delay: 30s
```

### 6.3 协议特定配置

```yaml
# protocols/modbus_tcp.yaml
channels:
  1001:
    # 协议特定的重连行为
    reconnect_on_timeout: true
    reconnect_on_crc_error: false
    connection_check_interval: 30s
```

## 7. 监控和诊断

### 7.1 重连事件

```rust
/// 重连事件
pub enum ReconnectEvent {
    /// 开始重连
    ReconnectStarted {
        channel_id: u16,
        attempt: u32,
        reason: String,
    },
    /// 重连成功
    ReconnectSucceeded {
        channel_id: u16,
        duration: Duration,
        attempts: u32,
    },
    /// 重连失败
    ReconnectFailed {
        channel_id: u16,
        error: String,
        attempts: u32,
    },
    /// 放弃重连
    ReconnectAbandoned {
        channel_id: u16,
        reason: String,
    },
}
```

### 7.2 指标输出

```rust
impl ReconnectHelper {
    /// 获取 Prometheus 指标
    pub fn metrics(&self) -> ReconnectMetrics {
        ReconnectMetrics {
            total_attempts: self.stats.total_attempts,
            successful_reconnects: self.stats.successful_reconnects,
            failed_reconnects: self.stats.failed_reconnects,
            current_state: self.state.connection_state.to_string(),
            uptime_seconds: self.calculate_uptime().as_secs(),
        }
    }
}
```

### 7.3 日志输出

```
2025-07-28T10:15:23.456789Z INFO comsrv::reconnect: Starting reconnect attempt 1/5 for channel 1001
2025-07-28T10:15:24.567890Z DEBUG comsrv::reconnect: Waiting 1.0s before reconnect (exponential backoff)
2025-07-28T10:15:25.678901Z INFO comsrv::reconnect: Reconnect successful for channel 1001 after 2.1s
```

## 8. 最佳实践

### 8.1 配置建议

1. **关键设备**：
   - `max_attempts: 0`（无限重试）
   - `initial_delay: 500ms`（快速响应）
   - `max_delay: 30s`（避免过于频繁）

2. **普通设备**：
   - `max_attempts: 5`（有限重试）
   - `initial_delay: 1s`
   - `max_delay: 60s`

3. **测试环境**：
   - `max_attempts: 3`（快速失败）
   - `initial_delay: 100ms`
   - `max_delay: 5s`

### 8.2 错误处理

1. **区分错误类型**：
   - 连接错误：触发重连
   - 协议错误：不触发重连
   - 配置错误：立即失败

2. **熔断机制**：
   - 短时间内多次失败后暂停重连
   - 提供手动恢复接口

### 8.3 性能考虑

1. **避免惊群效应**：
   - 使用抖动避免同时重连
   - 错开不同通道的重连时间

2. **资源控制**：
   - 限制并发重连数
   - 监控重连对系统的影响

## 9. 测试策略

### 9.1 单元测试

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_exponential_backoff() {
        let policy = ReconnectPolicy {
            max_attempts: 5,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(10),
            backoff_multiplier: 2.0,
            jitter: false,
        };
        
        let mut helper = ReconnectHelper::new(policy);
        
        // 验证延迟序列：100ms, 200ms, 400ms, 800ms, 1600ms
        assert_eq!(helper.calculate_next_delay(), Duration::from_millis(100));
        helper.state.current_attempt = 1;
        assert_eq!(helper.calculate_next_delay(), Duration::from_millis(200));
        helper.state.current_attempt = 2;
        assert_eq!(helper.calculate_next_delay(), Duration::from_millis(400));
    }
}
```

### 9.2 集成测试

1. **网络中断模拟**
2. **设备重启模拟**
3. **间歇性故障模拟**
4. **高负载下的重连行为**

## 10. 总结

重连机制是提高系统可靠性的关键组件。通过提供灵活的配置选项和智能的重试策略，可以在各种故障场景下保持服务的连续性。同时，完善的监控和诊断功能确保了问题的可追踪性和可调试性。