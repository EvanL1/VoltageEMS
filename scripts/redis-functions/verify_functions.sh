#!/bin/bash

# 验证 Redis Functions 整合后的兼容性

echo "=== 验证 Redis Functions 兼容性 ==="

# 颜色定义
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[0;33m'
NC='\033[0m' # No Color

# Redis 连接
REDIS_CLI="${REDIS_CLI:-redis-cli}"
REDIS_HOST="${REDIS_HOST:-localhost}"
REDIS_PORT="${REDIS_PORT:-6379}"

# 需要验证的函数列表
declare -A FUNCTIONS=(
    # Core functions
    ["generic_store"]="core.lua"
    ["generic_batch_sync"]="core.lua"
    ["generic_query"]="core.lua"
    ["generic_state_machine"]="core.lua"
    ["generic_multi_index"]="core.lua"
    ["generic_condition_eval"]="core.lua"
    ["generic_batch_collect"]="core.lua"
    ["generic_event_publish"]="core.lua"
    ["generic_statistics"]="core.lua"
    
    # Specific functions
    ["dag_executor"]="specific.lua"
    ["line_protocol_converter"]="specific.lua"
    
    # Domain functions - Alarm
    ["store_alarm"]="domain.lua"
    ["acknowledge_alarm"]="domain.lua"
    ["resolve_alarm"]="domain.lua"
    ["cleanup_old_alarms"]="domain.lua"
    ["query_alarms"]="domain.lua"
    
    # Domain functions - Rule
    ["save_rule"]="domain.lua"
    ["store_rule"]="domain.lua (alias)"
    ["delete_rule"]="domain.lua"
    ["save_rule_group"]="domain.lua"
    ["delete_rule_group"]="domain.lua"
    ["save_execution_history"]="domain.lua"
    
    # Domain functions - Sync
    ["sync_channel_data"]="domain.lua"
    ["sync_all_channels"]="domain.lua"
    ["calculate_device_delta"]="domain.lua"
    ["set_thresholds"]="domain.lua"
    
    # Service functions - AlarmSrv
    ["alarmsrv_store_alarm"]="services.lua"
    
    # Service functions - HisSrv
    ["hissrv_collect_data"]="services.lua"
    ["hissrv_convert_to_line_protocol"]="services.lua"
    ["hissrv_get_batch"]="services.lua"
    ["hissrv_ack_batch"]="services.lua (新增)"
    ["hissrv_get_batch_lines"]="services.lua (新增)"
    
    # Service functions - ModSrv
    ["modsrv_init_mappings"]="services.lua"
    ["modsrv_sync_measurement"]="services.lua"
    ["modsrv_send_control"]="services.lua"
    
    # Service functions - NetSrv
    ["netsrv_collect_data"]="services.lua"
    ["netsrv_forward_data"]="services.lua"
    ["netsrv_get_stats"]="services.lua"
    ["netsrv_configure_route"]="services.lua (新增)"
    ["netsrv_get_routes"]="services.lua (新增)"
    ["netsrv_clear_queues"]="services.lua (新增)"
    
    # Service functions - RuleSrv
    ["rulesrv_store_rule"]="services.lua"
    ["rulesrv_get_rule"]="services.lua"
    ["rulesrv_query_rules"]="services.lua"
    ["rulesrv_execute_dag"]="services.lua"
)

# 检查 Redis 连接
echo -n "检查 Redis 连接... "
if $REDIS_CLI -h $REDIS_HOST -p $REDIS_PORT ping > /dev/null 2>&1; then
    echo -e "${GREEN}成功${NC}"
else
    echo -e "${RED}失败${NC}"
    echo "请确保 Redis 已启动"
    exit 1
fi

# 获取已加载的函数
echo ""
echo "获取已加载的函数..."
LOADED_FUNCTIONS=$($REDIS_CLI -h $REDIS_HOST -p $REDIS_PORT FUNCTION LIST 2>/dev/null | grep -E "name" | awk -F'"' '{print $4}')

# 统计
TOTAL=0
FOUND=0
MISSING=0

echo ""
echo "验证函数列表："
echo "================================================================"
printf "%-35s %-20s %s\n" "函数名" "来源文件" "状态"
echo "================================================================"

# 验证每个函数
for func in "${!FUNCTIONS[@]}"; do
    TOTAL=$((TOTAL + 1))
    SOURCE="${FUNCTIONS[$func]}"
    
    if echo "$LOADED_FUNCTIONS" | grep -q "^${func}$"; then
        printf "%-35s %-20s " "$func" "$SOURCE"
        echo -e "${GREEN}✓ 已加载${NC}"
        FOUND=$((FOUND + 1))
    else
        printf "%-35s %-20s " "$func" "$SOURCE"
        echo -e "${RED}✗ 未找到${NC}"
        MISSING=$((MISSING + 1))
    fi
done

echo "================================================================"
echo ""
echo "统计结果："
echo "  总计函数: $TOTAL"
echo -e "  已加载: ${GREEN}$FOUND${NC}"
echo -e "  缺失: ${RED}$MISSING${NC}"

# 检查服务调用的函数
echo ""
echo "检查服务兼容性："
echo "================================"

# AlarmSrv
echo -n "AlarmSrv: "
ALARM_FUNCS=("store_alarm" "acknowledge_alarm" "resolve_alarm" "query_alarms")
ALARM_OK=true
for func in "${ALARM_FUNCS[@]}"; do
    if ! echo "$LOADED_FUNCTIONS" | grep -q "^${func}$"; then
        ALARM_OK=false
        break
    fi
done
if $ALARM_OK; then
    echo -e "${GREEN}✓ 兼容${NC}"
else
    echo -e "${RED}✗ 不兼容${NC}"
fi

# HisSrv
echo -n "HisSrv: "
HISSRV_FUNCS=("hissrv_collect_data" "hissrv_get_batch" "hissrv_ack_batch" "hissrv_get_batch_lines")
HISSRV_OK=true
for func in "${HISSRV_FUNCS[@]}"; do
    if ! echo "$LOADED_FUNCTIONS" | grep -q "^${func}$"; then
        HISSRV_OK=false
        break
    fi
done
if $HISSRV_OK; then
    echo -e "${GREEN}✓ 兼容${NC}"
else
    echo -e "${RED}✗ 不兼容${NC}"
fi

# RuleSrv
echo -n "RuleSrv: "
RULESRV_FUNCS=("store_rule" "get_rule" "delete_rule" "execute_dag_rule")
RULESRV_OK=true
# RuleSrv 使用 store_rule 而不是 save_rule
if echo "$LOADED_FUNCTIONS" | grep -q "^store_rule$"; then
    echo -e "${GREEN}✓ 兼容（store_rule 别名可用）${NC}"
else
    echo -e "${RED}✗ 不兼容${NC}"
fi

# NetSrv
echo -n "NetSrv: "
NETSRV_FUNCS=("netsrv_collect_data" "netsrv_forward_data" "netsrv_configure_route" "netsrv_get_routes" "netsrv_clear_queues")
NETSRV_OK=true
for func in "${NETSRV_FUNCS[@]}"; do
    if ! echo "$LOADED_FUNCTIONS" | grep -q "^${func}$"; then
        NETSRV_OK=false
        break
    fi
done
if $NETSRV_OK; then
    echo -e "${GREEN}✓ 兼容${NC}"
else
    echo -e "${RED}✗ 不兼容${NC}"
fi

# ComSrv
echo -n "ComSrv: "
if echo "$LOADED_FUNCTIONS" | grep -q "^sync_channel_data$"; then
    echo -e "${GREEN}✓ 兼容${NC}"
else
    echo -e "${RED}✗ 不兼容${NC}"
fi

echo "================================"

if [ $MISSING -eq 0 ]; then
    echo ""
    echo -e "${GREEN}✅ 所有函数已正确加载，服务兼容性验证通过！${NC}"
else
    echo ""
    echo -e "${YELLOW}⚠️  有 $MISSING 个函数未加载，请重启 Redis 服务或运行独立加载脚本${NC}"
fi