#!/bin/bash
# 收集GPIO测试日志

if [ $# -ne 1 ]; then
    echo "用法: $0 <输出目录>"
    exit 1
fi

OUTPUT_DIR=$1
mkdir -p $OUTPUT_DIR

echo "收集GPIO测试日志..."

# 收集GPIO状态
echo "=== GPIO状态 ===" > $OUTPUT_DIR/gpio_status.log
for gpio in /sys/class/gpio/gpio*; do
    if [ -d "$gpio" ]; then
        PIN=$(basename $gpio)
        echo "--- $PIN ---" >> $OUTPUT_DIR/gpio_status.log
        cat $gpio/direction >> $OUTPUT_DIR/gpio_status.log 2>/dev/null || echo "无法读取方向" >> $OUTPUT_DIR/gpio_status.log
        cat $gpio/value >> $OUTPUT_DIR/gpio_status.log 2>/dev/null || echo "无法读取值" >> $OUTPUT_DIR/gpio_status.log
        echo "" >> $OUTPUT_DIR/gpio_status.log
    fi
done

# 收集GPIO调试信息
if [ -f /sys/kernel/debug/gpio ]; then
    cp /sys/kernel/debug/gpio $OUTPUT_DIR/gpio_debug.log 2>/dev/null || true
fi

# 收集测试日志
if [ -d /tmp/gpio_test_logs ]; then
    cp -r /tmp/gpio_test_logs/* $OUTPUT_DIR/ 2>/dev/null || true
fi

echo "GPIO日志收集完成"