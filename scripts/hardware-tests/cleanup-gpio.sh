#!/bin/bash
# GPIO测试环境清理脚本

echo "清理GPIO测试环境..."

# 定义测试使用的GPIO引脚
TEST_GPIO_PINS="17 27 22"

# 取消导出GPIO引脚
for pin in $TEST_GPIO_PINS; do
    if [ -d /sys/class/gpio/gpio$pin ]; then
        echo "取消导出GPIO $pin..."
        echo $pin > /sys/class/gpio/unexport 2>/dev/null || true
    fi
done

# 清理测试日志
rm -rf /tmp/gpio_test_logs

echo "GPIO测试环境清理完成"