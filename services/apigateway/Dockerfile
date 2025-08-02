# Build stage
FROM rust:1.88-bullseye AS builder

WORKDIR /app

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY libs ./libs
COPY services ./services

# Build only apigateway
WORKDIR /app
RUN cargo build --release -p apigateway

# Runtime stage
FROM debian:bullseye-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl1.1 \
    curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy binary
COPY --from=builder /app/target/release/apigateway /app/apigateway

# Copy config if exists (ignore if not present)
# COPY --from=builder /app/services/apigateway/config ./config

# Create log directory
RUN mkdir -p /app/logs

EXPOSE 8080

ENV RUST_LOG=info

CMD ["/app/apigateway"]