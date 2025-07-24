# 多阶段构建 - hissrv服务
FROM rust:1.88-bullseye as builder

# 设置工作目录
WORKDIR /workspace

# 复制整个工作空间根目录
COPY . .

# 检查并构建hissrv
RUN ls -la && \
    find . -name "Cargo.toml" -type f && \
    cargo build --release -p hissrv

# 运行时镜像
FROM debian:bullseye-slim

# 安装运行时依赖
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# 创建工作目录
WORKDIR /app

# 复制编译好的二进制文件
COPY --from=builder /workspace/target/release/hissrv /usr/local/bin/hissrv

# 复制配置文件
COPY --from=builder /workspace/services/hissrv/config /app/config

# 创建日志目录
RUN mkdir -p /app/logs

# 设置环境变量
ENV RUST_LOG=info

# 启动服务
CMD ["hissrv"]