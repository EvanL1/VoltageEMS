# COMSRV Docker测试环境

## 概述

本测试环境使用Docker Compose构建完整的COMSRV测试环境，包括：
- Redis数据库
- Modbus TCP模拟器
- COMSRV主服务
- 集成测试运行器
- 日志收集器

**重要特性**：
- ✅ 内部网络隔离，不对外暴露任何端口
- ✅ 完整的日志收集和存储
- ✅ 配置文件和日志映射到本地
- ✅ 自动化测试执行

## 快速开始

### 1. 运行测试

```bash
cd services/comsrv
./run_test.sh
```

或使用详细脚本：

```bash
./scripts/run_docker_test.sh
```

### 2. 手动运行

```bash
# 启动测试环境
docker-compose -f docker-compose.test.yml up -d

# 查看日志
docker-compose -f docker-compose.test.yml logs -f

# 停止并清理
docker-compose -f docker-compose.test.yml down -v
```

## 测试内容

### 基础集成测试
- 服务健康检查
- API接口测试
- Modbus通信测试
- Redis数据流测试

### 按位解析测试
- 单个位提取测试
- 多位批量读取测试
- 高位(8-15)测试
- 动态位模式测试

### 性能测试
- 批量读取性能
- 并发请求处理
- 响应时间测试

## 日志位置

所有日志会自动收集到 `logs/docker-test-{timestamp}/` 目录：

```
logs/docker-test-2025-07-24_14-30-00/
├── all-services.log          # 所有服务合并日志
├── services/
│   ├── redis.log             # Redis服务日志
│   ├── modbus-simulator.log  # Modbus模拟器日志
│   ├── comsrv.log           # COMSRV主服务日志
│   ├── test-runner.log      # 测试执行日志
│   └── log-collector.log    # 日志收集器日志
├── config/                   # 使用的配置文件备份
├── test-report.md           # 测试报告
└── final-status.txt         # 最终状态

```

## 网络架构

```
内部网络: comsrv-test-network (internal: true)
├── redis:6379
├── modbus-simulator:502
├── comsrv:3000 (仅内部访问)
├── influxdb:8086 (可选)
└── test-runner (执行测试)
```

## 配置说明

### 主配置文件
- `config/docker-test.yml` - Docker环境专用配置
- `config/test-points/` - 测试点表配置
- `config/test-points/mappings/` - 协议映射配置

### 环境变量
- `RUST_LOG=comsrv=debug` - 日志级别
- `REDIS_URL` - Redis连接字符串
- 内部网络自动处理，无需手动配置

## 测试数据

### Modbus模拟器预设值

| 寄存器 | 值 | 用途 |
|--------|-----|------|
| 1 | 0xA5 | 按位测试 (10100101) |
| 2 | 0x5A | 按位测试 (01011010) |
| 3 | 0xF00F | 高低位测试 |
| 4 | 0x8001 | 最高最低位测试 |
| 5 | 动态 | 每秒变化的位模式 |
| 40001-40010 | 温度数据 | 模拟温度值 |
| 40011-40020 | 电压数据 | 模拟电压值 |

## 故障排查

### 服务无法启动
```bash
# 检查容器状态
docker-compose -f docker-compose.test.yml ps

# 查看详细日志
docker-compose -f docker-compose.test.yml logs comsrv
```

### 测试失败
1. 检查 `logs/docker-test-*/services/test-runner.log`
2. 查看COMSRV服务日志
3. 验证Redis连接状态

### 清理环境
```bash
# 完全清理（包括数据卷）
docker-compose -f docker-compose.test.yml down -v

# 清理所有测试相关镜像
docker rmi $(docker images | grep comsrv-test | awk '{print $3}')
```

## 注意事项

1. **端口隔离**：所有服务运行在内部网络，不暴露到主机
2. **日志保存**：每次运行会创建新的日志目录，历史日志不会被覆盖
3. **配置备份**：运行时的配置文件会自动备份到日志目录
4. **资源使用**：完整测试环境需要约2GB内存和1GB磁盘空间