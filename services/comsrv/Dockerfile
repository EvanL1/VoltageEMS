# Build stage
FROM rust:1.88-bullseye AS builder

# Install dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /app

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY libs ./libs
COPY services ./services

# Build only comsrv
RUN cargo build --release -p comsrv

# Runtime stage
FROM debian:bullseye-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl1.1 \
    && rm -rf /var/lib/apt/lists/*

# Create app user
RUN useradd -m -u 1000 appuser

# Create necessary directories
RUN mkdir -p /app/config /app/logs /app/test-results && \
    chown -R appuser:appuser /app

# Copy binary from build stage
COPY --from=builder /app/target/release/comsrv /app/comsrv

# Switch to app user
USER appuser

# Set working directory
WORKDIR /app

# Expose port (internal only)
EXPOSE 8001

# Run the application
CMD ["/app/comsrv", "--config", "/app/config/comsrv.yaml"]