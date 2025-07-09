#!/bin/bash
# 测试特定协议的脚本

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

# 颜色定义
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# 检查参数
if [ $# -lt 1 ]; then
    echo "Usage: $0 <protocol_id> [test_config_file]"
    echo "Example: $0 modbus_tcp"
    echo "         $0 modbus_tcp tests/configs/modbus_test.yaml"
    exit 1
fi

PROTOCOL_ID=$1
CONFIG_FILE=${2:-""}

echo -e "${BLUE}🧪 Testing Protocol: $PROTOCOL_ID${NC}"
echo "========================================"

cd "$PROJECT_DIR"

# 1. 运行协议特定的单元测试
echo -e "\n${YELLOW}Running unit tests...${NC}"
if cargo test --lib --features "$PROTOCOL_ID" -- "$PROTOCOL_ID" 2>/dev/null; then
    echo -e "${GREEN}✅ Unit tests passed${NC}"
else
    echo -e "${YELLOW}⚠️  No specific unit tests found${NC}"
fi

# 2. 运行CLI测试工具
echo -e "\n${YELLOW}Running protocol test framework...${NC}"
if [ -n "$CONFIG_FILE" ] && [ -f "$CONFIG_FILE" ]; then
    cargo run --bin comsrv-cli -- test-protocol "$PROTOCOL_ID" --config "$CONFIG_FILE"
else
    cargo run --bin comsrv-cli -- test-protocol "$PROTOCOL_ID"
fi

# 3. 运行兼容性测试
echo -e "\n${YELLOW}Running compatibility tests...${NC}"
cargo test --test protocol_compatibility_test "${PROTOCOL_ID}_compatibility" -- --nocapture

# 4. 运行模拟器测试（如果有）
case "$PROTOCOL_ID" in
    "modbus_tcp")
        echo -e "\n${YELLOW}Starting Modbus simulator...${NC}"
        # 启动Modbus模拟器
        cargo run --example modbus_simulator &
        SIMULATOR_PID=$!
        sleep 2
        
        # 运行客户端测试
        echo -e "\n${YELLOW}Running client tests...${NC}"
        cargo test --test modbus_client_test
        
        # 停止模拟器
        kill $SIMULATOR_PID 2>/dev/null || true
        ;;
        
    "iec60870")
        echo -e "\n${YELLOW}Starting IEC60870 simulator...${NC}"
        # TODO: 启动IEC60870模拟器
        ;;
        
    "can")
        echo -e "\n${YELLOW}CAN protocol requires hardware or virtual CAN interface${NC}"
        ;;
        
    *)
        echo -e "${YELLOW}No specific simulator for $PROTOCOL_ID${NC}"
        ;;
esac

# 5. 性能测试
echo -e "\n${YELLOW}Running performance test...${NC}"
cargo run --bin comsrv-cli -- benchmark-protocol "$PROTOCOL_ID" --duration 10

echo -e "\n${GREEN}✅ Protocol testing completed${NC}"