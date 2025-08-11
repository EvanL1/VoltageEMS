# Build stage - 使用预编译的依赖
FROM voltageems-dependencies:latest AS builder

# Install protobuf-compiler
RUN apt-get update && apt-get install -y protobuf-compiler && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# 复制必要的配置文件
COPY Cargo.toml Cargo.lock ./
COPY libs ./libs
COPY services ./services

# 直接构建最终二进制文件（依赖已经在基础镜像中编译好了）
RUN cargo build --release -p comsrv

# Runtime stage
FROM debian:bullseye-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl1.1 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy binary
COPY --from=builder /app/target/release/comsrv /app/comsrv

# Create log and config directories
RUN mkdir -p /app/logs /app/config

ENV RUST_LOG=info

# Expose ports
EXPOSE 3000 8081

CMD ["/app/comsrv"]
