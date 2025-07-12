# comsrv Redis Hash结构设计

## 概述

comsrv（通信服务）负责工业协议数据采集，是整个系统的数据源头。采用Hash结构存储实时数据，按通道（channel）组织，实现高效的批量读写。

## Hash键结构

### 实时数据存储

```
Key Pattern: comsrv:realtime:channel:{channel_id}
```

### 字段结构

每个Hash包含该通道下所有点位的实时数据：

```
Field: {point_id}
Value: JSON格式的点位数据
```

### 数据示例

```json
Key: comsrv:realtime:channel:1
Fields:
  point_1001: {
    "id": "1001",
    "name": "主变压器温度",
    "value": "65.3",
    "timestamp": "2025-01-10T10:30:00.123Z",
    "quality": "good",
    "unit": "°C",
    "telemetry_type": "Measurement",
    "description": "主变压器A相绕组温度"
  }
  
  point_1002: {
    "id": "1002", 
    "name": "断路器状态",
    "value": "1",
    "timestamp": "2025-01-10T10:30:00.123Z",
    "quality": "good",
    "unit": "",
    "telemetry_type": "Signal",
    "description": "10kV进线断路器位置"
  }
  
  point_1003: {
    "id": "1003",
    "name": "有功功率",
    "value": "1234.56",
    "timestamp": "2025-01-10T10:30:00.123Z",
    "quality": "good",
    "unit": "kW",
    "telemetry_type": "Measurement",
    "description": "总有功功率"
  }
```

## 遥测类型说明

### Measurement（遥测）

- 原YC类型
- 模拟量测量值
- 示例：温度、电压、电流、功率

### Signal（遥信）

- 原YX类型
- 数字量状态
- 示例：开关状态、告警信号、运行状态

### Control（遥控）

- 原YK类型
- 控制命令
- 示例：分合闸命令、启停命令

### Adjustment（遥调）

- 原YT类型
- 模拟量设定
- 示例：功率设定、电压调节

## 写入策略

### 批量更新

```rust
// 使用Pipeline批量更新
pub async fn batch_update_values(&self, updates: Vec<PointData>) -> Result<()> {
    let mut pipe = self.client.pipeline();
    let hash_key = format!("comsrv:realtime:channel:{}", self.channel_id);
  
    for point in updates {
        let field = point.id.to_string();
        let value = serde_json::to_string(&point)?;
        pipe.hset(&hash_key, &field, &value);
    }
  
    pipe.execute().await?;
    Ok(())
}
```

### 性能优化

1. **批量大小**：建议100-500个点位一批
2. **更新频率**：根据协议轮询周期，通常1-5秒
3. **Pipeline使用**：减少网络往返
4. **异步写入**：不阻塞数据采集

## 读取模式

### 获取通道所有数据

```redis
HGETALL comsrv:realtime:channel:1
```

### 获取特定点位

```redis
HGET comsrv:realtime:channel:1 point_1001
```

### 批量获取点位

```redis
HMGET comsrv:realtime:channel:1 point_1001 point_1002 point_1003
```

## 协议适配

### Modbus协议

- 按功能码组织数据
- 支持批量读取优化
- 地址映射：`slave_id:function_code:register`

### IEC60870协议

- 按信息对象地址组织
- 支持突发传输
- 时标精度：毫秒级

### CAN协议

- 按CAN ID过滤
- 支持周期性和事件触发
- 数据解析：位域映射

## 监控指标

### 性能指标

- 写入QPS：每秒写入次数
- 批量大小：平均批量点位数
- 延迟：从采集到写入的时间
- 错误率：写入失败比例

### 数据质量

- good：数据正常
- bad：通信错误
- uncertain：数据可疑
- timeout：超时无更新

## 配置示例

### 通道配置

```yaml
channels:
  - id: 1
    name: "主站通道1"
    protocol: "modbus_tcp"
    points_count: 1000
    update_interval: 1000  # 毫秒
    batch_size: 200
```

### Redis配置

```yaml
redis:
  batch_sync:
    enabled: true
    batch_size: 200
    flush_interval: 1000
    max_retry: 3
    pipeline_enabled: true
```

## 故障处理

### 连接中断

1. 缓存最新数据
2. 标记quality为"timeout"
3. 重连后批量更新

### Redis故障

1. 本地缓冲队列
2. 限流保护
3. 自动重试机制

## 数据一致性

### 原子性保证

- 单个Hash的更新是原子的
- 使用事务保证多Hash一致性

### 时间戳管理

- 采集时间戳：数据产生时间
- 写入时间戳：Redis写入时间
- 用于数据新鲜度判断

## 扩展性设计

### 水平扩展

- 按channel_id分片
- 支持多Redis实例
- 一致性Hash路由

### 垂直扩展

- 增加字段不影响现有数据
- JSON格式保证灵活性
- 向后兼容性

## 最佳实践

1. **合理分配通道**：每通道500-1000个点位
2. **定期清理**：删除长期离线的通道数据
3. **监控告警**：实时监控Hash大小和内存使用
4. **数据压缩**：对大量重复数据考虑压缩存储
5. **访问控制**：使用Redis ACL限制访问权限
