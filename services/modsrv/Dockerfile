FROM rust:1.81-slim-bullseye as builder

WORKDIR /usr/src/modsrv
COPY . .

RUN apt-get update && \
    apt-get install -y pkg-config libssl-dev && \
    cargo build --release

FROM debian:bullseye-slim

RUN apt-get update && \
    apt-get install -y ca-certificates && \
    rm -rf /var/lib/apt/lists/*

# Create necessary directories
RUN mkdir -p /opt/voltageems/modsrv/templates && \
    mkdir -p /etc/voltageems/config/modsrv

# Copy the built binary from the builder stage
COPY --from=builder /usr/src/modsrv/target/release/modsrv /opt/voltageems/modsrv/modsrv

# Copy templates to the appropriate location
COPY templates /opt/voltageems/modsrv/templates/

# Set the working directory
WORKDIR /opt/voltageems/modsrv

# Volume for external configuration
VOLUME /etc/voltageems/config/modsrv

# Expose port for API
EXPOSE 8000

# Set the entry point
ENTRYPOINT ["/opt/voltageems/modsrv/modsrv"] 