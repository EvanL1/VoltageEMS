# VoltageEMS 文档索引

## 系统架构文档

- [系统架构概述](architecture/system-architecture.md) - VoltageEMS 整体架构设计
- [Redis 存储架构](architecture/redis-storage-architecture.md) - 扁平化键值存储设计
- [数据流架构](architecture/data-flow-architecture.md) - 实时数据流处理架构

## 服务文档

### comsrv - 通信服务
- [架构设计](../services/comsrv/docs/architecture.md) - 插件化协议架构
- [Redis 存储](../services/comsrv/docs/redis-storage.md) - 数据存储实现
- [Modbus 用户指南](../services/comsrv/docs/MODBUS_USER_GUIDE.md) - Modbus 协议使用
- [传输层架构](../services/comsrv/docs/TRANSPORT_LAYER_ARCHITECTURE.md) - 统一传输层设计

### modsrv - 计算服务
- [架构设计](../services/modsrv/docs/architecture.md) - 计算引擎架构
- [设备模型](../services/modsrv/docs/device-model.md) - 物模型系统
- [Redis 接口](../services/modsrv/docs/redis-interface.md) - 数据接口设计

### hissrv - 历史数据服务
- [架构设计](../services/hissrv/docs/architecture.md) - 时序数据存储

### netsrv - 云网关服务
- [架构设计](../services/netsrv/docs/architecture.md) - 多云接入设计

### alarmsrv - 告警服务
- [架构设计](../services/alarmsrv/docs/architecture.md) - 告警系统设计

### apigateway - API 网关
- [配置服务 API](../services/apigateway/docs/CONFIG_SERVICE_API.md) - 配置管理接口

## 开发指南

- [开发指南](development-guide.md) - 开发环境搭建和编码规范
- [部署指南](deployment-guide.md) - 生产环境部署方案
- [CI/CD 架构](VOLTAGEEMS_CICD_ARCHITECTURE.md) - 持续集成部署流程
- [本地 CI 工具](LOCAL_CI.md) - 本地开发工具使用

## 配置指南

- [配置指南](CONFIGURATION_GUIDE.md) - 系统配置说明
- [配置快速参考](CONFIG_QUICK_REFERENCE.md) - 配置速查表

## 系统集成

- [系统集成测试计划](system-integration-test-plan.md) - 集成测试方案

## 版本历史

查看各服务的修复日志：
- [修复日志目录](fixlog/) - 按日期记录的系统更新

## 快速链接

### 常用命令

```bash
# 启动所有服务
docker-compose up -d

# 查看服务状态
docker-compose ps

# 查看日志
docker-compose logs -f [service_name]

# 运行测试
cargo test --workspace

# 代码检查
cargo fmt --all && cargo clippy --all
```

### 环境要求

- Rust 1.70+
- Redis 7.0+
- Docker 20.10+ (可选)
- InfluxDB 2.0+ (历史数据存储)

### 获取帮助

- [GitHub Issues](https://github.com/VoltageEMS/VoltageEMS/issues)
- [项目 Wiki](https://github.com/VoltageEMS/VoltageEMS/wiki)
- [Discord 社区](https://discord.gg/voltageems)