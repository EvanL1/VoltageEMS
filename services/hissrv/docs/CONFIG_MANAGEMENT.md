# hissrv 配置管理指南

## 概述

hissrv 支持通过 RESTful API 进行配置管理，允许在运行时动态修改数据映射规则，无需重启服务。

## 启用配置管理

在 `config/hissrv.yaml` 中添加以下配置：

```yaml
service:
  name: "hissrv"
  polling_interval: 10s
  enable_api: true      # 启用配置管理 API
  api_port: 8082        # API 服务端口
```

## API 端点

### 1. 获取完整配置
```bash
GET /config
```

### 2. 映射规则管理

#### 列出所有映射
```bash
GET /mappings
```

#### 查找特定映射
```bash
GET /mappings/{source}
# 例如: GET /mappings/archive:1m:*
```

#### 添加新映射
```bash
POST /mappings
Content-Type: application/json

{
  "source": "archive:15m:*",
  "measurement": "metrics_15m",
  "tags": [
    {"type": "extract", "field": "channel"},
    {"type": "static", "value": "interval=15m"}
  ],
  "fields": [
    {"name": "voltage_avg", "field_type": "float"},
    {"name": "current_avg", "field_type": "float"}
  ]
}
```

#### 更新映射
```bash
PUT /mappings/{source}
Content-Type: application/json

{
  "source": "archive:1m:*",
  "measurement": "metrics_1m_v2",
  "tags": [...],
  "fields": [...]
}
```

#### 删除映射
```bash
DELETE /mappings/{source}
```

### 3. 配置验证和重载

#### 验证配置
```bash
POST /validate
```

#### 重新加载配置（从文件）
```bash
POST /reload
```

## 配置热重载

### 方式一：通过 API
```bash
curl -X POST http://localhost:8082/reload
```

### 方式二：通过信号（Unix/Linux）
```bash
kill -HUP <hissrv_pid>
```

## 数据映射规则说明

### 映射规则结构
```yaml
mappings:
  - source: "archive:1m:*"      # Redis key 模式
    measurement: "metrics_1m"    # InfluxDB measurement 名称
    tags:                        # 标签规则
      - type: "extract"          # 从 key 中提取
        field: "channel"         # 提取的字段名
      - type: "static"           # 静态标签
        value: "interval=1m"     # 标签键值对
    fields:                      # 字段映射
      - name: "voltage_avg"      # 字段名
        field_type: "float"      # 数据类型
```

### 标签规则类型

1. **extract**: 从 Redis key 中提取值
   - 例如：key "archive:1m:1001" 提取 channel=1001

2. **static**: 静态标签值
   - 例如：添加固定标签 interval=1m

### 字段类型

- `float`: 浮点数
- `int`: 整数
- `bool`: 布尔值
- `string`: 字符串

## 使用示例

完整的使用示例见 `scripts/hissrv-config-examples.sh`

```bash
# 运行示例脚本
./scripts/hissrv-config-examples.sh
```

## 注意事项

1. 所有配置修改会自动保存到配置文件
2. 配置修改立即生效，无需重启服务
3. 修改连接参数（Redis/InfluxDB URL）时会自动重连
4. 建议在修改前先验证配置的正确性

## API Gateway 集成

hissrv 的配置管理 API 可以通过 API Gateway 访问：

```bash
# 通过 API Gateway 访问（需要认证）
curl -H "Authorization: Bearer <token>" \
     http://api-gateway:8080/api/v1/hissrv/config

# 直接访问 hissrv（仅限内部网络）
curl http://hissrv:8082/config
```