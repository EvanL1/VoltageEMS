#!/usr/bin/env bash
# EMS Console Launcher Script
# Provides environment checking and helpful error messages for remote SSH usage

set -euo pipefail

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}================================${NC}"
echo -e "${BLUE}  VoltageEMS Console Launcher  ${NC}"
echo -e "${BLUE}================================${NC}"
echo ""

# Check 1: Display environment
if [[ -z "${DISPLAY:-}" ]]; then
    echo -e "${RED}Error: No DISPLAY environment variable set${NC}"
    echo ""
    echo "EMS Console requires a graphical environment to run."
    echo ""
    echo -e "${YELLOW}For SSH remote access, use X11 forwarding:${NC}"
    echo "  ssh -X user@host"
    echo "  ssh -Y user@host  (trusted X11 forwarding)"
    echo ""
    echo -e "${YELLOW}Or configure your SSH client:${NC}"
    echo "  Add to ~/.ssh/config:"
    echo "    Host yourhost"
    echo "      ForwardX11 yes"
    echo "      ForwardX11Trusted yes"
    echo ""
    echo -e "${YELLOW}On the server side, ensure:${NC}"
    echo "  - X11Forwarding yes in /etc/ssh/sshd_config"
    echo "  - xauth is installed: sudo apt-get install xauth"
    echo ""
    exit 1
fi

echo -e "${GREEN}✓ Display environment: $DISPLAY${NC}"

# Check 2: Redis connection
REDIS_URL="${REDIS_URL:-redis://127.0.0.1:6379}"
echo -e "${YELLOW}Checking Redis connection...${NC}"

if command -v redis-cli &>/dev/null; then
    # Extract host and port from REDIS_URL
    REDIS_HOST=$(echo "$REDIS_URL" | sed -E 's|redis://([^:]+):.*|\1|')
    REDIS_PORT=$(echo "$REDIS_URL" | sed -E 's|redis://[^:]+:([0-9]+).*|\1|')

    if redis-cli -h "$REDIS_HOST" -p "$REDIS_PORT" ping &>/dev/null; then
        echo -e "${GREEN}✓ Redis connection OK${NC}"
    else
        echo -e "${YELLOW}Warning: Cannot connect to Redis at $REDIS_URL${NC}"
        echo "  Console will start but data may not be available"
    fi
else
    echo -e "${YELLOW}Note: redis-cli not found, skipping Redis check${NC}"
fi

# Check 3: Database file
VOLTAGE_DB_PATH="${VOLTAGE_DB_PATH:-data/voltage.db}"
echo -e "${YELLOW}Checking database...${NC}"

if [[ -f "$VOLTAGE_DB_PATH" ]]; then
    echo -e "${GREEN}✓ Database found: $VOLTAGE_DB_PATH${NC}"
elif [[ -f "/opt/MonarchEdge/data/voltage.db" ]]; then
    export VOLTAGE_DB_PATH="/opt/MonarchEdge/data/voltage.db"
    echo -e "${GREEN}✓ Database found: $VOLTAGE_DB_PATH${NC}"
else
    echo -e "${YELLOW}Warning: Database not found at $VOLTAGE_DB_PATH${NC}"
    echo "  Console will start but channel list may be empty"
    echo "  Run: monarch init all && monarch sync all"
fi

# Check 4: X11 libraries (optional check)
if command -v ldconfig &>/dev/null; then
    if ldconfig -p | grep -q libX11; then
        echo -e "${GREEN}✓ X11 libraries available${NC}"
    else
        echo -e "${YELLOW}Warning: X11 libraries not found${NC}"
        echo "  Install with: sudo apt-get install libx11-6 libxcursor1 libxrandr2"
    fi
fi

echo ""
echo -e "${GREEN}Starting EMS Console...${NC}"
echo ""

# Export environment variables
export RUST_LOG="${RUST_LOG:-info}"
export REDIS_URL="$REDIS_URL"
export VOLTAGE_DB_PATH="$VOLTAGE_DB_PATH"

# Find and execute ems-console
if command -v ems-console &>/dev/null; then
    exec ems-console "$@"
elif [[ -f "/usr/local/bin/ems-console" ]]; then
    exec /usr/local/bin/ems-console "$@"
else
    echo -e "${RED}Error: ems-console binary not found${NC}"
    echo "  Expected location: /usr/local/bin/ems-console"
    echo "  Please reinstall or check PATH"
    exit 1
fi
