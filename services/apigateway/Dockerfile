FROM rust:1.82-slim AS builder

RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Copy workspace manifests
COPY Cargo.toml Cargo.lock ./
COPY libs/ ./libs/
COPY services/apigateway/ ./services/apigateway/

# Build only the apigateway binary
RUN cargo build --release -p apigateway

# ── Runtime ────────────────────────────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /build/target/release/apigateway /app/apigateway

RUN mkdir -p /app/data /app/logs

ENV RUST_LOG=info
ENV API_HOST=0.0.0.0
ENV API_PORT=6005
ENV REDIS_HOST=voltage-redis
ENV REDIS_PORT=6379
ENV VOLTAGE_DB_PATH=/app/data/voltage.db
ENV LOG_DIR=/app/logs
ENV JWT_SECRET_KEY=your-secret-key-here-change-in-production
ENV ACCESS_TOKEN_EXPIRE_MINUTES=30
ENV REFRESH_TOKEN_EXPIRE_DAYS=7
ENV NETWORK_CONFIG_DIR=/etc/systemd/network
ENV DATA_FETCH_INTERVAL=1

EXPOSE 6005

CMD ["/app/apigateway"]
