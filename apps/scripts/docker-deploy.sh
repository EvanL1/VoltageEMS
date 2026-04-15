#!/usr/bin/env bash
# ============================================================
# VoltageEMS 前端 Docker 本地部署脚本（Linux / macOS）
#
# 用法：
#   ./scripts/docker-deploy.sh [TAG] [--no-cache]
#
# 示例：
#   ./scripts/docker-deploy.sh              # 使用 latest 标签
#   ./scripts/docker-deploy.sh v1.2.3       # 指定标签
#   ./scripts/docker-deploy.sh latest --no-cache  # 完整重建
# ============================================================
set -euo pipefail

# ── 配置 ──────────────────────────────────────────────────────────────────────
IMAGE_NAME="voltage-apps"
CONTAINER_NAME="voltage-apps"
HOST_PORT=8080
TAG="${1:-latest}"
NO_CACHE="${2:-}"

FULL_IMAGE="${IMAGE_NAME}:${TAG}"

# ── 颜色输出 ──────────────────────────────────────────────────────────────────
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
RED='\033[0;31m'
GRAY='\033[0;37m'
NC='\033[0m'

step()  { echo -e "\n${YELLOW}[$1/4] $2${NC}"; }
ok()    { echo -e "  ${GREEN}✓ $1${NC}"; }
info()  { echo -e "  ${GRAY}· $1${NC}"; }
fail()  { echo -e "${RED}[错误] $1${NC}"; exit 1; }

echo ""
echo -e "${CYAN}================================================${NC}"
echo -e "${CYAN}   VoltageEMS 前端 Docker 部署${NC}"
echo -e "${CYAN}================================================${NC}"

# ── 前置检查 ──────────────────────────────────────────────────────────────────
docker info >/dev/null 2>&1 || fail "Docker 未运行，请先启动 Docker"

# ── Step 1：清理旧容器 ────────────────────────────────────────────────────────
step 1 "清理端口 ${HOST_PORT} 上的现有容器..."

# 先按端口查找
CONTAINERS=$(docker ps -a --filter "publish=${HOST_PORT}" --format "{{.ID}} {{.Names}}" 2>/dev/null || true)
if [ -n "$CONTAINERS" ]; then
    while IFS=' ' read -r cid cname; do
        [ -z "$cid" ] && continue
        info "停止容器: ${cname} (${cid})"
        docker stop "$cid"  >/dev/null
        docker rm   "$cid"  >/dev/null
        ok "已移除: ${cname}"
    done <<< "$CONTAINERS"
else
    # 按容器名查找
    BY_NAME=$(docker ps -a --filter "name=^${CONTAINER_NAME}$" --format "{{.ID}}" 2>/dev/null || true)
    if [ -n "$BY_NAME" ]; then
        info "停止容器: ${CONTAINER_NAME}"
        docker stop "$BY_NAME" >/dev/null
        docker rm   "$BY_NAME" >/dev/null
        ok "已移除: ${CONTAINER_NAME}"
    else
        info "端口 ${HOST_PORT} 上无运行中的容器，跳过清理"
    fi
fi

# ── Step 2：构建镜像 ──────────────────────────────────────────────────────────
step 2 "构建 Docker 镜像: ${FULL_IMAGE} ..."

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APPS_DIR="$(dirname "$SCRIPT_DIR")"

[ -f "${APPS_DIR}/Dockerfile" ] || fail "在 ${APPS_DIR} 中未找到 Dockerfile"

BUILD_OPTS="-t ${FULL_IMAGE}"
if [ "$NO_CACHE" = "--no-cache" ]; then
    BUILD_OPTS="--no-cache ${BUILD_OPTS}"
    info "使用 --no-cache 完整重建"
fi

# shellcheck disable=SC2086
docker build $BUILD_OPTS "$APPS_DIR"
ok "镜像构建成功: ${FULL_IMAGE}"

# ── Step 3：启动容器 ──────────────────────────────────────────────────────────
step 3 "启动新容器..."

docker run -d \
    --name "$CONTAINER_NAME" \
    --restart unless-stopped \
    -p "${HOST_PORT}:8080" \
    "$FULL_IMAGE" >/dev/null

ok "容器启动成功: ${CONTAINER_NAME}"

# ── Step 4：验证 ──────────────────────────────────────────────────────────────
step 4 "验证部署状态..."
sleep 3

STATUS=$(docker ps --filter "name=^${CONTAINER_NAME}$" --filter "status=running" --format "{{.Status}}" 2>/dev/null || true)
if [ -n "$STATUS" ]; then
    ok "容器状态: ${STATUS}"
    echo ""
    echo -e "${CYAN}================================================${NC}"
    echo -e "${CYAN}   部署成功！访问地址: http://localhost:${HOST_PORT}${NC}"
    echo -e "${CYAN}================================================${NC}"
    echo ""
    echo -e "${GRAY}常用命令：${NC}"
    echo -e "${GRAY}  查看日志  : docker logs -f ${CONTAINER_NAME}${NC}"
    echo -e "${GRAY}  进入容器  : docker exec -it ${CONTAINER_NAME} sh${NC}"
    echo -e "${GRAY}  停止容器  : docker stop ${CONTAINER_NAME}${NC}"
else
    echo ""
    echo -e "${YELLOW}[警告] 容器可能未正常运行，最近日志：${NC}"
    docker logs --tail 20 "$CONTAINER_NAME" || true
    fail "请检查上方日志排查问题"
fi
