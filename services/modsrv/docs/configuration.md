# ModSrv v2.0 配置文档

## 概述

ModSrv (Model Service) v2.0 是一个工业设备模型管理服务，支持设备模型定义、实时数据监控和控制命令执行。

**v2.0重要变更**: 
- 模型配置从主配置文件中分离，使用独立的YAML模型文件
- 简化为`monitoring`/`control`二分类架构
- 物理地址映射独立管理在`mappings/`目录
- 支持动态加载模型配置

本文档详细说明了服务的配置方式。

## 主要配置文件

### 1. 服务主配置 (`test-configs/config.yml`)

**注意**: ModSrv v2.0采用分离的配置架构，主配置文件负责服务配置，模型定义使用独立的YAML文件。

```yaml
# 服务基本信息
service_name: "modsrv"
version: "2.0.0"

# Redis配置
redis:
  url: "redis://localhost:6379"
  key_prefix: "modsrv:"
  connection_timeout_ms: 5000
  retry_attempts: 3

# 日志配置
log:
  level: "info"  # trace, debug, info, warn, error
  file_path: "logs/modsrv.log"

# API配置
api:
  host: "0.0.0.0"
  port: 8092
  timeout_seconds: 30

# 模型配置
models:
  # 模型配置文件目录
  models_dir: "/config/models"
  # 映射配置目录
  mappings_dir: "/config/mappings"
  # 自动加载模型配置文件
  auto_load: true
  # 模型配置文件格式 (yaml | json)
  config_format: "json"

# 更新间隔
update_interval_ms: 1000
```

### 2. 模型配置文件 (`test-configs/models/*.json`)

每个模型使用独立的JSON文件定义，包含监视点和控制点配置。

#### 电表模型配置 (`test-configs/models/power_meter_demo.json`)

```json
{
  "id": "power_meter_demo",
  "name": "演示电表模型",
  "description": "用于演示的简单电表监控模型v2.0",
  "version": "2.0.0",
  "enabled": true,
  "monitoring": {
    "voltage_a": {
      "description": "A相电压",
      "unit": "V"
    },
    "current_a": {
      "description": "A相电流",
      "unit": "A"
    },
    "power": {
      "description": "有功功率",
      "unit": "kW"
    }
  },
  "control": {
    "main_switch": {
      "description": "主开关"
    },
    "power_limit": {
      "description": "功率限制设定",
      "unit": "kW"
    }
  },
  "metadata": {
    "category": "power_meter",
    "manufacturer": "demo",
    "created_at": "2025-07-25",
    "tags": ["demo", "power", "meter"]
  }
}
```

### 3. 点位映射配置 (`test-configs/mappings/*.json`)

映射配置将模型中的逻辑点位名称映射到实际的通道和点位ID。

#### 电表模型映射 (`test-configs/mappings/power_meter_demo.json`)

```json
{
  "monitoring": {
    "voltage_a": {
      "channel": 1001,
      "point": 10001,
      "type": "m"
    },
    "voltage_b": {
      "channel": 1001,
      "point": 10002,
      "type": "m"
    },
    "voltage_c": {
      "channel": 1001,
      "point": 10003,
      "type": "m"
    },
    "current_a": {
      "channel": 1001,
      "point": 10004,
      "type": "m"
    },
    "current_b": {
      "channel": 1001,
      "point": 10005,
      "type": "m"
    },
    "current_c": {
      "channel": 1001,
      "point": 10006,
      "type": "m"
    },
    "power": {
      "channel": 1001,
      "point": 10007,
      "type": "m"
    },
    "energy_total": {
      "channel": 1001,
      "point": 10008,
      "type": "m"
    }
  },
  "control": {
    "main_switch": {
      "channel": 1001,
      "point": 20001,
      "type": "c"
    },
    "power_limit": {
      "channel": 1001,
      "point": 20002,
      "type": "a"
    }
  }
}
```

## 环境变量配置

| 环境变量 | 默认值 | 说明 |
|---------|--------|------|
| `CONFIG_FILE` | `test-configs/config.yml` | 主配置文件路径 |
| `MODELS_DIR` | `test-configs/models` | 模型配置目录路径 |
| `MAPPINGS_DIR` | `test-configs/mappings` | 映射配置目录路径 |
| `REDIS_URL` | `redis://localhost:6379` | Redis连接URL |
| `RUST_LOG` | `info` | 日志级别配置 |
| `RUST_BACKTRACE` | `0` | 错误堆栈跟踪 |

## 配置项详细说明

### Redis配置

- **url**: Redis服务器连接地址
- **key_prefix**: Redis键前缀，用于命名空间隔离
- **connection_timeout_ms**: 连接超时时间（毫秒）
- **retry_attempts**: 连接重试次数

### API配置

- **host**: API服务监听地址（0.0.0.0表示监听所有接口）
- **port**: API服务端口
- **timeout_seconds**: HTTP请求超时时间

### 模型配置

#### 主配置中的模型部分

- **models_dir**: 模型配置文件目录路径
- **mappings_dir**: 映射配置文件目录路径  
- **auto_load**: 是否自动加载模型配置文件
- **config_format**: 模型配置文件格式（yaml | json）

#### 模型配置文件结构

每个模型配置文件包含以下字段：

- **id**: 模型唯一标识符
- **name**: 模型显示名称
- **description**: 模型描述
- **version**: 模型版本
- **enabled**: 是否启用该模型
- **monitoring**: 监视点位配置
  - 每个监视点包含 `description`（描述）和可选的 `unit`（单位）
- **control**: 控制点位配置
  - 每个控制点包含 `description`（描述）和可选的 `unit`（单位）
- **metadata**: 模型元数据
  - `category`: 模型分类
  - `manufacturer`: 制造商
  - `created_at`: 创建时间
  - `tags`: 标签列表

### 映射配置

映射配置将逻辑点位名称映射到物理地址：

- **channel**: 通道ID（对应comsrv的通道）
- **point**: 点位ID
- **type**: 点位类型
  - `m`: 测量点（measurement）
  - `s`: 信号点（signal）
  - `c`: 控制点（control）
  - `a`: 调节点（adjustment）

## Docker部署配置

### 环境变量设置

```yaml
environment:
  - RUST_LOG=modsrv=debug,redis=info
  - REDIS_URL=redis://redis:6379
  - CONFIG_FILE=/config/config.yml
  - MAPPINGS_DIR=/config/mappings
```

### 卷映射

```yaml
volumes:
  - ./config:/config:ro          # 配置文件目录
  - ./logs/modsrv:/logs          # 日志目录
  - ./templates:/app/templates:ro # 模板目录
```

## 配置验证

服务启动时会自动验证配置：

1. **配置文件格式验证**: 检查YAML语法和必需字段
2. **映射配置验证**: 检查映射文件格式和引用完整性
3. **Redis连接验证**: 测试Redis连接可用性
4. **端口绑定验证**: 检查API端口是否可用

## 配置最佳实践

1. **环境分离**: 为不同环境（开发、测试、生产）使用不同的配置文件
2. **敏感信息**: 使用环境变量存储Redis密码等敏感信息
3. **日志级别**: 生产环境建议使用 `info` 或 `warn` 级别
4. **监控配置**: 定期检查配置文件的完整性和映射的准确性
5. **备份配置**: 定期备份配置文件，特别是映射配置

## 故障排查

### 常见配置问题

1. **映射文件缺失**
   ```
   ERROR: 控制点映射不存在: model_id:control_name
   ```
   - 检查 `MAPPINGS_DIR` 环境变量
   - 确认映射文件存在且格式正确

2. **Redis连接失败**
   ```
   ERROR: Redis连接失败
   ```
   - 检查 `REDIS_URL` 配置
   - 确认Redis服务运行状态

3. **端口绑定失败**
   ```
   ERROR: 绑定地址失败 0.0.0.0:8092
   ```
   - 检查端口是否被占用
   - 确认防火墙设置

## 模型配置架构

### v2.0分离配置架构

ModSrv v2.0采用分离的配置架构，职责明确的多文件配置：

```
test-configs/
├── config.yml              # 主配置文件，服务配置
├── models/                 # 模型配置目录
│   ├── power_meter_demo.json   # 电表模型配置
│   └── transformer_demo.json  # 变压器模型配置
└── mappings/               # 映射配置目录
    ├── power_meter_demo.json   # 电表模型映射
    └── transformer_demo.json   # 变压器模型映射
```

**设计原则**:
- 主配置文件专注于服务配置，不包含模型定义
- 每个模型使用独立的JSON文件定义
- 物理地址映射单独管理在`mappings/`目录
- 简化为`monitoring`/`control`二分类
- 支持动态加载和模型管理

**主配置示例**:
```yaml
# 模型配置
models:
  models_dir: "/config/models"
  mappings_dir: "/config/mappings"
  auto_load: true
  config_format: "json"
```

**模型配置示例**:
```json
// power_meter_demo.json
{
  "id": "power_meter_demo",
  "name": "演示电表模型",
  "enabled": true,
  "monitoring": {
    "voltage_a": {
      "description": "A相电压",
      "unit": "V"
    }
  },
  "control": {
    "main_switch": {
      "description": "主开关"
    }
  }
}
```

## 配置更新

配置更新需要重启服务。建议的更新流程：

1. 备份当前配置
2. 更新配置文件
3. 验证配置语法
4. 重启服务
5. 验证服务正常运行