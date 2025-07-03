# Build stage
FROM rust:1.70 as builder

WORKDIR /app

# Copy manifests
COPY Cargo.toml Cargo.lock ./

# Build dependencies
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src

# Copy source code
COPY src ./src

# Build application
RUN cargo build --release

# Runtime stage
FROM debian:bullseye-slim

# Install required dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl1.1 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the binary from builder
COPY --from=builder /app/target/release/apigateway /app/apigateway

# Copy configuration file
COPY apigateway.yml /app/apigateway.yml

# Expose port
EXPOSE 8080

# Set environment variables
ENV RUST_LOG=info
ENV APIGATEWAY_SERVER_HOST=0.0.0.0
ENV APIGATEWAY_SERVER_PORT=8080

# Run the binary
CMD ["./apigateway"]