# Comsrv CI/CD使用指南

## 概述

本文档说明comsrv服务的CI/CD流程，包括自动化构建、测试、部署和回滚机制。

## CI/CD架构

```
代码提交 → Jenkins/GitHub Actions → 构建 → 测试 → 推送镜像 → 部署 → 健康检查
```

## 快速开始

### 1. 本地构建和测试

```bash
# 进入服务目录
cd services/comsrv

# 运行单元测试
./scripts/test.sh

# 构建Docker镜像
./scripts/build.sh

# 运行集成测试
./scripts/run-integration-test.sh
```

### 2. 使用项目级脚本

```bash
# 构建所有服务（包括comsrv）
./scripts/build-all.sh

# 运行所有服务的集成测试
./scripts/run-integration-tests.sh

# 部署到生产环境
./scripts/deploy.sh production 1.0.0

# 回滚到之前版本
./scripts/rollback.sh
```

## CI/CD流程详解

### 构建阶段

1. **代码检查**
   - 格式检查：`cargo fmt`
   - 代码质量：`cargo clippy`
   - 编译检查：`cargo check`

2. **单元测试**
   ```bash
   cargo test --all-features
   ```

3. **Docker镜像构建**
   - 多阶段构建优化镜像大小
   - 自动生成版本标签
   - 推送到本地Registry

### 测试阶段

1. **测试环境准备**
   - 启动Redis容器
   - 启动Modbus模拟服务器
   - 加载测试配置

2. **集成测试执行**
   - 通信协议测试
   - Redis数据同步测试
   - API接口测试
   - 健康检查验证

3. **性能测试**（可选）
   - 10000点压力测试
   - 响应时间测试
   - 内存占用测试

### 部署阶段

1. **部署前检查**
   - 验证目标环境
   - 检查磁盘空间
   - 备份当前版本

2. **部署执行**
   - 拉取新镜像
   - 停止旧容器
   - 启动新容器
   - 健康检查

3. **部署后验证**
   - 服务状态检查
   - 功能验证
   - 性能监控

## 环境配置

### 测试环境变量

配置文件：`services/comsrv/test.env`

```bash
# 服务配置
SERVICE_NAME=comsrv-test
SERVICE_PORT=3000

# 日志配置
RUST_LOG=debug,comsrv=trace

# Redis配置
REDIS_URL=redis://localhost:6379
REDIS_PREFIX=test:comsrv:

# 测试服务器
TEST_MODBUS_SERVER_1=localhost:5021
TEST_MODBUS_SERVER_2=localhost:5022
TEST_MODBUS_SERVER_3=localhost:5023
```

### 生产环境配置

```bash
# 使用配置中心
CONFIG_CENTER_URL=http://config-center:8080
CONFIG_CENTER_TOKEN=your-token-here

# 或使用本地配置
CONFIG_FILE=/app/config/comsrv.yaml
```

## Jenkins Pipeline使用

### 触发构建

1. **自动触发**
   - 推送到main分支
   - 创建Pull Request
   - 定时构建（每晚）

2. **手动触发**
   - Jenkins界面点击"Build Now"
   - 选择构建参数

### 查看构建结果

1. 访问Jenkins界面
2. 查看构建历史
3. 点击构建号查看详细日志
4. 下载测试报告

## GitHub Actions使用

### 工作流触发

推送代码到以下分支会触发CI：
- main
- develop
- feature/comsrv

### 查看运行结果

1. 访问GitHub仓库
2. 点击Actions标签
3. 选择工作流运行记录
4. 查看各步骤日志

## 故障排查

### 常见问题

1. **构建失败**
   ```bash
   # 检查Rust版本
   rustc --version
   
   # 清理构建缓存
   cargo clean
   ```

2. **测试失败**
   ```bash
   # 检查Redis连接
   redis-cli ping
   
   # 查看测试日志
   cat integration-test-report.txt
   ```

3. **部署失败**
   ```bash
   # 检查Docker状态
   docker ps
   
   # 查看容器日志
   docker logs comsrv
   ```

### 回滚操作

如果部署出现问题：

```bash
# 列出可用备份
./scripts/rollback.sh list

# 回滚到指定版本
./scripts/rollback.sh 2025-01-08-10-30-45
```

## 最佳实践

1. **代码提交前**
   - 运行本地测试
   - 检查代码格式
   - 更新文档

2. **版本管理**
   - 使用语义化版本
   - 记录变更日志
   - 标记重要版本

3. **监控和告警**
   - 关注构建状态
   - 设置失败通知
   - 定期检查日志

## 性能优化

1. **构建优化**
   - 使用构建缓存
   - 并行化测试
   - 优化Docker层

2. **部署优化**
   - 使用本地Registry
   - 预拉取镜像
   - 滚动更新

## 安全注意事项

1. **镜像安全**
   - 定期更新基础镜像
   - 扫描安全漏洞
   - 最小权限原则

2. **配置安全**
   - 使用环境变量
   - 加密敏感信息
   - 定期更换密钥

## 相关文档

- [VoltageEMS CI/CD架构](../../../docs/VOLTAGEEMS_CICD_ARCHITECTURE.md)
- [Comsrv配置指南](./comsrv配置指南.md)
- [Docker CI/CD指南](../../../docs/DOCKER_CICD_GUIDE.md)