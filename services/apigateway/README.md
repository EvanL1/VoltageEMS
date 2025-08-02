# API Gateway 服务配置说明

## 配置文件结构

API Gateway 使用以下配置结构：

```yaml
# 服务器配置
server:
  host: "0.0.0.0"
  port: 8080
  workers: 4

# Redis 配置
redis:
  url: "redis://localhost:6379"
  pool_size: 10
  timeout_seconds: 30

# API 配置
api:
  prefix: "/api/v1"

# 认证配置
auth:
  jwt_secret: "your-secret-key-here"
  jwt_expiry_hours: 24

# 下游服务配置
services:
  comsrv:
    url: "http://comsrv:8081"
    timeout_seconds: 30
  modsrv:
    url: "http://modsrv:8082"
    timeout_seconds: 30
  alarmsrv:
    url: "http://alarmsrv:8083"
    timeout_seconds: 30
  rulesrv:
    url: "http://rulesrv:8084"
    timeout_seconds: 30
  hissrv:
    url: "http://hissrv:8085"
    timeout_seconds: 30
  netsrv:
    url: "http://netsrv:8086"
    timeout_seconds: 30
```

## 配置加载方式

1. 服务会首先查找名为 `apigateway.yaml` 的文件
2. 然后使用 `APIGATEWAY_` 前缀的环境变量覆盖配置

## Docker 部署注意事项

在 Docker 中部署时，需要将配置文件复制或链接为 `apigateway.yaml`：

```bash
# 在容器内创建链接
ln -s /app/config/config.yaml /app/apigateway.yaml
```

或者修改 Dockerfile：

```dockerfile
COPY config.yaml /app/apigateway.yaml
```