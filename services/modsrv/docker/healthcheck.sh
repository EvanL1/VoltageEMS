#!/bin/sh
# Docker健康检查脚本

# 检查HTTP健康端点
curl -f http://localhost:8092/health > /dev/null 2>&1

# 返回curl的退出码
exit $?