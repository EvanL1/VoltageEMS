#!/bin/bash
# 运行所有测试的脚本

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "🧪 Running ComsRV Complete Test Suite"
echo "====================================="

# 颜色定义
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[0;33m'
NC='\033[0m' # No Color

# 测试结果统计
TOTAL_TESTS=0
PASSED_TESTS=0
FAILED_TESTS=0
SKIPPED_TESTS=0

# 运行测试并统计结果
run_test() {
    local test_name=$1
    local test_command=$2
    
    echo -e "\n🔍 Running: $test_name"
    echo "Command: $test_command"
    
    TOTAL_TESTS=$((TOTAL_TESTS + 1))
    
    if eval "$test_command"; then
        echo -e "${GREEN}✅ $test_name: PASSED${NC}"
        PASSED_TESTS=$((PASSED_TESTS + 1))
    else
        echo -e "${RED}❌ $test_name: FAILED${NC}"
        FAILED_TESTS=$((FAILED_TESTS + 1))
    fi
}

# 检查依赖
echo "🔧 Checking dependencies..."
if ! command -v cargo &> /dev/null; then
    echo "❌ cargo not found. Please install Rust."
    exit 1
fi

if ! command -v redis-cli &> /dev/null; then
    echo "⚠️  redis-cli not found. Some tests may be skipped."
fi

# 编译项目
echo -e "\n🔨 Building project..."
cd "$PROJECT_DIR"
cargo build --all-features

# 1. 单元测试
echo -e "\n${YELLOW}=== Unit Tests ===${NC}"
run_test "Core Unit Tests" "cargo test --lib"
run_test "Plugin Interface Tests" "cargo test --test plugin_interface_test"
run_test "Plugin Registry Tests" "cargo test --test plugin_registry_test"
run_test "Config Validation Tests" "cargo test --test config_validation_test"

# 2. 集成测试
echo -e "\n${YELLOW}=== Integration Tests ===${NC}"
run_test "Multi-Protocol Tests" "cargo test --test multi_protocol_test"
run_test "Protocol Compatibility Tests" "cargo test --test protocol_compatibility_test"

# 3. 协议特定测试
echo -e "\n${YELLOW}=== Protocol-Specific Tests ===${NC}"
run_test "Modbus Protocol Tests" "cargo test --test modbus_tests"
run_test "IEC60870 Protocol Tests" "cargo test --test iec60870_tests" || SKIPPED_TESTS=$((SKIPPED_TESTS + 1))
run_test "CAN Protocol Tests" "cargo test --test can_tests" || SKIPPED_TESTS=$((SKIPPED_TESTS + 1))

# 4. 性能测试
echo -e "\n${YELLOW}=== Performance Tests ===${NC}"
if [ "$RUN_BENCHMARKS" = "true" ]; then
    run_test "Performance Benchmarks" "cargo bench"
else
    echo "ℹ️  Skipping benchmarks (set RUN_BENCHMARKS=true to run)"
    SKIPPED_TESTS=$((SKIPPED_TESTS + 1))
fi

# 5. E2E测试
echo -e "\n${YELLOW}=== End-to-End Tests ===${NC}"
if redis-cli ping > /dev/null 2>&1; then
    run_test "E2E System Tests" "cargo test --test full_system_test -- --ignored"
else
    echo "⚠️  Redis not running. Skipping E2E tests."
    SKIPPED_TESTS=$((SKIPPED_TESTS + 1))
fi

# 6. 文档测试
echo -e "\n${YELLOW}=== Documentation Tests ===${NC}"
run_test "Doc Tests" "cargo test --doc"

# 7. 代码质量检查
echo -e "\n${YELLOW}=== Code Quality Checks ===${NC}"
run_test "Format Check" "cargo fmt -- --check"
run_test "Clippy Lints" "cargo clippy -- -D warnings"

# 8. 测试覆盖率（如果安装了tarpaulin）
if command -v cargo-tarpaulin &> /dev/null; then
    echo -e "\n${YELLOW}=== Code Coverage ===${NC}"
    run_test "Coverage Report" "cargo tarpaulin --out Html --output-dir coverage"
else
    echo "ℹ️  cargo-tarpaulin not installed. Skipping coverage report."
fi

# 打印测试总结
echo -e "\n${YELLOW}==============================${NC}"
echo -e "${YELLOW}📊 Test Summary${NC}"
echo -e "${YELLOW}==============================${NC}"
echo "Total Tests: $TOTAL_TESTS"
echo -e "${GREEN}Passed: $PASSED_TESTS${NC}"
echo -e "${RED}Failed: $FAILED_TESTS${NC}"
echo -e "${YELLOW}Skipped: $SKIPPED_TESTS${NC}"

if [ $FAILED_TESTS -eq 0 ]; then
    echo -e "\n${GREEN}🎉 All tests passed!${NC}"
    exit 0
else
    echo -e "\n${RED}❌ Some tests failed.${NC}"
    exit 1
fi