# Build stage
FROM voltageems-dependencies:latest AS builder

WORKDIR /app

COPY Cargo.toml Cargo.lock ./
COPY libs ./libs
COPY services ./services

RUN cargo build --release -p apigateway

# Runtime stage
FROM debian:bullseye-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl1.1 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy binary
COPY --from=builder /app/target/release/apigateway /app/apigateway

# Create log directory
RUN mkdir -p /app/logs

ENV RUST_LOG=info

CMD ["/app/apigateway"]
