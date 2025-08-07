#!/bin/bash

# ==================================================
# VoltageEMS Docker 测试脚本
# 构建、运行和测试Docker容器
# ==================================================

set -e

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# 配置
PROJECT_NAME="voltageems"
COMPOSE_FILE="docker-compose.yml"
TEST_TIMEOUT=60

# 日志函数
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

# 检查Docker环境
check_docker() {
    log_info "Checking Docker environment..."
    
    if ! command -v docker &> /dev/null; then
        log_error "Docker is not installed"
        exit 1
    fi
    
    if ! docker info &> /dev/null; then
        log_error "Docker daemon is not running"
        exit 1
    fi
    
    if ! command -v docker-compose &> /dev/null; then
        log_warning "docker-compose not found, using docker compose"
        COMPOSE_CMD="docker compose"
    else
        COMPOSE_CMD="docker-compose"
    fi
    
    log_success "Docker environment is ready"
}

# 清理旧容器和镜像
cleanup_old() {
    log_info "Cleaning up old containers and images..."
    
    # 停止并删除容器
    $COMPOSE_CMD down -v 2>/dev/null || true
    
    # 删除悬空镜像
    docker image prune -f
    
    log_success "Cleanup completed"
}

# 构建Docker镜像
build_images() {
    log_info "Building Docker images..."
    
    # 使用BuildKit加速构建
    export DOCKER_BUILDKIT=1
    export COMPOSE_DOCKER_CLI_BUILD=1
    
    # 构建所有服务
    if $COMPOSE_CMD build --parallel; then
        log_success "All images built successfully"
    else
        log_error "Failed to build images"
        exit 1
    fi
    
    # 显示镜像信息
    log_info "Docker images:"
    docker images | grep $PROJECT_NAME | awk '{printf "  %-30s %s\n", $1":"$2, $7}'
}

# 启动服务
start_services() {
    log_info "Starting services..."
    
    if $COMPOSE_CMD up -d; then
        log_success "Services started"
    else
        log_error "Failed to start services"
        exit 1
    fi
    
    # 等待服务就绪
    log_info "Waiting for services to be ready..."
    sleep 10
}

# 检查服务健康状态
check_health() {
    log_info "Checking service health..."
    
    local unhealthy=0
    local services=("redis" "comsrv" "modsrv" "alarmsrv" "rulesrv" "hissrv" "apigateway" "nginx")
    
    for service in "${services[@]}"; do
        local container="${PROJECT_NAME}-${service}"
        local status=$(docker inspect --format='{{.State.Status}}' $container 2>/dev/null || echo "not found")
        
        if [ "$status" = "running" ]; then
            log_success "$service is running"
        else
            log_error "$service is not running (status: $status)"
            ((unhealthy++))
        fi
    done
    
    if [ $unhealthy -eq 0 ]; then
        log_success "All services are healthy"
        return 0
    else
        log_error "$unhealthy services are unhealthy"
        return 1
    fi
}

# 运行集成测试
run_integration_tests() {
    log_info "Running integration tests..."
    
    # 测试Redis连接
    if docker exec ${PROJECT_NAME}-redis redis-cli ping &> /dev/null; then
        log_success "Redis connection test passed"
    else
        log_error "Redis connection test failed"
    fi
    
    # 测试Nginx
    if curl -f -s http://localhost/health &> /dev/null; then
        log_success "Nginx proxy test passed"
    else
        log_error "Nginx proxy test failed"
    fi
    
    # 测试API Gateway
    if curl -f -s http://localhost:6005/health &> /dev/null; then
        log_success "API Gateway test passed"
    else
        log_warning "API Gateway direct access test failed (may be expected)"
    fi
    
    # 测试数据持久化
    docker exec ${PROJECT_NAME}-redis redis-cli SET test:key "test_value" &> /dev/null
    local value=$(docker exec ${PROJECT_NAME}-redis redis-cli GET test:key 2>/dev/null)
    if [ "$value" = "test_value" ]; then
        log_success "Data persistence test passed"
        docker exec ${PROJECT_NAME}-redis redis-cli DEL test:key &> /dev/null
    else
        log_error "Data persistence test failed"
    fi
}

# 查看日志
show_logs() {
    local service=$1
    
    if [ -z "$service" ]; then
        log_info "Recent logs from all services:"
        $COMPOSE_CMD logs --tail=10
    else
        log_info "Recent logs from $service:"
        $COMPOSE_CMD logs --tail=20 $service
    fi
}

# 停止服务
stop_services() {
    log_info "Stopping services..."
    
    if $COMPOSE_CMD stop; then
        log_success "Services stopped"
    else
        log_warning "Some services may not have stopped cleanly"
    fi
}

# 生成测试报告
generate_report() {
    log_info "Generating test report..."
    
    local report_file="tests/reports/docker-test-$(date +%Y%m%d-%H%M%S).txt"
    mkdir -p tests/reports
    
    {
        echo "VoltageEMS Docker Test Report"
        echo "=============================="
        echo "Date: $(date)"
        echo ""
        echo "Docker Version:"
        docker version --format 'Client: {{.Client.Version}}, Server: {{.Server.Version}}'
        echo ""
        echo "Images:"
        docker images | grep $PROJECT_NAME
        echo ""
        echo "Containers:"
        docker ps -a | grep $PROJECT_NAME
        echo ""
        echo "Container Stats:"
        docker stats --no-stream | grep $PROJECT_NAME || true
    } > $report_file
    
    log_success "Report saved to $report_file"
}

# 主测试流程
main() {
    echo "======================================"
    echo "    VoltageEMS Docker Test Suite"
    echo "======================================"
    echo ""
    
    case "${1:-test}" in
        build)
            check_docker
            cleanup_old
            build_images
            ;;
        start)
            check_docker
            start_services
            check_health
            ;;
        test)
            check_docker
            cleanup_old
            build_images
            start_services
            check_health
            run_integration_tests
            generate_report
            ;;
        stop)
            stop_services
            ;;
        clean)
            cleanup_old
            ;;
        logs)
            show_logs "$2"
            ;;
        *)
            echo "Usage: $0 {build|start|test|stop|clean|logs [service]}"
            exit 1
            ;;
    esac
    
    log_success "Docker test completed successfully!"
}

# 错误处理
trap 'log_error "Test failed! Showing recent logs..."; show_logs; exit 1' ERR

# 运行测试
main "$@"