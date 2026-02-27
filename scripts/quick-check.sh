#!/bin/bash
# VoltageEMS Quick Check Script

set -e

# Color definitions
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
RED='\033[0;31m'
NC='\033[0m'

echo -e "${GREEN}=== VoltageEMS Quick Check ===${NC}"

# Sync git submodules (e.g. product-lib)
echo -e "${YELLOW}Syncing git submodules...${NC}"
git submodule update --init --recursive

# Check for forbidden mod.rs files (project convention)
echo -e "${YELLOW}Checking for mod.rs files...${NC}"
MOD_RS_FILES=$(find . -name "mod.rs" -not -path "./target/*" 2>/dev/null || true)
if [ -n "$MOD_RS_FILES" ]; then
    echo -e "${RED}ERROR: mod.rs files are forbidden (project convention)${NC}"
    echo "$MOD_RS_FILES"
    exit 1
fi
echo -e "${GREEN}No mod.rs files found${NC}"

# Check compilation
echo -e "${YELLOW}Checking compilation...${NC}"
cargo check --workspace

# Format check
echo -e "${YELLOW}Checking code format...${NC}"
cargo fmt --all -- --check

# Clippy check (all features enabled)
echo -e "${YELLOW}Running Clippy...${NC}"
cargo clippy --all-targets --all-features -- -D warnings

# Run unit tests (no external dependencies required)
echo -e "${YELLOW}Running unit tests...${NC}"
cargo test --workspace --lib

# Check command line arguments
RUN_INTEGRATION=false
RUN_COVERAGE=false

for arg in "$@"; do
    case $arg in
        --with-integration)
            RUN_INTEGRATION=true
            ;;
        --with-coverage)
            RUN_COVERAGE=true
            ;;
    esac
done

# Run integration tests (optional - requires Redis)
if [ "$RUN_INTEGRATION" = true ]; then
    echo -e "${YELLOW}Running integration tests...${NC}"
    cargo test --workspace --test '*'
else
    echo -e "${YELLOW}Skipping integration tests (use --with-integration to run)${NC}"
    echo -e "${YELLOW}Integration tests require Redis${NC}"
fi

# Run coverage analysis (optional)
if [ "$RUN_COVERAGE" = true ]; then
    echo -e "${YELLOW}Running coverage analysis...${NC}"
    if command -v cargo-llvm-cov &> /dev/null; then
        cargo llvm-cov --workspace --lib --bins
    else
        echo -e "${RED}cargo-llvm-cov not installed. Run: cargo install cargo-llvm-cov${NC}"
    fi
fi

# Check Frontend (Vue.js)
echo -e "${YELLOW}Checking Frontend (Vue.js)...${NC}"
if [[ -d "apps" ]]; then
    if [[ -f "apps/package.json" ]]; then
        echo -e "${GREEN}✓ Frontend directory and package.json found${NC}"
        
        # Check for required frontend files
        if [[ -f "apps/Dockerfile" ]]; then
            echo -e "${GREEN}✓ Frontend Dockerfile found${NC}"
        else
            echo -e "${YELLOW}⚠ Frontend Dockerfile not found${NC}"
        fi
        
        if [[ -f "apps/nginx.conf" ]]; then
            echo -e "${GREEN}✓ Frontend nginx.conf found${NC}"
        else
            echo -e "${YELLOW}⚠ Frontend nginx.conf not found${NC}"
        fi
        
        if [[ -f "apps/vite.config.ts" ]] || [[ -f "apps/vite.config.js" ]]; then
            echo -e "${GREEN}✓ Frontend Vite config found${NC}"
        else
            echo -e "${YELLOW}⚠ Frontend Vite config not found${NC}"
        fi
    else
        echo -e "${YELLOW}⚠ Frontend package.json not found${NC}"
    fi
else
    echo -e "${YELLOW}⚠ Frontend directory (apps) not found${NC}"
fi

echo -e "${GREEN}All checks passed!${NC}"