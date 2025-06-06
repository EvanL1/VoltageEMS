# =============================================================================
# Multi-stage build for Comsrv
# =============================================================================

# -----------------------------------------------------------------------------
# Build stage
# -----------------------------------------------------------------------------
FROM rust:1.75 AS builder

# Install system dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /app

# Copy dependency files first for better caching
COPY Cargo.toml Cargo.lock ./

# Create a dummy main.rs to build dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs

# Build dependencies (this layer will be cached unless Cargo.* files change)
RUN cargo build --release && rm src/main.rs target/release/deps/comsrv*

# Copy source code
COPY src ./src

# Build the application
RUN cargo build --release

# Strip the binary to reduce size
RUN strip target/release/comsrv

# -----------------------------------------------------------------------------
# Runtime stage
# -----------------------------------------------------------------------------
FROM debian:bullseye-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    tzdata \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create a non-root user
RUN groupadd -r comsrv && useradd -r -g comsrv comsrv

# Create necessary directories
RUN mkdir -p /app/config /app/logs /app/data && \
    chown -R comsrv:comsrv /app

# Set working directory
WORKDIR /app

# Copy the binary from builder stage
COPY --from=builder /app/target/release/comsrv /app/bin/comsrv

# Copy configuration template
COPY config/comsrv.example.yaml /app/config/comsrv.example.yaml

# Make binary executable
RUN chmod +x /app/bin/comsrv

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