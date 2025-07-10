# =============================================================================
# Multi-stage build for Comsrv
# =============================================================================

# -----------------------------------------------------------------------------
# Build stage
# -----------------------------------------------------------------------------
FROM rust:1.88-bullseye AS builder

# Install system dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /app

# Copy workspace files (this assumes Docker build context is at repository root)
COPY cargo.toml /workspace/Cargo.toml
COPY Cargo.lock /workspace/
COPY libs /workspace/libs/
COPY services /workspace/services/

WORKDIR /workspace

# Build only the comsrv service
RUN unset http_proxy https_proxy HTTP_PROXY HTTPS_PROXY && \
    cargo build --release -p comsrv

# Copy the binary to app directory
WORKDIR /app
RUN cp /workspace/target/release/comsrv .

# Strip the binary to reduce size
RUN strip comsrv

# -----------------------------------------------------------------------------
# Runtime stage
# -----------------------------------------------------------------------------
FROM debian:bullseye-slim

# Install runtime dependencies
RUN sed -i 's/deb.debian.org/mirrors.aliyun.com/g' /etc/apt/sources.list
RUN unset http_proxy https_proxy HTTP_PROXY HTTPS_PROXY && \
    apt-get update && apt-get install -y \
    ca-certificates \
    tzdata \
    && rm -rf /var/lib/apt/lists/*

# Create a non-root user
RUN groupadd -r comsrv && useradd -r -g comsrv comsrv

# Create necessary directories
RUN mkdir -p /app/config /app/logs /app/data && \
    chown -R comsrv:comsrv /app

# Set working directory
WORKDIR /app

# Copy the binary from builder stage
COPY --from=builder /app/comsrv /app/bin/comsrv

# Copy configuration files and scripts
COPY config /app/config/
COPY scripts /app/scripts/

# Make binary and scripts executable
RUN chmod +x /app/bin/comsrv /app/scripts/*.sh

# Switch to non-root user
USER comsrv

# Set environment variables
ENV CONFIG_FILE=/app/config/comsrv.yaml
ENV LOG_DIR=/app/logs
ENV DATA_DIR=/app/data
ENV RUST_LOG=info
ENV RUST_BACKTRACE=1

# Expose ports
EXPOSE 3000 9090

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:3000/health || exit 1

# Default command
CMD ["/app/bin/comsrv"]

# -----------------------------------------------------------------------------
# Labels
# -----------------------------------------------------------------------------
LABEL maintainer="Your Team <team@example.com>"
LABEL description="Comsrv - Industrial Communication Service"
LABEL version="1.0.0"
LABEL org.opencontainers.image.source="https://github.com/your-org/comsrv"
LABEL org.opencontainers.image.documentation="https://github.com/your-org/comsrv/blob/main/README.md"
LABEL org.opencontainers.image.licenses="MIT" 