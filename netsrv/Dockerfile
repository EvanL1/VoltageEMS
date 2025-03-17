FROM rust:1.67-slim as builder

WORKDIR /usr/src/netsrv
COPY . .

# 构建应用程序
RUN cargo build --release

# 运行时镜像
FROM debian:bullseye-slim

# 安装运行时依赖
RUN apt-get update && apt-get install -y \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# 创建日志目录
RUN mkdir -p /var/log/ems

# 从构建阶段复制二进制文件和配置
COPY --from=builder /usr/src/netsrv/target/release/netsrv /usr/local/bin/netsrv
COPY netsrv.json /etc/netsrv.json

# 设置入口点
ENTRYPOINT ["/usr/local/bin/netsrv", "--config", "/etc/netsrv.json"] 