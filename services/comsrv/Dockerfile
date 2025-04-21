FROM rust:slim as builder

WORKDIR /usr/src/app

# 安装依赖
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    build-essential \
    git \
    ca-certificates

# 拷贝Cargo文件
COPY Cargo.toml Cargo.lock ./

# 为了利用Docker层缓存，先只构建依赖
RUN mkdir -p src && \
    echo "fn main() {println!(\"placeholder\")}" > src/main.rs && \
    cargo build --release && \
    rm -rf src

# 拷贝源码
COPY . .

# 删除placeholder后重新构建项目
RUN touch src/main.rs && cargo build --release

# 运行阶段 - 使用相同的基础镜像以确保glibc版本兼容
FROM rust:slim

WORKDIR /app

# 安装运行时依赖
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# 拷贝编译后的二进制文件
COPY --from=builder /usr/src/app/target/release/comsrv /app/
# 拷贝配置文件
COPY --from=builder /usr/src/app/config /app/config

# 添加start脚本
COPY start.sh /app/
RUN chmod +x /app/start.sh

# 设置环境变量
ENV RUST_LOG=info
ENV CONFIG_PATH=/app/config
ENV LOG_PATH=/app/logs

# 创建日志目录
RUN mkdir -p /app/logs

# 暴露API端口和Metrics端口
EXPOSE 8080 9090

# 启动服务
CMD ["/app/start.sh"] 