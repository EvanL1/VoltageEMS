#!/bin/bash
# VoltageEMS 部署脚本

set -e

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# 参数
ENVIRONMENT=${1:-"production"}
VERSION=${2:-"latest"}
REGISTRY=${3:-"localhost:5000"}
PROJECT="voltageems"

# 部署配置
DEPLOY_DIR="/opt/voltageems"
BACKUP_DIR="/opt/voltageems/backups"
LOG_DIR="/var/log/voltageems"

echo -e "${BLUE}=== VoltageEMS 部署脚本 ===${NC}"
echo "环境: $ENVIRONMENT"
echo "版本: $VERSION"
echo "Registry: $REGISTRY"
echo ""

# 检查权限
if [ "$EUID" -ne 0 ]; then 
    echo -e "${RED}请使用sudo运行此脚本${NC}"
    exit 1
fi

# 创建必要的目录
echo -e "${YELLOW}创建目录...${NC}"
mkdir -p $DEPLOY_DIR
mkdir -p $BACKUP_DIR
mkdir -p $LOG_DIR
mkdir -p $DEPLOY_DIR/data/redis
mkdir -p $DEPLOY_DIR/data/influxdb
mkdir -p $DEPLOY_DIR/config

# 备份当前部署
if [ -f "$DEPLOY_DIR/docker-compose.yml" ]; then
    echo -e "${YELLOW}备份当前部署...${NC}"
    BACKUP_NAME="backup-$(date +%Y%m%d-%H%M%S)"
    mkdir -p $BACKUP_DIR/$BACKUP_NAME
    
    # 保存当前运行的容器信息
    docker-compose -f $DEPLOY_DIR/docker-compose.yml ps > $BACKUP_DIR/$BACKUP_NAME/containers.txt
    
    # 备份配置文件
    cp $DEPLOY_DIR/docker-compose.yml $BACKUP_DIR/$BACKUP_NAME/ 2>/dev/null || true
    cp -r $DEPLOY_DIR/config/* $BACKUP_DIR/$BACKUP_NAME/ 2>/dev/null || true
    
    echo -e "${GREEN}✓ 备份完成: $BACKUP_NAME${NC}"
fi

# 复制部署文件
echo -e "${YELLOW}复制部署文件...${NC}"
cp docker-compose.prod.yml $DEPLOY_DIR/docker-compose.yml

# 更新镜像版本
echo -e "${YELLOW}更新镜像版本...${NC}"
cd $DEPLOY_DIR
sed -i "s|image: .*comsrv:.*|image: ${REGISTRY}/${PROJECT}/comsrv:${VERSION}|g" docker-compose.yml
sed -i "s|image: .*modsrv:.*|image: ${REGISTRY}/${PROJECT}/modsrv:${VERSION}|g" docker-compose.yml
sed -i "s|image: .*hissrv:.*|image: ${REGISTRY}/${PROJECT}/hissrv:${VERSION}|g" docker-compose.yml
sed -i "s|image: .*netsrv:.*|image: ${REGISTRY}/${PROJECT}/netsrv:${VERSION}|g" docker-compose.yml
sed -i "s|image: .*alarmsrv:.*|image: ${REGISTRY}/${PROJECT}/alarmsrv:${VERSION}|g" docker-compose.yml
sed -i "s|image: .*apigateway:.*|image: ${REGISTRY}/${PROJECT}/apigateway:${VERSION}|g" docker-compose.yml
sed -i "s|image: .*frontend:.*|image: ${REGISTRY}/${PROJECT}/frontend:${VERSION}|g" docker-compose.yml

# 拉取新镜像
echo -e "${YELLOW}拉取新镜像...${NC}"
docker-compose pull

# 停止旧容器
echo -e "${YELLOW}停止旧容器...${NC}"
docker-compose down --remove-orphans

# 启动新容器
echo -e "${YELLOW}启动新容器...${NC}"
docker-compose up -d

# 等待服务启动
echo -e "${YELLOW}等待服务启动...${NC}"
sleep 10

# 健康检查
echo -e "${YELLOW}执行健康检查...${NC}"
HEALTH_CHECK_PASSED=true

# 检查各个服务
SERVICES=("apigateway:8080" "frontend:80" "comsrv:8081" "modsrv:8082" "hissrv:8083")
for service in "${SERVICES[@]}"; do
    IFS=':' read -r name port <<< "$service"
    
    if curl -f -s http://localhost:$port/health >/dev/null 2>&1; then
        echo -e "${GREEN}✓ $name 健康检查通过${NC}"
    else
        echo -e "${RED}✗ $name 健康检查失败${NC}"
        HEALTH_CHECK_PASSED=false
    fi
done

# 检查Redis
if docker exec voltageems_redis_1 redis-cli ping >/dev/null 2>&1; then
    echo -e "${GREEN}✓ Redis 健康检查通过${NC}"
else
    echo -e "${RED}✗ Redis 健康检查失败${NC}"
    HEALTH_CHECK_PASSED=false
fi

# 部署结果
if [ "$HEALTH_CHECK_PASSED" = true ]; then
    echo ""
    echo -e "${GREEN}=== 部署成功！===${NC}"
    echo "版本: $VERSION"
    echo "环境: $ENVIRONMENT"
    echo ""
    echo "查看日志: docker-compose logs -f"
    echo "查看状态: docker-compose ps"
    
    # 清理旧镜像
    echo -e "${YELLOW}清理旧镜像...${NC}"
    docker image prune -f
    
else
    echo ""
    echo -e "${RED}=== 部署失败！===${NC}"
    echo "部分服务健康检查未通过"
    echo ""
    echo "查看日志: docker-compose logs"
    
    # 询问是否回滚
    read -p "是否回滚到上一版本？(y/n): " -n 1 -r
    echo
    if [[ $REPLY =~ ^[Yy]$ ]]; then
        ./scripts/rollback.sh
    fi
    
    exit 1
fi

# 记录部署信息
echo "{
  \"version\": \"$VERSION\",
  \"environment\": \"$ENVIRONMENT\",
  \"deploy_time\": \"$(date -u +"%Y-%m-%dT%H:%M:%SZ")\",
  \"deployed_by\": \"$(whoami)\",
  \"host\": \"$(hostname)\"
}" > $DEPLOY_DIR/deployment.json

echo -e "${GREEN}部署完成！${NC}"