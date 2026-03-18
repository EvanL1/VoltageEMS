# syntax=docker/dockerfile:1

# ── Build stage ────────────────────────────────────────────────────────────────
FROM rust:1.82-slim AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Copy workspace manifests first for layer caching
COPY Cargo.toml Cargo.lock ./
COPY libs/ libs/
COPY services/hissrv/ services/hissrv/

# Stub out other services so the workspace resolves without their sources
RUN for svc in alarmsrv apigateway comsrv modsrv; do \
        mkdir -p services/$svc/src && \
        echo 'fn main(){}' > services/$svc/src/main.rs && \
        cp services/hissrv/Cargo.toml services/$svc/Cargo.toml && \
        sed -i "s/name = \"hissrv\"/name = \"$svc\"/" services/$svc/Cargo.toml; \
    done

RUN cargo build --release -p hissrv

# ── Runtime stage ─────────────────────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libssl3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /build/target/release/hissrv /app/hissrv

RUN mkdir -p /app/logs

ENV API_HOST=0.0.0.0
ENV API_PORT=6004
ENV REDIS_HOST=voltage-redis
ENV REDIS_PORT=6379
ENV DATABASE_URL=postgres://postgres:postgres@timescaledb:5432/hissrv
ENV STORAGE_BACKEND=timescaledb
ENV LOG_DIR=/app/logs

EXPOSE 6004

CMD ["/app/hissrv"]
