# Build stage
FROM rust:1.88-bullseye as builder

# 安装构建依赖
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/voltage

# 设置Cargo使用所有可用CPU核心
ENV CARGO_BUILD_JOBS=-1
ENV CARGO_TARGET_DIR=/usr/src/voltage/target

# 复制workspace根目录文件
COPY Cargo.toml Cargo.lock ./

# 复制libs目录（包含voltage-common）
COPY libs ./libs

# 复制services目录
COPY services ./services

# 构建API Gateway - 使用所有可用核心
WORKDIR /usr/src/voltage
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/src/voltage/target \
    cargo build --release -p apigateway -j $(nproc) && \
    cp /usr/src/voltage/target/release/apigateway /tmp/apigateway

# Runtime stage
FROM debian:bullseye-slim

# 安装运行时依赖
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl1.1 \
    curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# 从构建阶段复制二进制文件
COPY --from=builder /tmp/apigateway /app/apigateway

# 创建配置目录
RUN mkdir -p /app/config /app/logs

# 设置可执行权限
RUN chmod +x /app/apigateway

# 暴露端口
EXPOSE 8089

# 设置环境变量
ENV RUST_LOG=info
ENV APIGATEWAY_SERVER_HOST=0.0.0.0
ENV APIGATEWAY_SERVER_PORT=8089

# 健康检查
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8089/health || exit 1

# 运行API Gateway
CMD ["/app/apigateway"]