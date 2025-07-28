# Modbus批量读取测试环境

本测试环境用于验证comsrv的Modbus批量读取功能，包含8个通道（6个TCP + 2个RTU），总共配置了395个测量点和146个信号点。

## 测试环境架构

```
├── Redis (8-alpine) - 中央数据存储
├── Modbus TCP模拟器 x3 - 模拟电表和变压器设备
├── Modbus RTU模拟器 x2 - 通过虚拟串口模拟RTU设备
├── comsrv - 被测试的通信服务
├── 测试运行器 - 执行测试脚本
└── 日志收集器 - 收集和分析日志
```

## 通道配置

| 通道ID | 类型 | 测量点 | 信号点 | 批量大小 | 设备说明 |
|--------|------|--------|--------|----------|----------|
| 1001 | TCP | 50 | 20 | 100 | 电表组1 |
| 1002 | TCP | 40 | 15 | 50 | 电表组2 |
| 1003 | TCP | 60 | 25 | 80 | 电表组3 |
| 1004 | TCP | 45 | 18 | 60 | 电表组4 |
| 1005 | TCP | 80 | 30 | 120 | 变压器组1 |
| 1006 | TCP | 70 | 25 | 100 | 变压器组2 |
| 2001 | RTU | 30 | 10 | 40 | RTU设备组1 |
| 2002 | RTU | 25 | 8 | 30 | RTU设备组2 |

## 快速开始

1. **启动测试环境**
   ```bash
   ./start-test.sh
   ```

2. **监控实时日志**
   ```bash
   # 查看comsrv日志
   docker-compose -f docker-compose.test.yml logs -f comsrv
   
   # 监控Redis活动
   docker exec voltage-test-redis redis-cli monitor
   ```

3. **查看测试结果**
   ```bash
   # 测试完成后查看结果
   ls -la test-results/
   cat test-results/test_report.txt
   ```

4. **停止测试环境**
   ```bash
   docker-compose -f docker-compose.test.yml down
   ```

## 测试内容

1. **批量读取性能测试**
   - 测试每个通道的批量读取功能
   - 记录读取时间和成功率
   - 验证数据完整性

2. **系统性能监控**
   - Redis操作性能（ops/sec）
   - 内存使用情况
   - 通道数据更新频率

3. **日志分析**
   - 错误和警告统计
   - 关键事件记录
   - 性能瓶颈识别

## 输出文件

- `test-results/batch_read_results.json` - 详细的批量读取测试数据
- `test-results/batch_read_summary.json` - 测试摘要
- `test-results/performance_metrics.json` - 系统性能指标
- `test-results/test_report.txt` - 人类可读的测试报告
- `test-results/performance_analysis.png` - 性能分析图表
- `test-logs/` - 所有服务的日志文件

## 自定义测试

修改环境变量来自定义测试：

```yaml
# 在docker-compose.test.yml中修改
TEST_DURATION: 600  # 测试时长（秒）
CHANNEL_COUNT: 8    # 测试通道数
```

## 注意事项

1. 测试环境完全隔离，不对外暴露端口
2. 所有数据都保存在本地映射目录中
3. 测试默认运行5分钟，可通过环境变量调整
4. 确保Docker有足够的资源分配（建议至少4GB内存）