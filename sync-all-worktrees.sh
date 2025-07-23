#!/bin/bash

# Worktree同步脚本
declare -A worktrees=(
    ["/Users/lyf/dev/VoltageEMS-alarmsrv"]="feature/alarmsrv-modsrv-subscription"
    ["/Users/lyf/dev/VoltageEMS-apigateway"]="feature/apigateway-axum-migration"
    ["/Users/lyf/dev/VoltageEMS-bugfix"]="bugfix/redis-performance"
    ["/Users/lyf/dev/VoltageEMS-feature-comsrv"]="feature/comsrv"
    ["/Users/lyf/dev/VoltageEMS-frontend"]="feature/frontend-ui"
    ["/Users/lyf/dev/VoltageEMS-hissrv"]="feature/hissrv-influxdb"
    ["/Users/lyf/dev/VoltageEMS-modsrv"]="feature/modsrv"
    ["/Users/lyf/dev/VoltageEMS-monitoring"]="feature/monitoring-metrics"
    ["/Users/lyf/dev/VoltageEMS-predsrv"]="feature/predsrv-implementation"
    ["/Users/lyf/dev/VoltageEMS-rulesrv"]="feature/rulesrv"
    ["/Users/lyf/dev/VoltageEMS-tauri-ui"]="feature/tauri-desktop-app"
    ["/Users/lyf/dev/VoltageEMS-websocket"]="feature/websocket-realtime"
)

echo "开始同步所有worktree..."
echo ""

for path in "${!worktrees[@]}"; do
    branch="${worktrees[$path]}"
    echo "========================================="
    echo "同步 $branch"
    echo "路径: $path"
    echo "========================================="
    
    cd "$path"
    
    # 检查是否有未提交的更改
    if ! git diff --quiet || ! git diff --cached --quiet; then
        echo "⚠️  检测到未提交的更改，先提交或暂存："
        git status --porcelain
        echo ""
        echo "建议操作："
        echo "  git add . && git commit -m '临时提交：同步前保存工作'"
        echo "  或者 git stash '临时暂存'"
        echo ""
        echo "跳过此worktree，请手动处理后重新运行"
        echo ""
        continue
    fi
    
    # 获取远程更新
    echo "-> 获取远程更新..."
    git fetch origin
    
    # Rebase到最新的develop
    echo "-> 将 origin/develop 的更改合并到 $branch..."
    if git rebase origin/develop; then
        echo "✓ 同步成功!"
        
        # 检查是否需要推送
        local_commit=$(git rev-parse HEAD)
        remote_commit=$(git rev-parse origin/$branch 2>/dev/null || echo "")
        
        if [ "$local_commit" != "$remote_commit" ]; then
            echo "-> 检测到本地有新提交，是否推送到远程？ (y/n)"
            read -r response
            if [[ "$response" =~ ^[Yy]$ ]]; then
                echo "-> 推送到远程..."
                git push origin $branch --force-with-lease
                echo "✓ 推送完成!"
            else
                echo "-> 跳过推送"
            fi
        fi
    else
        echo "✗ 同步失败，可能存在冲突"
        echo "-> 请手动解决冲突，然后运行："
        echo "   git rebase --continue"
        echo "   或者"
        echo "   git rebase --abort  # 放弃此次rebase"
        echo ""
    fi
    
    echo ""
done

echo "========================================="
echo "同步完成！"
echo "========================================="