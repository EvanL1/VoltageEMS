#!/bin/bash

# ==================================================
# VoltageEMS CI 验证脚本
# 本地测试 GitHub Actions 工作流程
# ==================================================

set -e

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo "========================================"
echo "    VoltageEMS CI 本地验证"
echo "========================================"
echo ""

# 测试计数
TOTAL=0
PASSED=0
FAILED=0

# 测试函数
run_test() {
    local name=$1
    local cmd=$2
    
    echo -e "${BLUE}[TEST]${NC} $name"
    ((TOTAL++))
    
    if eval "$cmd" > /dev/null 2>&1; then
        echo -e "${GREEN}[✓]${NC} $name passed"
        ((PASSED++))
    else
        echo -e "${RED}[✗]${NC} $name failed"
        ((FAILED++))
    fi
}

# 1. 代码格式检查
echo -e "\n${YELLOW}1. Code Quality Checks${NC}"
run_test "Rust formatting" "cargo fmt --all -- --check"
run_test "Compilation check" "cargo check --workspace"

# 2. Clippy 检查（使用 CI 相同的配置）
echo -e "\n${YELLOW}2. Clippy Analysis${NC}"
run_test "Clippy linting" "cargo clippy --all-targets --all-features -- -D warnings -A clippy::new_without_default -A clippy::uninlined_format_args -A clippy::approx_constant -A clippy::derivable_impls"

# 3. 测试 Redis 连接
echo -e "\n${YELLOW}3. Redis Connection${NC}"
if docker ps | grep -q redis; then
    run_test "Redis connectivity" "redis-cli ping"
else
    echo -e "${YELLOW}[!]${NC} Redis not running, starting..."
    docker run -d --name redis-test -p 6379:6379 redis:8-alpine
    sleep 3
    run_test "Redis connectivity" "redis-cli ping"
fi

# 4. 加载 Lua 函数
echo -e "\n${YELLOW}4. Redis Functions${NC}"
run_test "Load Lua functions" "cd scripts/redis-functions && for f in *.lua; do redis-cli FUNCTION LOAD REPLACE \"\$(cat \$f)\" 2>/dev/null; done"

# 5. 单元测试
echo -e "\n${YELLOW}5. Unit Tests${NC}"
export REDIS_URL=redis://localhost:6379
run_test "Libs tests" "cargo test -p voltage-libs --lib -- --test-threads=1"
run_test "Comsrv tests" "cargo test -p comsrv --lib -- --test-threads=1"
run_test "Modsrv tests" "cargo test -p modsrv --lib -- --test-threads=1"
run_test "Alarmsrv tests" "cargo test -p alarmsrv --lib -- --test-threads=1"
run_test "Rulesrv tests" "cargo test -p rulesrv --lib -- --test-threads=1"
run_test "Hissrv tests" "cargo test -p hissrv --lib -- --test-threads=1"

# 6. Docker 构建测试
echo -e "\n${YELLOW}6. Docker Build${NC}"
run_test "Build Redis image" "docker build -t voltageems-redis -f docker/redis/Dockerfile ."
run_test "Build Comsrv image" "docker build -t voltageems-comsrv -f services/comsrv/Dockerfile ."

# 报告
echo ""
echo "========================================"
echo "          TEST SUMMARY"
echo "========================================"
echo -e "Total:  $TOTAL"
echo -e "Passed: ${GREEN}$PASSED${NC}"
echo -e "Failed: ${RED}$FAILED${NC}"

if [ $FAILED -eq 0 ]; then
    echo -e "\n${GREEN}✅ All CI checks passed!${NC}"
    echo "GitHub Actions should run successfully."
    exit 0
else
    echo -e "\n${RED}❌ Some checks failed!${NC}"
    echo "Please fix the issues before pushing."
    exit 1
fi