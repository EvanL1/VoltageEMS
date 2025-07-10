# VoltageEMS 本地 CI 指南

本文档介绍如何在本地运行完整的 CI/CD 流程，确保代码质量后再推送到 GitHub。

## 概述

我们提供了多种工具来支持本地 CI：

1. **Earthly** - 统一的多语言构建工具
2. **Lefthook** - Git 钩子管理器
3. **Act** - 本地运行 GitHub Actions
4. **本地 CI 脚本** - 一键运行所有检查

## 安装必要的工具

### macOS (使用 Homebrew)

```bash
# 安装 Earthly
brew install earthly/earthly/earthly

# 安装 Lefthook
brew install lefthook

# 安装 Act
brew install act

# 安装其他有用的工具
cargo install taplo-cli        # TOML 格式化
cargo install cargo-nextest    # 更快的测试运行器
brew install yamllint          # YAML 检查
npm install -g markdownlint-cli # Markdown 检查
```

### Linux

```bash
# 安装 Earthly
sudo /bin/sh -c 'wget https://github.com/earthly/earthly/releases/latest/download/earthly-linux-amd64 -O /usr/local/bin/earthly && chmod +x /usr/local/bin/earthly'

# 其他工具使用相应的包管理器安装
```

## 快速开始

### 1. 初始化 Git 钩子

```bash
# 安装 Lefthook 钩子
lefthook install
```

这会自动设置以下钩子：
- **pre-commit**: 运行格式检查和 linting
- **commit-msg**: 检查提交消息格式
- **pre-push**: 运行测试和构建

### 2. 运行本地 CI

最简单的方式是使用我们提供的脚本：

```bash
# 运行完整的本地 CI 检查
./scripts/local-ci.sh

# 同时运行 Act 检查（模拟 GitHub Actions）
./scripts/local-ci.sh --with-act
```

### 3. 使用 Earthly

Earthly 提供了更细粒度的控制：

```bash
# 运行所有检查
earthly +ci

# 只运行格式检查
earthly +fmt-rust

# 只运行 Clippy
earthly +clippy-rust

# 只运行测试
earthly +test-rust

# 构建所有服务
earthly +build-rust-services

# 构建 Docker 镜像
earthly +docker-all
```

### 4. 使用 Act 测试 GitHub Actions

```bash
# 运行所有工作流
act

# 只运行特定的 job
act -j fmt
act -j clippy
act -j test

# 使用特定事件
act push
act pull_request
```

## 工作流程

### 推荐的开发流程

1. **编写代码**
2. **本地测试**: `cargo test`
3. **格式化**: `cargo fmt`
4. **提交**: `git commit` (Lefthook 会自动检查)
5. **完整 CI**: `./scripts/local-ci.sh`
6. **推送**: `git push` (Lefthook 会运行测试)

### CI 检查内容

1. **代码格式** (rustfmt)
2. **代码质量** (clippy)
3. **单元测试** (cargo test)
4. **构建检查** (cargo build)
5. **Docker 镜像构建** (可选)

## 配置文件说明

### Earthfile

`Earthfile` 定义了所有构建步骤：
- 支持 Rust 和 C++ 构建（为未来扩展准备）
- 使用 cargo-chef 优化 Docker 构建
- 并行执行检查以提高速度

### lefthook.yml

配置了 Git 钩子：
- **pre-commit**: 快速检查（格式、lint）
- **commit-msg**: 确保提交消息符合规范
- **pre-push**: 完整测试

### .actrc

Act 的配置文件：
- 使用合适的 Docker 镜像
- 设置环境变量
- 配置网络模式

## 故障排除

### Redis 连接错误

如果测试需要 Redis，脚本会自动启动一个 Docker 容器。确保 Docker 正在运行。

### Earthly 构建失败

```bash
# 清理 Earthly 缓存
earthly prune

# 使用 --no-cache 选项
earthly --no-cache +ci
```

### Act 运行缓慢

Act 需要下载 Docker 镜像，首次运行可能较慢。使用 `.actrc` 中的配置可以重用容器。

### Lefthook 钩子未触发

```bash
# 重新安装钩子
lefthook uninstall
lefthook install

# 手动运行钩子
lefthook run pre-commit
```

## 高级用法

### 自定义 Earthly 目标

在 `Earthfile` 中添加新目标：

```earthfile
my-custom-check:
    FROM +rust-base
    COPY . .
    RUN my-custom-command
```

### 添加新的 Lefthook 钩子

编辑 `lefthook.yml`：

```yaml
pre-commit:
  commands:
    my-check:
      glob: "*.rs"
      run: my-custom-check {staged_files}
```

### 并行运行 CI 任务

```bash
# 使用 GNU Parallel
parallel ::: \
  "cargo fmt --check" \
  "cargo clippy" \
  "cargo test"
```

## 持续改进

我们的本地 CI 工具会持续更新。如果您有建议或遇到问题，请：

1. 查看最新的工具文档
2. 在项目中提出 Issue
3. 贡献改进方案

## 相关链接

- [Earthly 文档](https://docs.earthly.dev/)
- [Lefthook 文档](https://github.com/evilmartians/lefthook)
- [Act 文档](https://github.com/nektos/act)
- [GitHub Actions 文档](https://docs.github.com/en/actions)