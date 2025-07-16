#!/bin/bash

# HisSrv 测试脚本
# 用于启动测试环境、生成数据并运行测试

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# 打印带颜色的消息
print_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

print_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# 检查依赖
check_dependencies() {
    print_info "检查依赖..."
    
    # 检查 Docker
    if ! command -v docker &> /dev/null; then
        print_error "Docker 未安装，请先安装 Docker"
        exit 1
    fi
    
    # 检查 Python
    if ! command -v python3 &> /dev/null; then
        print_error "Python3 未安装，请先安装 Python3"
        exit 1
    fi
    
    # 检查 Python redis 模块
    if ! python3 -c "import redis" 2>/dev/null; then
        print_warn "Python redis 模块未安装，正在安装..."
        pip3 install redis
    fi
    
    print_info "依赖检查完成"
}

# 启动 Redis
start_redis() {
    print_info "启动 Redis..."
    
    # 检查是否已在运行
    if docker ps | grep -q "redis-hissrv-test"; then
        print_warn "Redis 已在运行"
    else
        docker run -d \
            --name redis-hissrv-test \
            -p 6379:6379 \
            redis:7-alpine
        
        # 等待 Redis 启动
        sleep 2
        
        # 验证连接
        docker exec redis-hissrv-test redis-cli ping > /dev/null
        print_info "Redis 启动成功"
    fi
}

# 启动 InfluxDB
start_influxdb() {
    print_info "启动 InfluxDB..."
    
    # 检查是否已在运行
    if docker ps | grep -q "influxdb-hissrv-test"; then
        print_warn "InfluxDB 已在运行"
    else
        docker run -d \
            --name influxdb-hissrv-test \
            -p 8086:8086 \
            -e INFLUXDB_DB=hissrv_test \
            influxdb:1.8
        
        # 等待 InfluxDB 启动
        sleep 3
        
        # 创建测试数据库
        docker exec influxdb-hissrv-test influx -execute "CREATE DATABASE hissrv_test"
        print_info "InfluxDB 启动成功"
    fi
}

# 生成测试数据
generate_test_data() {
    print_info "生成测试数据..."
    
    cd "$SCRIPT_DIR"
    
    # 设置参数
    CHANNELS="1001 1002 1003"
    INTERVAL="0.5"  # 每0.5秒生成一次数据
    DURATION="30"   # 运行30秒
    
    print_info "生成数据参数："
    print_info "  通道: $CHANNELS"
    print_info "  间隔: ${INTERVAL}秒"
    print_info "  时长: ${DURATION}秒"
    
    # 在后台运行数据生成器
    python3 tests/generate_redis_data.py \
        --channels $CHANNELS \
        --interval $INTERVAL \
        --duration $DURATION &
    
    GENERATOR_PID=$!
    print_info "数据生成器 PID: $GENERATOR_PID"
    
    # 返回 PID 供后续使用
    echo $GENERATOR_PID
}

# 运行 HisSrv
run_hissrv() {
    print_info "编译并运行 HisSrv..."
    
    cd "$PROJECT_ROOT"
    
    # 编译 HisSrv
    print_info "编译 HisSrv..."
    cargo build -p hissrv
    
    # 创建测试配置
    cat > "$SCRIPT_DIR/hissrv-test.yaml" << EOF
service:
  name: hissrv-test
  version: 0.2.0
  port: 8089
  host: 127.0.0.1

redis:
  connection:
    host: 127.0.0.1
    port: 6379
    password: ""
    socket: ""
    database: 0
    pool_size: 10
    timeout: 5
    timeout_seconds: 5
    max_retries: 3
  subscription:
    channels:
      - "*:m:*"
      - "*:s:*"
      - "*:c:*"
      - "*:a:*"
      - "event:*"
      - "channel:*:data"
    key_patterns:
      - "*"
    channel_ids:
      - 1001
      - 1002
      - 1003

storage:
  default: influxdb
  backends:
    influxdb:
      enabled: true
      url: http://localhost:8086
      database: hissrv_test
      username: ""
      password: ""
      retention_days: 7
      batch_size: 100
      flush_interval: 2
    postgresql:
      enabled: false
      host: localhost
      port: 5432
      database: hissrv
      username: postgres
      password: ""
      pool_size: 10
    mongodb:
      enabled: false
      uri: mongodb://localhost:27017
      database: hissrv
      collection: data

data:
  filters:
    default_policy: store
    rules: []
  transformations: []

api:
  enabled: true
  prefix: /api/v1
  swagger_ui: true
  cors:
    enabled: true
    origins:
      - "*"
    methods:
      - GET
      - POST
      - PUT
      - DELETE

monitoring:
  enabled: true
  metrics_port: 9091
  health_check: true

logging:
  level: debug
  format: json
  file: logs/hissrv-test.log
  max_size: 100MB
  max_files: 5

performance:
  worker_threads: 4
  max_concurrent_requests: 1000
  queue_size: 10000
  batch_processing: true
EOF
    
    # 运行 HisSrv
    print_info "启动 HisSrv..."
    RUST_LOG=hissrv=debug,voltage_common=debug cargo run -p hissrv -- \
        --config "$SCRIPT_DIR/hissrv-test.yaml" &
    
    HISSRV_PID=$!
    print_info "HisSrv PID: $HISSRV_PID"
    
    # 等待服务启动
    sleep 5
    
    # 检查健康状态
    if curl -s http://localhost:8089/health > /dev/null; then
        print_info "HisSrv 启动成功"
    else
        print_error "HisSrv 启动失败"
        exit 1
    fi
    
    echo $HISSRV_PID
}

# 运行单元测试
run_unit_tests() {
    print_info "运行单元测试..."
    
    cd "$PROJECT_ROOT"
    cargo test -p hissrv -- --nocapture
}

# 验证数据
verify_data() {
    print_info "验证数据..."
    
    # 检查 Redis 数据
    print_info "检查 Redis 数据..."
    REDIS_KEYS=$(docker exec redis-hissrv-test redis-cli --scan --pattern "*:m:*" | wc -l)
    print_info "Redis 中找到 $REDIS_KEYS 个测量点键"
    
    # 检查 InfluxDB 数据
    print_info "检查 InfluxDB 数据..."
    sleep 5  # 等待数据写入
    
    INFLUX_COUNT=$(docker exec influxdb-hissrv-test influx -database hissrv_test -execute "SELECT COUNT(*) FROM /./" | tail -n 1 | awk '{print $2}' || echo "0")
    print_info "InfluxDB 中找到 $INFLUX_COUNT 条记录"
    
    # 通过 API 查询数据
    print_info "通过 API 查询数据..."
    
    # 查询最新数据
    curl -s http://localhost:8089/api/v1/data/latest?channels=1001,1002,1003 | jq '.' || true
    
    # 查询历史数据
    END_TIME=$(date +%s)
    START_TIME=$((END_TIME - 60))
    curl -s "http://localhost:8089/api/v1/data/history?channel_id=1001&start_time=${START_TIME}&end_time=${END_TIME}" | jq '.' || true
}

# 清理环境
cleanup() {
    print_info "清理测试环境..."
    
    # 停止进程
    if [[ ! -z "${GENERATOR_PID}" ]]; then
        kill $GENERATOR_PID 2>/dev/null || true
    fi
    
    if [[ ! -z "${HISSRV_PID}" ]]; then
        kill $HISSRV_PID 2>/dev/null || true
    fi
    
    # 清理测试数据
    if [[ "$CLEAR_DATA" == "true" ]]; then
        print_info "清理 Redis 测试数据..."
        cd "$SCRIPT_DIR"
        python3 tests/generate_redis_data.py --clear --channels 1001 1002 1003
    fi
    
    # 停止并删除容器
    if [[ "$STOP_CONTAINERS" == "true" ]]; then
        print_info "停止并删除容器..."
        docker stop redis-hissrv-test 2>/dev/null || true
        docker rm redis-hissrv-test 2>/dev/null || true
        docker stop influxdb-hissrv-test 2>/dev/null || true
        docker rm influxdb-hissrv-test 2>/dev/null || true
    fi
    
    # 删除测试配置
    rm -f "$SCRIPT_DIR/hissrv-test.yaml"
    
    print_info "清理完成"
}

# 主函数
main() {
    print_info "=== HisSrv 测试脚本 ==="
    
    # 解析参数
    CLEAR_DATA=false
    STOP_CONTAINERS=false
    RUN_UNIT_TESTS=false
    
    while [[ $# -gt 0 ]]; do
        case $1 in
            --clear)
                CLEAR_DATA=true
                shift
                ;;
            --stop-containers)
                STOP_CONTAINERS=true
                shift
                ;;
            --unit-tests)
                RUN_UNIT_TESTS=true
                shift
                ;;
            *)
                print_error "未知参数: $1"
                echo "用法: $0 [--clear] [--stop-containers] [--unit-tests]"
                echo "  --clear          清理测试数据"
                echo "  --stop-containers 停止并删除容器"
                echo "  --unit-tests     运行单元测试"
                exit 1
                ;;
        esac
    done
    
    # 设置清理函数
    trap cleanup EXIT
    
    # 执行测试流程
    check_dependencies
    start_redis
    start_influxdb
    
    if [[ "$RUN_UNIT_TESTS" == "true" ]]; then
        run_unit_tests
    fi
    
    GENERATOR_PID=$(generate_test_data)
    HISSRV_PID=$(run_hissrv)
    
    # 等待数据生成
    print_info "等待数据生成和处理..."
    sleep 20
    
    # 验证数据
    verify_data
    
    print_info "测试完成！"
    print_info "你可以访问以下地址查看更多信息："
    print_info "  - Swagger UI: http://localhost:8089/api/v1/swagger-ui"
    print_info "  - 健康检查: http://localhost:8089/health"
    print_info "  - Prometheus 指标: http://localhost:9091/metrics"
    
    # 等待用户输入
    read -p "按 Enter 键结束测试..."
}

# 运行主函数
main "$@"