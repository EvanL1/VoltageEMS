#!/bin/bash
# 从comsrv日志中提取最近一次轮询的所有位解析值

echo "============================================================"
echo "从comsrv日志提取实际读取的位值"
echo "============================================================"

# 获取最近一次轮询的位解析日志
echo "提取最近一次轮询的位解析日志..."
docker-compose -f docker-compose.test.yml logs comsrv | \
    grep "Bit extraction" | \
    tail -20 | \
    sort -k10 | \
    while read -r line; do
        # 提取寄存器值、位位置和位值
        register=$(echo "$line" | sed -n 's/.*register=\(0x[0-9A-F]*\).*/\1/p')
        bit_pos=$(echo "$line" | sed -n 's/.*bit_pos=\([0-9]*\).*/\1/p')
        bit_value=$(echo "$line" | sed -n 's/.*bit_value=\([01]\).*/\1/p')
        
        if [[ -n "$register" && -n "$bit_pos" && -n "$bit_value" ]]; then
            register_decimal=$((register))
            if [[ $register_decimal -eq 165 ]]; then  # 0xA5 = 165
                register_addr=1
            elif [[ $register_decimal -eq 90 ]]; then  # 0x5A = 90
                register_addr=2
            else
                register_addr="未知"
            fi
            
            # 计算对应的点位ID
            if [[ $register_addr -eq 1 ]]; then
                point_id=$((bit_pos + 1))
            elif [[ $register_addr -eq 2 ]]; then
                point_id=$((bit_pos + 9))
            else
                point_id="未知"
            fi
            
            echo "点位$point_id -> 寄存器${register_addr}位${bit_pos} = ${bit_value} (寄存器值=$register)"
        fi
    done | sort -V

echo ""
echo "============================================================"
echo "注意：由于多次轮询，可能有重复的位解析日志"
echo "建议与期望值进行对比验证"
echo "============================================================"