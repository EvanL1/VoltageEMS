#!/bin/bash
# 
# ModSrv Docker 测试执行脚本（本地测试版）
# 用于在Docker环境中启动ModSrv服务，使用本地Python环境执行测试

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# 显示帮助信息
show_help() {
  echo "ModSrv Docker 本地测试执行脚本"
  echo "用法: $0 [选项]"
  echo "选项:"
  echo "  -h, --help     显示此帮助信息"
  echo "  -b, --build    重新构建镜像（默认使用已有镜像）"
  echo "  -c, --clean    测试后清理（删除容器和网络）"
  echo "  -l, --logs     显示modsrv服务的日志" 
  echo "  --debug        调试模式，显示更多信息"
}

# 参数初始化
BUILD=false
CLEAN=false
LOGS=false
DEBUG=false

# 解析命令行参数
while [[ $# -gt 0 ]]; do
  case "$1" in
    -h|--help)
      show_help
      exit 0
      ;;
    -b|--build)
      BUILD=true
      shift
      ;;
    -c|--clean)
      CLEAN=true
      shift
      ;;
    -l|--logs)
      LOGS=true
      shift
      ;;
    --debug)
      DEBUG=true
      shift
      ;;
    *)
      echo "错误: 未知参数 $1"
      show_help
      exit 1
      ;;
  esac
done

# 打印调试信息
if $DEBUG; then
  echo "执行目录: $SCRIPT_DIR"
  echo "参数设置:"
  echo "  BUILD=$BUILD"
  echo "  CLEAN=$CLEAN"
  echo "  LOGS=$LOGS"
  echo "  DEBUG=$DEBUG"
fi

# 检查Docker是否已安装
if ! command -v docker &> /dev/null || ! command -v docker-compose &> /dev/null; then
  echo "错误: 请先安装Docker和docker-compose"
  exit 1
fi

# 检查Python环境
if ! command -v python3 &> /dev/null; then
  echo "错误: 请先安装Python"
  exit 1
fi

# 检查pip和安装依赖
if ! command -v pip3 &> /dev/null; then
  echo "错误: 请先安装pip"
  exit 1
fi

# 安装测试依赖
echo "=== 安装测试依赖 ==="
pip3 install -q requests pytest pytest-timeout python-dotenv

# 启动Docker服务
echo "=== 启动ModSrv服务 ==="
if $BUILD; then
  echo "重新构建镜像..."
  docker-compose build
fi
docker-compose up -d

# 等待服务启动
echo "=== 等待服务启动 ==="
MAX_RETRY=30
RETRY=0
SERVICE_UP=false

while [ $RETRY -lt $MAX_RETRY ]; do
  # 尝试健康检查和基础路径，有些服务器可能提供不同的健康检查路径
  if curl -s http://localhost:8000/health > /dev/null || curl -s http://localhost:8000/api/health > /dev/null; then
    SERVICE_UP=true
    break
  fi
  echo "等待ModSrv服务启动... 尝试 $((RETRY+1))/$MAX_RETRY"
  sleep 2
  RETRY=$((RETRY+1))
done

if ! $SERVICE_UP; then
  echo "错误: ModSrv服务未能在规定时间内启动"
  docker-compose logs modsrv
  docker-compose down
  exit 1
fi

echo "=== ModSrv服务已启动 ==="

# 如果指定了查看日志
if $LOGS; then
  echo "=== 显示服务日志（按Ctrl+C退出日志查看，测试将继续运行）==="
  docker-compose logs -f modsrv
fi

# 运行测试
echo "=== 运行API测试 ==="
python3 test-api.py
TEST_EXIT_CODE=$?

# 如果指定了测试完成后清理
if $CLEAN; then
  echo "=== 清理测试环境 ==="
  docker-compose down -v
fi

# 输出测试结果
if [ $TEST_EXIT_CODE -eq 0 ]; then
  echo "✅ 测试通过！"
  exit 0
else
  echo "❌ 测试失败！退出代码: $TEST_EXIT_CODE"
  exit $TEST_EXIT_CODE
fi 