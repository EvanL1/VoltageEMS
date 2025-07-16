# API 前缀配置说明

## 概述

从现在开始，所有服务的API路径前缀都是可配置的。默认情况下，API前缀为空（即直接使用服务路径），但可以通过配置文件或环境变量设置前缀。

## 配置方式

### 1. 配置文件方式

在服务的 `config/default.yml` 中设置：

```yaml
api:
  prefix: ""  # 默认为空，可以设置为 "/api/v1" 或其他前缀
  enable_versioning: false
  version: "v1"
```

### 2. 环境变量方式

```bash
# 设置API前缀
export API_PREFIX=/api/v1

# 运行服务
cargo run -p rulesrv -- service
```

## URL示例

### 默认配置（无前缀）

- 规则列表：`GET http://localhost:8083/rules`
- 创建规则：`POST http://localhost:8083/rules`
- 获取规则：`GET http://localhost:8083/rules/{rule_id}`
- 执行规则：`POST http://localhost:8083/rules/{rule_id}/execute`

### 带前缀配置（prefix="/api/v1"）

- 规则列表：`GET http://localhost:8083/api/v1/rules`
- 创建规则：`POST http://localhost:8083/api/v1/rules`
- 获取规则：`GET http://localhost:8083/api/v1/rules/{rule_id}`
- 执行规则：`POST http://localhost:8083/api/v1/rules/{rule_id}/execute`

## API Gateway 配置

API Gateway 也支持相同的配置方式。当配置了前缀时，所有服务路由都会在该前缀下：

### 默认配置（无前缀）
- Rules服务：`/rulesrv/rules`
- Comsrv服务：`/comsrv/channels`

### 带前缀配置（prefix="/api/v1"）
- Rules服务：`/api/v1/rulesrv/rules`
- Comsrv服务：`/api/v1/comsrv/channels`

## 迁移说明

如果您的客户端代码使用了硬编码的 `/api/v1` 前缀，有两种选择：

1. **保持兼容**：在配置文件中设置 `api.prefix: "/api/v1"`
2. **更新客户端**：移除客户端代码中的 `/api/v1` 前缀，直接使用服务路径

## 测试脚本更新

测试脚本已更新为使用新的默认配置（无前缀）。如果您的服务配置了前缀，请在运行测试脚本时指定正确的URL：

```bash
python test_rule_trigger.py --api-url http://localhost:8083/api/v1
```