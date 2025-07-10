#!/bin/bash
# 设置GitHub Actions Runner为系统服务

set -e

RUNNER_DIR=${RUNNER_DIR:-"$HOME/actions-runner"}

if [ ! -d "$RUNNER_DIR" ]; then
    echo "错误: Runner目录不存在: $RUNNER_DIR"
    echo "请先运行 install-runner.sh"
    exit 1
fi

cd $RUNNER_DIR

if [ ! -f ".runner" ]; then
    echo "错误: Runner未配置"
    echo "请先运行 ./config.sh 配置runner"
    exit 1
fi

echo "=== 设置Runner系统服务 ==="

# 检查是否已安装服务
if systemctl is-active --quiet actions.runner.*; then
    echo "Runner服务已在运行"
    echo "停止现有服务..."
    sudo ./svc.sh stop
    sudo ./svc.sh uninstall
fi

# 安装服务
echo "安装Runner服务..."
sudo ./svc.sh install

# 配置服务自动启动
echo "配置服务自动启动..."
sudo systemctl enable $(systemctl list-units --type=service | grep actions.runner | awk '{print $1}')

# 启动服务
echo "启动Runner服务..."
sudo ./svc.sh start

# 检查状态
echo
echo "=== 服务状态 ==="
sudo ./svc.sh status

# 显示日志位置
echo
echo "=== 日志位置 ==="
echo "服务日志: sudo journalctl -u actions.runner.* -f"
echo "Runner日志: $RUNNER_DIR/_diag/"

# 创建日志查看脚本
cat > $RUNNER_DIR/view-logs.sh << 'EOF'
#!/bin/bash
# 查看Runner日志

echo "=== 最近的Runner日志 ==="
tail -n 50 _diag/Runner_*.log 2>/dev/null || echo "暂无日志"

echo
echo "=== 服务日志 ==="
sudo journalctl -u actions.runner.* --no-pager -n 50

echo
echo "提示: 使用 'sudo journalctl -u actions.runner.* -f' 实时查看日志"
EOF

chmod +x $RUNNER_DIR/view-logs.sh

echo
echo "日志查看脚本: $RUNNER_DIR/view-logs.sh"
echo
echo "Runner服务设置完成！"