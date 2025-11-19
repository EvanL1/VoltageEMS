# VoltageEMS Multi-stage Build for Alpine Linux ARM64
# Simplified single-layer build for reliability
# Stage 1: Build binaries
# Stage 2: Minimal runtime image

# Support multi-platform builds, default to Linux ARM64 (for ARM IPC target)
ARG TARGETPLATFORM=linux/arm64

# ============================================================================
# Stage 1: Builder
# ============================================================================
FROM --platform=$TARGETPLATFORM rust:1.90-alpine AS builder

# Accept build parallelism argument (defaults to 4 cores)
ARG BUILD_JOBS=4

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
RUN cargo build --release -p comsrv -p modsrv -p rulesrv

# ============================================================================
# Stage 2: Runtime Image
# ============================================================================
FROM --platform=$TARGETPLATFORM alpine:3.19

# Install only essential runtime dependencies
RUN apk add --no-cache \
    ca-certificates \
    tzdata

# Set working directory
WORKDIR /app

# Copy binaries from builder stage
COPY --from=builder /build/target/release/comsrv /usr/local/bin/comsrv
COPY --from=builder /build/target/release/modsrv /usr/local/bin/modsrv
COPY --from=builder /build/target/release/rulesrv /usr/local/bin/rulesrv

# Make binaries executable
RUN chmod +x /usr/local/bin/*

# Copy default configuration from template
# This provides a working default configuration out-of-the-box
COPY config.template/ /app/config/

# Create all necessary directories with proper permissions
RUN mkdir -p data logs && \
    mkdir -p logs/channels logs/models && \
    mkdir -p logs/comsrv logs/modsrv logs/rulesrv && \
    chmod -R 775 config data logs

# Default environment variables
ENV RUST_LOG=info
ENV REDIS_URL=redis://localhost:6379

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD ["/bin/sh", "-c", "pgrep -x comsrv || pgrep -x modsrv || pgrep -x rulesrv || exit 1"]

# Default to comsrv, but can be overridden in docker-compose
CMD ["comsrv"]
