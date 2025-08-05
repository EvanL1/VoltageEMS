# Build stage
FROM rust:1.88-bullseye AS builder

# Install protobuf-compiler
RUN apt-get update && apt-get install -y protobuf-compiler && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# First stage: Copy dependency files only
COPY Cargo.toml Cargo.lock ./
COPY libs/Cargo.toml ./libs/
COPY services/comsrv/Cargo.toml ./services/comsrv/
COPY services/alarmsrv/Cargo.toml ./services/alarmsrv/
COPY services/hissrv/Cargo.toml ./services/hissrv/
COPY services/rulesrv/Cargo.toml ./services/rulesrv/
COPY services/modsrv/Cargo.toml ./services/modsrv/
COPY services/apigateway/Cargo.toml ./services/apigateway/
COPY services/netsrv/Cargo.toml ./services/netsrv/

# Copy libs source first (it rarely changes)
COPY libs ./libs

# Create dummy source files for services only
RUN mkdir -p services/comsrv/src/bin && echo "fn main() {}" > services/comsrv/src/bin/comsrv.rs && echo "" > services/comsrv/src/lib.rs && \
    mkdir -p services/alarmsrv/src && echo "fn main() {}" > services/alarmsrv/src/main.rs && \
    mkdir -p services/hissrv/src && echo "fn main() {}" > services/hissrv/src/main.rs && \
    mkdir -p services/rulesrv/src && echo "fn main() {}" > services/rulesrv/src/main.rs && \
    mkdir -p services/modsrv/src && echo "fn main() {}" > services/modsrv/src/main.rs && \
    mkdir -p services/apigateway/src && echo "fn main() {}" > services/apigateway/src/main.rs && \
    mkdir -p services/netsrv/src && echo "fn main() {}" > services/netsrv/src/main.rs

# Build dependencies - this layer will be cached
RUN cargo build --release -p comsrv

# Second stage: Copy actual source code  
# libs is already copied, only copy services
COPY services ./services
COPY scripts ./scripts

# Touch binary to ensure rebuild
RUN touch services/comsrv/src/bin/comsrv.rs

# Build final binary
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

# Create log and config directories
RUN mkdir -p /app/logs /app/config

ENV RUST_LOG=info

# Expose ports
EXPOSE 3000 8081

CMD ["/app/comsrv"]