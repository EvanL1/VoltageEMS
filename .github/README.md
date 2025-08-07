# GitHub Actions CI/CD

## 工作流程

### 1. CI (ci.yml)
主要的持续集成工作流程，在每次推送和 PR 时运行。

**触发条件：**
- Push 到 main, develop, feature/* 分支
- Pull Request 到 main, develop

**步骤：**
1. **Check** - 代码质量检查
   - 格式检查 (cargo fmt)
   - 编译检查 (cargo check)
   - Clippy 分析

2. **Test** - 单元测试
   - 启动 Redis 服务
   - 加载 Lua 函数
   - 运行所有测试

3. **Docker** - 构建镜像
   - 为每个服务构建 Docker 镜像
   - 使用 BuildKit 缓存优化

4. **Integration** - 集成测试
   - 使用 docker-compose 启动服务
   - 检查健康端点
   - 运行基础 API 测试

### 2. Debug (debug.yml)
调试工作流程，用于排查 CI 问题。

**触发条件：**
- 手动触发 (workflow_dispatch)
- Push 到 debug-ci 分支

**用途：**
- 检查运行环境
- 验证项目结构
- 测试编译和依赖

## 本地验证

运行以下命令在本地验证 CI 配置：

```bash
./scripts/validate-ci.sh
```

## 常见问题

### 1. Clippy 警告
已配置忽略以下 Clippy 警告：
- `clippy::new_without_default`
- `clippy::uninlined_format_args`
- `clippy::approx_constant`
- `clippy::derivable_impls`

### 2. Redis Functions
CI 会自动加载 `scripts/redis-functions/*.lua` 中的所有函数。

### 3. 测试并行
测试使用 `--test-threads=1` 避免并发问题。

## 需要的 Secrets

如果要启用部署功能，需要在 GitHub 仓库设置以下 secrets：
- `STAGING_HOST` - 测试服务器地址
- `STAGING_USER` - 测试服务器用户
- `STAGING_KEY` - SSH 私钥
- `PROD_HOST` - 生产服务器地址
- `PROD_USER` - 生产服务器用户  
- `PROD_KEY` - SSH 私钥