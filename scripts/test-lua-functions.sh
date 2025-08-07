#!/bin/bash

# ==================================================
# VoltageEMS Lua函数测试脚本
# 测试所有Redis Lua函数的功能
# ==================================================

set -e

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Redis配置
REDIS_HOST=${REDIS_HOST:-localhost}
REDIS_PORT=${REDIS_PORT:-6379}
REDIS_CLI="redis-cli -h $REDIS_HOST -p $REDIS_PORT"

# 测试计数
TOTAL_TESTS=0
PASSED_TESTS=0
FAILED_TESTS=0

# 日志函数
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[✓]${NC} $1"
    ((PASSED_TESTS++))
    ((TOTAL_TESTS++))
}

log_error() {
    echo -e "${RED}[✗]${NC} $1"
    ((FAILED_TESTS++))
    ((TOTAL_TESTS++))
}

# 加载Lua函数
load_functions() {
    log_info "Loading Lua functions..."
    
    cd scripts/redis-functions
    if ./load_functions.sh > /dev/null 2>&1; then
        log_success "Lua functions loaded successfully"
    else
        log_error "Failed to load Lua functions"
        exit 1
    fi
    cd ../..
}

# 验证函数加载
verify_functions() {
    log_info "Verifying loaded functions..."
    
    local engines=("modsrv_engine" "alarm_engine" "rule_engine")
    
    for engine in "${engines[@]}"; do
        if $REDIS_CLI FUNCTION LIST | grep -q "$engine"; then
            log_success "$engine loaded"
        else
            log_error "$engine not found"
        fi
    done
}

# 测试ModSrv函数
test_modsrv_functions() {
    log_info "Testing ModSrv functions..."
    
    # 测试创建模板
    local result=$($REDIS_CLI FCALL modsrv_upsert_template 1 "test_template_lua" \
        '{"name":"Test Template","description":"Lua test template"}' 2>&1)
    
    if echo "$result" | grep -q "OK"; then
        log_success "modsrv_upsert_template: Template created"
    else
        log_error "modsrv_upsert_template: Failed - $result"
    fi
    
    # 测试获取模板
    local template=$($REDIS_CLI FCALL modsrv_get_template 1 "test_template_lua" 2>&1)
    if echo "$template" | grep -q "Test Template"; then
        log_success "modsrv_get_template: Template retrieved"
    else
        log_error "modsrv_get_template: Failed to retrieve"
    fi
    
    # 测试列出模板
    local templates=$($REDIS_CLI FCALL modsrv_list_templates 0 2>&1)
    if echo "$templates" | grep -q "test_template_lua"; then
        log_success "modsrv_list_templates: Templates listed"
    else
        log_error "modsrv_list_templates: Failed to list"
    fi
    
    # 测试创建模型
    local model_result=$($REDIS_CLI FCALL modsrv_upsert_model 1 "test_model_lua" \
        '{"name":"Test Model","template_id":"test_template_lua","channel_id":1001}' 2>&1)
    
    if echo "$model_result" | grep -q "OK"; then
        log_success "modsrv_upsert_model: Model created"
    else
        log_error "modsrv_upsert_model: Failed - $model_result"
    fi
    
    # 测试获取模型数据
    local model_data=$($REDIS_CLI FCALL modsrv_get_model_data 1 "test_model_lua" 2>&1)
    if echo "$model_data" | grep -q "model_id"; then
        log_success "modsrv_get_model_data: Data retrieved"
    else
        log_error "modsrv_get_model_data: Failed to retrieve data"
    fi
    
    # 清理测试数据
    $REDIS_CLI FCALL modsrv_delete_model 1 "test_model_lua" > /dev/null 2>&1
    $REDIS_CLI FCALL modsrv_delete_template 1 "test_template_lua" > /dev/null 2>&1
}

# 测试AlarmSrv函数
test_alarmsrv_functions() {
    log_info "Testing AlarmSrv functions..."
    
    # 测试触发告警
    local alarm_result=$($REDIS_CLI FCALL alarmsrv_trigger_alarm 1 "test_alarm_lua" \
        '{"title":"Test Alarm","level":"Warning","description":"Lua test alarm"}' 2>&1)
    
    if echo "$alarm_result" | grep -q "OK"; then
        log_success "alarmsrv_trigger_alarm: Alarm triggered"
    else
        log_error "alarmsrv_trigger_alarm: Failed - $alarm_result"
    fi
    
    # 测试获取告警
    local alarm=$($REDIS_CLI FCALL alarmsrv_get_alarm 1 "test_alarm_lua" 2>&1)
    if echo "$alarm" | grep -q "Test Alarm"; then
        log_success "alarmsrv_get_alarm: Alarm retrieved"
    else
        log_error "alarmsrv_get_alarm: Failed to retrieve"
    fi
    
    # 测试确认告警
    local ack_result=$($REDIS_CLI FCALL alarmsrv_acknowledge_alarm 1 "test_alarm_lua" \
        '{"user":"test_user","note":"Test acknowledgment"}' 2>&1)
    
    if echo "$ack_result" | grep -q "OK"; then
        log_success "alarmsrv_acknowledge_alarm: Alarm acknowledged"
    else
        log_error "alarmsrv_acknowledge_alarm: Failed - $ack_result"
    fi
    
    # 测试列出告警
    local alarms=$($REDIS_CLI FCALL alarmsrv_list_alarms 0 "query" \
        '{"status":"Acknowledged","limit":10}' 2>&1)
    
    if echo "$alarms" | grep -q "\["; then
        log_success "alarmsrv_list_alarms: Alarms listed"
    else
        log_error "alarmsrv_list_alarms: Failed to list"
    fi
    
    # 测试统计信息
    local stats=$($REDIS_CLI FCALL alarmsrv_get_statistics 0 2>&1)
    if echo "$stats" | grep -q "total"; then
        log_success "alarmsrv_get_statistics: Statistics retrieved"
    else
        log_error "alarmsrv_get_statistics: Failed to get statistics"
    fi
    
    # 清理测试数据
    $REDIS_CLI FCALL alarmsrv_clear_alarm 1 "test_alarm_lua" > /dev/null 2>&1
}

# 测试RuleSrv函数
test_rulesrv_functions() {
    log_info "Testing RuleSrv functions..."
    
    # 测试创建规则
    local rule_result=$($REDIS_CLI FCALL rulesrv_upsert_rule 1 "test_rule_lua" \
        '{"name":"Test Rule","condition":{"source":"test.value","operator":">","target":"100"},"action":{"type":"set","target":"test.alert","value":"1"}}' 2>&1)
    
    if echo "$rule_result" | grep -q "OK"; then
        log_success "rulesrv_upsert_rule: Rule created"
    else
        log_error "rulesrv_upsert_rule: Failed - $rule_result"
    fi
    
    # 测试获取规则
    local rule=$($REDIS_CLI FCALL rulesrv_get_rule 1 "test_rule_lua" 2>&1)
    if echo "$rule" | grep -q "Test Rule"; then
        log_success "rulesrv_get_rule: Rule retrieved"
    else
        log_error "rulesrv_get_rule: Failed to retrieve"
    fi
    
    # 测试启用规则
    local enable_result=$($REDIS_CLI FCALL rulesrv_enable_rule 1 "test_rule_lua" 2>&1)
    if echo "$enable_result" | grep -q "OK"; then
        log_success "rulesrv_enable_rule: Rule enabled"
    else
        log_error "rulesrv_enable_rule: Failed - $enable_result"
    fi
    
    # 测试执行批量规则
    # 先设置测试数据
    $REDIS_CLI SET "test.value" "150" > /dev/null
    
    local exec_result=$($REDIS_CLI FCALL rulesrv_execute_batch 1 "test_batch_001" "10" 2>&1)
    if echo "$exec_result" | grep -q "rules_executed"; then
        log_success "rulesrv_execute_batch: Batch executed"
    else
        log_error "rulesrv_execute_batch: Failed - $exec_result"
    fi
    
    # 测试列出执行历史
    local executions=$($REDIS_CLI FCALL rulesrv_list_executions 1 "5" 2>&1)
    if echo "$executions" | grep -q "\["; then
        log_success "rulesrv_list_executions: Executions listed"
    else
        log_error "rulesrv_list_executions: Failed to list"
    fi
    
    # 测试统计信息
    local stats=$($REDIS_CLI FCALL rulesrv_get_statistics 0 2>&1)
    if echo "$stats" | grep -q "total_rules"; then
        log_success "rulesrv_get_statistics: Statistics retrieved"
    else
        log_error "rulesrv_get_statistics: Failed to get statistics"
    fi
    
    # 清理测试数据
    $REDIS_CLI FCALL rulesrv_delete_rule 1 "test_rule_lua" > /dev/null 2>&1
    $REDIS_CLI DEL "test.value" "test.alert" > /dev/null 2>&1
}

# 测试错误处理
test_error_handling() {
    log_info "Testing error handling..."
    
    # 测试无效JSON
    local error_result=$($REDIS_CLI FCALL modsrv_upsert_template 1 "error_test" \
        '{invalid json}' 2>&1)
    
    if echo "$error_result" | grep -q -i "invalid"; then
        log_success "Error handling: Invalid JSON detected"
    else
        log_error "Error handling: Failed to detect invalid JSON"
    fi
    
    # 测试缺少参数
    local missing_result=$($REDIS_CLI FCALL rulesrv_upsert_rule 1 2>&1)
    if echo "$missing_result" | grep -q -i "required"; then
        log_success "Error handling: Missing parameters detected"
    else
        log_error "Error handling: Failed to detect missing parameters"
    fi
    
    # 测试不存在的资源
    local notfound_result=$($REDIS_CLI FCALL modsrv_get_model 1 "nonexistent_model" 2>&1)
    if echo "$notfound_result" | grep -q -i "not found"; then
        log_success "Error handling: Not found error handled"
    else
        log_error "Error handling: Failed to handle not found"
    fi
}

# 清理所有测试数据
cleanup_all() {
    log_info "Cleaning up all test data..."
    
    # 清理测试键
    $REDIS_CLI --scan --pattern "test*" | xargs -r $REDIS_CLI DEL 2>/dev/null || true
    $REDIS_CLI --scan --pattern "*test_*_lua*" | xargs -r $REDIS_CLI DEL 2>/dev/null || true
    
    # 清理测试集合
    $REDIS_CLI SREM modsrv:templates test_template_lua 2>/dev/null || true
    $REDIS_CLI SREM modsrv:models test_model_lua 2>/dev/null || true
    $REDIS_CLI SREM alarmsrv:alarms test_alarm_lua 2>/dev/null || true
    $REDIS_CLI SREM rulesrv:rules test_rule_lua 2>/dev/null || true
    
    log_success "Test data cleaned up"
}

# 打印测试报告
print_report() {
    echo ""
    echo "======================================"
    echo "       LUA FUNCTION TEST REPORT"
    echo "======================================"
    echo -e "Total Tests:  ${TOTAL_TESTS}"
    echo -e "Passed:       ${GREEN}${PASSED_TESTS}${NC}"
    echo -e "Failed:       ${RED}${FAILED_TESTS}${NC}"
    
    if [ $FAILED_TESTS -eq 0 ]; then
        echo -e "\n${GREEN}All Lua function tests passed!${NC}"
        return 0
    else
        echo -e "\n${RED}Some tests failed. Please check the functions.${NC}"
        return 1
    fi
}

# 主测试流程
main() {
    echo "======================================"
    echo "   VoltageEMS Lua Function Test Suite"
    echo "======================================"
    echo ""
    
    # 加载和验证函数
    load_functions
    verify_functions
    
    # 运行测试
    test_modsrv_functions
    test_alarmsrv_functions
    test_rulesrv_functions
    test_error_handling
    
    # 清理
    cleanup_all
    
    # 打印报告
    print_report
}

# 运行测试
main "$@"