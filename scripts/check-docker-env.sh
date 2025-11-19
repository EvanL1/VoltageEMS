#!/bin/bash
# 验证Docker环境配置

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
BLUE='\033[0;34m'
NC='\033[0m'

echo -e "${BLUE}🔍 VoltageEMS Docker环境检查${NC}"
echo ""

# 检查Docker Compose
echo "1. Docker Compose 版本："
if docker compose version &>/dev/null 2>&1; then
    docker compose version | head -1
    COMPOSE_OK=true
elif command -v docker-compose &>/dev/null; then
    docker-compose --version
    echo -e "${YELLOW}  ⚠️  警告：使用V1（已废弃），建议升级到V2${NC}"
    COMPOSE_OK=true
else
    echo -e "${RED}  ❌ 未安装Docker Compose${NC}"
    COMPOSE_OK=false
fi
echo ""

# 检查.env文件
echo "2. UID/GID 配置："
if [ -f .env ]; then
    source .env
    echo "  HOST_UID=$HOST_UID"
    echo "  HOST_GID=$HOST_GID"

    CURRENT_UID=$(id -u)
    CURRENT_GID=$(id -g)

    if [ "$HOST_UID" != "$CURRENT_UID" ]; then
        echo -e "${YELLOW}  ⚠️  警告：配置的UID ($HOST_UID) 与当前用户UID ($CURRENT_UID) 不一致${NC}"
        echo -e "${YELLOW}  建议运行: ./scripts/install.sh 重新生成配置${NC}"
    else
        echo -e "${GREEN}  ✅ UID匹配${NC}"
    fi

    if [ "$HOST_GID" != "$CURRENT_GID" ]; then
        echo -e "${BLUE}  ℹ️  配置的GID ($HOST_GID) 与当前用户主组GID ($CURRENT_GID) 不同${NC}"
        echo -e "${BLUE}     （这可能是正常的，如Linux上使用docker组）${NC}"
    else
        echo -e "${GREEN}  ✅ GID匹配${NC}"
    fi
else
    echo -e "${RED}  ❌ .env文件不存在${NC}"
    echo -e "${YELLOW}  运行: ./scripts/install.sh 自动生成${NC}"
fi
echo ""

# 检查Docker卷权限
echo "3. Docker 卷状态："
if docker volume inspect voltageems_data &>/dev/null; then
    echo -e "${GREEN}  ✅ voltageems_data 卷存在${NC}"
else
    echo -e "${YELLOW}  ⚠️  voltageems_data 卷不存在（首次部署时会自动创建）${NC}"
fi

echo ""
if [ "$COMPOSE_OK" = true ] && [ -f .env ]; then
    echo -e "${GREEN}✅ 环境检查通过${NC}"
    exit 0
else
    echo -e "${RED}❌ 环境检查失败，请修复上述问题${NC}"
    exit 1
fi
