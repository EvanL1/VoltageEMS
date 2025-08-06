#!/bin/bash

# Claude Code Hook: Validate comsrv configuration
# This hook runs automatically when comsrv config files are modified

# Check if the tool operation involves comsrv config files
if [[ "$CLAUDE_TOOL_NAME" == "Write" ]] || [[ "$CLAUDE_TOOL_NAME" == "Edit" ]] || [[ "$CLAUDE_TOOL_NAME" == "MultiEdit" ]]; then
    # Extract file path from tool parameters
    FILE_PATH=$(echo "$CLAUDE_TOOL_PARAMS" | grep -oE '"file_path":\s*"[^"]*"' | sed 's/.*"file_path":\s*"\([^"]*\)".*/\1/')
    
    # Check if it's a comsrv config file
    if [[ "$FILE_PATH" == *"services/comsrv/config"*.csv ]]; then
        echo "🔍 Validating comsrv configuration file: $(basename $FILE_PATH)"
        
        # Check for bit_position in bool mappings
        if [[ "$FILE_PATH" == *"signal_mapping.csv" ]] || [[ "$FILE_PATH" == *"control_mapping.csv" ]]; then
            # Check if bit_position column exists
            if ! head -1 "$CLAUDE_PROJECT_DIR/$FILE_PATH" 2>/dev/null | grep -q "bit_position"; then
                echo "⚠️  Warning: bit_position column missing in $(basename $FILE_PATH)"
                echo "   Bool type mappings should include bit_position (0-15)"
            fi
            
            # Validate bit_position range if column exists
            if head -1 "$CLAUDE_PROJECT_DIR/$FILE_PATH" 2>/dev/null | grep -q "bit_position"; then
                while IFS=, read -r line; do
                    # Extract bit_position value (assuming it's the last column)
                    bit_pos=$(echo "$line" | awk -F, '{print $NF}')
                    if [[ "$bit_pos" =~ ^[0-9]+$ ]]; then
                        if [ "$bit_pos" -lt 0 ] || [ "$bit_pos" -gt 15 ]; then
                            echo "❌ Error: bit_position=$bit_pos is out of range (0-15)"
                            exit 1
                        fi
                    fi
                done < <(tail -n +2 "$CLAUDE_PROJECT_DIR/$FILE_PATH" 2>/dev/null)
            fi
        fi
        
        # Check for slave_id in YAML (should not be there)
        if [[ "$FILE_PATH" == *"comsrv.yaml" ]]; then
            if grep -q "slave_id:" "$CLAUDE_PROJECT_DIR/$FILE_PATH" 2>/dev/null; then
                echo "❌ Error: slave_id should be in mapping CSV files, not in YAML"
                exit 1
            fi
        fi
        
        echo "✅ Comsrv configuration validation passed"
    fi
fi

exit 0