#!/bin/bash

# Comsrv配置文件验证脚本
# 用于检查四遥点表和映射文件的正确性

set -e

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

CONFIG_DIR="${1:-services/comsrv/config}"
ERRORS=0
WARNINGS=0

echo "=== Comsrv配置文件验证 ==="
echo "配置目录: $CONFIG_DIR"
echo ""

# 检查必需文件是否存在
check_required_files() {
    local required_files=(
        "telemetry.csv"
        "signal.csv"
        "control.csv"
        "adjustment.csv"
        "telemetry_mapping.csv"
        "signal_mapping.csv"
        "control_mapping.csv"
        "adjustment_mapping.csv"
    )
    
    echo "1. 检查必需文件..."
    for file in "${required_files[@]}"; do
        if [ ! -f "$CONFIG_DIR/$file" ]; then
            echo -e "${RED}✗ 缺少文件: $file${NC}"
            ((ERRORS++))
        else
            echo -e "${GREEN}✓ $file${NC}"
        fi
    done
    echo ""
}

# 验证CSV文件格式
validate_csv_headers() {
    echo "2. 验证CSV文件头..."
    
    # 四遥点表必需的列
    local point_headers="point_id,signal_name,scale,offset,unit,reverse,data_type"
    
    # 映射文件必需的列（注意bit_position对bool类型是必需的）
    local mapping_headers="point_id,slave_id,function_code,register_address,data_type,byte_order"
    
    # 检查四遥点表
    for type in telemetry signal control adjustment; do
        if [ -f "$CONFIG_DIR/${type}.csv" ]; then
            header=$(head -1 "$CONFIG_DIR/${type}.csv")
            if [[ ! "$header" == *"point_id"* ]] || [[ ! "$header" == *"signal_name"* ]]; then
                echo -e "${RED}✗ ${type}.csv 缺少必需的列${NC}"
                ((ERRORS++))
            else
                echo -e "${GREEN}✓ ${type}.csv 格式正确${NC}"
            fi
        fi
    done
    
    # 检查映射文件
    for type in telemetry signal control adjustment; do
        if [ -f "$CONFIG_DIR/${type}_mapping.csv" ]; then
            header=$(head -1 "$CONFIG_DIR/${type}_mapping.csv")
            if [[ ! "$header" == *"point_id"* ]] || [[ ! "$header" == *"slave_id"* ]]; then
                echo -e "${RED}✗ ${type}_mapping.csv 缺少必需的列${NC}"
                ((ERRORS++))
            else
                # 对于signal和control，检查是否有bit_position列
                if [[ "$type" == "signal" ]] || [[ "$type" == "control" ]]; then
                    if [[ ! "$header" == *"bit_position"* ]]; then
                        echo -e "${YELLOW}⚠ ${type}_mapping.csv 建议添加bit_position列（对bool类型）${NC}"
                        ((WARNINGS++))
                    fi
                fi
                echo -e "${GREEN}✓ ${type}_mapping.csv 格式正确${NC}"
            fi
        fi
    done
    echo ""
}

# 验证数据内容
validate_data_content() {
    echo "3. 验证数据内容..."
    
    # 检查signal_mapping中的bit_position范围
    if [ -f "$CONFIG_DIR/signal_mapping.csv" ]; then
        while IFS=, read -r point_id slave_id fc addr dtype order bit_pos; do
            if [ ! -z "$bit_pos" ] && [ "$bit_pos" != "bit_position" ]; then
                if [ "$bit_pos" -lt 0 ] || [ "$bit_pos" -gt 15 ]; then
                    echo -e "${RED}✗ signal_mapping.csv: point_id=$point_id 的bit_position=$bit_pos 超出范围(0-15)${NC}"
                    ((ERRORS++))
                fi
            fi
        done < "$CONFIG_DIR/signal_mapping.csv"
    fi
    
    # 检查function_code的合法性
    for mapping_file in "$CONFIG_DIR"/*_mapping.csv; do
        if [ -f "$mapping_file" ]; then
            while IFS=, read -r point_id slave_id fc rest; do
                if [ "$fc" != "function_code" ] && [ ! -z "$fc" ]; then
                    case "$fc" in
                        1|2|3|4|5|6|15|16)
                            # 合法的function_code
                            ;;
                        *)
                            echo -e "${YELLOW}⚠ $(basename $mapping_file): point_id=$point_id 使用了不常见的function_code=$fc${NC}"
                            ((WARNINGS++))
                            ;;
                    esac
                fi
            done < "$mapping_file"
        fi
    done
    
    echo -e "${GREEN}✓ 数据内容验证完成${NC}"
    echo ""
}

# 检查点ID一致性
check_point_id_consistency() {
    echo "4. 检查点ID一致性..."
    
    for type in telemetry signal control adjustment; do
        if [ -f "$CONFIG_DIR/${type}.csv" ] && [ -f "$CONFIG_DIR/${type}_mapping.csv" ]; then
            # 获取点表中的point_id列表
            point_ids=$(tail -n +2 "$CONFIG_DIR/${type}.csv" | cut -d',' -f1 | sort -u)
            mapping_ids=$(tail -n +2 "$CONFIG_DIR/${type}_mapping.csv" | cut -d',' -f1 | sort -u)
            
            # 检查是否有映射文件中的ID在点表中不存在
            for id in $mapping_ids; do
                if ! echo "$point_ids" | grep -q "^$id$"; then
                    echo -e "${RED}✗ ${type}_mapping.csv 中的point_id=$id 在${type}.csv中不存在${NC}"
                    ((ERRORS++))
                fi
            done
            
            # 检查是否有点表中的ID在映射文件中不存在
            for id in $point_ids; do
                if ! echo "$mapping_ids" | grep -q "^$id$"; then
                    echo -e "${YELLOW}⚠ ${type}.csv 中的point_id=$id 在${type}_mapping.csv中没有映射${NC}"
                    ((WARNINGS++))
                fi
            done
        fi
    done
    
    echo -e "${GREEN}✓ 点ID一致性检查完成${NC}"
    echo ""
}

# 生成配置规范文档
generate_spec() {
    echo "5. 生成配置规范..."
    
    cat > "$CONFIG_DIR/CONFIG_SPEC.md" << 'EOF'
# Comsrv配置文件规范

## 文件结构

### 四遥点表文件
- `telemetry.csv` - 遥测点定义
- `signal.csv` - 遥信点定义
- `control.csv` - 遥控点定义
- `adjustment.csv` - 遥调点定义

**必需列**: point_id,signal_name,scale,offset,unit,reverse,data_type

### 协议映射文件
- `telemetry_mapping.csv` - 遥测映射
- `signal_mapping.csv` - 遥信映射
- `control_mapping.csv` - 遥控映射
- `adjustment_mapping.csv` - 遥调映射

**必需列**: point_id,slave_id,function_code,register_address,data_type,byte_order
**bool类型额外列**: bit_position (范围0-15)

## 数据规范

### bit_position
- 统一按16位寄存器处理
- 范围: 0-15
- 默认值: 0

### Function Code
- 1: 读线圈
- 2: 读离散输入
- 3: 读保持寄存器
- 4: 读输入寄存器
- 5: 写单个线圈
- 6: 写单个寄存器

### 数据类型
- bool: 1位
- int16/uint16: 2字节
- int32/uint32/float32: 4字节
- int64/uint64/float64: 8字节

### 字节序(byte_order)
- 16位: AB, BA
- 32位: ABCD, DCBA, BADC, CDAB
- 64位: ABCDEFGH等
EOF
    
    echo -e "${GREEN}✓ 已生成CONFIG_SPEC.md${NC}"
    echo ""
}

# 执行所有检查
check_required_files
validate_csv_headers
validate_data_content
check_point_id_consistency
generate_spec

# 总结
echo "=== 验证总结 ==="
if [ $ERRORS -eq 0 ]; then
    echo -e "${GREEN}✓ 所有检查通过！${NC}"
else
    echo -e "${RED}✗ 发现 $ERRORS 个错误${NC}"
fi

if [ $WARNINGS -gt 0 ]; then
    echo -e "${YELLOW}⚠ 发现 $WARNINGS 个警告${NC}"
fi

# 返回错误码
exit $ERRORS