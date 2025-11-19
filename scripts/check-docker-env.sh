#!/bin/bash
# 验证Docker环境配置 / Validate Docker environment configuration

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}🔍 VoltageEMS Docker Environment Check${NC}"
echo ""

# 检查Docker Compose / Check Docker Compose
echo "1. Docker Compose version:"
if docker compose version &>/dev/null 2>&1; then
    docker compose version | head -1
    COMPOSE_OK=true
elif command -v docker-compose &>/dev/null; then
    docker-compose --version
    echo -e "${YELLOW}  ⚠️  Warning: using Docker Compose V1 (deprecated); please upgrade to V2${NC}"
    COMPOSE_OK=true
else
    echo -e "${RED}  ❌ Docker Compose is not installed${NC}"
    COMPOSE_OK=false
fi
echo ""

# 检查.env文件 / Check .env file
echo "2. UID/GID configuration:"
if [ -f .env ]; then
    source .env
    echo "  HOST_UID=$HOST_UID"
    echo "  HOST_GID=$HOST_GID"

    CURRENT_UID=$(id -u)
    CURRENT_GID=$(id -g)

    if [ "$HOST_UID" != "$CURRENT_UID" ]; then
        echo -e "${YELLOW}  ⚠️  Warning: configured UID ($HOST_UID) does not match current user UID ($CURRENT_UID)${NC}"
        echo -e "${YELLOW}  Suggestion: run ./scripts/install.sh to regenerate configuration${NC}"
    else
        echo -e "${GREEN}  ✅ UID matches${NC}"
    fi

    if [ "$HOST_GID" != "$CURRENT_GID" ]; then
        echo -e "${BLUE}  ℹ️  Configured GID ($HOST_GID) is different from current primary group GID ($CURRENT_GID)${NC}"
        echo -e "${BLUE}     (This may be expected, e.g., when using the docker group on Linux)${NC}"
    else
        echo -e "${GREEN}  ✅ GID matches${NC}"
    fi
else
    echo -e "${RED}  ❌ .env file does not exist${NC}"
    echo -e "${YELLOW}  Run ./scripts/install.sh to generate it automatically${NC}"
fi
echo ""

# 检查Docker卷权限 / Check Docker volume status
echo "3. Docker volume status:"
if docker volume inspect voltageems_data &>/dev/null; then
    echo -e "${GREEN}  ✅ Docker volume 'voltageems_data' exists${NC}"
else
    echo -e "${YELLOW}  ⚠️  Docker volume 'voltageems_data' does not exist (it will be created automatically on first deploy)${NC}"
fi

echo ""
if [ "$COMPOSE_OK" = true ] && [ -f .env ]; then
    echo -e "${GREEN}✅ Environment check passed${NC}"
    exit 0
else
    echo -e "${RED}❌ Environment check failed, please fix the issues above${NC}"
    exit 1
fi
