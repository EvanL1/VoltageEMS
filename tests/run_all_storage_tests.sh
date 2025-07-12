#!/bin/bash
# run_all_storage_tests.sh - 运行所有存储相关测试

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
REPORT_DIR="$PROJECT_ROOT/test-reports"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
REPORT_FILE="$REPORT_DIR/storage_test_report_$TIMESTAMP.txt"

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 创建报告目录
mkdir -p "$REPORT_DIR"

# 开始测试报告
echo "=== VoltageEMS 存储系统测试报告 ===" | tee "$REPORT_FILE"
echo "测试时间: $(date)" | tee -a "$REPORT_FILE"
echo "测试环境: $(uname -a)" | tee -a "$REPORT_FILE"
echo "" | tee -a "$REPORT_FILE"

# 检查Redis状态
echo -e "${BLUE}检查Redis状态...${NC}" | tee -a "$REPORT_FILE"
if redis-cli ping >/dev/null 2>&1; then
    echo -e "${GREEN}✓ Redis正在运行${NC}" | tee -a "$REPORT_FILE"
    REDIS_RUNNING=true
else
    echo -e "${YELLOW}⚠ Redis未运行，启动Docker容器...${NC}" | tee -a "$REPORT_FILE"
    docker run -d --name redis-test -p 6379:6379 redis:7-alpine >/dev/null 2>&1 || true
    sleep 2
    REDIS_RUNNING=false
fi
echo "" | tee -a "$REPORT_FILE"

# 进入项目目录
cd "$PROJECT_ROOT/services/comsrv"

# 测试套件列表
declare -a TEST_SUITES=(
    "test_flat_storage:扁平化存储基础测试"
    "plugin_storage_integration_test:插件存储集成测试"
    "concurrent_storage_test:并发存储测试"
    "performance_test:性能测试"
    "recovery_test:错误恢复测试"
)

# 运行每个测试套件
TOTAL_TESTS=${#TEST_SUITES[@]}
PASSED_TESTS=0
FAILED_TESTS=0

for suite in "${TEST_SUITES[@]}"; do
    IFS=':' read -r test_name test_desc <<< "$suite"
    
    echo -e "${BLUE}运行测试: $test_desc${NC}" | tee -a "$REPORT_FILE"
    echo "命令: cargo test --test $test_name -- --nocapture" | tee -a "$REPORT_FILE"
    
    # 运行测试并捕获输出
    TEST_OUTPUT_FILE="$REPORT_DIR/${test_name}_output_$TIMESTAMP.log"
    
    if cargo test --test "$test_name" -- --nocapture > "$TEST_OUTPUT_FILE" 2>&1; then
        echo -e "${GREEN}✓ $test_desc 通过${NC}" | tee -a "$REPORT_FILE"
        PASSED_TESTS=$((PASSED_TESTS + 1))
        
        # 提取关键性能指标
        if [[ "$test_name" == "performance_test" ]]; then
            echo "  性能指标:" | tee -a "$REPORT_FILE"
            grep -E "(批次大小|updates/sec|点/秒)" "$TEST_OUTPUT_FILE" | tail -10 | sed 's/^/    /' | tee -a "$REPORT_FILE"
        elif [[ "$test_name" == "concurrent_storage_test" ]]; then
            echo "  并发性能:" | tee -a "$REPORT_FILE"
            grep -E "(更新速率|耗时)" "$TEST_OUTPUT_FILE" | tail -5 | sed 's/^/    /' | tee -a "$REPORT_FILE"
        fi
    else
        echo -e "${RED}✗ $test_desc 失败${NC}" | tee -a "$REPORT_FILE"
        FAILED_TESTS=$((FAILED_TESTS + 1))
        echo "  错误详情见: $TEST_OUTPUT_FILE" | tee -a "$REPORT_FILE"
        
        # 显示最后几行错误信息
        echo "  错误摘要:" | tee -a "$REPORT_FILE"
        tail -10 "$TEST_OUTPUT_FILE" | sed 's/^/    /' | tee -a "$REPORT_FILE"
    fi
    
    echo "" | tee -a "$REPORT_FILE"
done

# 运行单元测试
echo -e "${BLUE}运行单元测试...${NC}" | tee -a "$REPORT_FILE"
UNIT_TEST_OUTPUT="$REPORT_DIR/unit_tests_output_$TIMESTAMP.log"

if cargo test --lib -- --nocapture > "$UNIT_TEST_OUTPUT" 2>&1; then
    echo -e "${GREEN}✓ 单元测试通过${NC}" | tee -a "$REPORT_FILE"
    # 提取测试统计
    grep -E "test result:|passed|failed" "$UNIT_TEST_OUTPUT" | tail -5 | tee -a "$REPORT_FILE"
else
    echo -e "${RED}✗ 单元测试失败${NC}" | tee -a "$REPORT_FILE"
    echo "  详情见: $UNIT_TEST_OUTPUT" | tee -a "$REPORT_FILE"
fi
echo "" | tee -a "$REPORT_FILE"

# 测试总结
echo "=== 测试总结 ===" | tee -a "$REPORT_FILE"
echo "总测试套件数: $TOTAL_TESTS" | tee -a "$REPORT_FILE"
echo -e "${GREEN}通过: $PASSED_TESTS${NC}" | tee -a "$REPORT_FILE"
echo -e "${RED}失败: $FAILED_TESTS${NC}" | tee -a "$REPORT_FILE"
echo "" | tee -a "$REPORT_FILE"

# Redis数据样例
if redis-cli ping >/dev/null 2>&1; then
    echo "=== Redis数据样例 ===" | tee -a "$REPORT_FILE"
    echo "键模式分布:" | tee -a "$REPORT_FILE"
    redis-cli --scan --pattern "*:*:*" | head -20 | tee -a "$REPORT_FILE"
    echo "" | tee -a "$REPORT_FILE"
    
    echo "数据样例:" | tee -a "$REPORT_FILE"
    SAMPLE_KEY=$(redis-cli --scan --pattern "*:m:*" | head -1)
    if [ -n "$SAMPLE_KEY" ]; then
        echo "键: $SAMPLE_KEY" | tee -a "$REPORT_FILE"
        echo "值: $(redis-cli get "$SAMPLE_KEY")" | tee -a "$REPORT_FILE"
    fi
    echo "" | tee -a "$REPORT_FILE"
fi

# 清理
if [ "$REDIS_RUNNING" = false ]; then
    echo -e "${BLUE}清理Redis容器...${NC}" | tee -a "$REPORT_FILE"
    docker stop redis-test >/dev/null 2>&1 || true
    docker rm redis-test >/dev/null 2>&1 || true
fi

# 最终状态
echo "" | tee -a "$REPORT_FILE"
if [ $FAILED_TESTS -eq 0 ]; then
    echo -e "${GREEN}=== 所有测试通过！===${NC}" | tee -a "$REPORT_FILE"
    EXIT_CODE=0
else
    echo -e "${RED}=== 有 $FAILED_TESTS 个测试失败 ===${NC}" | tee -a "$REPORT_FILE"
    EXIT_CODE=1
fi

echo "" | tee -a "$REPORT_FILE"
echo "详细报告已保存至: $REPORT_FILE" | tee -a "$REPORT_FILE"
echo "测试日志目录: $REPORT_DIR" | tee -a "$REPORT_FILE"

exit $EXIT_CODE