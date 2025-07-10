# alarmsrv Redis Hash结构设计

## 概述
alarmsrv（告警服务）负责实时告警检测、状态管理和通知分发。采用时间分片的Hash结构存储告警数据，优化了时间范围查询和告警生命周期管理。

## Hash键结构

### 时间分片存储
```
Key Pattern: ems:alarms:shard:{YYYYMMDDHH}:{alarm_id}
```

### 设计理念
- **时间分片**：按小时分片，便于快速定位时间范围
- **告警隔离**：每个告警独立Hash，支持原子更新
- **自动过期**：历史分片可设置TTL自动清理

### 数据示例
```json
Key: ems:alarms:shard:2025011010:alarm_45678
Fields:
  alarm_id: "45678"
  channel: "channel_1"
  point_id: "point_1001"
  point_name: "主变压器温度"
  alarm_type: "high_limit"
  alarm_level: "critical"
  current_value: "105.5"
  limit_value: "100.0"
  unit: "°C"
  start_time: "2025-01-10T10:15:30.123Z"
  last_update: "2025-01-10T10:30:00.456Z"
  duration_seconds: "870"
  status: "active"
  ack_status: "unacknowledged"
  ack_time: ""
  ack_user: ""
  ack_comment: ""
  suppress_until: ""
  occurrence_count: "3"
  description: "主变压器温度超过上限值"
  recommended_action: "检查冷却系统，必要时降低负载"
```

## 告警类型

### 限值告警
- **high_limit**: 高限告警
- **high_high_limit**: 高高限告警
- **low_limit**: 低限告警
- **low_low_limit**: 低低限告警

### 状态告警
- **state_change**: 状态变化
- **state_abnormal**: 异常状态
- **communication_loss**: 通信中断

### 系统告警
- **device_fault**: 设备故障
- **system_error**: 系统错误
- **data_quality**: 数据质量

### 计算告警
- **rate_of_change**: 变化率异常
- **deviation**: 偏差告警
- **pattern_match**: 模式匹配

## 告警级别

### 严重程度分级
1. **critical**: 严重 - 立即处理
2. **major**: 重要 - 尽快处理
3. **minor**: 次要 - 计划处理
4. **warning**: 警告 - 关注即可
5. **info**: 信息 - 仅供参考

## 写入流程

### 新告警产生
```rust
pub async fn create_alarm(&mut self, alarm: &Alarm) -> Result<()> {
    let shard_key = format!("ems:alarms:shard:{}:{}", 
        alarm.start_time.format("%Y%m%d%H"),
        alarm.id
    );
    
    let fields = vec![
        ("alarm_id", alarm.id.as_str()),
        ("channel", alarm.channel.as_str()),
        ("point_id", alarm.point_id.as_str()),
        ("alarm_type", alarm.alarm_type.as_str()),
        ("status", "active"),
        ("start_time", alarm.start_time.to_rfc3339().as_str()),
        // ... 其他字段
    ];
    
    self.redis.hset_multiple(&shard_key, fields).await?;
    
    // 更新索引
    self.update_alarm_index(&alarm).await?;
    
    Ok(())
}
```

### 告警更新
```rust
pub async fn update_alarm_status(&mut self, alarm_id: &str, status: &str) -> Result<()> {
    // 查找告警所在分片
    let shard_key = self.find_alarm_shard(alarm_id).await?;
    
    let updates = vec![
        ("status", status),
        ("last_update", Utc::now().to_rfc3339().as_str()),
    ];
    
    self.redis.hset_multiple(&shard_key, updates).await?;
    Ok(())
}
```

## 查询优化

### 时间范围查询
```rust
pub async fn get_alarms_by_time_range(
    &self, 
    start: DateTime<Utc>, 
    end: DateTime<Utc>
) -> Result<Vec<Alarm>> {
    let mut alarms = Vec::new();
    
    // 计算涉及的时间分片
    let shards = self.calculate_shards(start, end);
    
    // 并行查询多个分片
    let mut tasks = Vec::new();
    for shard in shards {
        let pattern = format!("ems:alarms:shard:{}:*", shard);
        tasks.push(self.redis.scan_match(pattern));
    }
    
    let results = futures::future::join_all(tasks).await;
    
    // 合并结果
    for result in results {
        alarms.extend(self.parse_alarm_data(result?));
    }
    
    Ok(alarms)
}
```

### 索引结构
```
# 活跃告警索引
Key: ems:alarms:index:active
Type: Set
Members: ["shard:alarm_id", ...]

# 通道告警索引  
Key: ems:alarms:index:channel:{channel_id}
Type: Sorted Set
Score: timestamp
Member: "shard:alarm_id"

# 级别告警索引
Key: ems:alarms:index:level:{level}
Type: Sorted Set
Score: timestamp
Member: "shard:alarm_id"
```

## 告警生命周期

### 状态流转
```
产生(new) → 活跃(active) → 确认(acknowledged) → 恢复(cleared) → 归档(archived)
                ↓                    ↓
            抑制(suppressed)    手动清除(manually_cleared)
```

### 自动恢复
```rust
pub async fn check_auto_recovery(&mut self) -> Result<()> {
    let active_alarms = self.get_active_alarms().await?;
    
    for alarm in active_alarms {
        // 获取当前值
        let current_value = self.get_point_value(&alarm.point_id).await?;
        
        // 检查是否恢复正常
        if self.is_value_normal(&alarm, current_value) {
            self.clear_alarm(&alarm.id, "auto_recovered").await?;
        }
    }
    
    Ok(())
}
```

## 告警抑制

### 抑制规则
1. **重复抑制**：相同告警在时间窗口内只报一次
2. **级联抑制**：上级告警抑制下级
3. **维护抑制**：维护期间抑制相关告警
4. **条件抑制**：满足特定条件时抑制

### 抑制实现
```rust
pub struct SuppressionRule {
    pub rule_type: SuppressionType,
    pub condition: String,
    pub duration: Duration,
    pub scope: Vec<String>,  // 影响的点位或通道
}
```

## 通知管理

### 通知策略
```json
{
  "notification_rules": [
    {
      "level": ["critical", "major"],
      "channels": ["sms", "email", "wechat"],
      "delay": 0,
      "repeat_interval": 300
    },
    {
      "level": ["minor", "warning"],
      "channels": ["email"],
      "delay": 300,
      "repeat_interval": 3600
    }
  ]
}
```

### 通知记录
```
Key: ems:alarms:notification:{alarm_id}
Fields:
  sms_sent: "2025-01-10T10:16:00Z"
  sms_status: "success"
  email_sent: "2025-01-10T10:16:01Z"
  email_status: "success"
  wechat_sent: "2025-01-10T10:16:02Z"
  wechat_status: "failed"
  wechat_error: "timeout"
```

## 统计分析

### 实时统计
```
Key: ems:alarms:stats:realtime
Fields:
  total_active: "45"
  critical_count: "2"
  major_count: "8"
  minor_count: "15"
  warning_count: "20"
  last_update: "2025-01-10T10:30:00Z"
```

### 历史统计
```
Key: ems:alarms:stats:daily:20250110
Fields:
  total_generated: "234"
  total_cleared: "189"
  avg_duration: "1845"  # 秒
  top_alarm_point: "point_1001"
  top_alarm_count: "23"
```

## 性能优化

### 分片策略
1. **时间分片粒度**：根据告警频率调整（小时/天）
2. **分片大小限制**：单分片不超过10000个告警
3. **热点分离**：高频告警单独分片

### 查询优化
1. **索引利用**：优先使用索引定位
2. **并行查询**：多分片并发查询
3. **结果缓存**：缓存常用查询结果
4. **分页支持**：大结果集分页返回

## 数据清理

### 自动清理策略
```rust
pub async fn cleanup_old_alarms(&mut self) -> Result<()> {
    let retention_days = self.config.retention_days;
    let cutoff_date = Utc::now() - Duration::days(retention_days);
    
    // 获取过期分片
    let pattern = "ems:alarms:shard:*";
    let shards = self.redis.scan_match(pattern).await?;
    
    for shard in shards {
        if let Some(date) = parse_shard_date(&shard) {
            if date < cutoff_date {
                self.redis.del(&shard).await?;
            }
        }
    }
    
    Ok(())
}
```

### 归档策略
1. **冷数据归档**：超过30天的告警归档到文件
2. **压缩存储**：归档数据压缩存储
3. **快速检索**：保留索引信息

## 高可用设计

### 主从复制
- 告警数据实时同步到从库
- 读写分离减轻主库压力
- 故障自动切换

### 分布式部署
- 按地域或业务分片
- 跨区域数据同步
- 全局告警视图

## 最佳实践

1. **合理设置告警阈值**：避免告警风暴
2. **告警分级明确**：确保重要告警不被淹没
3. **抑制规则完善**：减少重复和无效告警
4. **定期清理归档**：保持系统性能
5. **监控告警本身**：告警系统的可用性监控
6. **告警知识库**：积累处理经验和方案