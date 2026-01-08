# VoltageEMS Dockerfile for multi-architecture builds
# Uses pre-compiled binaries from cargo-zigbuild for fast builds
# No compilation happens in Docker - just packaging the pre-built binaries

# Build argument for target triple (set by build script)
ARG TARGET_TRIPLE=aarch64-unknown-linux-musl

FROM alpine:3.19

# Install only essential runtime dependencies
RUN apk add --no-cache \
    ca-certificates \
    tzdata

# Set working directory
WORKDIR /app

# Copy pre-compiled binaries (built with cargo-zigbuild)
# These are already built by the build script before Docker runs
ARG TARGET_TRIPLE
COPY target/${TARGET_TRIPLE}/release/comsrv /usr/local/bin/comsrv
COPY target/${TARGET_TRIPLE}/release/modsrv /usr/local/bin/modsrv

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
