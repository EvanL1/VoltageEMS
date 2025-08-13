# VoltageEMS 配置指南

## 概述

VoltageEMS 采用分层配置架构，各服务独立配置，通过 Redis 进行数据交互。本指南介绍系统的配置方法和最佳实践。

## 服务配置结构

```
config/
├── comsrv/          # 通信服务配置
│   ├── comsrv.yaml
│   └── {channel_id}/
│       ├── *.csv
│       └── mapping/*.csv
├── modsrv/          # 模型服务配置
│   ├── modsrv.yaml
│   └── models.yaml
├── alarmsrv/        # 告警服务配置
├── rulesrv/         # 规则引擎配置
├── hissrv/          # 历史服务配置
├── apigateway/      # API网关配置
└── netsrv/          # 网络服务配置
```

## 核心服务配置

### [ComSrv - 通信服务](./comsrv-config.md)

负责数据采集的核心服务，支持多种工业协议：
- Modbus TCP/RTU
- Virtual（模拟数据）
- gRPC（计划支持）

**关键配置**：
- 通道定义和协议参数
- CSV 点表配置
- 协议映射关系

### [ModSrv - 模型服务](./modsrv-config.md)  

提供面向对象的数据建模：
- 模板化配置
- 设备模型实例
- 数据映射和聚合

**关键配置**：
- 模型模板定义
- 实例映射配置
- Redis 同步参数

## 配置优先级

配置加载优先级（从高到低）：

1. **环境变量** - 用于容器化部署
2. **命令行参数** - 用于临时覆盖
3. **配置文件** - 基础配置
4. **默认值** - 代码内置默认值

### 环境变量命名规则

```bash
# 服务名_配置节_参数名（全大写）
COMSRV_CSV_BASE_PATH=/custom/path
MODSRV_REDIS_URL=redis://localhost:6379
APIGATEWAY_SERVICE_PORT=6005
```

## 快速开始

### 1. 最简配置示例

查看 `config-examples/minimal/` 目录：

```bash
config-examples/minimal/
├── comsrv.yaml         # 单通道配置
├── 1001/
│   ├── telemetry.csv   # 3个基本测点
│   └── mapping/
│       └── telemetry_mapping.csv
├── modsrv.yaml         # 基础服务配置
└── models.yaml         # 单个模型
```

### 2. 启动服务

```bash
# 使用最简配置启动 comsrv
./comsrv -c config-examples/minimal/comsrv.yaml

# 启动 modsrv
./modsrv -c config-examples/minimal/modsrv.yaml
```

### 3. 验证配置

```bash
# 验证 CSV 配置
./scripts/validate-comsrv-config.sh config-examples/minimal

# 检查服务状态
curl http://localhost:6000/health
curl http://localhost:6001/health
```

## Docker 部署配置

### 使用 Docker Compose

```yaml
services:
  comsrv:
    image: voltageems/comsrv
    volumes:
      - ./config:/app/config
    environment:
      - CSV_BASE_PATH=/app/config
      - REDIS_URL=redis://redis:6379
```

### 使用 ConfigMap (Kubernetes)

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: comsrv-config
data:
  comsrv.yaml: |
    csv_base_path: "/config"
    channels:
      - id: 1001
        protocol: "modbus_tcp"
```

## 配置管理最佳实践

### 1. 版本控制

```bash
# 创建配置仓库
git init config-repo
cd config-repo

# 组织配置文件
mkdir -p production staging development

# 提交配置
git add .
git commit -m "Initial configuration"
```

### 2. 配置模板化

使用环境变量实现配置复用：

```yaml
# base-config.yaml
channels:
  - id: ${CHANNEL_ID}
    protocol: "modbus_tcp"
    parameters:
      host: ${MODBUS_HOST}
      port: ${MODBUS_PORT:-502}
```

### 3. 配置验证

在部署前验证配置：

```bash
# 语法检查
yamllint config/*.yaml

# 业务逻辑验证
./scripts/validate-config.sh

# 模拟运行
./comsrv --validate -c config/comsrv.yaml
```

### 4. 配置备份

定期备份重要配置：

```bash
# 自动备份脚本
#!/bin/bash
BACKUP_DIR="/backups/config/$(date +%Y%m%d)"
mkdir -p $BACKUP_DIR
cp -r /app/config/* $BACKUP_DIR/
```

## 故障排除

### 配置加载失败

1. 检查文件路径和权限
2. 验证 YAML/CSV 语法
3. 查看服务启动日志

### 环境变量未生效

1. 确认变量名称格式正确
2. 检查变量是否导出 (`export VAR=value`)
3. 验证服务是否支持该配置项

### CSV 文件问题

1. 确保使用 UTF-8 编码
2. 检查分隔符（必须是逗号）
3. 验证必填字段完整性

## 配置示例库

在 `config-examples/` 目录中提供了多种配置示例：

- **minimal/**: 最简配置，快速启动
- **standard/**: 标准生产配置
- **multi-channel/**: 多通道配置示例
- **high-availability/**: 高可用配置

## 工具和脚本

- `validate-comsrv-config.sh`: 验证 ComSrv 配置
- `generate-config.py`: 配置生成工具
- `config-diff.sh`: 配置对比工具
- `backup-config.sh`: 配置备份脚本

## 相关文档

- [ComSrv 详细配置](./comsrv-config.md)
- [ModSrv 详细配置](./modsrv-config.md)
- [部署指南](../deployment/README.md)
- [API 文档](../api/README.md)