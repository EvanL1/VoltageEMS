#!/bin/bash

# Comsrv configuration file validation script
# Used to check the correctness of telemetry point tables and mapping files
# (Comsrv配置文件验证脚本 - 用于检查四遥点表和映射文件的正确性)

set -e

# Color output (颜色输出)
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

CONFIG_DIR="${1:-services/comsrv/config}"
ERRORS=0
WARNINGS=0

echo "=== Comsrv Configuration File Validation ==="
echo "Configuration directory: $CONFIG_DIR"
echo ""

# Check if required files exist (检查必需文件是否存在)
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
    
    echo "1. Checking required files..."
    for file in "${required_files[@]}"; do
        if [ ! -f "$CONFIG_DIR/$file" ]; then
            echo -e "${RED}✗ Missing file: $file${NC}"
            ((ERRORS++))
        else
            echo -e "${GREEN}✓ $file${NC}"
        fi
    done
    echo ""
}

# Validate CSV file format (验证CSV文件格式)
validate_csv_headers() {
    echo "2. Validating CSV headers..."
    
    # Required columns for telemetry point tables (四遥点表必需的列)
    local point_headers="point_id,signal_name,scale,offset,unit,reverse,data_type"
    
    # Required columns for mapping files (note: bit_position is required for bool types) (映射文件必需的列，注意bit_position对bool类型是必需的)
    local mapping_headers="point_id,slave_id,function_code,register_address,data_type,byte_order"
    
    # Check telemetry point tables (检查四遥点表)
    for type in telemetry signal control adjustment; do
        if [ -f "$CONFIG_DIR/${type}.csv" ]; then
            header=$(head -1 "$CONFIG_DIR/${type}.csv")
            if [[ ! "$header" == *"point_id"* ]] || [[ ! "$header" == *"signal_name"* ]]; then
                echo -e "${RED}✗ ${type}.csv missing required columns${NC}"
                ((ERRORS++))
            else
                echo -e "${GREEN}✓ ${type}.csv format correct${NC}"
            fi
        fi
    done
    
    # Check mapping files (检查映射文件)
    for type in telemetry signal control adjustment; do
        if [ -f "$CONFIG_DIR/${type}_mapping.csv" ]; then
            header=$(head -1 "$CONFIG_DIR/${type}_mapping.csv")
            if [[ ! "$header" == *"point_id"* ]] || [[ ! "$header" == *"slave_id"* ]]; then
                echo -e "${RED}✗ ${type}_mapping.csv missing required columns${NC}"
                ((ERRORS++))
            else
                # For signal and control, check for bit_position column (对于signal咍control，检查是否有bit_position列)
                if [[ "$type" == "signal" ]] || [[ "$type" == "control" ]]; then
                    if [[ ! "$header" == *"bit_position"* ]]; then
                        echo -e "${YELLOW}⚠ ${type}_mapping.csv should have bit_position column (for bool types)${NC}"
                        ((WARNINGS++))
                    fi
                fi
                echo -e "${GREEN}✓ ${type}_mapping.csv format correct${NC}"
            fi
        fi
    done
    echo ""
}

# Validate data content (验证数据内容)
validate_data_content() {
    echo "3. Validating data content..."
    
    # Check bit_position range in signal_mapping (检查signal_mapping中的bit_position范围)
    if [ -f "$CONFIG_DIR/signal_mapping.csv" ]; then
        while IFS=, read -r point_id slave_id fc addr dtype order bit_pos; do
            if [ ! -z "$bit_pos" ] && [ "$bit_pos" != "bit_position" ]; then
                if [ "$bit_pos" -lt 0 ] || [ "$bit_pos" -gt 15 ]; then
                    echo -e "${RED}✗ signal_mapping.csv: point_id=$point_id bit_position=$bit_pos out of range(0-15)${NC}"
                    ((ERRORS++))
                fi
            fi
        done < "$CONFIG_DIR/signal_mapping.csv"
    fi
    
    # Check function_code validity (检查function_code的合法性)
    for mapping_file in "$CONFIG_DIR"/*_mapping.csv; do
        if [ -f "$mapping_file" ]; then
            while IFS=, read -r point_id slave_id fc rest; do
                if [ "$fc" != "function_code" ] && [ ! -z "$fc" ]; then
                    case "$fc" in
                        1|2|3|4|5|6|15|16)
                            # Valid function_code (合法的function_code)
                            ;;
                        *)
                            echo -e "${YELLOW}⚠ $(basename $mapping_file): point_id=$point_id uses uncommon function_code=$fc${NC}"
                            ((WARNINGS++))
                            ;;
                    esac
                fi
            done < "$mapping_file"
        fi
    done
    
    echo -e "${GREEN}✓ Data content validation completed${NC}"
    echo ""
}

# Check point ID consistency (检查点ID一致性)
check_point_id_consistency() {
    echo "4. Checking point ID consistency..."
    
    for type in telemetry signal control adjustment; do
        if [ -f "$CONFIG_DIR/${type}.csv" ] && [ -f "$CONFIG_DIR/${type}_mapping.csv" ]; then
            # Get point_id list from point tables (获取点表中的point_id列表)
            point_ids=$(tail -n +2 "$CONFIG_DIR/${type}.csv" | cut -d',' -f1 | sort -u)
            mapping_ids=$(tail -n +2 "$CONFIG_DIR/${type}_mapping.csv" | cut -d',' -f1 | sort -u)
            
            # Check if any mapping file IDs don't exist in point tables (检查是否有映射文件中的ID在点表中不存在)
            for id in $mapping_ids; do
                if ! echo "$point_ids" | grep -q "^$id$"; then
                    echo -e "${RED}✗ ${type}_mapping.csv point_id=$id not found in ${type}.csv${NC}"
                    ((ERRORS++))
                fi
            done
            
            # Check if any point table IDs don't exist in mapping files (检查是否有点表中的ID在映射文件中不存在)
            for id in $point_ids; do
                if ! echo "$mapping_ids" | grep -q "^$id$"; then
                    echo -e "${YELLOW}⚠ ${type}.csv point_id=$id has no mapping in ${type}_mapping.csv${NC}"
                    ((WARNINGS++))
                fi
            done
        fi
    done
    
    echo -e "${GREEN}✓ Point ID consistency check completed${NC}"
    echo ""
}

# Generate configuration specification document (生成配置规范文档)
generate_spec() {
    echo "5. Generating configuration specification..."
    
    cat > "$CONFIG_DIR/CONFIG_SPEC.md" << 'EOF'
# Comsrv Configuration File Specification

## File Structure

### Telemetry Point Table Files
- `telemetry.csv` - Telemetry point definitions
- `signal.csv` - Signal point definitions
- `control.csv` - Control point definitions
- `adjustment.csv` - Adjustment point definitions

**Required columns**: point_id,signal_name,scale,offset,unit,reverse,data_type

### Protocol Mapping Files
- `telemetry_mapping.csv` - Telemetry mapping
- `signal_mapping.csv` - Signal mapping
- `control_mapping.csv` - Control mapping
- `adjustment_mapping.csv` - Adjustment mapping

**Required columns**: point_id,slave_id,function_code,register_address,data_type,byte_order
**Additional column for bool types**: bit_position (range 0-15)

## Data Specification

### bit_position
- Uniformly handled as 16-bit registers
- Range: 0-15
- Default value: 0

### Function Code
- 1: Read coils
- 2: Read discrete inputs
- 3: Read holding registers
- 4: Read input registers
- 5: Write single coil
- 6: Write single register

### Data Types
- bool: 1 bit
- int16/uint16: 2 bytes
- int32/uint32/float32: 4 bytes
- int64/uint64/float64: 8 bytes

### Byte Order
- 16-bit: AB, BA
- 32-bit: ABCD, DCBA, BADC, CDAB
- 64-bit: ABCDEFGH, etc.
EOF
    
    echo -e "${GREEN}✓ CONFIG_SPEC.md generated${NC}"
    echo ""
}

# Execute all checks (执行所有检查)
check_required_files
validate_csv_headers
validate_data_content
check_point_id_consistency
generate_spec

# Summary (总结)
echo "=== Validation Summary ==="
if [ $ERRORS -eq 0 ]; then
    echo -e "${GREEN}✓ All checks passed!${NC}"
else
    echo -e "${RED}✗ Found $ERRORS error(s)${NC}"
fi

if [ $WARNINGS -gt 0 ]; then
    echo -e "${YELLOW}⚠ Found $WARNINGS warning(s)${NC}"
fi

# Return error code (返回错误码)
exit $ERRORS