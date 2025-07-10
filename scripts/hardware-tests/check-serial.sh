#!/bin/bash
# 串口硬件可用性检查脚本

set -e

echo "=== 串口硬件检查 ==="

# 检查串口设备
echo "检查串口设备..."
SERIAL_DEVICES=""

# 检查物理串口
for dev in /dev/ttyS* /dev/ttyAMA* /dev/ttyUSB* /dev/ttyACM*; do
    if [ -c "$dev" ]; then
        SERIAL_DEVICES="$SERIAL_DEVICES $dev"
    fi
done

if [ -z "$SERIAL_DEVICES" ]; then
    echo "警告: 未发现串口设备"
else
    echo "发现串口设备:"
    for dev in $SERIAL_DEVICES; do
        echo -n "  - $dev"
        
        # 检查权限
        if [ -r "$dev" ] && [ -w "$dev" ]; then
            echo " (可读写)"
        else
            echo " (需要权限)"
        fi
        
        # 获取设备信息
        if command -v udevadm &> /dev/null; then
            VENDOR=$(udevadm info --query=all --name=$dev 2>/dev/null | grep "ID_VENDOR=" | cut -d= -f2)
            MODEL=$(udevadm info --query=all --name=$dev 2>/dev/null | grep "ID_MODEL=" | cut -d= -f2)
            if [ -n "$VENDOR" ] || [ -n "$MODEL" ]; then
                echo "      厂商: $VENDOR, 型号: $MODEL"
            fi
        fi
    done
fi

# 检查用户组
echo ""
echo "检查用户权限..."
if groups | grep -q dialout; then
    echo "当前用户已在dialout组"
else
    echo "警告: 当前用户不在dialout组"
    echo "建议运行: sudo usermod -a -G dialout $USER"
fi

# 检查串口工具
echo ""
echo "检查串口工具..."
TOOLS="minicom screen picocom stty"
for tool in $TOOLS; do
    if command -v $tool &> /dev/null; then
        echo "  ✓ $tool 已安装"
    else
        echo "  ✗ $tool 未安装"
    fi
done

# 检查Python串口库
echo ""
echo "检查Python串口支持..."
if python3 -c "import serial" 2>/dev/null; then
    echo "  ✓ pyserial已安装"
    python3 -c "import serial; print(f'    版本: {serial.__version__}')"
else
    echo "  ✗ pyserial未安装"
    echo "    建议: pip3 install pyserial"
fi

# 检查虚拟串口支持
echo ""
echo "检查虚拟串口支持..."
if command -v socat &> /dev/null; then
    echo "  ✓ socat已安装 (可创建虚拟串口对)"
    echo "    示例: socat -d -d pty,raw,echo=0 pty,raw,echo=0"
else
    echo "  ✗ socat未安装"
    echo "    建议: sudo apt-get install socat"
fi

# 系统信息
echo ""
echo "系统串口配置:"
if [ -f /proc/tty/driver/serial ]; then
    echo "硬件串口信息:"
    cat /proc/tty/driver/serial | grep -v "serinfo" | grep -v "^$"
fi

# 检查串口参数
if [ -n "$SERIAL_DEVICES" ]; then
    echo ""
    echo "串口参数示例 (使用第一个可用设备):"
    FIRST_SERIAL=$(echo $SERIAL_DEVICES | awk '{print $1}')
    if [ -r "$FIRST_SERIAL" ]; then
        stty -F $FIRST_SERIAL -a 2>/dev/null | head -n 2 || echo "无法读取串口参数"
    fi
fi

# 测试建议
echo ""
echo "测试建议:"
echo "1. 物理串口测试需要回环连接器或另一设备"
echo "2. 虚拟串口测试可使用: socat -d -d pty,raw,echo=0 pty,raw,echo=0"
echo "3. 设置串口参数: stty -F /dev/ttyS0 9600 cs8 -cstopb -parenb"
echo "4. 测试通信: echo 'test' > /dev/ttyS0"

echo "串口硬件检查完成"
exit 0