#!/bin/bash
# VoltageEMS Quick Check Script

set -e

# Color definitions
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
RED='\033[0;31m'
NC='\033[0m'

echo -e "${GREEN}=== VoltageEMS Quick Check ===${NC}"

# Check compilation
echo -e "${YELLOW}Checking compilation...${NC}"
cargo check --workspace

# Format check
echo -e "${YELLOW}Checking code format...${NC}"
cargo fmt --all -- --check

# Clippy check
echo -e "${YELLOW}Running Clippy...${NC}"
cargo clippy --all-targets --all-features -- -D warnings

# Run unit tests (no external dependencies required)
echo -e "${YELLOW}Running unit tests...${NC}"
cargo test --workspace --lib

# Run integration tests (optional - requires Redis)
if [ "$1" = "--with-integration" ]; then
    echo -e "${YELLOW}Running integration tests...${NC}"
    cargo test --workspace --test '*'
else
    echo -e "${YELLOW}Skipping integration tests (use --with-integration to run)${NC}"
    echo -e "${YELLOW}Integration tests require Redis${NC}"
fi

echo -e "${GREEN}All checks passed!${NC}"