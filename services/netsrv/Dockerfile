# syntax=docker/dockerfile:1

# ── Build stage ────────────────────────────────────────────────────────────────
FROM rust:1.82-slim AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

COPY Cargo.toml Cargo.lock ./
COPY libs/ libs/
COPY services/netsrv/ services/netsrv/

# Stub other workspace services to allow selective build
RUN for svc in alarmsrv apigateway comsrv hissrv modsrv; do \
        mkdir -p services/$svc/src && \
        echo 'fn main(){}' > services/$svc/src/main.rs && \
        cp services/netsrv/Cargo.toml services/$svc/Cargo.toml && \
        sed -i "s/name = \"netsrv\"/name = \"$svc\"/" services/$svc/Cargo.toml; \
    done

RUN cargo build --release -p netsrv

# ── Runtime stage ─────────────────────────────────────────────────────────────
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
RUN mkdir -p /app/logs /app/data /app/config/cert

COPY --from=builder /build/target/release/netsrv /app/netsrv

ENV API_HOST=0.0.0.0
ENV API_PORT=6006
ENV REDIS_HOST=voltage-redis
ENV REDIS_PORT=6379
ENV VOLTAGE_DB_PATH=/app/data/voltage.db
ENV VOLTAGE_LOG_DIR=/app/logs

EXPOSE 6006

CMD ["/app/netsrv"]
