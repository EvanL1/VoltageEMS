# Build stage
FROM rust:1.88-bullseye AS builder

WORKDIR /app

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY libs ./libs
COPY services ./services

# Build only netsrv
WORKDIR /app
RUN cargo build --release -p netsrv

# Runtime stage
FROM debian:bullseye-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl1.1 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy binary
COPY --from=builder /app/target/release/netsrv /app/netsrv

# Copy config if exists
# COPY --from=builder /app/services/netsrv/config ./config

# Create log directory
RUN mkdir -p /app/logs

ENV RUST_LOG=info

CMD ["/app/netsrv"]