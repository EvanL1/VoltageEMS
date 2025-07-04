# API Gateway 修改日志

## 2025-07-04

### 配置中心集成实现
- 创建 config_client.rs 实现配置服务客户端
- 支持从配置中心动态获取配置
- 实现配置版本检查和自动更新（30秒轮询）
- 添加配置校验和错误处理
- 支持配置服务不可用时回退到本地配置
- 添加配置订阅通知机制接口
- 创建 CONFIG_SERVICE_API.md 文档定义 REST API
- 更新 error.rs 添加配置相关错误类型
- 创建 start-with-config-service.sh 启动脚本

### 配置管理架构
- 配置中心服务提供统一的配置管理 REST API
- 支持配置版本管理、历史记录、回滚功能
- 支持配置导入导出和验证
- 通过 webhook 实现配置变更通知
- API Gateway 作为配置客户端定期拉取更新

## 2025-07-04

### 统一服务端口配置
- 统一所有服务使用 8001-8005 端口范围（之前存在 8081-8085 的不一致）
- 修改 apigateway.yaml 配置文件中的服务端口
- 修改 src/config.rs 中的默认端口配置
- 删除重复的 apigateway.yml 文件（保留 apigateway.yaml）
- 更新 README.md 中的端口配置示例
- 修正 CORS 配置中的前端地址为 8082

## 2024-01-15

### 实现认证和授权功能
- 添加 JWT 认证模块 (auth/jwt.rs)
- 实现认证中间件 (auth/middleware.rs)
- 添加登录、刷新token、登出等认证端点 (handlers/auth.rs)
- 支持四种角色: admin、operator、engineer、viewer

### 实现统一响应格式
- 创建标准响应结构 (response.rs)
- 成功响应: `{success: true, data: {...}, timestamp: "..."}`
- 错误响应: `{success: false, error: {...}, timestamp: "..."}`
- 更新所有端点使用统一格式

### 项目配置增强
- 添加 JWT 相关依赖
- 创建配置文件示例 (apigateway.yml)
- 创建 Dockerfile 支持容器化部署
- 支持环境变量覆盖配置

### API 设计
- 公开端点: /api/v1/auth/login, /api/v1/auth/refresh, /health
- 受保护端点: 所有服务代理路由需要 Bearer Token
- 添加用户信息端点: /api/v1/auth/me

### 错误处理改进
- 添加认证相关错误类型
- 使用统一的错误响应格式
- 改进错误代码命名规范