# VoltageEMS 配置示例

本目录包含 VoltageEMS 的配置示例，帮助您快速开始使用系统。

## 目录结构

```
config-examples/
├── minimal/          # 最简配置 - 快速入门
├── modbus-rtu/       # Modbus RTU 串口配置示例
├── standard/         # 标准配置 - 生产环境参考（待添加）
├── multi-channel/    # 多通道配置 - 复杂场景（待添加）
└── README.md        # 本文档
```

## 最简配置 (minimal/)

最简单的可运行配置，包含：
- 1个虚拟通道（无需外部设备）
- 3个测点（温度、压力、流量）
- 1个设备模型

### 使用方法

1. **启动 Redis**
```bash
docker run -d --name redis -p 6379:6379 redis:latest
```

2. **启动 ComSrv**
```bash
cd /path/to/VoltageEMS
./target/release/comsrv -c config-examples/minimal/comsrv.yaml
```

3. **启动 ModSrv**
```bash
./target/release/modsrv -c config-examples/minimal/modsrv.yaml
```

4. **验证服务**
```bash
# 检查服务健康状态
curl http://localhost:6000/health
curl http://localhost:6001/health

# 查看通道状态
curl http://localhost:6000/api/channels

# 查看模型数据
curl http://localhost:6001/api/models
```

### 配置说明

**comsrv.yaml**: 定义通信通道
- 通道 1001：使用虚拟协议，自动生成测试数据
- 无需外部设备或网络配置

**telemetry.csv**: 定义数据点
- 3个浮点数测点
- 包含单位和缩放系数

**models.yaml**: 设备模型
- 将通道数据映射到逻辑模型
- 提供语义化的属性名称

## Modbus RTU 配置 (modbus-rtu/)

串口通信配置示例：
- 支持 RS232/RS485 串口
- 可配置波特率、数据位、停止位、校验位
- 适用于串口连接的 Modbus 设备

## 标准配置 (standard/)

适合生产环境的标准配置，包含：
- 多个通道（Modbus TCP、Virtual）
- 完整的四遥点表（遥测、遥信、遥控、遥调）
- 模型模板和实例
- 告警规则配置

## 多通道配置 (multi-channel/)

展示复杂场景的配置，包含：
- 多种协议混合使用
- 跨通道数据聚合
- 级联模型定义
- 高级映射关系

## 配置验证

使用提供的脚本验证配置：

```bash
# 验证 CSV 格式
./scripts/validate-comsrv-config.sh config-examples/minimal

# 检查 YAML 语法
yamllint config-examples/minimal/*.yaml
```

## 自定义配置

基于示例创建自己的配置：

1. 复制最接近需求的示例
```bash
cp -r config-examples/minimal my-config
```

2. 修改配置文件
- 更新通道参数（IP、端口等）
- 调整点表定义
- 修改模型映射

3. 验证配置
```bash
./scripts/validate-comsrv-config.sh my-config
```

4. 启动服务测试
```bash
./comsrv -c my-config/comsrv.yaml
```

## 注意事项

1. **路径配置**：示例中使用相对路径，生产环境建议使用绝对路径
2. **Redis 连接**：确保 Redis 服务可访问
3. **网络配置**：Modbus 设备需要网络可达
4. **权限问题**：确保配置文件可读

## 相关文档

- [配置指南总览](../docs/configuration/README.md)
- [ComSrv 配置详解](../docs/configuration/comsrv-config.md)
- [ModSrv 配置详解](../docs/configuration/modsrv-config.md)