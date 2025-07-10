#!/bin/bash
# CAN总线硬件可用性检查脚本

set -e

echo "=== CAN总线硬件检查 ==="

# 检查CAN工具
if ! command -v cansend &> /dev/null; then
    echo "错误: can-utils未安装"
    echo "请运行: sudo apt-get install can-utils"
    exit 1
fi

# 检查CAN内核模块
echo "检查CAN内核模块..."
REQUIRED_MODULES="can can_raw can_dev"
MISSING_MODULES=""

for module in $REQUIRED_MODULES; do
    if ! lsmod | grep -q "^$module"; then
        MISSING_MODULES="$MISSING_MODULES $module"
    fi
done

if [ -n "$MISSING_MODULES" ]; then
    echo "加载缺失的内核模块:$MISSING_MODULES"
    for module in $MISSING_MODULES; do
        sudo modprobe $module 2>/dev/null || echo "警告: 无法加载模块 $module"
    done
fi

# 检查物理CAN接口
echo "检查物理CAN接口..."
PHYSICAL_CAN=$(ip -details link show type can 2>/dev/null | grep -E "^[0-9]+: can" | cut -d: -f2 | tr -d ' ')

if [ -n "$PHYSICAL_CAN" ]; then
    echo "发现物理CAN接口:"
    for iface in $PHYSICAL_CAN; do
        echo "  - $iface"
        ip -details link show $iface
    done
else
    echo "未发现物理CAN接口"
fi

# 检查虚拟CAN接口
echo "检查虚拟CAN接口..."
VCAN_INTERFACES=$(ip -details link show type vcan 2>/dev/null | grep -E "^[0-9]+: vcan" | cut -d: -f2 | tr -d ' ')

if [ -z "$VCAN_INTERFACES" ]; then
    echo "创建虚拟CAN接口用于测试..."
    sudo modprobe vcan 2>/dev/null || true
    sudo ip link add dev vcan0 type vcan 2>/dev/null || true
    sudo ip link set up vcan0 2>/dev/null || true
    
    if ip link show vcan0 &>/dev/null; then
        echo "成功创建vcan0接口"
    else
        echo "警告: 无法创建虚拟CAN接口"
    fi
else
    echo "发现虚拟CAN接口:"
    for iface in $VCAN_INTERFACES; do
        echo "  - $iface"
    done
fi

# 检查CAN接口状态
echo "CAN接口状态:"
ip -details -statistics link show type can 2>/dev/null || echo "无CAN接口统计信息"

# 检查CAN硬件（如果存在）
if [ -d /sys/class/net ]; then
    for iface in /sys/class/net/can*; do
        if [ -d "$iface" ]; then
            IFACE_NAME=$(basename $iface)
            echo "接口 $IFACE_NAME 详情:"
            
            # 检查比特率
            if [ -f "$iface/can_bittiming/bitrate" ]; then
                BITRATE=$(cat "$iface/can_bittiming/bitrate" 2>/dev/null || echo "未知")
                echo "  比特率: $BITRATE"
            fi
            
            # 检查状态
            if [ -f "$iface/operstate" ]; then
                STATE=$(cat "$iface/operstate" 2>/dev/null || echo "未知")
                echo "  状态: $STATE"
            fi
        fi
    done
fi

# 测试环境建议
echo ""
echo "测试环境建议:"
echo "1. 对于物理CAN测试，确保CAN收发器已连接"
echo "2. 对于虚拟测试，使用vcan0接口"
echo "3. 设置CAN接口: sudo ip link set can0 type can bitrate 500000"
echo "4. 启动接口: sudo ip link set up can0"

echo "CAN硬件检查完成"
exit 0