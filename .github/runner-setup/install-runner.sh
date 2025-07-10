#!/bin/bash
# GitHub Actions Self-hosted Runner 安装脚本

set -e

# 配置变量
RUNNER_VERSION=${RUNNER_VERSION:-"2.311.0"}
RUNNER_ARCH=${RUNNER_ARCH:-"x64"}  # x64, arm, arm64
RUNNER_OS=${RUNNER_OS:-"linux"}
GITHUB_ORG=${GITHUB_ORG:-"your-org"}
GITHUB_REPO=${GITHUB_REPO:-"VoltageEMS"}

echo "=== GitHub Actions Self-hosted Runner 安装脚本 ==="
echo "版本: $RUNNER_VERSION"
echo "架构: $RUNNER_ARCH"
echo "仓库: $GITHUB_ORG/$GITHUB_REPO"
echo

# 检查依赖
echo "检查系统依赖..."
DEPS_MISSING=false

for cmd in curl tar; do
    if ! command -v $cmd &> /dev/null; then
        echo "缺少: $cmd"
        DEPS_MISSING=true
    fi
done

if [ "$DEPS_MISSING" = true ]; then
    echo "请先安装缺失的依赖"
    exit 1
fi

# 创建runner目录
RUNNER_DIR="$HOME/actions-runner"
echo "创建runner目录: $RUNNER_DIR"
mkdir -p $RUNNER_DIR
cd $RUNNER_DIR

# 下载runner
DOWNLOAD_URL="https://github.com/actions/runner/releases/download/v${RUNNER_VERSION}/actions-runner-${RUNNER_OS}-${RUNNER_ARCH}-${RUNNER_VERSION}.tar.gz"
echo "下载runner..."
echo "URL: $DOWNLOAD_URL"

curl -o runner.tar.gz -L $DOWNLOAD_URL

# 解压
echo "解压runner..."
tar xzf runner.tar.gz
rm runner.tar.gz

# 安装依赖
echo "安装runner依赖..."
./bin/installdependencies.sh

# 配置说明
echo
echo "=== 配置Runner ==="
echo "1. 获取注册令牌:"
echo "   访问: https://github.com/$GITHUB_ORG/$GITHUB_REPO/settings/actions/runners/new"
echo
echo "2. 运行配置命令:"
echo "   cd $RUNNER_DIR"
echo "   ./config.sh --url https://github.com/$GITHUB_ORG/$GITHUB_REPO \\"
echo "     --token YOUR_RUNNER_TOKEN \\"
echo "     --name $(hostname)-runner \\"
echo "     --labels self-hosted,linux,$RUNNER_ARCH,hw-test \\"
echo "     --work _work"
echo
echo "3. 安装为系统服务:"
echo "   sudo ./svc.sh install"
echo "   sudo ./svc.sh start"
echo
echo "4. 查看服务状态:"
echo "   sudo ./svc.sh status"

# 创建标签配置文件
cat > $RUNNER_DIR/.labels << EOF
# Runner标签配置
# 根据实际硬件能力添加标签

# 基础标签
self-hosted
linux
$RUNNER_ARCH

# 硬件能力标签（根据实际情况启用）
# gpio        # 如果支持GPIO
# can         # 如果支持CAN总线
# serial      # 如果支持串口
# modbus      # 如果可以访问Modbus设备
# integration # 如果用于集成测试
# performance # 如果用于性能测试
# production  # 如果用于生产部署
EOF

echo
echo "标签配置文件创建在: $RUNNER_DIR/.labels"
echo "请根据实际硬件能力修改标签"

# 创建环境检查脚本
cat > $RUNNER_DIR/check-env.sh << 'EOF'
#!/bin/bash
# Runner环境检查脚本

echo "=== Runner环境检查 ==="

# 检查Rust
if command -v cargo &> /dev/null; then
    echo "✓ Rust已安装: $(cargo --version)"
else
    echo "✗ Rust未安装"
    echo "  安装: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
fi

# 检查Docker
if command -v docker &> /dev/null; then
    echo "✓ Docker已安装: $(docker --version)"
    if docker ps &>/dev/null; then
        echo "  Docker服务正常"
    else
        echo "  需要将用户添加到docker组: sudo usermod -aG docker $USER"
    fi
else
    echo "✗ Docker未安装"
fi

# 检查Python
if command -v python3 &> /dev/null; then
    echo "✓ Python3已安装: $(python3 --version)"
else
    echo "✗ Python3未安装"
fi

# 检查硬件访问权限
echo
echo "=== 硬件访问权限 ==="
echo "用户组: $(groups)"

# GPIO权限
if [ -d /sys/class/gpio ]; then
    if [ -w /sys/class/gpio/export ]; then
        echo "✓ GPIO访问权限正常"
    else
        echo "✗ 无GPIO访问权限"
    fi
fi

# 串口权限
if groups | grep -q dialout; then
    echo "✓ 串口访问权限正常(dialout组)"
else
    echo "✗ 无串口访问权限"
    echo "  添加到dialout组: sudo usermod -aG dialout $USER"
fi

echo
echo "环境检查完成"
EOF

chmod +x $RUNNER_DIR/check-env.sh

echo
echo "运行环境检查: $RUNNER_DIR/check-env.sh"
echo
echo "安装完成！"