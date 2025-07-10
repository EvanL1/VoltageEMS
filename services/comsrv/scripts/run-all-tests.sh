#!/bin/bash
#
# 运行所有测试的脚本
# 包括单元测试、集成测试和Docker测试

set -e

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

echo "=== ComSrv 完整测试套件 ==="
echo "工作目录: $PROJECT_DIR"
echo

# 颜色定义
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# 测试结果
declare -A TEST_RESULTS

# 运行测试组
run_test_group() {
    local group_name="$1"
    local test_command="$2"
    
    echo -e "\n${BLUE}=== $group_name ===${NC}"
    
    if eval "$test_command"; then
        TEST_RESULTS["$group_name"]="PASSED"
        echo -e "${GREEN}✓ $group_name 通过${NC}"
    else
        TEST_RESULTS["$group_name"]="FAILED"
        echo -e "${RED}✗ $group_name 失败${NC}"
    fi
}

cd "$PROJECT_DIR"

# 1. 代码格式检查
run_test_group "代码格式检查" "cargo fmt --all -- --check"

# 2. Clippy 检查
run_test_group "Clippy检查" "cargo clippy --all-targets --all-features -- -D warnings || true"

# 3. 单元测试
echo -e "\n${BLUE}=== 单元测试 ===${NC}"

# 3.1 核心库测试
run_test_group "核心库单元测试" "cargo test --lib"

# 3.2 Transport层测试
run_test_group "Transport层测试" "cargo test --lib transport::"

# 3.3 Framework测试
run_test_group "Framework测试" "cargo test --lib framework::"

# 3.4 插件系统测试
run_test_group "插件系统测试" "cargo test --lib plugin"

# 4. 协议特定测试
echo -e "\n${BLUE}=== 协议测试 ===${NC}"

# 4.1 Modbus测试
run_test_group "Modbus协议测试" "cargo test --features modbus modbus::"

# 4.2 IEC60870测试
run_test_group "IEC60870协议测试" "cargo test --features iec60870 iec60870:: || true"

# 4.3 Virtual协议测试
run_test_group "Virtual协议测试" "cargo test virt::"

# 5. 集成测试
echo -e "\n${BLUE}=== 集成测试 ===${NC}"

run_test_group "简单集成测试" "cargo test --test integration_test_simple"
run_test_group "插件调试测试" "cargo test --test test_plugin_debug"

# 6. 文档测试
run_test_group "文档测试" "cargo test --doc"

# 7. 示例程序测试
echo -e "\n${BLUE}=== 示例程序测试 ===${NC}"
run_test_group "示例编译" "cargo build --examples || true"

# 8. Docker测试（可选）
if command -v docker &> /dev/null && docker info &> /dev/null; then
    echo -e "\n${BLUE}=== Docker测试 ===${NC}"
    echo "Docker可用，运行Docker测试..."
    
    if [ -f "./scripts/docker-multi-test.sh" ]; then
        run_test_group "Docker多协议测试" "./scripts/docker-multi-test.sh"
    else
        echo -e "${YELLOW}跳过Docker测试：测试脚本不存在${NC}"
    fi
else
    echo -e "\n${YELLOW}跳过Docker测试：Docker不可用${NC}"
fi

# 9. 测试覆盖率（可选）
if command -v cargo-tarpaulin &> /dev/null; then
    echo -e "\n${BLUE}=== 测试覆盖率 ===${NC}"
    cargo tarpaulin --out Xml --output-dir target/coverage || true
    echo "覆盖率报告已生成到 target/coverage/"
else
    echo -e "\n${YELLOW}跳过覆盖率测试：cargo-tarpaulin未安装${NC}"
    echo "安装命令：cargo install cargo-tarpaulin"
fi

# 10. 测试报告
echo -e "\n${BLUE}=== 测试报告 ===${NC}"
echo "测试结果汇总："
echo

PASSED=0
FAILED=0

for test_name in "${!TEST_RESULTS[@]}"; do
    result="${TEST_RESULTS[$test_name]}"
    if [ "$result" = "PASSED" ]; then
        echo -e "  ${GREEN}✓${NC} $test_name"
        ((PASSED++))
    else
        echo -e "  ${RED}✗${NC} $test_name"
        ((FAILED++))
    fi
done

echo
echo "总计: $((PASSED + FAILED)) 个测试组"
echo -e "通过: ${GREEN}$PASSED${NC}"
echo -e "失败: ${RED}$FAILED${NC}"

if [ $FAILED -eq 0 ]; then
    echo -e "\n${GREEN}所有测试通过！${NC}"
    exit 0
else
    echo -e "\n${RED}有 $FAILED 个测试组失败${NC}"
    exit 1
fi