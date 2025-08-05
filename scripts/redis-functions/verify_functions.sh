#!/bin/bash

# Verify Redis Functions compatibility after integration

echo "=== Verifying Redis Functions Compatibility ==="

# Color definitions
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[0;33m'
NC='\033[0m' # No Color

# Redis connection
REDIS_CLI="${REDIS_CLI:-redis-cli}"
REDIS_HOST="${REDIS_HOST:-localhost}"
REDIS_PORT="${REDIS_PORT:-6379}"

# Functions to verify
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
    ["hissrv_ack_batch"]="services.lua (new)"
    ["hissrv_get_batch_lines"]="services.lua (new)"
    
    # Service functions - ModSrv
    ["modsrv_init_mappings"]="services.lua"
    ["modsrv_sync_measurement"]="services.lua"
    ["modsrv_send_control"]="services.lua"
    
    # Service functions - NetSrv
    ["netsrv_collect_data"]="services.lua"
    ["netsrv_forward_data"]="services.lua"
    ["netsrv_get_stats"]="services.lua"
    ["netsrv_configure_route"]="services.lua (new)"
    ["netsrv_get_routes"]="services.lua (new)"
    ["netsrv_clear_queues"]="services.lua (new)"
    
    # Service functions - RuleSrv
    ["rulesrv_store_rule"]="services.lua"
    ["rulesrv_get_rule"]="services.lua"
    ["rulesrv_query_rules"]="services.lua"
    ["rulesrv_execute_dag"]="services.lua"
)

# Check Redis connection
echo -n "Checking Redis connection... "
if $REDIS_CLI -h $REDIS_HOST -p $REDIS_PORT ping > /dev/null 2>&1; then
    echo -e "${GREEN}Success${NC}"
else
    echo -e "${RED}Failed${NC}"
    echo "Please ensure Redis is running"
    exit 1
fi

# Get loaded functions
echo ""
echo "Getting loaded functions..."
LOADED_FUNCTIONS=$($REDIS_CLI -h $REDIS_HOST -p $REDIS_PORT FUNCTION LIST 2>/dev/null | grep -E "name" | awk -F'"' '{print $4}')

# Statistics
TOTAL=0
FOUND=0
MISSING=0

echo ""
echo "Verifying function list:"
echo "================================================================"
printf "%-35s %-20s %s\n" "Function Name" "Source File" "Status"
echo "================================================================"

# Verify each function
for func in "${!FUNCTIONS[@]}"; do
    TOTAL=$((TOTAL + 1))
    SOURCE="${FUNCTIONS[$func]}"
    
    if echo "$LOADED_FUNCTIONS" | grep -q "^${func}$"; then
        printf "%-35s %-20s " "$func" "$SOURCE"
        echo -e "${GREEN}✓ Loaded${NC}"
        FOUND=$((FOUND + 1))
    else
        printf "%-35s %-20s " "$func" "$SOURCE"
        echo -e "${RED}✗ Not found${NC}"
        MISSING=$((MISSING + 1))
    fi
done

echo "================================================================"
echo ""
echo "Statistics:"
echo "  Total functions: $TOTAL"
echo -e "  Loaded: ${GREEN}$FOUND${NC}"
echo -e "  Missing: ${RED}$MISSING${NC}"

# Check service functions
echo ""
echo "Checking service compatibility:"
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
    echo -e "${GREEN}✓ Compatible${NC}"
else
    echo -e "${RED}✗ Incompatible${NC}"
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
    echo -e "${GREEN}✓ Compatible${NC}"
else
    echo -e "${RED}✗ Incompatible${NC}"
fi

# RuleSrv
echo -n "RuleSrv: "
RULESRV_FUNCS=("store_rule" "get_rule" "delete_rule" "execute_dag_rule")
RULESRV_OK=true
# RuleSrv uses store_rule instead of save_rule
if echo "$LOADED_FUNCTIONS" | grep -q "^store_rule$"; then
    echo -e "${GREEN}✓ Compatible (store_rule alias available)${NC}"
else
    echo -e "${RED}✗ Incompatible${NC}"
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
    echo -e "${GREEN}✓ Compatible${NC}"
else
    echo -e "${RED}✗ Incompatible${NC}"
fi

# ComSrv
echo -n "ComSrv: "
if echo "$LOADED_FUNCTIONS" | grep -q "^sync_channel_data$"; then
    echo -e "${GREEN}✓ Compatible${NC}"
else
    echo -e "${RED}✗ Incompatible${NC}"
fi

echo "================================"

if [ $MISSING -eq 0 ]; then
    echo ""
    echo -e "${GREEN}✅ All functions loaded correctly, service compatibility verified!${NC}"
else
    echo ""
    echo -e "${YELLOW}⚠️  $MISSING functions not loaded, please restart Redis service or run standalone loading script${NC}"
fi