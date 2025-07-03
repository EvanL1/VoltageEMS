# API Gateway 修改日志

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