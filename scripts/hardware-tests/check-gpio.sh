#!/bin/bash
# GPIO硬件可用性检查脚本

set -e

echo "=== GPIO硬件检查 ==="

# 检查GPIO子系统
if [ ! -d /sys/class/gpio ]; then
    echo "错误: GPIO子系统不可用"
    echo "请确保内核已启用GPIO支持"
    exit 1
fi

# 检查GPIO访问权限
if [ ! -w /sys/class/gpio/export ]; then
    echo "错误: 无GPIO导出权限"
    echo "需要root权限或gpio组成员身份"
    exit 1
fi

# 获取GPIO芯片信息
echo "GPIO芯片信息:"
if [ -f /sys/kernel/debug/gpio ]; then
    cat /sys/kernel/debug/gpio 2>/dev/null || echo "无法读取GPIO调试信息"
fi

# 检查常用GPIO引脚
COMMON_PINS="17 27 22 23 24 25"
AVAILABLE_PINS=""

for pin in $COMMON_PINS; do
    # 尝试导出GPIO
    echo $pin > /sys/class/gpio/export 2>/dev/null || true

    if [ -d /sys/class/gpio/gpio$pin ]; then
        AVAILABLE_PINS="$AVAILABLE_PINS $pin"
        # 清理
        echo $pin > /sys/class/gpio/unexport 2>/dev/null || true
    fi
done

if [ -z "$AVAILABLE_PINS" ]; then
    echo "警告: 未找到可用的GPIO引脚"
    echo "测试可能需要特定的硬件配置"
else
    echo "可用GPIO引脚:$AVAILABLE_PINS"
fi

# 检查GPIO工具
if command -v gpio &> /dev/null; then
    echo "GPIO工具已安装"
    gpio -v
else
    echo "提示: 未安装gpio命令行工具"
fi

# 检查是否为树莓派
if [ -f /proc/device-tree/model ]; then
    MODEL=$(cat /proc/device-tree/model)
    echo "设备型号: $MODEL"

    if [[ $MODEL == *"Raspberry Pi"* ]]; then
        echo "检测到树莓派设备"
        # 树莓派特定检查
        if [ -f /boot/config.txt ]; then
            echo "检查GPIO配置..."
            grep -E "^dtoverlay=.*gpio" /boot/config.txt 2>/dev/null || echo "未发现GPIO overlay配置"
        fi
    fi
fi

echo "GPIO硬件检查完成"
exit 0
