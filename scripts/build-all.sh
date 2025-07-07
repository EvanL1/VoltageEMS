#!/bin/bash
# VoltageEMS Docker镜像批量构建脚本

set -e

# 颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

# 参数
VERSION=${1:-"latest"}
REGISTRY=${2:-"localhost:5000"}
PROJECT="voltageems"

echo -e "${GREEN}=== VoltageEMS Docker构建脚本 ===${NC}"
echo "版本: $VERSION"
echo "Registry: $REGISTRY"
echo ""

# 服务列表
RUST_SERVICES=(
    "comsrv"
    "modsrv"
    "hissrv"
    "netsrv"
    "alarmsrv"
    "apigateway"
    "config-framework"
)

# 构建基础镜像（如果需要）
echo -e "${YELLOW}构建Rust基础镜像...${NC}"
docker build -t ${REGISTRY}/${PROJECT}/rust-base:latest -f docker/rust-base.Dockerfile . 2>/dev/null || {
    # 如果没有基础镜像文件，使用官方镜像
    echo "使用官方Rust镜像"
}

# 构建Rust服务
for service in "${RUST_SERVICES[@]}"; do
    echo -e "${YELLOW}构建 $service...${NC}"
    
    if [ -f "services/$service/Dockerfile" ]; then
        docker build \
            -t ${REGISTRY}/${PROJECT}/${service}:${VERSION} \
            -t ${REGISTRY}/${PROJECT}/${service}:latest \
            -f services/${service}/Dockerfile \
            --build-arg VERSION=${VERSION} \
            --build-arg BUILD_DATE=$(date -u +"%Y-%m-%dT%H:%M:%SZ") \
            --build-arg GIT_COMMIT=$(git rev-parse --short HEAD 2>/dev/null || echo "unknown") \
            .
        
        echo -e "${GREEN}✓ $service 构建成功${NC}"
    else
        # 如果没有单独的Dockerfile，使用通用模板
        echo "为 $service 创建Dockerfile..."
        
        cat > services/${service}/Dockerfile <<EOF
FROM rust:1.70 as builder

WORKDIR /app
COPY . .
WORKDIR /app/services/${service}

RUN cargo build --release

FROM debian:bullseye-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/${service} /usr/local/bin/${service}
COPY --from=builder /app/services/${service}/config /app/config

EXPOSE 8080
CMD ["${service}"]
EOF
        
        docker build \
            -t ${REGISTRY}/${PROJECT}/${service}:${VERSION} \
            -t ${REGISTRY}/${PROJECT}/${service}:latest \
            -f services/${service}/Dockerfile \
            .
        
        echo -e "${GREEN}✓ $service 构建成功${NC}"
    fi
done

# 构建前端
echo -e "${YELLOW}构建前端应用...${NC}"
if [ -f "frontend/Dockerfile" ]; then
    docker build \
        -t ${REGISTRY}/${PROJECT}/frontend:${VERSION} \
        -t ${REGISTRY}/${PROJECT}/frontend:latest \
        -f frontend/Dockerfile \
        --build-arg VERSION=${VERSION} \
        .
else
    # 创建前端Dockerfile
    cat > frontend/Dockerfile <<EOF
FROM node:16 as builder
WORKDIR /app
COPY frontend/package*.json ./
RUN npm ci
COPY frontend/ .
RUN npm run build

FROM nginx:alpine
COPY --from=builder /app/dist /usr/share/nginx/html
COPY frontend/nginx.conf /etc/nginx/conf.d/default.conf
EXPOSE 80
CMD ["nginx", "-g", "daemon off;"]
EOF
    
    docker build \
        -t ${REGISTRY}/${PROJECT}/frontend:${VERSION} \
        -t ${REGISTRY}/${PROJECT}/frontend:latest \
        -f frontend/Dockerfile \
        .
fi
echo -e "${GREEN}✓ 前端构建成功${NC}"

# 显示构建结果
echo ""
echo -e "${GREEN}=== 构建完成 ===${NC}"
echo "构建的镜像:"
docker images | grep ${PROJECT} | grep ${VERSION}

# 导出镜像列表
echo ""
echo "导出镜像列表到 images.txt..."
docker images | grep ${PROJECT} | grep ${VERSION} > images.txt

echo -e "${GREEN}所有镜像构建成功！${NC}"