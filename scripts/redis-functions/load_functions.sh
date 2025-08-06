#!/bin/bash

# VoltageEMS Redis Functions Unified Loading Script

echo "=== Loading VoltageEMS Redis Functions ==="

# Color definitions
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[0;33m'
NC='\033[0m' # No Color

# Redis connection configuration
REDIS_CLI="${REDIS_CLI:-redis-cli}"
REDIS_HOST="${REDIS_HOST:-localhost}"
REDIS_PORT="${REDIS_PORT:-6379}"

# Function files directory
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# Check Redis connection
echo -n "Checking Redis connection ($REDIS_HOST:$REDIS_PORT)... "
if $REDIS_CLI -h $REDIS_HOST -p $REDIS_PORT ping > /dev/null 2>&1; then
    printf "${GREEN}Success${NC}\n"
else
    printf "${RED}Failed${NC}\n"
    echo "Please ensure Redis is running"
    exit 1
fi

echo ""

# All Lua files to load (in dependency order)
LUA_FILES=(
    # Common functions
    "core.lua"
    "specific.lua"
    "domain.lua"
    "services.lua"
    # Generic sync engine (new)
    "sync_engine.lua"
    "sync_config_init.lua"
    # Service functions
    "rulesrv.lua"
    "modsrv.lua"
    "alarmsrv.lua"
    "hissrv.lua"
)

# Load all functions
SUCCESS_COUNT=0
FAIL_COUNT=0

for lua_file in "${LUA_FILES[@]}"; do
    if [ -f "$SCRIPT_DIR/$lua_file" ]; then
        echo -n "Loading $lua_file... "
        if cat "$SCRIPT_DIR/$lua_file" | $REDIS_CLI -h $REDIS_HOST -p $REDIS_PORT -x FUNCTION LOAD REPLACE > /dev/null 2>&1; then
            printf "${GREEN}Success${NC}\n"
            ((SUCCESS_COUNT++))
        else
            printf "${RED}Failed${NC}\n"
            echo "Error message:"
            cat "$SCRIPT_DIR/$lua_file" | $REDIS_CLI -h $REDIS_HOST -p $REDIS_PORT -x FUNCTION LOAD REPLACE
            ((FAIL_COUNT++))
        fi
    else
        printf "${YELLOW}Not found: $lua_file${NC}\n"
        ((FAIL_COUNT++))
    fi
done

# Show loading results
echo ""
echo "=== Loading Results ==="
printf "${GREEN}Success: $SUCCESS_COUNT${NC}\n"
if [ $FAIL_COUNT -gt 0 ]; then
    printf "${RED}Failed: $FAIL_COUNT${NC}\n"
else
    printf "${GREEN}All loaded successfully!${NC}\n"
fi

# List loaded function libraries
echo ""
echo "=== Loaded Function Libraries ==="
$REDIS_CLI -h $REDIS_HOST -p $REDIS_PORT FUNCTION LIST | grep -E "library_name|functions" | head -20

# Show available functions statistics
echo ""
echo "=== Function Statistics ==="
TOTAL_FUNCTIONS=$($REDIS_CLI -h $REDIS_HOST -p $REDIS_PORT FUNCTION LIST | grep -c "name")
echo "Total functions: $TOTAL_FUNCTIONS"

# Initialize sync configurations if available
INIT_SYNC=${INIT_SYNC:-true}
if [ "$INIT_SYNC" = "true" ] && [ $FAIL_COUNT -eq 0 ]; then
    echo ""
    echo "=== Initializing Sync Configurations ==="
    INIT_RESULT=$($REDIS_CLI -h $REDIS_HOST -p $REDIS_PORT FCALL init_sync_configs 0 2>&1 || echo "not available")
    
    if [[ "$INIT_RESULT" == *"success"* ]]; then
        printf "${GREEN}Sync configurations initialized successfully${NC}\n"
        # Show initialized rules
        echo "Initialized sync rules:"
        $REDIS_CLI -h $REDIS_HOST -p $REDIS_PORT SMEMBERS sync:rules 2>/dev/null | head -10
    elif [[ "$INIT_RESULT" == *"not available"* ]]; then
        printf "${YELLOW}Sync configuration not available (optional)${NC}\n"
    else
        printf "${YELLOW}Sync configuration initialization partial${NC}\n"
    fi
fi

exit $FAIL_COUNT