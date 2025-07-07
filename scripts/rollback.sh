#!/bin/bash
# VoltageEMS 回滚脚本

set -e

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# 配置
DEPLOY_DIR="/opt/voltageems"
BACKUP_DIR="/opt/voltageems/backups"

echo -e "${BLUE}=== VoltageEMS 回滚脚本 ===${NC}"

# 检查权限
if [ "$EUID" -ne 0 ]; then 
    echo -e "${RED}请使用sudo运行此脚本${NC}"
    exit 1
fi

# 列出可用的备份
echo -e "${YELLOW}可用的备份:${NC}"
if [ -d "$BACKUP_DIR" ]; then
    backups=($(ls -1d $BACKUP_DIR/backup-* 2>/dev/null | sort -r))
    
    if [ ${#backups[@]} -eq 0 ]; then
        echo -e "${RED}没有可用的备份${NC}"
        exit 1
    fi
    
    for i in "${!backups[@]}"; do
        backup_name=$(basename ${backups[$i]})
        echo "$((i+1)). $backup_name"
    done
else
    echo -e "${RED}备份目录不存在${NC}"
    exit 1
fi

# 选择备份
echo ""
read -p "选择要回滚的备份编号 (1-${#backups[@]}): " selection

if [[ ! "$selection" =~ ^[0-9]+$ ]] || [ $selection -lt 1 ] || [ $selection -gt ${#backups[@]} ]; then
    echo -e "${RED}无效的选择${NC}"
    exit 1
fi

SELECTED_BACKUP=${backups[$((selection-1))]}
echo -e "${YELLOW}选择的备份: $(basename $SELECTED_BACKUP)${NC}"

# 确认回滚
echo ""
echo -e "${YELLOW}警告: 回滚将停止当前运行的服务！${NC}"
read -p "确认回滚? (yes/no): " confirm

if [ "$confirm" != "yes" ]; then
    echo "取消回滚"
    exit 0
fi

# 开始回滚
echo -e "${YELLOW}开始回滚...${NC}"

# 停止当前服务
cd $DEPLOY_DIR
echo "停止当前服务..."
docker-compose down --remove-orphans

# 恢复配置文件
echo "恢复配置文件..."
if [ -f "$SELECTED_BACKUP/docker-compose.yml" ]; then
    cp $SELECTED_BACKUP/docker-compose.yml $DEPLOY_DIR/
fi

if [ -d "$SELECTED_BACKUP/config" ]; then
    cp -r $SELECTED_BACKUP/config/* $DEPLOY_DIR/config/ 2>/dev/null || true
fi

# 读取原始容器信息
if [ -f "$SELECTED_BACKUP/containers.txt" ]; then
    echo ""
    echo "原始容器状态:"
    cat $SELECTED_BACKUP/containers.txt
fi

# 启动服务
echo ""
echo -e "${YELLOW}启动服务...${NC}"
docker-compose up -d

# 等待服务启动
sleep 10

# 健康检查
echo ""
echo -e "${YELLOW}执行健康检查...${NC}"
HEALTH_CHECK_PASSED=true

# 简单检查主要服务
if curl -f -s http://localhost:8080/health >/dev/null 2>&1; then
    echo -e "${GREEN}✓ API Gateway 健康检查通过${NC}"
else
    echo -e "${RED}✗ API Gateway 健康检查失败${NC}"
    HEALTH_CHECK_PASSED=false
fi

if curl -f -s http://localhost/health >/dev/null 2>&1; then
    echo -e "${GREEN}✓ Frontend 健康检查通过${NC}"
else
    echo -e "${RED}✗ Frontend 健康检查失败${NC}"
    HEALTH_CHECK_PASSED=false
fi

# 显示结果
echo ""
if [ "$HEALTH_CHECK_PASSED" = true ]; then
    echo -e "${GREEN}=== 回滚成功！===${NC}"
    echo "已回滚到: $(basename $SELECTED_BACKUP)"
    
    # 记录回滚信息
    echo "{
      \"rollback_from\": \"$(cat $DEPLOY_DIR/deployment.json 2>/dev/null | grep version || echo 'unknown')\",
      \"rollback_to\": \"$(basename $SELECTED_BACKUP)\",
      \"rollback_time\": \"$(date -u +"%Y-%m-%dT%H:%M:%SZ")\",
      \"rollback_by\": \"$(whoami)\"
    }" > $DEPLOY_DIR/rollback.json
    
else
    echo -e "${RED}=== 回滚后健康检查失败！===${NC}"
    echo "请检查服务日志: docker-compose logs"
fi

# 显示当前状态
echo ""
echo "当前服务状态:"
docker-compose ps

echo ""
echo -e "${GREEN}回滚操作完成${NC}"