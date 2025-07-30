#!/bin/bash

# VoltageEMS 映射初始化脚本
# 用于初始化所有服务间的数据映射和 Lua 脚本

REDIS_CLI="${REDIS_CLI:-redis-cli}"
REDIS_HOST="${REDIS_HOST:-localhost}"
REDIS_PORT="${REDIS_PORT:-6379}"

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

echo -e "${GREEN}=== VoltageEMS 映射初始化工具 ===${NC}"
echo -e "Redis: ${REDIS_HOST}:${REDIS_PORT}"
echo ""

# Redis 连接命令
REDIS_CMD="$REDIS_CLI -h $REDIS_HOST -p $REDIS_PORT"

# 脚本路径
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
LUA_SCRIPTS_DIR="$PROJECT_ROOT/scripts"

# 检查 Redis 连接
echo -n "检查 Redis 连接... "
if $REDIS_CMD ping > /dev/null 2>&1; then
    echo -e "${GREEN}OK${NC}"
else
    echo -e "${RED}失败${NC}"
    echo "无法连接到 Redis，请检查配置"
    exit 1
fi

# 清理旧的映射数据和脚本
if [ "$1" == "--clean" ]; then
    echo -e "${YELLOW}清理旧的映射数据和脚本...${NC}"
    $REDIS_CMD --scan --pattern "mapping:*" | xargs -r $REDIS_CMD DEL
    $REDIS_CMD --scan --pattern "script:*" | xargs -r $REDIS_CMD DEL
    $REDIS_CMD --scan --pattern "rule:*" | xargs -r $REDIS_CMD DEL
    $REDIS_CMD SCRIPT FLUSH > /dev/null 2>&1
fi

# 加载 Lua 脚本
echo -e "\n${BLUE}=== 加载 Lua 脚本 ===${NC}"

# 1. 加载统一同步脚本
echo -n "加载 unified_sync.lua... "
if [ -f "$LUA_SCRIPTS_DIR/unified_sync.lua" ]; then
    UNIFIED_SYNC_SHA=$($REDIS_CMD SCRIPT LOAD "$(cat $LUA_SCRIPTS_DIR/unified_sync.lua)")
    if [ -n "$UNIFIED_SYNC_SHA" ]; then
        $REDIS_CMD SET "script:unified_sync" "$UNIFIED_SYNC_SHA" > /dev/null
        echo -e "${GREEN}OK${NC} (SHA: ${UNIFIED_SYNC_SHA:0:8}...)"
    else
        echo -e "${RED}失败${NC}"
    fi
else
    echo -e "${RED}文件不存在${NC}"
fi

# 2. 加载规则引擎脚本
echo -n "加载 rule_engine.lua... "
if [ -f "$LUA_SCRIPTS_DIR/rule_engine.lua" ]; then
    RULE_ENGINE_SHA=$($REDIS_CMD SCRIPT LOAD "$(cat $LUA_SCRIPTS_DIR/rule_engine.lua)")
    if [ -n "$RULE_ENGINE_SHA" ]; then
        $REDIS_CMD SET "script:rule_engine" "$RULE_ENGINE_SHA" > /dev/null
        echo -e "${GREEN}OK${NC} (SHA: ${RULE_ENGINE_SHA:0:8}...)"
    else
        echo -e "${RED}失败${NC}"
    fi
else
    echo -e "${RED}文件不存在${NC}"
fi

# 3. 加载数据处理脚本
echo -n "加载 data_processor.lua... "
if [ -f "$LUA_SCRIPTS_DIR/data_processor.lua" ]; then
    DATA_PROCESSOR_SHA=$($REDIS_CMD SCRIPT LOAD "$(cat $LUA_SCRIPTS_DIR/data_processor.lua)")
    if [ -n "$DATA_PROCESSOR_SHA" ]; then
        $REDIS_CMD SET "script:data_processor" "$DATA_PROCESSOR_SHA" > /dev/null
        echo -e "${GREEN}OK${NC} (SHA: ${DATA_PROCESSOR_SHA:0:8}...)"
    else
        echo -e "${RED}失败${NC}"
    fi
else
    echo -e "${RED}文件不存在${NC}"
fi

# 4. 加载 sync.lua 脚本
if [ -f "$LUA_SCRIPTS_DIR/sync.lua" ]; then
    echo -n "加载 sync.lua... "
    SYNC_SHA=$($REDIS_CMD SCRIPT LOAD "$(cat $LUA_SCRIPTS_DIR/sync.lua)")
    if [ -n "$SYNC_SHA" ]; then
        $REDIS_CMD SET "script:sync" "$SYNC_SHA" > /dev/null
        echo -e "${GREEN}OK${NC} (SHA: ${SYNC_SHA:0:8}...)"
    else
        echo -e "${RED}失败${NC}"
    fi
else
    echo -e "${YELLOW}警告：sync.lua 文件不存在${NC}"
fi

# 初始化服务间映射
echo -e "\n${GREEN}=== 初始化服务间映射 ===${NC}"

# 1. ComsRv -> ModSrv 映射
echo -e "\n${GREEN}1. 初始化 ComsRv -> ModSrv 映射${NC}"

# 通道 1001 - 电表数据
echo "  配置通道 1001 (电表)..."
$REDIS_CMD SET "mapping:comsrv:1001:m:1" "modsrv:power_meter:voltage_a" > /dev/null
$REDIS_CMD SET "mapping:comsrv:1001:m:2" "modsrv:power_meter:current_a" > /dev/null
$REDIS_CMD SET "mapping:comsrv:1001:m:3" "modsrv:power_meter:power" > /dev/null
$REDIS_CMD SET "mapping:comsrv:1001:m:4" "modsrv:power_meter:power_factor" > /dev/null
$REDIS_CMD SET "mapping:comsrv:1001:m:5" "modsrv:power_meter:frequency" > /dev/null

# 信号映射
$REDIS_CMD SET "mapping:comsrv:1001:s:1" "modsrv:power_meter:breaker_status" > /dev/null
$REDIS_CMD SET "mapping:comsrv:1001:s:2" "modsrv:power_meter:fault_alarm" > /dev/null

# 通道 1002 - 变压器数据
echo "  配置通道 1002 (变压器)..."
$REDIS_CMD SET "mapping:comsrv:1002:m:1" "modsrv:transformer:voltage_primary" > /dev/null
$REDIS_CMD SET "mapping:comsrv:1002:m:2" "modsrv:transformer:voltage_secondary" > /dev/null
$REDIS_CMD SET "mapping:comsrv:1002:m:3" "modsrv:transformer:current_primary" > /dev/null
$REDIS_CMD SET "mapping:comsrv:1002:m:4" "modsrv:transformer:temperature" > /dev/null

# 初始化反向映射 (ModSrv -> ComsRv)
echo -e "\n${GREEN}2. 初始化反向映射 (ModSrv -> ComsRv)${NC}"

# 电表控制
echo "  配置电表控制映射..."
$REDIS_CMD SET "mapping:reverse:power_meter:breaker_control" "1001:c:1" > /dev/null
$REDIS_CMD SET "mapping:reverse:power_meter:reset_alarm" "1001:c:2" > /dev/null

# 电表调节
$REDIS_CMD SET "mapping:reverse:power_meter:voltage_setpoint" "1001:a:1" > /dev/null
$REDIS_CMD SET "mapping:reverse:power_meter:power_limit" "1001:a:2" > /dev/null

# 变压器控制
echo "  配置变压器控制映射..."
$REDIS_CMD SET "mapping:reverse:transformer:tap_up" "1002:c:1" > /dev/null
$REDIS_CMD SET "mapping:reverse:transformer:tap_down" "1002:c:2" > /dev/null

# 告警阈值配置
echo -e "\n${GREEN}3. 配置告警阈值${NC}"
$REDIS_CMD SET "alarm:threshold:power_meter:voltage_a" "250" > /dev/null
$REDIS_CMD SET "alarm:threshold:power_meter:current_a" "100" > /dev/null
$REDIS_CMD SET "alarm:threshold:transformer:temperature" "80" > /dev/null

# 云端映射配置（NetSrv）
echo -e "\n${GREEN}4. 配置云端映射 (NetSrv)${NC}"
echo "  配置电表数据上云映射..."
$REDIS_CMD SET "mapping:cloud:power_meter:voltage" "modsrv:power_meter:voltage_a" > /dev/null
$REDIS_CMD SET "mapping:cloud:power_meter:current" "modsrv:power_meter:current_a" > /dev/null
$REDIS_CMD SET "mapping:cloud:power_meter:power" "modsrv:power_meter:power" > /dev/null

# 服务间路由配置
echo -e "\n${GREEN}5. 配置服务间路由${NC}"
echo "  配置 ModSrv -> AlarmSrv 路由..."
$REDIS_CMD SET "mapping:route:modsrv:power_meter:alarmsrv:power_alarm:voltage" "voltage_a:voltage" > /dev/null
$REDIS_CMD SET "mapping:route:modsrv:power_meter:alarmsrv:power_alarm:current" "current_a:current" > /dev/null

# 规则引擎配置
echo -e "\n${GREEN}6. 配置示例规则${NC}"

# 简单规则：电压过高告警
echo "  配置电压过高告警规则..."
$REDIS_CMD HSET "rule:voltage_high" "type" "simple" > /dev/null
$REDIS_CMD HSET "rule:voltage_high" "enabled" "true" > /dev/null
$REDIS_CMD HSET "rule:voltage_high" "condition" "voltage_a > 250" > /dev/null
$REDIS_CMD HSET "rule:voltage_high" "action_type" "alarm" > /dev/null
$REDIS_CMD HSET "rule:voltage_high" "action_target" "power_meter" > /dev/null
$REDIS_CMD HSET "rule:voltage_high" "action_value" "电压过高告警" > /dev/null

# 规则输入配置
$REDIS_CMD SET "rule:voltage_high:input:voltage_a" "modsrv:power_meter:voltage_a" > /dev/null

# DAG 规则示例：功率限制控制
echo "  配置功率限制控制规则..."
$REDIS_CMD HSET "rule:power_limit" "type" "dag" > /dev/null
$REDIS_CMD HSET "rule:power_limit" "enabled" "true" > /dev/null

# DAG 节点配置
$REDIS_CMD HSET "rule:power_limit:dag" "condition_1" '{"type":"condition","expression":"power > 100","required":true}' > /dev/null
$REDIS_CMD HSET "rule:power_limit:dag" "calc_1" '{"type":"calculation","operation":"average","inputs":["voltage_a","current_a"]}' > /dev/null
$REDIS_CMD HSET "rule:power_limit:dag" "action_1" '{"type":"action","action_type":"control","model_id":"power_meter","control_name":"power_limit","value":"80","dependencies":["condition_1"]}' > /dev/null

# 规则输入
$REDIS_CMD SET "rule:power_limit:input:power" "modsrv:power_meter:power" > /dev/null
$REDIS_CMD SET "rule:power_limit:input:voltage_a" "modsrv:power_meter:voltage_a" > /dev/null
$REDIS_CMD SET "rule:power_limit:input:current_a" "modsrv:power_meter:current_a" > /dev/null

# 显示统计信息
echo -e "\n${GREEN}=== 初始化统计 ===${NC}"

# Lua 脚本统计
echo -e "\n${BLUE}Lua 脚本:${NC}"
SCRIPT_COUNT=$($REDIS_CMD --scan --pattern "script:*" | wc -l)
echo "  已加载脚本数量: $SCRIPT_COUNT"

# 映射统计
echo -e "\n${BLUE}数据映射:${NC}"
COMSRV_FORWARD=$($REDIS_CMD --scan --pattern "mapping:comsrv:*" | wc -l)
REVERSE_COUNT=$($REDIS_CMD --scan --pattern "mapping:reverse:*" | wc -l)
CLOUD_COUNT=$($REDIS_CMD --scan --pattern "mapping:cloud:*" | wc -l)
ROUTE_COUNT=$($REDIS_CMD --scan --pattern "mapping:route:*" | wc -l)

echo "  ComsRv 正向映射: $COMSRV_FORWARD"
echo "  反向控制映射: $REVERSE_COUNT"
echo "  云端数据映射: $CLOUD_COUNT"
echo "  服务路由映射: $ROUTE_COUNT"

# 规则统计
echo -e "\n${BLUE}规则引擎:${NC}"
RULE_COUNT=$($REDIS_CMD --scan --pattern "rule:*" | grep -v ":input:" | grep -v ":dag" | wc -l)
echo "  配置规则数量: $RULE_COUNT"

# 其他配置
echo -e "\n${BLUE}其他配置:${NC}"
THRESHOLD_COUNT=$($REDIS_CMD --scan --pattern "alarm:threshold:*" | wc -l)
echo "  告警阈值数量: $THRESHOLD_COUNT"

# 测试映射
if [ "$2" == "--test" ]; then
    echo -e "\n${GREEN}5. 测试映射${NC}"
    
    # 测试正向映射
    echo -n "  测试正向映射 (1001:m:1)... "
    RESULT=$($REDIS_CMD GET "mapping:comsrv:1001:m:1")
    if [ "$RESULT" == "modsrv:power_meter:voltage_a" ]; then
        echo -e "${GREEN}OK${NC}"
    else
        echo -e "${RED}失败${NC}"
    fi
    
    # 测试反向映射
    echo -n "  测试反向映射 (power_meter:breaker_control)... "
    RESULT=$($REDIS_CMD GET "mapping:reverse:power_meter:breaker_control")
    if [ "$RESULT" == "1001:c:1" ]; then
        echo -e "${GREEN}OK${NC}"
    else
        echo -e "${RED}失败${NC}"
    fi
fi

echo -e "\n${GREEN}初始化完成！${NC}"
echo ""
echo "使用提示:"
echo "  1. 启动服务时启用 Lua 同步:"
echo "     - ComsRv: export COMSRV_LUA_SYNC_ENABLED=true"
echo "     - 其他服务会自动使用加载的脚本"
echo "  "
echo "  2. 查看配置:"
echo "     - 映射: redis-cli keys 'mapping:*'"
echo "     - 脚本: redis-cli keys 'script:*'"
echo "     - 规则: redis-cli keys 'rule:*'"
echo "  "
echo "  3. 测试功能:"
echo "     - 数据同步: redis-cli evalsha \$(redis-cli get script:unified_sync) 0 sync_measurement 1001 m 1 230.5"
echo "     - 规则执行: redis-cli evalsha \$(redis-cli get script:rule_engine) 0 execute_rule voltage_high"
echo "     - 数据聚合: redis-cli evalsha \$(redis-cli get script:data_processor) 0 get_data_summary '[\"modsrv\",\"alarmsrv\"]'"
echo "  "
echo "  4. 管理命令:"
echo "     - 清理所有: $0 --clean"
echo "     - 测试映射: $0 --init --test"
echo "     - 重新加载: $0 --reload"