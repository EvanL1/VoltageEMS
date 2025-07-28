#!/bin/bash

# 需要更新的worktree列表（没有未提交更改的）
CLEAN_WORKTREES=(
    "/Users/lyf/dev/VoltageEMS-apigateway:feature/apigateway-axum-migration"
    "/Users/lyf/dev/VoltageEMS-bugfix:bugfix/redis-performance"
    "/Users/lyf/dev/VoltageEMS-frontend:feature/frontend-ui"
    "/Users/lyf/dev/VoltageEMS-modsrv:feature/modsrv"
    "/Users/lyf/dev/VoltageEMS-monitoring:feature/monitoring-metrics"
    "/Users/lyf/dev/VoltageEMS-predsrv:feature/predsrv-implementation"
    "/Users/lyf/dev/VoltageEMS-tauri-ui:feature/tauri-desktop-app"
    "/Users/lyf/dev/VoltageEMS-websocket:feature/websocket-realtime"
)

echo "更新干净的worktree到最新develop..."
echo "当前develop commit: $(git rev-parse --short develop)"

for entry in "${CLEAN_WORKTREES[@]}"; do
    IFS=':' read -r worktree_path branch_name <<< "$entry"
    echo ""
    echo "=== 更新 $(basename $worktree_path) ==="
    
    if [ -d "$worktree_path" ]; then
        cd "$worktree_path"
        
        echo "重置到最新develop..."
        git reset --hard develop
        
        if [ $? -eq 0 ]; then
            echo "✅ 成功更新 $branch_name 到 $(git rev-parse --short HEAD)"
        else
            echo "❌ 更新失败: $worktree_path"
        fi
    else
        echo "❌ Worktree路径不存在: $worktree_path"
    fi
done

echo ""
echo "批量更新完成！"