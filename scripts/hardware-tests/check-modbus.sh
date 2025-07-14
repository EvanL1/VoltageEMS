#!/bin/bash
# Modbus设备可用性检查脚本

set -e

echo "=== Modbus设备检查 ==="

# 检查网络连接（Modbus TCP）
echo "检查Modbus TCP连接..."

# 定义测试的Modbus设备
MODBUS_TCP_DEVICES="
192.168.1.100:502
192.168.1.101:502
localhost:5020
"

echo "测试Modbus TCP设备连接:"
for device in $MODBUS_TCP_DEVICES; do
    if [ -n "$device" ]; then
        HOST=$(echo $device | cut -d: -f1)
        PORT=$(echo $device | cut -d: -f2)

        echo -n "  - $device: "
        if timeout 2 bash -c "echo >/dev/tcp/$HOST/$PORT" 2>/dev/null; then
            echo "可达"
        else
            echo "不可达"
        fi
    fi
done

# 检查Modbus工具
echo ""
echo "检查Modbus测试工具..."

# Python modbus库
if python3 -c "import pymodbus" 2>/dev/null; then
    echo "  ✓ pymodbus已安装"
    python3 -c "import pymodbus; print(f'    版本: {pymodbus.__version__}')"
else
    echo "  ✗ pymodbus未安装"
    echo "    建议: pip3 install pymodbus"
fi

# modbus-cli工具
if command -v modbus &> /dev/null; then
    echo "  ✓ modbus-cli已安装"
else
    echo "  ✗ modbus-cli未安装"
fi

# 检查Modbus RTU（串口）
echo ""
echo "检查Modbus RTU支持..."
SERIAL_DEVICES=$(ls /dev/ttyS* /dev/ttyUSB* 2>/dev/null || true)
if [ -n "$SERIAL_DEVICES" ]; then
    echo "可用于Modbus RTU的串口设备:"
    for dev in $SERIAL_DEVICES; do
        if [ -c "$dev" ]; then
            echo "  - $dev"
        fi
    done
else
    echo "未发现串口设备（Modbus RTU需要串口）"
fi

# 检查模拟器
echo ""
echo "检查Modbus模拟器..."
SIMULATOR_PATH="tests/simulators/modbus_tcp_simulator.py"
if [ -f "$SIMULATOR_PATH" ]; then
    echo "  ✓ Modbus TCP模拟器存在: $SIMULATOR_PATH"

    # 检查模拟器是否在运行
    if pgrep -f "modbus_tcp_simulator.py" > /dev/null; then
        echo "    状态: 运行中"
        echo "    PID: $(pgrep -f modbus_tcp_simulator.py)"
    else
        echo "    状态: 未运行"
        echo "    启动命令: python3 $SIMULATOR_PATH"
    fi
else
    echo "  ✗ 未找到Modbus模拟器"
fi

# 网络诊断
echo ""
echo "网络接口信息:"
ip -4 addr show | grep -E "inet " | grep -v "127.0.0.1" | awk '{print "  - " $NF ": " $2}'

# 防火墙检查
echo ""
echo "防火墙状态:"
if command -v ufw &> /dev/null; then
    sudo ufw status | grep -E "(502|modbus)" || echo "  未发现Modbus相关规则"
elif command -v firewall-cmd &> /dev/null; then
    sudo firewall-cmd --list-ports | grep -E "(502|modbus)" || echo "  未发现Modbus相关规则"
else
    echo "  未检测到防火墙工具"
fi

# 测试建议
echo ""
echo "测试建议:"
echo "1. 启动Modbus模拟器: python3 tests/simulators/modbus_tcp_simulator.py"
echo "2. 测试连接: modbus -v localhost:5020 1 0 10"
echo "3. 对于RTU测试，使用USB-RS485转换器"
echo "4. 确保防火墙允许502端口（Modbus默认端口）"

# 快速测试脚本
cat << 'EOF' > /tmp/test_modbus.py
#!/usr/bin/env python3
import sys
try:
    from pymodbus.client import ModbusTcpClient
    client = ModbusTcpClient('localhost', port=5020)
    if client.connect():
        result = client.read_holding_registers(0, 1, unit=1)
        if not result.isError():
            print("Modbus测试成功！寄存器值:", result.registers)
        else:
            print("Modbus读取错误:", result)
        client.close()
    else:
        print("无法连接到Modbus服务器")
except ImportError:
    print("请安装pymodbus: pip3 install pymodbus")
except Exception as e:
    print("测试错误:", e)
EOF

echo ""
echo "创建了快速测试脚本: /tmp/test_modbus.py"
echo "运行: python3 /tmp/test_modbus.py"

echo "Modbus设备检查完成"
exit 0
