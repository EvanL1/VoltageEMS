# 代码质量标准

本文档说明 VoltageEMS 项目的代码质量检查标准和建议。

## 检查级别

### 1. 必需检查（阻止提交/CI失败）
这些检查确保代码的基本质量：

- **格式化**: `cargo fmt --all`
- **编译检查**: `cargo check --workspace`
- **基础 Clippy**: `cargo clippy --all-targets --all-features -- -D warnings`
- **测试通过**: `cargo test --workspace`

### 2. 建议检查（警告但不阻止）
这些检查帮助提高代码质量，但在某些情况下可以有例外：

- **println! 使用**: 
  - 生产代码中应使用 `tracing` 或 `log`
  - 例外：二进制文件的启动横幅、CLI 输出
  
- **unwrap() 使用**:
  - 建议使用 `?` 操作符或 `.expect()` 与描述性消息
  - 例外：测试代码、确定安全的情况
  
- **TODO/FIXME 注释**:
  - 仅提示新增的 TODO
  - 不阻止提交，但应及时处理

### 3. 严格检查（可选，用于代码审查）
在发布前或代码审查时使用：

```bash
cargo clippy-strict  # 检查 unwrap、expect、panic 等
```

## 使用建议

### 日常开发
```bash
# 快速检查
cargo quality-check

# 或分步执行
cargo fmt
cargo clippy-standard
cargo test
```

### 提交前
Git hooks 会自动运行基础检查。如果需要跳过（紧急修复等）：
```bash
git commit --no-verify
```

### CI 环境
- PR 必须通过所有必需检查
- 主分支可选择性启用严格检查

## 例外情况

以下情况可以有合理的例外：

1. **测试代码**: 可以使用 `unwrap()`、`expect()`
2. **示例代码**: 可以使用 `println!`
3. **二进制入口**: 启动信息可以使用 `println!`
4. **原型开发**: 可以暂时使用 `todo!()`

## 逐步改进

对于现有代码：
1. 新代码必须符合标准
2. 修改现有代码时逐步改进
3. 不要求一次性修复所有问题

## 工具命令

```bash
# 标准检查
cargo clippy-standard

# 严格检查（可选）
cargo clippy-strict

# 自动修复
cargo fix
cargo fmt
cargo clippy --fix

# 完整质量检查
cargo quality-check
```