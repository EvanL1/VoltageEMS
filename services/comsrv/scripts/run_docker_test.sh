#!/bin/bash
# Docker测试环境运行脚本
# 确保不对外暴露端口，严格记录所有测试日志

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
LOG_DIR="$PROJECT_DIR/logs"
DATE_DIR=$(date +"%Y-%m-%d_%H-%M-%S")
TEST_LOG_DIR="$LOG_DIR/docker-test-$DATE_DIR"

echo "=== COMSRV Docker测试环境启动脚本 ==="
echo "项目目录: $PROJECT_DIR"
echo "日志目录: $TEST_LOG_DIR"
echo ""

# 创建日志目录
mkdir -p "$TEST_LOG_DIR"
mkdir -p "$TEST_LOG_DIR/services"
mkdir -p "$TEST_LOG_DIR/tests"
mkdir -p "$TEST_LOG_DIR/config"

# 复制当前配置文件到日志目录（用于追溯）
cp -r "$PROJECT_DIR/config/"*.yml "$TEST_LOG_DIR/config/" 2>/dev/null || true
cp -r "$PROJECT_DIR/config/"*.csv "$TEST_LOG_DIR/config/" 2>/dev/null || true

# 检查是否有运行中的容器
echo "检查并清理旧容器..."
cd "$PROJECT_DIR"
docker-compose -f docker-compose.test.yml down -v 2>/dev/null || true

# 构建镜像
echo ""
echo "构建Docker镜像..."
docker-compose -f docker-compose.test.yml build

# 启动服务
echo ""
echo "启动测试环境..."
docker-compose -f docker-compose.test.yml up -d

# 等待服务健康检查通过
echo ""
echo "等待服务启动..."
MAX_WAIT=60
WAIT_COUNT=0
while [ $WAIT_COUNT -lt $MAX_WAIT ]; do
    if docker-compose -f docker-compose.test.yml ps | grep -q "unhealthy\|starting"; then
        echo -n "."
        sleep 2
        WAIT_COUNT=$((WAIT_COUNT + 2))
    else
        echo ""
        echo "所有服务已就绪"
        break
    fi
done

if [ $WAIT_COUNT -ge $MAX_WAIT ]; then
    echo ""
    echo "警告: 服务启动超时"
fi

# 显示服务状态
echo ""
echo "服务状态:"
docker-compose -f docker-compose.test.yml ps

# 开始收集日志
echo ""
echo "开始收集日志到: $TEST_LOG_DIR"

# 实时收集各服务日志
docker-compose -f docker-compose.test.yml logs -f --no-color > "$TEST_LOG_DIR/all-services.log" 2>&1 &
LOGS_PID=$!

# 单独收集每个服务的日志
docker-compose -f docker-compose.test.yml logs -f --no-color redis > "$TEST_LOG_DIR/services/redis.log" 2>&1 &
docker-compose -f docker-compose.test.yml logs -f --no-color modbus-simulator > "$TEST_LOG_DIR/services/modbus-simulator.log" 2>&1 &
docker-compose -f docker-compose.test.yml logs -f --no-color comsrv > "$TEST_LOG_DIR/services/comsrv.log" 2>&1 &
docker-compose -f docker-compose.test.yml logs -f --no-color test-runner > "$TEST_LOG_DIR/services/test-runner.log" 2>&1 &
docker-compose -f docker-compose.test.yml logs -f --no-color log-collector > "$TEST_LOG_DIR/services/log-collector.log" 2>&1 &

# 监控测试执行
echo ""
echo "监控测试执行..."
echo "按 Ctrl+C 停止测试环境"
echo ""

# 等待测试完成或用户中断
TEST_COMPLETED=false
trap 'echo "收到中断信号，准备清理..."; TEST_COMPLETED=true' INT TERM

while [ "$TEST_COMPLETED" = false ]; do
    # 检查test-runner是否已退出
    if ! docker-compose -f docker-compose.test.yml ps test-runner | grep -q "Up\|running"; then
        echo "测试运行器已完成"
        TEST_COMPLETED=true
        break
    fi
    sleep 5
done

# 收集最终状态
echo ""
echo "收集最终状态..."
docker-compose -f docker-compose.test.yml ps > "$TEST_LOG_DIR/final-status.txt"

# 收集容器日志
echo "导出容器日志..."
for container in comsrv-test-redis comsrv-test-modbus-simulator comsrv-test-main comsrv-integration-tests comsrv-log-collector; do
    if docker ps -a | grep -q "$container"; then
        docker logs "$container" > "$TEST_LOG_DIR/services/${container}-full.log" 2>&1 || true
    fi
done

# 复制COMSRV本地日志
echo "复制本地日志文件..."
if [ -d "$PROJECT_DIR/logs" ]; then
    # 复制comsrv生成的日志
    find "$PROJECT_DIR/logs" -name "*.log" -newer "$TEST_LOG_DIR" -exec cp {} "$TEST_LOG_DIR/" \; 2>/dev/null || true
fi

# 生成测试报告
echo ""
echo "生成测试报告..."
cat > "$TEST_LOG_DIR/test-report.md" << EOF
# COMSRV Docker测试报告

**测试时间**: $(date)
**测试目录**: $TEST_LOG_DIR

## 测试环境

- Docker Compose版本: $(docker-compose --version)
- Docker版本: $(docker --version)
- 内部网络: comsrv-test-network (不对外暴露)

## 服务列表

| 服务 | 容器名 | 状态 |
|------|--------|------|
| Redis | comsrv-test-redis | $(docker ps -a | grep comsrv-test-redis | awk '{print $7, $8, $9}' || echo "未启动") |
| Modbus模拟器 | comsrv-test-modbus-simulator | $(docker ps -a | grep comsrv-test-modbus-simulator | awk '{print $7, $8, $9}' || echo "未启动") |
| COMSRV主服务 | comsrv-test-main | $(docker ps -a | grep comsrv-test-main | awk '{print $7, $8, $9}' || echo "未启动") |
| 测试运行器 | comsrv-integration-tests | $(docker ps -a | grep comsrv-integration-tests | awk '{print $7, $8, $9}' || echo "未启动") |
| 日志收集器 | comsrv-log-collector | $(docker ps -a | grep comsrv-log-collector | awk '{print $7, $8, $9}' || echo "未启动") |

## 日志文件

### 服务日志
- all-services.log - 所有服务的合并日志
- services/redis.log - Redis服务日志
- services/modbus-simulator.log - Modbus模拟器日志
- services/comsrv.log - COMSRV主服务日志
- services/test-runner.log - 测试运行器日志
- services/log-collector.log - 日志收集器日志

### 测试结果
- 查看 test-runner.log 获取详细测试结果
- 查看 comsrv.log 获取主服务运行日志

## 配置文件
- 使用的配置文件已备份到 config/ 目录

## 网络隔离
- 所有服务运行在内部网络 comsrv-test-network
- 未暴露任何端口到主机
- 服务间通过容器名称互相访问

EOF

# 显示测试总结
echo ""
echo "=== 测试总结 ==="
echo "日志目录: $TEST_LOG_DIR"
echo "查看测试报告: $TEST_LOG_DIR/test-report.md"
echo ""

# 询问是否清理环境
read -p "是否清理Docker环境? (y/n) " -n 1 -r
echo ""
if [[ $REPLY =~ ^[Yy]$ ]]; then
    echo "清理Docker环境..."
    docker-compose -f docker-compose.test.yml down -v
    echo "清理完成"
else
    echo "保留Docker环境，使用以下命令手动清理:"
    echo "  cd $PROJECT_DIR && docker-compose -f docker-compose.test.yml down -v"
fi

# 终止日志收集进程
kill $LOGS_PID 2>/dev/null || true
jobs -p | xargs kill 2>/dev/null || true

echo ""
echo "测试完成！"