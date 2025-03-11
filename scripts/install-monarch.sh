#!/bin/bash
#
# Install Monarch configuration management tool
# This script builds and installs Monarch for local or CI use

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Determine project root (where Cargo.toml with [workspace] exists)
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_ROOT="$( cd "$SCRIPT_DIR/.." && pwd )"

echo -e "${GREEN}Installing Monarch Configuration Manager...${NC}"

# Change to project root
cd "$PROJECT_ROOT"

# Check if Cargo.toml exists
if [ ! -f "Cargo.toml" ]; then
    echo -e "${RED}Error: Cargo.toml not found in project root${NC}"
    exit 1
fi

# Build Monarch in release mode
echo -e "${YELLOW}Building Monarch...${NC}"
if cargo build --release --package monarch; then
    echo -e "${GREEN}✓ Monarch built successfully${NC}"
else
    echo -e "${RED}✗ Failed to build Monarch${NC}"
    exit 1
fi

# Determine installation path
if [ -n "$1" ]; then
    # Custom installation path provided
    INSTALL_PATH="$1"
elif [ -n "$CI" ]; then
    # Running in CI, install to a location in PATH
    INSTALL_PATH="/usr/local/bin"
else
    # Local development, create symlink in project bin directory
    INSTALL_PATH="$PROJECT_ROOT/bin"
    mkdir -p "$INSTALL_PATH"
fi

# Copy or link the binary
MONARCH_BINARY="$PROJECT_ROOT/target/release/monarch"
if [ ! -f "$MONARCH_BINARY" ]; then
    echo -e "${RED}Error: Monarch binary not found at $MONARCH_BINARY${NC}"
    exit 1
fi

# Install the binary
if [ "$INSTALL_PATH" = "/usr/local/bin" ] && [ -n "$CI" ]; then
    # In CI, need sudo to install to system path
    echo -e "${YELLOW}Installing to system path (requires sudo in CI)...${NC}"
    sudo cp "$MONARCH_BINARY" "$INSTALL_PATH/monarch"
    sudo chmod +x "$INSTALL_PATH/monarch"
elif [ -w "$INSTALL_PATH" ]; then
    # Local installation
    echo -e "${YELLOW}Installing to $INSTALL_PATH...${NC}"
    cp "$MONARCH_BINARY" "$INSTALL_PATH/monarch"
    chmod +x "$INSTALL_PATH/monarch"
else
    echo -e "${RED}Error: Cannot write to $INSTALL_PATH${NC}"
    echo -e "${YELLOW}Try running with sudo or specify a different path${NC}"
    exit 1
fi

# Verify installation
if command -v monarch &> /dev/null; then
    echo -e "${GREEN}✓ Monarch installed successfully${NC}"
    echo -e "${GREEN}Version: $(monarch --version 2>&1 | head -n1)${NC}"
elif [ -f "$INSTALL_PATH/monarch" ]; then
    echo -e "${GREEN}✓ Monarch installed to $INSTALL_PATH${NC}"
    echo -e "${YELLOW}Note: Add $INSTALL_PATH to your PATH if needed${NC}"
else
    echo -e "${RED}✗ Installation verification failed${NC}"
    exit 1
fi

echo -e "${GREEN}Installation complete!${NC}"
echo -e "${GREEN}Usage: monarch --help${NC}"