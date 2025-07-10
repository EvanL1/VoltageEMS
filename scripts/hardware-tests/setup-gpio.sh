#!/bin/bash
# GPIO测试环境设置脚本

set -e

echo "设置GPIO测试环境..."

# 定义测试使用的GPIO引脚
TEST_GPIO_PINS="17 27 22"

# 导出GPIO引脚
for pin in $TEST_GPIO_PINS; do
    if [ ! -d /sys/class/gpio/gpio$pin ]; then
        echo "导出GPIO $pin..."
        echo $pin > /sys/class/gpio/export 2>/dev/null || {
            echo "警告: 无法导出GPIO $pin (可能已导出)"
        }
    fi
    
    # 设置为输出模式（默认）
    if [ -d /sys/class/gpio/gpio$pin ]; then
        echo "out" > /sys/class/gpio/gpio$pin/direction 2>/dev/null || true
        echo "0" > /sys/class/gpio/gpio$pin/value 2>/dev/null || true
    fi
done

# 创建测试目录
mkdir -p /tmp/gpio_test_logs

echo "GPIO测试环境准备完成"