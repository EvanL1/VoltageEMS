#!/bin/bash
# 提交前检查脚本 - 模拟 CI 环境

set -e

echo "🔍 Running pre-commit checks..."

# 格式检查
echo "📝 Checking formatting..."
cargo fmt -- --check
echo "✅ Format check passed"

# 构建检查
echo "🔨 Building project..."
cargo build --all-features
echo "✅ Build passed"

# 关键 clippy 检查（会阻塞提交）
echo "🚨 Running critical clippy checks..."
cargo clippy --all-targets --all-features -- \
    -D clippy::correctness \
    -D clippy::suspicious \
    -D deprecated
echo "✅ Critical checks passed"

# 运行测试
echo "🧪 Running tests..."
cargo test --all-features
echo "✅ Tests passed"

# 完整 clippy 检查（仅供参考）
echo ""
echo "📊 Running full clippy analysis (informational)..."
cargo clippy --all-targets --all-features 2>&1 || true

echo ""
echo "✅ All critical checks passed! Safe to commit."
echo "💡 See clippy output above for additional suggestions."