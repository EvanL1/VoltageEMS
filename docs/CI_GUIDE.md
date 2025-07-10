# VoltageEMS CI/CD 指南

## 概述

本指南帮助团队成员在推送代码前完成必要的质量检查，确保代码库的健康和稳定。

## 快速开始

### 1. 安装必要工具

```bash
# 安装 Lefthook（Git hooks 管理器）
brew install lefthook

# 安装 Git hooks
lefthook install

# （可选）安装其他 CI 工具
brew install earthly/earthly/earthly
brew install act
```

### 2. 日常开发流程

#### 提交前自动检查

当你运行 `git commit` 时，会自动进行：
- ✅ 代码格式检查
- ✅ 基础编译检查
- ✅ voltage-common 库的质量检查

#### 推送前检查

运行 `git push` 前，系统会自动执行 `pre-push-check.sh`：
- 代码格式验证
- 编译检查
- Clippy 静态分析（部分）
- Git 状态检查
- 大文件检查

### 3. 手动检查命令

```bash
# 快速检查（推荐日常使用）
./scripts/quick-check.sh

# 完整的推送前检查
./scripts/pre-push-check.sh

# 运行特定检查
cargo fmt --all -- --check    # 格式检查
cargo clippy -p voltage-common # 检查核心库
cargo test --workspace         # 运行测试
```

### 4. 处理检查失败

#### 格式问题
```bash
# 自动修复格式
cargo fmt --all
```

#### Clippy 警告
```bash
# 查看详细警告
cargo clippy --all-targets -- -W warnings

# 只检查特定包
cargo clippy -p voltage-common
```

#### 编译错误
```bash
# 查看详细错误
cargo check --workspace

# 清理并重新构建
cargo clean
cargo build --workspace
```

## CI 工具说明

### 1. Lefthook
- 自动在 Git 操作时运行检查
- 配置文件：`lefthook.yml`
- 跳过钩子：`LEFTHOOK=0 git commit`

### 2. 本地 CI 脚本
- `quick-check.sh` - 快速基础检查（1-2秒）
- `pre-push-check.sh` - 完整推送前检查（10-30秒）
- `local-ci.sh` - 完整 CI 流程（包括 Earthly）

### 3. Earthly（可选）
- 容器化的构建系统
- 配置文件：`Earthfile`
- 运行：`earthly +ci`

## 最佳实践

### 1. 提交前
- 运行 `cargo fmt --all`
- 修复明显的编译错误
- 确保至少 voltage-common 通过检查

### 2. 功能开发
- 创建功能分支：`git checkout -b feature/xxx`
- 小步提交，频繁推送
- 在合并前运行完整检查

### 3. 代码审查
- PR 前运行 `./scripts/pre-push-check.sh`
- 修复所有错误，尽量减少警告
- 更新相关文档

## 常见问题

### Q: 检查太慢怎么办？
A: 使用 `./scripts/quick-check.sh` 进行日常检查，只在推送前运行完整检查。

### Q: 如何跳过 Git hooks？
A: 使用 `LEFTHOOK=0 git commit` 或 `git commit --no-verify`（不推荐）。

### Q: Clippy 警告太多怎么办？
A: 
1. 优先修复 voltage-common 的警告
2. 其他服务的警告可以逐步修复
3. 使用 `#[allow(clippy::xxx)]` 临时忽略特定警告

### Q: 如何在 CI 中排除某些检查？
A: 编辑相应的脚本文件，注释掉不需要的检查步骤。

## 持续改进

1. 定期更新依赖：`cargo update`
2. 关注编译警告，及时修复
3. 逐步提高代码质量标准
4. 分享 CI/CD 最佳实践

## 联系支持

如果遇到 CI/CD 相关问题：
1. 查看 `docs/fixlog/` 中的修复记录
2. 在团队频道中提问
3. 提交 Issue 到项目仓库