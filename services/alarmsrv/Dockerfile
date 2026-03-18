# ── Build stage ──────────────────────────────────────────────────────────────
FROM rust:1.82-slim AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Copy workspace manifests for dependency caching
COPY Cargo.toml Cargo.lock ./
COPY libs/ libs/
COPY services/alarmsrv/ services/alarmsrv/

RUN cargo build --release -p alarmsrv

# ── Runtime stage ─────────────────────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /build/target/release/alarmsrv /app/alarmsrv

RUN mkdir -p /app/data /app/logs

ENV SERVICE_PORT=6007
ENV REDIS_URL=redis://127.0.0.1:6379
ENV VOLTAGE_DB_PATH=/app/data/voltage.db
ENV VOLTAGE_LOG_DIR=/app/logs

EXPOSE 6007

CMD ["/app/alarmsrv"]
