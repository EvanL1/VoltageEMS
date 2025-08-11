# Build stage - 使用预编译的依赖
FROM voltageems-dependencies:latest AS builder

WORKDIR /app

# 复制必要的配置文件
COPY Cargo.toml Cargo.lock ./
COPY libs ./libs
COPY services ./services

# 直接构建最终二进制文件（依赖已经在基础镜像中编译好了）
RUN cargo build --release -p alarmsrv

# Runtime stage
FROM debian:bullseye-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl1.1 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy binary
COPY --from=builder /app/target/release/alarmsrv /app/alarmsrv

# Create log directory
RUN mkdir -p /app/logs

ENV RUST_LOG=info

CMD ["/app/alarmsrv"]
