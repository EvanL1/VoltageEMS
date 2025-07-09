#!/bin/bash

# Check dependency usage script
# This script checks if each dependency in Cargo.toml is actually used in the source code

echo "检查依赖使用情况..."
echo "====================="

# List of dependencies to check (from Cargo.toml)
dependencies=(
    "tokio"
    "tracing"
    "tracing_subscriber"
    "tracing_appender"
    "serde"
    "serde_yaml"
    "serde_json"
    "csv"
    "thiserror"
    "anyhow"
    "tokio_serial"
    "socket2"
    "axum"
    "utoipa"
    "utoipa_swagger_ui"
    "tower"
    "tower_http"
    "chrono"
    "dashmap"
    "ahash"
    "hex"
    "async_trait"
    "futures"
    "dotenv"
    "clap"
    "redis"
    "byteorder"
    "parking_lot"
    "bytes"
    "tokio_util"
    "once_cell"
    "rand"
    "figment"
    "tempfile"
    "reqwest"
    "tokio_tungstenite"
    "futures_util"
    "uuid"
    "semver"
    "regex"
    "rppal"
    "gpio_cdev"
    "i2cdev"
    "spidev"
    "socketcan"
)

# Color codes
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Check each dependency
for dep in "${dependencies[@]}"; do
    # Convert dependency name to the format used in Rust code
    # Replace - with _ for use statements
    rust_dep=$(echo "$dep" | sed 's/-/_/g')
    
    echo -n "检查 $dep ... "
    
    # Search for the dependency in source files
    # Look for: use statements, extern crate, or direct usage
    if grep -r -q -E "(use.*${rust_dep}|extern crate ${rust_dep}|${rust_dep}::)" src/ 2>/dev/null; then
        echo -e "${GREEN}✓ 使用${NC}"
        # Show where it's used (first 3 occurrences)
        echo "  使用位置:"
        grep -r -n -E "(use.*${rust_dep}|extern crate ${rust_dep}|${rust_dep}::)" src/ 2>/dev/null | head -3 | sed 's/^/    /'
    else
        # Check if it's an optional dependency
        if grep -q "${dep}.*optional = true" Cargo.toml; then
            echo -e "${YELLOW}⚠ 未使用 (可选依赖)${NC}"
        else
            echo -e "${RED}✗ 未使用${NC}"
        fi
    fi
    echo
done

echo "====================="
echo "检查完成！"
echo ""
echo "说明："
echo -e "${GREEN}✓${NC} - 依赖被使用"
echo -e "${RED}✗${NC} - 依赖未被使用（可以考虑移除）"
echo -e "${YELLOW}⚠${NC} - 可选依赖未被使用（这是正常的）"