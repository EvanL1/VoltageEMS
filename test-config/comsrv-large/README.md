# 大规模Modbus批量读取测试配置

本配置支持9000+个点位的批量读取测试，并包含完整的报文日志记录功能。

## 配置概览

### 点位规模
- **总点位数**: 9,030个
  - 测量点: 7,750个
  - 信号点: 1,280个
- **通道数**: 8个
  - Modbus TCP: 6个通道（1001-1006）
  - Modbus RTU: 2个通道（2001-2002）

### 通道详情

| 通道ID | 名称 | 协议 | 测量点 | 信号点 | 总计 | 批量大小 |
|--------|------|------|--------|--------|------|----------|
| 1001 | PowerMeter_Group1 | TCP | 1200 | 200 | 1400 | 200 |
| 1002 | PowerMeter_Group2 | TCP | 1000 | 150 | 1150 | 150 |
| 1003 | PowerMeter_Group3 | TCP | 1100 | 180 | 1280 | 180 |
| 1004 | PowerMeter_Group4 | TCP | 900 | 120 | 1020 | 120 |
| 1005 | Transformer_Group1 | TCP | 1300 | 250 | 1550 | 250 |
| 1006 | Transformer_Group2 | TCP | 1150 | 200 | 1350 | 200 |
| 2001 | RTU_Device_Group1 | RTU | 600 | 100 | 700 | 100 |
| 2002 | RTU_Device_Group2 | RTU | 500 | 80 | 580 | 80 |

## 报文日志配置

### 日志目录结构
```
logs/
├── PowerMeter_Group1/
│   ├── PowerMeter_Group1_2025-07-26.log    # 当日日志
│   ├── PowerMeter_Group1_2025-07-25.log    # 历史日志
│   └── ...
├── PowerMeter_Group2/
├── PowerMeter_Group3/
├── PowerMeter_Group4/
├── Transformer_Group1/
├── Transformer_Group2/
├── RTU_Device_Group1/
└── RTU_Device_Group2/
```

### 日志级别配置
- **debug**: 记录所有报文的详细内容（通道1001, 1002, 1005, 1006, 2001）
- **info**: 只记录重要事件和统计信息（通道1003, 1004, 2002）

### 日志轮转策略
- **类型**: 按日轮转（daily）
- **保留时间**: 
  - TCP通道: 30天
  - RTU通道: 14天
- **单文件大小限制**:
  - 电表组: 100MB
  - 变压器组: 200MB
  - RTU设备: 50MB

## 使用方法

### 1. 生成配置文件
```bash
# 已经生成，可以重新生成
uv run python test-scripts/generate-large-point-tables.py
```

### 2. 启动数据模拟
```bash
# 启动大规模数据模拟器
uv run python test-scripts/simulate-large-data.py
```

### 3. 运行comsrv
```bash
# 使用大规模配置启动comsrv
RUST_LOG=debug cargo run -p comsrv -- --config test-config/comsrv-large/channels.yml
```

### 4. 分析日志
```bash
# 分析报文日志
uv run python test-scripts/analyze-message-logs.py logs/
```

## 性能指标

### 预期性能
- **批量读取速率**: 10,000+ 点位/秒
- **单次批量读取**: 50-250个点位
- **响应时间**: <50ms（批量读取）
- **并发通道**: 8个同时工作

### 监控指标
- Redis内存使用
- 批量读取成功率
- 平均响应时间
- 错误和超时统计

## 测试场景

### 1. 正常负载测试
- 所有通道正常运行
- 每秒更新所有点位
- 监控性能指标

### 2. 压力测试
- 增加更新频率
- 模拟网络延迟
- 测试错误恢复

### 3. 长时间运行测试
- 连续运行24小时
- 验证日志轮转
- 检查内存泄漏

## 故障排查

### 常见问题

1. **Redis内存占用过高**
   - 检查点位数量配置
   - 清理历史数据: `redis-cli FLUSHDB`

2. **日志文件过大**
   - 调整日志级别从debug改为info
   - 减少日志保留天数

3. **批量读取超时**
   - 减小batch_size配置
   - 检查网络连接

### 性能优化建议

1. **批量大小优化**
   - TCP通道: 100-200个点位
   - RTU通道: 50-100个点位

2. **并发优化**
   - 使用多个Redis连接
   - 启用连接池

3. **日志优化**
   - 生产环境使用info级别
   - 定期归档历史日志