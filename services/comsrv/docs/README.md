# ComsrvDocumentation - 通信服务文档中心

VoltageEMS通信服务(comsrv)的完整文档集合。

## 📚 文档导航

### 🚀 快速开始

- **[快速入门指南](QUICK_START.md)** - 5分钟快速配置和运行
- **[配置指南](CONFIGURATION_GUIDE.md)** - 完整的配置文件编写说明
- **[端到端测试指南](END_TO_END_TEST.md)** - 完整端到端测试指南

### 📖 核心文档

- **[API接口文档](API_REFERENCE.md)** - REST API接口详细说明
- **[协议扩展指南](PROTOCOL_EXTENSION.md)** - 如何添加新的通信协议
- **[性能调优指南](PERFORMANCE_TUNING.md)** - 系统性能优化建议

### 🔧 运维文档

- **[部署运维指南](DEPLOYMENT_GUIDE.md)** - 生产环境部署和运维
- **[故障排除指南](TROUBLESHOOTING.md)** - 常见问题和解决方案
- **[监控指南](MONITORING.md)** - 系统监控和告警配置

### 📋 参考资料

- **[配置参考](CONFIG_REFERENCE.md)** - 所有配置项详细说明
- **[错误码参考](ERROR_CODES.md)** - 系统错误码和处理方法
- **[协议参考](PROTOCOL_REFERENCE.md)** - 支持的通信协议详细说明

## 🎯 根据需求选择文档

### 我是新手，想快速开始

👉 从[快速入门指南](QUICK_START.md)开始，5分钟内搭建基本环境

### 我需要详细配置系统

👉 查看[配置指南](CONFIGURATION_GUIDE.md)，了解所有配置选项

### 我需要集成API接口

👉 参考[API接口文档](API_REFERENCE.md)，获取完整的API规范

### 我遇到了问题

👉 查看[故障排除指南](TROUBLESHOOTING.md)，快速解决常见问题

### 我需要扩展协议

👉 阅读[协议扩展指南](PROTOCOL_EXTENSION.md)，了解如何添加新协议

### 我要部署到生产环境

👉 查看[部署运维指南](DEPLOYMENT_GUIDE.md)，确保安全可靠部署

## 📋 配置文件快速参考

### 基本配置结构

```yaml
version: "2.1"
service:        # 服务配置
  name: "comsrv"
  logging: {...}
  api: {...}
  redis: {...}
defaults:       # 默认路径配置
  channels_root: "channels"
  combase_dir: "combase"
  protocol_dir: "protocol"
  filenames: {...}
channels:       # 通道配置列表
  - id: 100
    name: "Virtual"
    protocol: "Virtual"
    parameters: {...}
    point_table: {...}
    source_tables: {...}
```

### 支持的协议类型

- **Virtual** - 虚拟协议，用于测试和演示
- **ModbusTcp** - Modbus TCP协议
- **ModbusRtu** - Modbus RTU串口协议
- **Iec104** - IEC 60870-5-104协议 (规划中)
- **Can** - CAN总线协议 (规划中)

### 标准目录结构

```
config/
├── comsrv.yaml                    # 主配置文件
├── channels/                      # 通道配置目录
│   ├── channel_100_virtual/       # 虚拟通道
│   │   ├── combase/              # ComBase四遥点表
│   │   │   ├── telemetry.csv     # 遥测点表
│   │   │   ├── signaling.csv     # 遥信点表
│   │   │   ├── control.csv       # 遥控点表
│   │   │   └── setpoint.csv      # 遥调点表
│   │   └── protocol/             # 协议数据源表
│   │       ├── modbus_tcp_source.csv
│   │       ├── calculation_source.csv
│   │       └── manual_source.csv
│   └── channel_101_plc_main/     # PLC通道
│       ├── combase/
│       └── protocol/
```

## 🔍 快速查找

### 配置相关问题

- 如何配置Modbus TCP？ → [配置指南 - 通道配置](CONFIGURATION_GUIDE.md#通道配置)
- 如何设置点表？ → [配置指南 - 点表配置](CONFIGURATION_GUIDE.md#点表配置)
- 如何配置数据源？ → [配置指南 - 数据源表配置](CONFIGURATION_GUIDE.md#数据源表配置)

### API相关问题

- 如何获取点表数据？ → [API接口文档](API_REFERENCE.md)
- 如何控制设备？ → [API接口文档](API_REFERENCE.md)
- 如何查看通道状态？ → [API接口文档](API_REFERENCE.md)

### 故障相关问题

- 服务启动失败？ → [故障排除指南](TROUBLESHOOTING.md)
- 通道连接不上？ → [故障排除指南](TROUBLESHOOTING.md)
- 数据不更新？ → [故障排除指南](TROUBLESHOOTING.md)

## 📞 获取帮助

### 文档反馈

如果您在使用文档过程中发现任何问题或建议，请：

1. 提交Issue到项目仓库
2. 联系技术支持团队
3. 参与社区讨论

*最后更新: 2025年6月*
*文档版本: 2.1*

**开始您的VoltageEMS之旅，从[快速入门指南](QUICK_START.md)开始！** 🚀
