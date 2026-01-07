#!/bin/bash
# 统一的Python服务启动脚本
# 根据SERVICE_NAME环境变量启动对应的服务

set -e

# 默认服务名
SERVICE_NAME="${SERVICE_NAME:-hissrv}"

# 服务配置映射
declare -A SERVICE_PATHS=(
    ["hissrv"]="/app/services/hissrv"
    ["apigateway"]="/app/services/apigateway"
    ["netsrv"]="/app/services/netsrv"
    ["alarmsrv"]="/app/services/alarmsrv"
)

declare -A SERVICE_MAIN=(
    ["hissrv"]="main.py"
    ["apigateway"]="main.py"
    ["netsrv"]="main.py"
    ["alarmsrv"]="main.py"
)

# 检查服务名是否有效
if [[ ! -v SERVICE_PATHS[$SERVICE_NAME] ]]; then
    echo "错误: 未知的服务名 '$SERVICE_NAME'"
    echo "支持的服务: ${!SERVICE_PATHS[@]}"
    exit 1
fi

# 获取服务路径和主文件
SERVICE_PATH="${SERVICE_PATHS[$SERVICE_NAME]}"
MAIN_FILE="${SERVICE_MAIN[$SERVICE_NAME]}"

# 检查服务目录是否存在
if [[ ! -d "$SERVICE_PATH" ]]; then
    echo "错误: 服务目录不存在: $SERVICE_PATH"
    exit 1
fi

# 检查主文件是否存在
if [[ ! -f "$SERVICE_PATH/$MAIN_FILE" ]]; then
    echo "错误: 主文件不存在: $SERVICE_PATH/$MAIN_FILE"
    exit 1
fi

# 切换到服务目录
cd "$SERVICE_PATH"

# 设置Python路径，确保可以导入服务模块
export PYTHONPATH="$SERVICE_PATH:$PYTHONPATH"

# 启动服务
echo "启动服务: $SERVICE_NAME"
echo "工作目录: $SERVICE_PATH"
echo "主文件: $MAIN_FILE"

exec python "$MAIN_FILE"

