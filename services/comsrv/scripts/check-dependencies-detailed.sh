#!/bin/bash

# Detailed dependency usage check script
# This script checks both regular and dev dependencies

echo "详细依赖使用情况检查..."
echo "====================="

# Function to check dependency usage
check_dependency() {
    local dep=$1
    local is_dev=$2
    local rust_dep=$(echo "$dep" | sed 's/-/_/g')
    
    echo -n "检查 $dep"
    if [ "$is_dev" = "true" ]; then
        echo -n " (dev)"
    fi
    echo -n " ... "
    
    # Search for usage patterns
    local usage_count=0
    local usage_files=""
    
    # Check for use statements
    if grep -r -q "use.*${rust_dep}" src/ 2>/dev/null; then
        usage_count=$((usage_count + 1))
        usage_files="${usage_files}$(grep -r -l "use.*${rust_dep}" src/ 2>/dev/null | head -3 | tr '\n' ' ')"
    fi
    
    # Check for extern crate
    if grep -r -q "extern crate ${rust_dep}" src/ 2>/dev/null; then
        usage_count=$((usage_count + 1))
        usage_files="${usage_files}$(grep -r -l "extern crate ${rust_dep}" src/ 2>/dev/null | head -3 | tr '\n' ' ')"
    fi
    
    # Check for direct usage (e.g., crate::method)
    if grep -r -q "${rust_dep}::" src/ 2>/dev/null; then
        usage_count=$((usage_count + 1))
        usage_files="${usage_files}$(grep -r -l "${rust_dep}::" src/ 2>/dev/null | head -3 | tr '\n' ' ')"
    fi
    
    # Check for attribute usage (e.g., #[test])
    if grep -r -q "#\[${rust_dep}" src/ 2>/dev/null; then
        usage_count=$((usage_count + 1))
        usage_files="${usage_files}$(grep -r -l "#\[${rust_dep}" src/ 2>/dev/null | head -3 | tr '\n' ' ')"
    fi
    
    # Check Cargo.toml for feature dependencies
    local is_optional=$(grep -E "${dep}.*optional = true" Cargo.toml)
    
    if [ $usage_count -gt 0 ]; then
        echo -e "\033[0;32m✓ 使用\033[0m"
        echo "  位置: $usage_files"
    elif [ -n "$is_optional" ]; then
        echo -e "\033[1;33m⚠ 未使用 (可选)\033[0m"
    else
        echo -e "\033[0;31m✗ 未使用\033[0m"
    fi
}

echo ""
echo "=== 常规依赖 ==="
echo ""

# Regular dependencies
deps=(
    "tokio"
    "tracing"
    "tracing-subscriber"
    "tracing-appender"
    "serde"
    "serde_yaml"
    "serde_json"
    "csv"
    "thiserror"
    "anyhow"
    "tokio-serial"
    "socket2"
    "axum"
    "utoipa"
    "chrono"
    "dashmap"
    "ahash"
    "hex"
    "async-trait"
    "futures"
    "dotenv"
    "clap"
    "redis"
    "byteorder"
    "parking_lot"
    "bytes"
    "once_cell"
    "figment"
    "tempfile"
    "reqwest"
    "semver"
    "regex"
    "rppal"
    "i2cdev"
    "spidev"
    "socketcan"
)

for dep in "${deps[@]}"; do
    check_dependency "$dep" false
done

echo ""
echo "=== 开发依赖 ==="
echo ""

# Dev dependencies
dev_deps=(
    "tokio-test"
    "mockall"
    "tempfile"
    "criterion"
    "async-trait"
    "tracing-test"
)

for dep in "${dev_deps[@]}"; do
    check_dependency "$dep" true
done

echo ""
echo "=== 依赖使用建议 ==="
echo ""

# Check for hex usage opportunity
echo -n "Hex 格式化检查: "
hex_format_count=$(grep -r -E "format!.*:02[xX]|format!.*:04[xX]" src/ | wc -l)
if [ $hex_format_count -gt 0 ]; then
    echo -e "\033[1;33m发现 $hex_format_count 处十六进制格式化，可以考虑使用 hex 库优化\033[0m"
    echo "  示例位置:"
    grep -r -n -E "format!.*:02[xX]|format!.*:04[xX]" src/ | head -3 | sed 's/^/    /'
else
    echo -e "\033[0;32m未发现需要优化的十六进制格式化\033[0m"
fi

echo ""
echo "====================="
echo "检查完成！"