# Build stage
FROM rust:1.88-bullseye AS builder

# Install protobuf-compiler
RUN apt-get update && apt-get install -y protobuf-compiler && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY libs ./libs
COPY services ./services
COPY scripts ./scripts

# Build only comsrv
WORKDIR /app
RUN cargo build --release -p comsrv

# Runtime stage
FROM debian:bullseye-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl1.1 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy binary
COPY --from=builder /app/target/release/comsrv /app/comsrv

# Copy config and plugins if exists
# COPY --from=builder /app/services/comsrv/config ./config
# COPY --from=builder /app/services/comsrv/plugins ./plugins

# Create log directory
RUN mkdir -p /app/logs

ENV RUST_LOG=info

CMD ["/app/comsrv"]