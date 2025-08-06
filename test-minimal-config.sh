#!/bin/bash

echo "=== 最简化comsrv配置测试 ==="

echo "1. 查看YAML配置（只有必需项）："
cat services/comsrv/config/comsrv.yaml

echo -e "\n2. 四遥点表文件："
echo "- telemetry.csv（遥测）："
head -3 services/comsrv/config/telemetry.csv

echo -e "\n- signal.csv（遥信）："  
head -3 services/comsrv/config/signal.csv

echo -e "\n- control.csv（遥控）："
head -2 services/comsrv/config/control.csv

echo -e "\n- adjustment.csv（遥调）："
head -2 services/comsrv/config/adjustment.csv

echo -e "\n3. 协议映射文件："
echo "- telemetry_mapping.csv："
head -3 services/comsrv/config/telemetry_mapping.csv

echo -e "\n- signal_mapping.csv："
head -3 services/comsrv/config/signal_mapping.csv

echo -e "\n- control_mapping.csv："
head -2 services/comsrv/config/control_mapping.csv

echo -e "\n- adjustment_mapping.csv："
head -2 services/comsrv/config/adjustment_mapping.csv

echo -e "\n=== 配置说明 ==="
echo "- YAML只保留必需配置项，其余使用默认值"
echo "- polling_config.interval_ms 默认值: 1000ms"  
echo "- table_config 默认路径: 四遥='comsrv', 映射='comsrv/protocol'"
echo "- enabled 默认值: true"
echo "- slave_id 在映射文件中定义，不在YAML中"
echo "- bit_position 对于bool类型很重要："
echo "  * 统一按16位寄存器处理，bit_position范围0-15"
echo "  * 一个寄存器可以存储16个bool值"
echo "  * 通过bit_position指定读取寄存器中的具体位"
echo "  * 这样所有Function Code处理方式统一，避免混乱"