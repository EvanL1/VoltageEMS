# Build stage
FROM rust:1.88-bullseye AS builder

WORKDIR /app

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY libs ./libs
COPY services ./services

# Build only hissrv
WORKDIR /app
RUN cargo build --release -p hissrv

# Runtime stage
FROM debian:bullseye-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl1.1 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy binary
COPY --from=builder /app/target/release/hissrv /app/hissrv

# Copy config if exists
# COPY --from=builder /app/services/hissrv/config ./config

# Create log and config directories
RUN mkdir -p /app/logs /app/config

ENV RUST_LOG=info

CMD ["/app/hissrv"]