# Build stage
FROM rust:1.88-bullseye AS builder

WORKDIR /app

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY libs ./libs
COPY services ./services

# Build only rulesrv
WORKDIR /app
RUN cargo build --release -p rulesrv

# Runtime stage
FROM debian:bullseye-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl1.1 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy binary
COPY --from=builder /app/target/release/rulesrv /app/rulesrv

# Copy config if exists
# COPY --from=builder /app/services/rulesrv/config ./config

# Create log directory
RUN mkdir -p /app/logs

ENV RUST_LOG=info

CMD ["/app/rulesrv"]