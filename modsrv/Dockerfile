FROM rust:1.67-slim as builder

WORKDIR /usr/src/modsrv
COPY . .

# Build the application with release profile
RUN cargo build --release

# Runtime stage
FROM debian:bullseye-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create log directory
RUN mkdir -p /var/log/ems

# Copy the binary from the builder stage
COPY --from=builder /usr/src/modsrv/target/release/modsrv /usr/local/bin/modsrv
COPY modsrv.toml /etc/modsrv.toml

# Set the entrypoint
ENTRYPOINT ["/usr/local/bin/modsrv", "--config", "/etc/modsrv.toml"] 