#!/bin/bash
# 启动前端开发服务器脚本

echo "Starting VoltageEMS Frontend on port 8083..."
cd "$(dirname "$0")"
export PORT=8083
npm run serve