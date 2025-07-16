# Build stage
FROM rust:1.88-bullseye AS builder

# Install dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /usr/src/app

# Copy workspace files
COPY cargo.toml ./Cargo.toml
COPY Cargo.lock ./
COPY libs ./libs
COPY services ./services

# Build the rulesrv service
WORKDIR /usr/src/app
RUN cargo build --release -p rulesrv

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Create app user
RUN useradd -m -u 1001 app

# Copy the binary from builder
COPY --from=builder /usr/src/app/services/rulesrv/target/release/rulesrv /usr/local/bin/rulesrv

# Copy configuration files
WORKDIR /app
COPY services/rulesrv/config ./config

# Set ownership
RUN chown -R app:app /app

# Switch to non-root user
USER app

# Expose port
EXPOSE 8083

# Set default environment variables
ENV RUST_LOG=info
ENV RULESRV_REDIS_URL=redis://localhost:6379

# Run the service
CMD ["rulesrv", "service"]