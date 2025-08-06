#!/bin/bash

# 同步配置管理工具
# 提供简单的命令行接口管理同步规则

set -e

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Redis 配置
REDIS_HOST=${REDIS_HOST:-localhost}
REDIS_PORT=${REDIS_PORT:-6379}
REDIS_CLI="redis-cli -h $REDIS_HOST -p $REDIS_PORT"

# 显示帮助
show_help() {
    echo "同步配置管理工具"
    echo
    echo "用法: $0 <命令> [参数]"
    echo
    echo "命令:"
    echo "  init              初始化预定义的同步规则"
    echo "  status            显示所有同步规则状态"
    echo "  list              列出所有同步规则"
    echo "  show <rule_id>    显示特定规则的详细配置"
    echo "  enable <rule_id>  启用同步规则"
    echo "  disable <rule_id> 禁用同步规则"
    echo "  sync <rule_id>    手动执行同步规则"
    echo "  sync-all          执行所有活动的同步规则"
    echo "  stats <rule_id>   显示同步统计信息"
    echo "  reset <rule_id>   重置统计信息"
    echo "  load <file>       从文件加载配置"
    echo "  delete <rule_id>  删除同步规则"
    echo "  test              运行测试同步"
    echo "  monitor           实时监控同步活动"
    echo
    echo "示例:"
    echo "  $0 init                          # 初始化所有预定义规则"
    echo "  $0 status                        # 查看所有规则状态"
    echo "  $0 enable comsrv_to_modsrv_T    # 启用特定规则"
    echo "  $0 sync comsrv_to_modsrv_T      # 手动执行同步"
    echo "  $0 load my_rule.json            # 从文件加载配置"
}

# 检查 Redis 连接
check_redis() {
    if ! $REDIS_CLI ping > /dev/null 2>&1; then
        echo -e "${RED}✗ Redis 未运行或无法连接${NC}"
        exit 1
    fi
}

# 初始化配置
init_configs() {
    echo -e "${YELLOW}初始化同步配置...${NC}"
    
    # 加载函数
    echo "加载同步引擎..."
    $REDIS_CLI -x FUNCTION LOAD REPLACE < scripts/redis-functions/sync_engine.lua > /dev/null 2>&1
    $REDIS_CLI -x FUNCTION LOAD REPLACE < scripts/redis-functions/sync_config_init.lua > /dev/null 2>&1
    
    # 初始化配置
    RESULT=$($REDIS_CLI FCALL init_sync_configs 0 2>&1)
    
    if [[ "$RESULT" == *"success"* ]]; then
        echo -e "${GREEN}✓ 配置初始化成功${NC}"
        echo "$RESULT" | python3 -m json.tool 2>/dev/null || echo "$RESULT"
    else
        echo -e "${RED}✗ 配置初始化失败${NC}"
        echo "$RESULT"
    fi
}

# 显示状态
show_status() {
    echo -e "${BLUE}同步规则状态：${NC}"
    echo
    
    RESULT=$($REDIS_CLI FCALL get_sync_status 0 2>&1)
    
    if [[ "$RESULT" != "" ]] && [[ "$RESULT" != "(nil)" ]]; then
        echo "$RESULT" | python3 -c "
import json
import sys

try:
    data = json.loads(sys.stdin.read())
    print(f'{'规则 ID':<25} {'状态':<10} {'描述':<40}')
    print('-' * 80)
    for rule in data:
        status = '✓ 启用' if rule.get('enabled') else '✗ 禁用'
        print(f\"{rule['rule_id']:<25} {status:<10} {rule.get('description', 'N/A'):<40}\")
        if rule.get('stats'):
            stats = rule['stats']
            if stats.get('sync_count'):
                print(f\"  └─ 同步次数: {stats.get('sync_count', 0)}, 最后同步: {stats.get('last_sync', 'N/A')}\")
except:
    print(sys.stdin.read())
" 2>/dev/null || echo "$RESULT"
    else
        echo "暂无同步规则"
    fi
}

# 列出规则
list_rules() {
    echo -e "${BLUE}所有同步规则：${NC}"
    RULES=$($REDIS_CLI SMEMBERS sync:rules 2>&1)
    
    if [[ "$RULES" != "" ]]; then
        echo "$RULES" | while read -r rule; do
            if [[ "$rule" != "" ]] && [[ "$rule" != "(empty array)" ]]; then
                # 检查是否在活动列表中
                IS_ACTIVE=$($REDIS_CLI SISMEMBER sync:rules:active "$rule" 2>&1)
                if [[ "$IS_ACTIVE" == "1" ]]; then
                    echo -e "  ${GREEN}● $rule (活动)${NC}"
                else
                    echo -e "  ${YELLOW}○ $rule (非活动)${NC}"
                fi
            fi
        done
    else
        echo "暂无同步规则"
    fi
}

# 显示规则详情
show_rule() {
    local rule_id="$1"
    
    echo -e "${BLUE}规则详情: $rule_id${NC}"
    echo
    
    CONFIG=$($REDIS_CLI FCALL sync_config_get 1 "$rule_id" 2>&1)
    
    if [[ "$CONFIG" != "(nil)" ]] && [[ "$CONFIG" != "" ]]; then
        echo "$CONFIG" | python3 -m json.tool 2>/dev/null || echo "$CONFIG"
    else
        echo -e "${RED}规则不存在: $rule_id${NC}"
    fi
}

# 启用规则
enable_rule() {
    local rule_id="$1"
    
    echo -e "${YELLOW}启用规则: $rule_id${NC}"
    
    RESULT=$($REDIS_CLI FCALL toggle_sync_rule 1 "$rule_id" true 2>&1)
    
    if [[ "$RESULT" == "OK" ]]; then
        echo -e "${GREEN}✓ 规则已启用${NC}"
    else
        echo -e "${RED}✗ 启用失败: $RESULT${NC}"
    fi
}

# 禁用规则
disable_rule() {
    local rule_id="$1"
    
    echo -e "${YELLOW}禁用规则: $rule_id${NC}"
    
    RESULT=$($REDIS_CLI FCALL toggle_sync_rule 1 "$rule_id" false 2>&1)
    
    if [[ "$RESULT" == "OK" ]]; then
        echo -e "${GREEN}✓ 规则已禁用${NC}"
    else
        echo -e "${RED}✗ 禁用失败: $RESULT${NC}"
    fi
}

# 执行同步
sync_rule() {
    local rule_id="$1"
    
    echo -e "${YELLOW}执行同步规则: $rule_id${NC}"
    
    RESULT=$($REDIS_CLI FCALL sync_pattern_execute 1 "$rule_id" 2>&1)
    
    if [[ "$RESULT" != "" ]]; then
        echo -e "${GREEN}✓ 同步完成${NC}"
        echo "$RESULT" | python3 -m json.tool 2>/dev/null || echo "$RESULT"
    else
        echo -e "${RED}✗ 同步失败${NC}"
    fi
}

# 执行所有同步
sync_all() {
    echo -e "${YELLOW}执行所有活动的同步规则...${NC}"
    
    RESULT=$($REDIS_CLI FCALL batch_sync_all 0 2>&1)
    
    if [[ "$RESULT" != "" ]]; then
        echo -e "${GREEN}✓ 批量同步完成${NC}"
        echo "$RESULT" | python3 -m json.tool 2>/dev/null || echo "$RESULT"
    else
        echo -e "${RED}✗ 批量同步失败${NC}"
    fi
}

# 显示统计
show_stats() {
    local rule_id="$1"
    
    echo -e "${BLUE}同步统计: $rule_id${NC}"
    echo
    
    STATS=$($REDIS_CLI FCALL sync_stats_get 1 "$rule_id" 2>&1)
    
    if [[ "$STATS" != "{}" ]] && [[ "$STATS" != "(nil)" ]]; then
        echo "$STATS" | python3 -m json.tool 2>/dev/null || echo "$STATS"
    else
        echo "暂无统计信息"
    fi
}

# 重置统计
reset_stats() {
    local rule_id="$1"
    
    echo -e "${YELLOW}重置统计: $rule_id${NC}"
    
    RESULT=$($REDIS_CLI FCALL sync_stats_reset 1 "$rule_id" 2>&1)
    
    if [[ "$RESULT" == "OK" ]]; then
        echo -e "${GREEN}✓ 统计已重置${NC}"
    else
        echo -e "${RED}✗ 重置失败: $RESULT${NC}"
    fi
}

# 从文件加载配置
load_config() {
    local file="$1"
    
    if [[ ! -f "$file" ]]; then
        echo -e "${RED}文件不存在: $file${NC}"
        exit 1
    fi
    
    # 从文件名提取规则 ID（去掉 .json 后缀）
    local rule_id=$(basename "$file" .json)
    
    echo -e "${YELLOW}加载配置: $rule_id${NC}"
    echo "从文件: $file"
    
    CONFIG=$(cat "$file")
    RESULT=$($REDIS_CLI FCALL sync_config_set 1 "$rule_id" "$CONFIG" 2>&1)
    
    if [[ "$RESULT" == "OK" ]]; then
        echo -e "${GREEN}✓ 配置加载成功${NC}"
    else
        echo -e "${RED}✗ 加载失败: $RESULT${NC}"
    fi
}

# 删除规则
delete_rule() {
    local rule_id="$1"
    
    echo -e "${YELLOW}删除规则: $rule_id${NC}"
    read -p "确认删除？(y/N) " -n 1 -r
    echo
    
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        RESULT=$($REDIS_CLI FCALL sync_config_delete 1 "$rule_id" 2>&1)
        
        if [[ "$RESULT" == "OK" ]]; then
            echo -e "${GREEN}✓ 规则已删除${NC}"
        else
            echo -e "${RED}✗ 删除失败: $RESULT${NC}"
        fi
    else
        echo "取消删除"
    fi
}

# 测试同步
test_sync() {
    echo -e "${YELLOW}运行同步测试...${NC}"
    
    # 创建测试数据
    echo "1. 创建测试数据..."
    $REDIS_CLI HSET test:source:1 field1 "value1" field2 "value2" > /dev/null
    
    # 创建测试规则
    echo "2. 创建测试规则..."
    TEST_CONFIG='{
        "enabled": true,
        "description": "测试同步规则",
        "source": {
            "pattern": "test:source:*",
            "type": "hash"
        },
        "target": {
            "pattern": "test:target:$1",
            "type": "hash"
        },
        "transform": {
            "type": "direct"
        }
    }'
    
    $REDIS_CLI FCALL sync_config_set 1 test_sync_rule "$TEST_CONFIG" > /dev/null
    
    # 执行同步
    echo "3. 执行同步..."
    RESULT=$($REDIS_CLI FCALL sync_execute 3 test_sync_rule "test:source:1" "test:target:1" 2>&1)
    
    # 验证结果
    echo "4. 验证结果..."
    TARGET_DATA=$($REDIS_CLI HGETALL test:target:1 2>&1)
    
    if [[ "$TARGET_DATA" == *"value1"* ]]; then
        echo -e "${GREEN}✓ 测试通过：数据同步成功${NC}"
    else
        echo -e "${RED}✗ 测试失败：数据未同步${NC}"
    fi
    
    # 清理测试数据
    echo "5. 清理测试数据..."
    $REDIS_CLI DEL test:source:1 test:target:1 > /dev/null
    $REDIS_CLI FCALL sync_config_delete 1 test_sync_rule > /dev/null
    
    echo -e "${GREEN}测试完成${NC}"
}

# 监控同步活动
monitor_sync() {
    echo -e "${BLUE}监控同步活动（按 Ctrl+C 退出）${NC}"
    echo
    
    while true; do
        clear
        echo -e "${BLUE}=== 同步监控 $(date '+%Y-%m-%d %H:%M:%S') ===${NC}"
        echo
        
        # 显示活动规则
        echo -e "${YELLOW}活动规则：${NC}"
        ACTIVE_RULES=$($REDIS_CLI SMEMBERS sync:rules:active 2>&1)
        echo "$ACTIVE_RULES" | while read -r rule; do
            if [[ "$rule" != "" ]] && [[ "$rule" != "(empty array)" ]]; then
                STATS=$($REDIS_CLI HGET "sync:stats:$rule" sync_count 2>&1)
                echo "  • $rule (同步次数: ${STATS:-0})"
            fi
        done
        
        echo
        echo -e "${YELLOW}最近同步：${NC}"
        
        # 显示每个规则的最后同步时间
        $REDIS_CLI SMEMBERS sync:rules 2>&1 | while read -r rule; do
            if [[ "$rule" != "" ]] && [[ "$rule" != "(empty array)" ]]; then
                LAST_SYNC=$($REDIS_CLI HGET "sync:stats:$rule" last_sync 2>&1)
                if [[ "$LAST_SYNC" != "(nil)" ]] && [[ "$LAST_SYNC" != "" ]]; then
                    echo "  • $rule: $(date -r "$LAST_SYNC" '+%H:%M:%S' 2>/dev/null || echo "$LAST_SYNC")"
                fi
            fi
        done
        
        sleep 5
    done
}

# 主程序
main() {
    check_redis
    
    case "$1" in
        init)
            init_configs
            ;;
        status)
            show_status
            ;;
        list)
            list_rules
            ;;
        show)
            if [[ -z "$2" ]]; then
                echo -e "${RED}请提供规则 ID${NC}"
                exit 1
            fi
            show_rule "$2"
            ;;
        enable)
            if [[ -z "$2" ]]; then
                echo -e "${RED}请提供规则 ID${NC}"
                exit 1
            fi
            enable_rule "$2"
            ;;
        disable)
            if [[ -z "$2" ]]; then
                echo -e "${RED}请提供规则 ID${NC}"
                exit 1
            fi
            disable_rule "$2"
            ;;
        sync)
            if [[ -z "$2" ]]; then
                echo -e "${RED}请提供规则 ID${NC}"
                exit 1
            fi
            sync_rule "$2"
            ;;
        sync-all)
            sync_all
            ;;
        stats)
            if [[ -z "$2" ]]; then
                echo -e "${RED}请提供规则 ID${NC}"
                exit 1
            fi
            show_stats "$2"
            ;;
        reset)
            if [[ -z "$2" ]]; then
                echo -e "${RED}请提供规则 ID${NC}"
                exit 1
            fi
            reset_stats "$2"
            ;;
        load)
            if [[ -z "$2" ]]; then
                echo -e "${RED}请提供配置文件路径${NC}"
                exit 1
            fi
            load_config "$2"
            ;;
        delete)
            if [[ -z "$2" ]]; then
                echo -e "${RED}请提供规则 ID${NC}"
                exit 1
            fi
            delete_rule "$2"
            ;;
        test)
            test_sync
            ;;
        monitor)
            monitor_sync
            ;;
        help|--help|-h)
            show_help
            ;;
        *)
            echo -e "${RED}未知命令: $1${NC}"
            echo
            show_help
            exit 1
            ;;
    esac
}

main "$@"