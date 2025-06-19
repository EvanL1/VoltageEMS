# 使用官方Rust镜像作为构建环境
FROM rust:1.75 as builder

# 设置工作目录
WORKDIR /app

# 复制Cargo文件
COPY Cargo.toml Cargo.toml

# 复制源代码
COPY src/ src/

# 构建应用
RUN cargo build --release

# 使用更小的运行时镜像
FROM debian:bookworm-slim

# 安装运行时依赖
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# 创建应用用户
RUN useradd -m -u 1000 appuser

# 复制构建好的二进制文件
COPY --from=builder /app/target/release/alarmsrv /usr/local/bin/alarmsrv

# 复制配置文件
COPY alarmsrv.yaml /etc/alarmsrv/alarmsrv.yaml

# 切换到应用用户
USER appuser

# 暴露端口
EXPOSE 8080

# 设置环境变量
ENV RUST_LOG=info

# 启动应用
CMD ["alarmsrv"] 