FROM rust:1.75 as builder

WORKDIR /usr/src/app

# Create a dummy project to cache dependencies
RUN USER=root cargo new --bin modsrv
WORKDIR /usr/src/app/modsrv
COPY Cargo.toml ./
RUN cargo build --release && \
    rm src/*.rs

# Copy actual source code
COPY . .
RUN cargo build --release

# Runtime stage
FROM debian:bullseye-slim

RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy binary
COPY --from=builder /usr/src/app/modsrv/target/release/modsrv /app/modsrv

# Create necessary directories for configuration
RUN mkdir -p /etc/voltageems/config/modsrv
RUN mkdir -p /app/templates

# Default port
EXPOSE 8000

# Default command
ENTRYPOINT ["/app/modsrv"]
CMD ["service"] 