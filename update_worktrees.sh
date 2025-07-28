#!/bin/bash

# 需要更新的worktree列表
OUTDATED_WORKTREES=(
    "/Users/lyf/dev/VoltageEMS-apigateway:feature/apigateway-axum-migration"
    "/Users/lyf/dev/VoltageEMS-bugfix:bugfix/redis-performance"
    "/Users/lyf/dev/VoltageEMS-frontend:feature/frontend-ui"
    "/Users/lyf/dev/VoltageEMS-modsrv:feature/modsrv"
    "/Users/lyf/dev/VoltageEMS-monitoring:feature/monitoring-metrics"
    "/Users/lyf/dev/VoltageEMS-predsrv:feature/predsrv-implementation"
    "/Users/lyf/dev/VoltageEMS-tauri-ui:feature/tauri-desktop-app"
    "/Users/lyf/dev/VoltageEMS-websocket:feature/websocket-realtime"
    "/Users/lyf/dev/VoltageEMS-rulesrv:feature/rulesrv"
)

echo "检查需要更新的worktree..."

for entry in "${OUTDATED_WORKTREES[@]}"; do
    IFS=':' read -r worktree_path branch_name <<< "$entry"
    echo ""
    echo "=== 检查 $(basename $worktree_path) ==="
    
    if [ -d "$worktree_path" ]; then
        cd "$worktree_path"
        
        # 检查是否有未提交的更改
        if [ -n "$(git status --porcelain)" ]; then
            echo "❌ $worktree_path 有未提交的更改，跳过更新"
            git status --short
        else
            echo "✅ $worktree_path 没有未提交的更改"
            echo "当前commit: $(git rev-parse --short HEAD)"
            echo "develop最新: $(cd /Users/lyf/dev/VoltageEMS && git rev-parse --short develop)"
            
            # 检查是否需要更新
            current_commit=$(git rev-parse HEAD)
            develop_commit=$(cd /Users/lyf/dev/VoltageEMS && git rev-parse develop)
            
            if [ "$current_commit" != "$develop_commit" ]; then
                echo "🔄 需要更新到最新develop"
            else
                echo "✅ 已经是最新版本"
            fi
        fi
    else
        echo "❌ Worktree路径不存在: $worktree_path"
    fi
done

echo ""
echo "检查完成！"