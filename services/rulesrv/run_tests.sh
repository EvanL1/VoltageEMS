#!/bin/bash
# rulesrv本地测试运行脚本

set -e

echo "🧪 Running rulesrv tests..."
echo "=========================="

# 设置环境变量
export RUST_BACKTRACE=1
export RUST_LOG=rulesrv=debug,info

# 检查Redis是否运行
if ! redis-cli ping > /dev/null 2>&1; then
    echo "❌ Redis is not running. Please start Redis first."
    echo "   Run: docker run -d --name redis-test -p 6379:6379 redis:8-alpine"
    exit 1
fi

echo "✅ Redis is running"

# 清理测试数据
echo "🧹 Cleaning test data..."
redis-cli --scan --pattern "rulesrv:*" | xargs -L 100 redis-cli DEL 2>/dev/null || true

# 运行单元测试
echo ""
echo "📦 Running unit tests..."
cargo test --lib -- --nocapture

# 运行集成测试
echo ""
echo "🔗 Running integration tests..."
cargo test --test '*' -- --nocapture

# 运行文档测试
echo ""
echo "📚 Running doc tests..."
cargo test --doc

# 检查代码
echo ""
echo "🔍 Running cargo check..."
cargo check --all-targets

# 运行clippy
echo ""
echo "📋 Running clippy..."
cargo clippy --all-targets --all-features -- -D warnings

# 格式检查
echo ""
echo "✨ Checking formatting..."
cargo fmt -- --check

echo ""
echo "✅ All tests passed!"