# ModSrv 配置指南

## 概述

ModSrv是一个轻量级的模型服务，专为边端设备设计。它提供简洁的API接口用于设备模型管理、数据查询和控制命令下发。与传统的内存缓存方案不同，ModSrv直接从Redis读取实时数据，通过Lua脚本实现与ComsRv的高效数据同步。

## 核心功能

1. **模型管理** - 定义和管理设备模型元数据
2. **数据查询** - 从Redis实时读取模型数据
3. **控制下发** - 通过Lua脚本转发控制命令
4. **WebSocket推送** - 实时数据变化通知

## 配置文件结构

### 主配置文件 (`config/default.yml`)

```yaml
# 服务基本信息
service_name: "modsrv"
version: "2.0.0"

# Redis连接配置
redis:
  url: "redis://localhost:6379"
  key_prefix: "modsrv:"

# API服务配置
api:
  host: "0.0.0.0"
  port: 8002
  
# 日志配置
log:
  level: "info"
  file: "logs/modsrv.log"

# 更新间隔（毫秒）
update_interval_ms: 1000

# 模型定义
models:
  - id: "power_meter_demo"
    name: "演示电表模型"
    description: "用于演示的简单电表监控模型"
    monitoring:
      voltage_a:
        description: "A相电压"
        unit: "V"
      current_a:
        description: "A相电流"  
        unit: "A"
      power:
        description: "有功功率"
        unit: "kW"
    control:
      main_switch:
        description: "主开关"
      power_limit:
        description: "功率限制设定"
        unit: "kW"
```

### 点位映射配置 (`config/mappings/{model_id}.json`)

映射文件定义了模型点位与ComsRv通道点位的对应关系：

```json
{
  "monitoring": {
    "voltage_a": {
      "channel": 1001,
      "point": 10001,
      "type": "m"
    },
    "current_a": {
      "channel": 1001,
      "point": 10002,
      "type": "m"
    },
    "power": {
      "channel": 1001,
      "point": 10003,
      "type": "m"
    }
  },
  "control": {
    "main_switch": {
      "channel": 1001,
      "point": 30001,
      "type": "c"
    },
    "power_limit": {
      "channel": 1001,
      "point": 40001,
      "type": "a"
    }
  }
}
```

## 环境变量

ModSrv支持以下环境变量，用于覆盖配置文件中的设置：

| 变量名 | 描述 | 默认值 |
|--------|------|--------|
| `REDIS_URL` | Redis连接URL | `redis://localhost:6379` |
| `CONFIG_FILE` | 配置文件路径 | `config/default.yml` |
| `MAPPINGS_DIR` | 映射文件目录 | `config/mappings` |
| `RUST_LOG` | 日志级别 | `info` |

## 数据同步机制

ModSrv使用Lua脚本（`edge_sync.lua`）实现与ComsRv的数据同步：

### 1. 测量数据同步
- ComsRv写入数据时，Lua脚本自动同步到ModSrv的Hash结构
- 数据存储在 `modsrv:{model_id}:measurement` 

### 2. 控制命令转发
- ModSrv收到控制命令后，通过Lua脚本查找映射
- 自动转发到对应的Comsrv通道

### 3. 映射管理
- 启动时将所有映射加载到Redis
- 支持动态更新映射关系

## API接口

### REST API

```bash
# 健康检查
GET /health

# 获取模型列表
GET /models

# 获取模型实时数据
GET /models/{model_id}/values

# 发送控制命令
POST /models/{model_id}/control/{control_name}
Content-Type: application/json
{
  "value": 1.0
}
```

### WebSocket

```bash
# 订阅模型实时数据
WS /ws/{model_id}

# 消息格式
{
  "type": "Update",
  "data": {
    "point": "voltage_a",
    "value": 220.5
  }
}
```

## 部署建议

### 边端设备部署

1. **资源要求**
   - 内存：< 50MB
   - CPU：单核即可
   - 存储：< 10MB（不包括日志）

2. **启动命令**
   ```bash
   # 直接运行
   ./modsrv service
   
   # 指定配置文件
   ./modsrv -c /path/to/config.yml service
   
   # Docker运行
   docker run -d \
     --name modsrv \
     -p 8002:8002 \
     -e REDIS_URL=redis://redis:6379 \
     voltage/modsrv
   ```

3. **性能优化**
   - 使用连接池管理Redis连接
   - 合理设置更新间隔
   - 启用日志轮转避免磁盘占用

## 监控和维护

### 日志查看

```bash
# 实时查看日志
tail -f logs/modsrv.log

# 查看错误日志
grep ERROR logs/modsrv.log
```

### 性能监控

```bash
# Redis监控
redis-cli monitor | grep modsrv

# 查看Hash大小
redis-cli hlen modsrv:power_meter_demo:measurement
```

### 故障排查

1. **连接失败**
   - 检查Redis是否运行
   - 验证网络连接
   - 查看防火墙设置

2. **数据不更新**
   - 检查映射配置是否正确
   - 验证Lua脚本是否加载成功
   - 查看ComsRv是否正常运行

3. **控制命令失败**
   - 检查控制点映射
   - 验证权限设置
   - 查看错误日志

## 配置示例

### 最小配置

```yaml
service_name: "modsrv"
version: "2.0.0"
redis:
  url: "redis://localhost:6379"
api:
  host: "0.0.0.0"
  port: 8002
models: []  # 无模型，仅作为API网关
```

### 多模型配置

```yaml
models:
  - id: "meter_1"
    name: "1号电表"
    monitoring:
      voltage: { description: "电压", unit: "V" }
      current: { description: "电流", unit: "A" }
    control:
      switch: { description: "开关" }
      
  - id: "meter_2"
    name: "2号电表"
    monitoring:
      voltage: { description: "电压", unit: "V" }
      current: { description: "电流", unit: "A" }
    control:
      switch: { description: "开关" }
```

## 最佳实践

1. **模型设计**
   - 保持模型简洁，只定义必要的点位
   - 使用有意义的点位名称
   - 合理设置单位信息

2. **映射管理**
   - 使用版本控制管理映射文件
   - 定期备份映射配置
   - 避免点位冲突

3. **性能优化**
   - 批量读取数据而非逐个查询
   - 合理使用WebSocket避免轮询
   - 监控Redis内存使用

4. **安全建议**
   - 限制API访问权限
   - 使用Redis密码认证
   - 定期更新依赖包