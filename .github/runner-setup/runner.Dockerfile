# GitHub Actions Self-hosted Runner Docker镜像
FROM ubuntu:22.04

# 防止交互式提示
ENV DEBIAN_FRONTEND=noninteractive

# 安装基础依赖
RUN apt-get update && apt-get install -y \
    curl \
    git \
    jq \
    build-essential \
    libssl-dev \
    libffi-dev \
    python3 \
    python3-venv \
    python3-dev \
    python3-pip \
    sudo \
    # 硬件测试相关
    can-utils \
    socat \
    # 清理
    && rm -rf /var/lib/apt/lists/*

# 安装Docker CLI（用于Docker-in-Docker）
RUN curl -fsSL https://get.docker.com | sh

# 创建runner用户
RUN useradd -m -s /bin/bash runner && \
    usermod -aG sudo runner && \
    usermod -aG docker runner && \
    echo "runner ALL=(ALL) NOPASSWD:ALL" >> /etc/sudoers

# 安装Rust（作为runner用户）
USER runner
WORKDIR /home/runner

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
ENV PATH="/home/runner/.cargo/bin:${PATH}"

# 安装GitHub Actions Runner
ENV RUNNER_VERSION=2.311.0
ENV RUNNER_ARCH=x64

RUN mkdir actions-runner && cd actions-runner && \
    curl -o actions-runner-linux-${RUNNER_ARCH}-${RUNNER_VERSION}.tar.gz -L \
    https://github.com/actions/runner/releases/download/v${RUNNER_VERSION}/actions-runner-linux-${RUNNER_ARCH}-${RUNNER_VERSION}.tar.gz && \
    tar xzf ./actions-runner-linux-${RUNNER_ARCH}-${RUNNER_VERSION}.tar.gz && \
    rm actions-runner-linux-${RUNNER_ARCH}-${RUNNER_VERSION}.tar.gz

# 安装Python依赖
RUN pip3 install --user \
    pymodbus \
    pyserial \
    python-can \
    pytest \
    pytest-asyncio

# 配置脚本
COPY --chown=runner:runner <<'EOF' /home/runner/entrypoint.sh
#!/bin/bash
set -e

# 必需的环境变量
: ${GITHUB_URL:?GITHUB_URL环境变量未设置}
: ${GITHUB_TOKEN:?GITHUB_TOKEN环境变量未设置}

# 可选的环境变量
RUNNER_NAME=${RUNNER_NAME:-$(hostname)}
RUNNER_LABELS=${RUNNER_LABELS:-"self-hosted,linux,x64,docker"}
RUNNER_WORK=${RUNNER_WORK:-"_work"}

cd /home/runner/actions-runner

# 配置runner
./config.sh \
    --url "$GITHUB_URL" \
    --token "$GITHUB_TOKEN" \
    --name "$RUNNER_NAME" \
    --labels "$RUNNER_LABELS" \
    --work "$RUNNER_WORK" \
    --unattended \
    --replace

# 运行runner
exec ./run.sh
EOF

RUN chmod +x /home/runner/entrypoint.sh

# 健康检查
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD pgrep Runner.Listener || exit 1

# 入口点
ENTRYPOINT ["/home/runner/entrypoint.sh"]