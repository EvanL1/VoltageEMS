# 构建阶段
FROM rust:1.88-bullseye AS builder

# 安装构建依赖
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    cmake \
    && rm -rf /var/lib/apt/lists/*

# 创建构建目录
WORKDIR /build

# 复制整个workspace的Cargo文件
COPY Cargo.toml Cargo.lock ./
COPY libs ./libs
# 复制整个services目录（这样workspace能找到所有成员）
COPY services ./services

# 构建comsrv
WORKDIR /build
RUN cargo build --release --package comsrv --features "modbus,test-utils"

# 运行阶段
FROM debian:bullseye-slim

# 安装运行时依赖
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl1.1 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# 创建非root用户
RUN useradd -m -u 1000 -s /bin/bash comsrv && \
    mkdir -p /opt/comsrv/{config,logs,data} && \
    chown -R comsrv:comsrv /opt/comsrv

# 复制构建产物
COPY --from=builder /build/target/release/comsrv /usr/local/bin/comsrv

# 复制配置文件目录结构
COPY --chown=comsrv:comsrv services/comsrv/config /opt/comsrv/config

# 设置工作目录
WORKDIR /opt/comsrv

# 切换到非root用户
USER comsrv

# 设置环境变量
ENV RUST_LOG=info
ENV COMSRV_CONFIG_FILE=/opt/comsrv/config/default.yml

# 健康检查
HEALTHCHECK --interval=30s --timeout=3s --start-period=10s --retries=3 \
    CMD curl -f http://localhost:3000/api/v1/status || exit 1

# 暴露端口
EXPOSE 3000

# 启动命令
ENTRYPOINT ["comsrv"]
CMD ["--config", "/opt/comsrv/config/default.yml"]