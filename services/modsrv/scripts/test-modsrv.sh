#!/bin/bash
set -e

echo "🧪 Testing ModSrv Service (Business Logic Focus)"
echo "================================================"

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# 检查Redis是否在运行
check_redis() {
    if ! redis-cli ping > /dev/null 2>&1; then
        echo "📦 Starting test dependencies..."
        docker run -d --name redis-modsrv-test -p 6379:6379 redis:8-alpine
        sleep 2
    else
        echo "✅ Redis is already running"
    fi
}

# 清理函数
cleanup() {
    echo -e "\n🧹 Cleaning up..."
    if docker ps -a | grep -q redis-modsrv-test; then
        docker stop redis-modsrv-test && docker rm redis-modsrv-test
    fi
}

# 设置清理钩子
trap cleanup EXIT

# 主测试流程
main() {
    # 启动测试环境
    check_redis

    # 加载测试数据
    echo -e "\n📊 Loading test data..."
    redis-cli HSET "comsrv:1001:T" "1" "25.5" > /dev/null
    redis-cli HSET "comsrv:1001:S" "1" "1" > /dev/null
    
    # 运行单元测试
    echo -e "\n🔬 Running unit tests..."
    if cargo test -p modsrv --lib -- --nocapture; then
        echo -e "${GREEN}✅ Unit tests passed${NC}"
    else
        echo -e "${RED}❌ Unit tests failed${NC}"
        exit 1
    fi

    # 运行所有测试（包括集成测试）
    echo -e "\n🔗 Running all tests..."
    export REDIS_URL=redis://localhost:6379
    if cargo test -p modsrv -- --test-threads=1; then
        echo -e "${GREEN}✅ All tests passed${NC}"
    else
        echo -e "${RED}❌ Some tests failed${NC}"
        exit 1
    fi

    # 生成测试报告
    echo -e "\n📊 Generating test report..."
    cargo test -p modsrv -- -Z unstable-options --format json 2>/dev/null | tee test-results.json > /dev/null

    # 测试覆盖率（可选）
    if command -v cargo-tarpaulin &> /dev/null; then
        echo -e "\n📈 Generating coverage report..."
        cargo tarpaulin -p modsrv --out Html --output-dir coverage/ || true
    fi

    echo -e "\n${GREEN}✅ All tests passed!${NC}"
}

# 执行主函数
main