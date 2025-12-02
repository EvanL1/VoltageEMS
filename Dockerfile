# VoltageEMS Multi-stage Build for Alpine Linux ARM64
# Simplified single-layer build for reliability
# Stage 1: Build binaries
# Stage 2: Minimal runtime image

# ============================================================================
# Stage 1: Builder
# ============================================================================
FROM rust:1.90-alpine AS builder

# Accept build parallelism argument (defaults to 4 cores)
ARG BUILD_JOBS=4

# Swagger UI flag (disabled by default for production)
ARG ENABLE_SWAGGER_UI=0

# Install build dependencies
RUN apk add --no-cache \
    musl-dev \
    pkgconfig \
    openssl-dev \
    curl

WORKDIR /build

# Set Cargo parallel build jobs
ENV CARGO_BUILD_JOBS=${BUILD_JOBS}

# Copy entire source code
COPY . .

# Build release binaries (only services, no apps or tools)
# Note: rules service has been merged into modsrv
# Features:
#   - comsrv: modbus
#   - modsrv: redis, sqlite, [swagger-ui optional]
RUN if [ "$ENABLE_SWAGGER_UI" = "1" ]; then \
        echo "Building with Swagger UI enabled"; \
        cargo build --release -p comsrv -p modsrv \
            --no-default-features \
            --features "modbus,redis,sqlite,swagger-ui"; \
    else \
        echo "Building without Swagger UI (production)"; \
        cargo build --release -p comsrv -p modsrv \
            --no-default-features \
            --features "modbus,redis,sqlite"; \
    fi

# ============================================================================
# Stage 2: Runtime Image
# ============================================================================
FROM alpine:3.19

# Install only essential runtime dependencies
RUN apk add --no-cache \
    ca-certificates \
    tzdata

# Set working directory
WORKDIR /app

# Copy binaries from builder stage
COPY --from=builder /build/target/release/comsrv /usr/local/bin/comsrv
COPY --from=builder /build/target/release/modsrv /usr/local/bin/modsrv

# Make binaries executable
RUN chmod +x /usr/local/bin/*

# Copy default configuration from template
# This provides a working default configuration out-of-the-box
COPY config.template/ /app/config/

# Create all necessary directories with proper permissions
RUN mkdir -p data logs && \
    mkdir -p logs/channels logs/models && \
    mkdir -p logs/comsrv logs/modsrv && \
    chmod -R 775 config data logs

# Default environment variables
ENV RUST_LOG=info
ENV REDIS_URL=redis://localhost:6379

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD ["/bin/sh", "-c", "pgrep -x comsrv || pgrep -x modsrv || exit 1"]

# Default to comsrv, but can be overridden in docker-compose
CMD ["comsrv"]
