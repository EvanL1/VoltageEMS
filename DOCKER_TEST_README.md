# Docker 测试环境说明

本测试环境提供了一个完全隔离的 Docker 环境来测试 VoltageEMS ComsRV 及其 gRPC 插件架构。

## 特性

- ✅ 不对外暴露任何端口（完全内部网络）
- ✅ 完整的日志记录和收集
- ✅ 支持 gRPC 插件架构测试
- ✅ 自动化监控和测试报告
- ✅ 本地日志映射

## 环境组件

1. **Redis** - 数据存储和消息总线
2. **ComsRV** - 主通信服务
3. **Modbus 模拟器** - 3个独立的 Modbus TCP 设备
   - 通道 1001 (172.31.1.11)
   - 通道 1002 (172.31.1.12)
   - 通道 1003 (172.31.1.13) - 用于 gRPC 插件
4. **Modbus gRPC 插件** - Python 实现的 Modbus 协议插件
5. **测试监控器** - 自动化测试和日志收集

## 使用方法

### 1. 启动测试环境

```bash
./run-docker-test.sh
```

这将：
- 清理旧的日志和容器
- 构建所有 Docker 镜像
- 启动所有服务
- 显示实时日志

### 2. 查看日志

在另一个终端中运行：

```bash
./view-test-logs.sh
```

可以选择查看：
- ComsRV 主日志
- 各个 Modbus 模拟器日志
- gRPC 插件日志
- Redis 数据监控
- 测试报告

### 3. 停止测试环境

```bash
./stop-docker-test.sh
```

这将：
- 收集最终的测试报告
- 保存所有容器日志
- 停止所有容器
- 可选择清理容器和卷

## 日志位置

所有日志都保存在 `test-logs/` 目录下：

```
test-logs/
├── comsrv/              # ComsRV 主服务日志
├── comsrv-debug/        # ComsRV 调试日志
├── modbus-sim-1001/     # Modbus 模拟器 1001 日志
├── modbus-sim-1002/     # Modbus 模拟器 1002 日志
├── modbus-sim-1003/     # Modbus 模拟器 1003 日志
├── modbus-plugin/       # gRPC 插件日志
├── monitor/             # 监控器日志
└── final-reports/       # 最终测试报告
```

## 测试验证

测试环境会自动验证以下内容：

1. **服务健康状态** - 通过健康检查端点
2. **数据流** - 验证数据从 Modbus 设备到 Redis
3. **gRPC 插件** - 验证通道 1003 通过 gRPC 插件工作
4. **Redis 存储** - 验证所有通道的数据存储

## 网络配置

内部网络：`172.31.0.0/16`

- Redis: `redis:6379`
- ComsRV: `comsrv:8001` (仅内部访问)
- Modbus 模拟器 1001: `172.31.1.11:5020`
- Modbus 模拟器 1002: `172.31.1.12:5020`
- Modbus 模拟器 1003: `172.31.1.13:5020`
- gRPC 插件: `modbus-plugin:50051`

## 故障排除

1. **查看特定容器日志**：
   ```bash
   docker logs -f voltage-comsrv-test
   ```

2. **进入容器调试**：
   ```bash
   docker exec -it voltage-comsrv-test /bin/sh
   ```

3. **查看 Redis 数据**：
   ```bash
   docker exec -it voltage-redis-test redis-cli
   > KEYS comsrv:*
   > HGETALL comsrv:1003:m
   ```

4. **检查网络连通性**：
   ```bash
   docker exec voltage-comsrv-test ping modbus-plugin
   ```

## 注意事项

- 所有服务都在内部网络中运行，无法从主机直接访问
- 日志文件会持续增长，建议定期清理
- 测试完成后记得运行停止脚本以释放资源