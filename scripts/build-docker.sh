#!/bin/bash

# ==================================================
# VoltageEMS Docker build script
# Optimized image build workflow
# (VoltageEMS Docker 构建脚本 - 优化的镜像构建流程)
# ==================================================

set -e

# Color definitions (颜色定义)
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

# Configuration (配置)
PROJECT_NAME="voltageems"
REGISTRY=${DOCKER_REGISTRY:-""}
TAG=${DOCKER_TAG:-"latest"}
BUILD_DATE=$(date -u +'%Y-%m-%dT%H:%M:%SZ')
GIT_COMMIT=$(git rev-parse --short HEAD 2>/dev/null || echo "unknown")

# Enable BuildKit (启用BuildKit)
export DOCKER_BUILDKIT=1
export BUILDKIT_PROGRESS=plain

# Service list (服务列表)
SERVICES=(
    "redis"
    "comsrv"
    "modsrv"
    "alarmsrv"
    "rulesrv"
    "hissrv"
    "apigateway"
)

# Logging functions (日志函数)
log_info() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

log_success() {
    echo -e "${GREEN}[✓]${NC} $1"
}

log_error() {
    echo -e "${RED}[✗]${NC} $1"
}

log_warning() {
    echo -e "${YELLOW}[!]${NC} $1"
}

log_build() {
    echo -e "${CYAN}[BUILD]${NC} $1"
}

# Clean up dangling images (清理悬空镜像)
cleanup_dangling() {
    log_info "Cleaning up dangling images..."
    
    local dangling=$(docker images -f "dangling=true" -q | wc -l)
    if [ "$dangling" -gt 0 ]; then
        docker image prune -f
        log_success "Removed $dangling dangling images"
    else
        log_success "No dangling images to remove"
    fi
}

# Build base image (for caching dependencies) (构建基础镜像，用于缓存依赖)
build_base_image() {
    log_build "Building base Rust image with cached dependencies..."
    
    cat > /tmp/Dockerfile.base << 'EOF'
FROM rust:1.83-alpine AS base
RUN apk add --no-cache musl-dev pkgconfig openssl-dev
WORKDIR /app

# Cache common dependencies (缓存常用依赖)
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p libs/src services/comsrv/src && \
    echo "fn main() {}" > services/comsrv/src/main.rs && \
    echo "pub fn dummy() {}" > libs/src/lib.rs && \
    cargo build --release --workspace && \
    rm -rf services libs
EOF

    if docker build -t ${PROJECT_NAME}-base:${TAG} -f /tmp/Dockerfile.base .; then
        log_success "Base image built successfully"
    else
        log_error "Failed to build base image"
        return 1
    fi
    
    rm /tmp/Dockerfile.base
}

# Build service images (构建服务镜像)
build_service() {
    local service=$1
    local dockerfile_path=""
    local context_path="."
    
    # Determine Dockerfile path (确定Dockerfile路径)
    if [ "$service" = "redis" ]; then
        dockerfile_path="docker/redis/Dockerfile"
    else
        dockerfile_path="services/${service}/Dockerfile"
    fi
    
    if [ ! -f "$dockerfile_path" ]; then
        log_warning "Dockerfile not found for $service at $dockerfile_path"
        return 1
    fi
    
    local image_name="${PROJECT_NAME}-${service}:${TAG}"
    
    log_build "Building $service..."
    
    # Build parameters (构建参数)
    local build_args=(
        "--build-arg" "BUILD_DATE=${BUILD_DATE}"
        "--build-arg" "GIT_COMMIT=${GIT_COMMIT}"
        "--label" "org.opencontainers.image.created=${BUILD_DATE}"
        "--label" "org.opencontainers.image.revision=${GIT_COMMIT}"
        "--label" "org.opencontainers.image.title=${PROJECT_NAME}-${service}"
        "--label" "org.opencontainers.image.description=VoltageEMS ${service} service"
    )
    
    # Use cache mounts to speed up builds (使用缓存挂载加速构建)
    if docker build \
        "${build_args[@]}" \
        --cache-from ${PROJECT_NAME}-base:${TAG} \
        --cache-from ${image_name} \
        -t ${image_name} \
        -f ${dockerfile_path} \
        ${context_path}; then
        
        log_success "$service built successfully"
        
        # Tag image (标记镜像)
        if [ -n "$REGISTRY" ]; then
            docker tag ${image_name} ${REGISTRY}/${image_name}
            log_success "$service tagged for registry: ${REGISTRY}/${image_name}"
        fi
        
        return 0
    else
        log_error "Failed to build $service"
        return 1
    fi
}

# Build all services in parallel (并行构建所有服务)
build_all_parallel() {
    log_info "Building all services in parallel..."
    
    local pids=()
    local failed=0
    
    for service in "${SERVICES[@]}"; do
        (build_service "$service") &
        pids+=($!)
    done
    
    # Wait for all builds to complete (等待所有构建完成)
    for pid in "${pids[@]}"; do
        if ! wait $pid; then
            ((failed++))
        fi
    done
    
    if [ $failed -eq 0 ]; then
        log_success "All services built successfully"
        return 0
    else
        log_error "$failed services failed to build"
        return 1
    fi
}

# Optimize image size (优化镜像大小)
optimize_images() {
    log_info "Optimizing image sizes..."
    
    for service in "${SERVICES[@]}"; do
        local image="${PROJECT_NAME}-${service}:${TAG}"
        
        # Export and re-import to remove history layers (导出并重新导入以去除历史层)
        docker save ${image} | docker load
    done
    
    log_success "Images optimized"
}

# Generate image report (生成镜像报告)
generate_report() {
    log_info "Generating build report..."
    
    echo ""
    echo "======================================"
    echo "       DOCKER BUILD REPORT"
    echo "======================================"
    echo "Build Date: ${BUILD_DATE}"
    echo "Git Commit: ${GIT_COMMIT}"
    echo ""
    echo "Image Sizes:"
    echo "----------------------------------------"
    
    local total_size=0
    
    for service in "${SERVICES[@]}"; do
        local image="${PROJECT_NAME}-${service}:${TAG}"
        if docker images --format "table {{.Repository}}:{{.Tag}}\t{{.Size}}" | grep -q "^${image}"; then
            local size=$(docker images --format "{{.Size}}" ${image})
            printf "  %-30s %s\n" "${service}" "${size}"
            
            # Calculate total size (simplified calculation, actual may have layer sharing) (计算总大小，简化计算，实际可能有层共享)
            local size_mb=$(echo $size | sed 's/MB//')
            if [[ $size_mb =~ ^[0-9]+(\.[0-9]+)?$ ]]; then
                total_size=$(echo "$total_size + $size_mb" | bc)
            fi
        fi
    done
    
    echo "----------------------------------------"
    echo "Estimated Total: ~${total_size}MB"
    echo ""
    
    # Save report (保存报告)
    local report_file="tests/reports/docker-build-$(date +%Y%m%d-%H%M%S).txt"
    mkdir -p tests/reports
    
    {
        echo "VoltageEMS Docker Build Report"
        echo "=============================="
        echo "Build Date: ${BUILD_DATE}"
        echo "Git Commit: ${GIT_COMMIT}"
        echo "Tag: ${TAG}"
        echo ""
        docker images | grep ${PROJECT_NAME}
    } > $report_file
    
    log_success "Report saved to $report_file"
}

# Push to image registry (推送到镜像仓库)
push_images() {
    if [ -z "$REGISTRY" ]; then
        log_warning "No registry configured, skipping push"
        return 0
    fi
    
    log_info "Pushing images to ${REGISTRY}..."
    
    for service in "${SERVICES[@]}"; do
        local image="${REGISTRY}/${PROJECT_NAME}-${service}:${TAG}"
        
        if docker push ${image}; then
            log_success "$service pushed to registry"
        else
            log_error "Failed to push $service"
        fi
    done
}

# Clean old images (清理旧镜像)
cleanup_old_images() {
    log_info "Cleaning up old images..."
    
    # Keep the latest 3 versions (保留最新的3个版本)
    for service in "${SERVICES[@]}"; do
        local images=$(docker images --format "{{.ID}}\t{{.CreatedAt}}" | \
            grep ${PROJECT_NAME}-${service} | \
            sort -k2 -r | \
            tail -n +4 | \
            awk '{print $1}')
        
        if [ -n "$images" ]; then
            echo "$images" | xargs docker rmi -f 2>/dev/null || true
            log_success "Cleaned old ${service} images"
        fi
    done
}

# Main build workflow (主构建流程)
main() {
    echo "======================================"
    echo "    VoltageEMS Docker Build System"
    echo "======================================"
    echo ""
    
    case "${1:-build}" in
        base)
            build_base_image
            ;;
        build)
            cleanup_dangling
            build_base_image
            build_all_parallel
            optimize_images
            generate_report
            ;;
        single)
            if [ -z "$2" ]; then
                log_error "Service name required for single build"
                exit 1
            fi
            build_service "$2"
            ;;
        push)
            push_images
            ;;
        clean)
            cleanup_old_images
            cleanup_dangling
            ;;
        report)
            generate_report
            ;;
        *)
            echo "Usage: $0 {build|base|single <service>|push|clean|report}"
            echo ""
            echo "  build   - Build all services with optimization"
            echo "  base    - Build base image only"
            echo "  single  - Build a single service"
            echo "  push    - Push images to registry"
            echo "  clean   - Clean old images"
            echo "  report  - Generate build report"
            exit 1
            ;;
    esac
    
    log_success "Build process completed!"
}

# Error handling (错误处理)
trap 'log_error "Build failed!"; exit 1' ERR

# Run build (运行构建)
main "$@"