#!/bin/bash

# ==================================================
# VoltageEMS 全服务测试脚本
# 测试所有服务的健康状态和基本功能
# ==================================================

set -e

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 配置
REDIS_HOST=${REDIS_HOST:-localhost}
REDIS_PORT=${REDIS_PORT:-6379}
BASE_URL=${BASE_URL:-http://localhost}

# 服务端口配置（硬编码）
COMSRV_PORT=6000
MODSRV_PORT=6001
ALARMSRV_PORT=6002
RULESRV_PORT=6003
HISSRV_PORT=6004
APIGATEWAY_PORT=6005

# 测试计数器
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

log_warning() {
    echo -e "${YELLOW}[!]${NC} $1"
}

# 测试函数
test_redis_connection() {
    log_info "Testing Redis connection..."
    
    if redis-cli -h $REDIS_HOST -p $REDIS_PORT ping > /dev/null 2>&1; then
        log_success "Redis is running at $REDIS_HOST:$REDIS_PORT"
    else
        log_error "Redis is not accessible at $REDIS_HOST:$REDIS_PORT"
        exit 1
    fi
}

test_redis_functions() {
    log_info "Testing Redis Lua functions..."
    
    # 检查函数是否加载
    local functions=$(redis-cli -h $REDIS_HOST -p $REDIS_PORT FUNCTION LIST 2>/dev/null | grep -E "modsrv_engine|alarm_engine|rule_engine" | wc -l)
    
    if [ "$functions" -ge 3 ]; then
        log_success "All required Lua functions are loaded"
    else
        log_warning "Some Lua functions are missing, loading them..."
        cd scripts/redis-functions
        ./load_functions.sh
        cd ../..
        log_success "Lua functions loaded"
    fi
}

test_service_health() {
    local service_name=$1
    local port=$2
    local url="$BASE_URL:$port/health"
    
    log_info "Testing $service_name health at $url..."
    
    if curl -f -s "$url" > /dev/null 2>&1; then
        local response=$(curl -s "$url")
        if echo "$response" | grep -q "healthy"; then
            log_success "$service_name is healthy"
        else
            log_error "$service_name returned unexpected response"
        fi
    else
        log_error "$service_name is not accessible at port $port"
    fi
}

test_modsrv_api() {
    log_info "Testing ModSrv API..."
    local base="$BASE_URL:$MODSRV_PORT"
    
    # 创建测试模板
    local template_response=$(curl -s -X POST "$base/api/templates" \
        -H "Content-Type: application/json" \
        -d '{
            "id": "test_template_001",
            "name": "Test Template",
            "description": "Test template for validation"
        }' 2>/dev/null)
    
    if echo "$template_response" | grep -q "test_template_001"; then
        log_success "ModSrv: Template created successfully"
    else
        log_error "ModSrv: Failed to create template"
    fi
    
    # 列出模板
    local list_response=$(curl -s "$base/api/templates" 2>/dev/null)
    if echo "$list_response" | grep -q "Test Template"; then
        log_success "ModSrv: Templates listed successfully"
    else
        log_error "ModSrv: Failed to list templates"
    fi
}

test_alarmsrv_api() {
    log_info "Testing AlarmSrv API..."
    local base="$BASE_URL:$ALARMSRV_PORT"
    
    # 触发测试告警
    local alarm_response=$(curl -s -X POST "$base/api/alarms" \
        -H "Content-Type: application/json" \
        -d '{
            "id": "test_alarm_001",
            "title": "Test Alarm",
            "level": "Warning",
            "description": "Test alarm for validation"
        }' 2>/dev/null)
    
    if echo "$alarm_response" | grep -q "test_alarm_001"; then
        log_success "AlarmSrv: Alarm triggered successfully"
    else
        log_error "AlarmSrv: Failed to trigger alarm"
    fi
    
    # 获取统计信息
    local stats_response=$(curl -s "$base/api/statistics" 2>/dev/null)
    if echo "$stats_response" | grep -q "total"; then
        log_success "AlarmSrv: Statistics retrieved successfully"
    else
        log_error "AlarmSrv: Failed to get statistics"
    fi
}

test_rulesrv_api() {
    log_info "Testing RuleSrv API..."
    local base="$BASE_URL:$RULESRV_PORT"
    
    # 创建测试规则
    local rule_response=$(curl -s -X POST "$base/api/rules" \
        -H "Content-Type: application/json" \
        -d '{
            "id": "test_rule_001",
            "name": "Test Rule",
            "condition": {
                "source": "test.value",
                "operator": ">",
                "target": "100"
            },
            "action": {
                "type": "set",
                "target": "test.alert",
                "value": "triggered"
            }
        }' 2>/dev/null)
    
    if echo "$rule_response" | grep -q "test_rule_001"; then
        log_success "RuleSrv: Rule created successfully"
    else
        log_error "RuleSrv: Failed to create rule"
    fi
    
    # 获取统计信息
    local stats_response=$(curl -s "$base/api/statistics" 2>/dev/null)
    if echo "$stats_response" | grep -q "total_rules"; then
        log_success "RuleSrv: Statistics retrieved successfully"
    else
        log_error "RuleSrv: Failed to get statistics"
    fi
}

test_hissrv_api() {
    log_info "Testing HisSrv API..."
    local base="$BASE_URL:$HISSRV_PORT"
    
    # 测试健康检查
    local health_response=$(curl -s "$base/health" 2>/dev/null)
    if echo "$health_response" | grep -q "healthy"; then
        log_success "HisSrv: Service is healthy"
    else
        log_error "HisSrv: Health check failed"
    fi
}

test_comsrv_api() {
    log_info "Testing ComSrv API..."
    local base="$BASE_URL:$COMSRV_PORT"
    
    # 获取通道状态
    local channels_response=$(curl -s "$base/api/channels" 2>/dev/null)
    if [ -n "$channels_response" ]; then
        log_success "ComSrv: Channels retrieved successfully"
    else
        log_warning "ComSrv: No channels configured"
    fi
    
    # 测试指标端点
    local metrics_response=$(curl -s "$base/metrics" 2>/dev/null)
    if echo "$metrics_response" | grep -q "comsrv"; then
        log_success "ComSrv: Metrics endpoint working"
    else
        log_warning "ComSrv: Metrics endpoint not available"
    fi
}

test_data_flow() {
    log_info "Testing data flow between services..."
    
    # 在Redis中设置测试数据
    redis-cli -h $REDIS_HOST -p $REDIS_PORT SET "test:flow:value" "123.456" > /dev/null
    
    # 验证数据可以被读取
    local value=$(redis-cli -h $REDIS_HOST -p $REDIS_PORT GET "test:flow:value")
    if [ "$value" = "123.456" ]; then
        log_success "Data flow: Redis read/write working"
    else
        log_error "Data flow: Redis read/write failed"
    fi
    
    # 清理测试数据
    redis-cli -h $REDIS_HOST -p $REDIS_PORT DEL "test:flow:value" > /dev/null
}

cleanup_test_data() {
    log_info "Cleaning up test data..."
    
    # 清理测试数据
    redis-cli -h $REDIS_HOST -p $REDIS_PORT --scan --pattern "test_*" | xargs -r redis-cli -h $REDIS_HOST -p $REDIS_PORT DEL 2>/dev/null || true
    redis-cli -h $REDIS_HOST -p $REDIS_PORT --scan --pattern "test:*" | xargs -r redis-cli -h $REDIS_HOST -p $REDIS_PORT DEL 2>/dev/null || true
    
    log_success "Test data cleaned up"
}

print_summary() {
    echo ""
    echo "======================================"
    echo "         TEST SUMMARY"
    echo "======================================"
    echo -e "Total Tests:  ${TOTAL_TESTS}"
    echo -e "Passed:       ${GREEN}${PASSED_TESTS}${NC}"
    echo -e "Failed:       ${RED}${FAILED_TESTS}${NC}"
    
    if [ $FAILED_TESTS -eq 0 ]; then
        echo -e "\n${GREEN}All tests passed successfully!${NC}"
        return 0
    else
        echo -e "\n${RED}Some tests failed. Please check the logs above.${NC}"
        return 1
    fi
}

# 主测试流程
main() {
    echo "======================================"
    echo "    VoltageEMS Service Test Suite"
    echo "======================================"
    echo ""
    
    # 基础设施测试
    test_redis_connection
    test_redis_functions
    
    # 服务健康检查
    test_service_health "ComSrv" $COMSRV_PORT
    test_service_health "ModSrv" $MODSRV_PORT
    test_service_health "AlarmSrv" $ALARMSRV_PORT
    test_service_health "RuleSrv" $RULESRV_PORT
    test_service_health "HisSrv" $HISSRV_PORT
    test_service_health "APIGateway" $APIGATEWAY_PORT
    
    # API功能测试
    test_comsrv_api
    test_modsrv_api
    test_alarmsrv_api
    test_rulesrv_api
    test_hissrv_api
    
    # 数据流测试
    test_data_flow
    
    # 清理
    cleanup_test_data
    
    # 打印总结
    print_summary
}

# 运行测试
main "$@"