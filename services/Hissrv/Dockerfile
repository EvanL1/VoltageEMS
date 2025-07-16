# HisSrv Docker 构建文件
FROM rust:1.88-bullseye AS builder

# 安装构建依赖
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# 设置工作目录
WORKDIR /build

# 复制整个工作区（从 context 根目录）
COPY Cargo.toml Cargo.lock ./
COPY libs ./libs
COPY services ./services

# 构建 HisSrv
RUN cargo build --release -p hissrv

# 运行时镜像
FROM debian:bullseye-slim

# 安装运行时依赖
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl1.1 \
    && rm -rf /var/lib/apt/lists/*

# 创建非 root 用户
RUN groupadd -g 1000 hissrv && \
    useradd -u 1000 -g hissrv -m -d /app hissrv

# 设置工作目录
WORKDIR /app

# 复制二进制文件
COPY --from=builder /build/target/release/hissrv /app/hissrv

# 创建必要目录
RUN mkdir -p /app/config /app/logs && \
    chown -R hissrv:hissrv /app

# 切换到非 root 用户
USER hissrv

# 健康检查
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD wget --no-verbose --tries=1 --spider http://localhost:8081/health || exit 1

# 启动服务
CMD ["/app/hissrv"]