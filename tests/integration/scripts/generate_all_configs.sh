#!/bin/bash
# Generate all test configurations for different phases

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONFIGS_DIR="$(cd "${SCRIPT_DIR}/../configs" && pwd)"

echo "Generating test configurations for all phases..."

# Generate Phase 1 configs
echo ""
echo "=== Phase 1 ==="
python3 "${SCRIPT_DIR}/generate_test_configs.py" phase1 "${CONFIGS_DIR}/phase1"

# Generate Phase 2 configs
echo ""
echo "=== Phase 2 ==="
python3 "${SCRIPT_DIR}/generate_test_configs.py" phase2 "${CONFIGS_DIR}/phase2"

# Generate Phase 3 configs
echo ""
echo "=== Phase 3 ==="
python3 "${SCRIPT_DIR}/generate_test_configs.py" phase3 "${CONFIGS_DIR}/phase3"

echo ""
echo "✓ All configurations generated successfully!"
echo ""
echo "Configuration summary:"
echo "  Phase 1: $(find "${CONFIGS_DIR}/phase1" -name "*.csv" | wc -l) CSV files"
echo "  Phase 2: $(find "${CONFIGS_DIR}/phase2" -name "*.csv" | wc -l) CSV files"
echo "  Phase 3: $(find "${CONFIGS_DIR}/phase3" -name "*.csv" | wc -l) CSV files"