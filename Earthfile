VERSION 0.8
ARG --global CARGO_HOME=/usr/local/cargo
ARG --global RUSTUP_HOME=/usr/local/rustup

# Base image for Rust builds
rust-base:
    FROM rust:1.75-slim
    RUN apt-get update && apt-get install -y \
        pkg-config \
        libssl-dev \
        build-essential \
        curl \
        git \
        && rm -rf /var/lib/apt/lists/*
    WORKDIR /workspace
    ENV CARGO_TERM_COLOR=always
    ENV RUST_BACKTRACE=1
    # Install cargo-chef for efficient Docker builds
    RUN cargo install cargo-chef --version 0.1.63

# Base image for C++ builds (for future use)
cpp-base:
    FROM gcc:13-slim
    RUN apt-get update && apt-get install -y \
        cmake \
        ninja-build \
        clang-format \
        clang-tidy \
        ccache \
        git \
        && rm -rf /var/lib/apt/lists/*
    WORKDIR /workspace
    ENV CC=/usr/bin/gcc
    ENV CXX=/usr/bin/g++

# Prepare Rust dependencies using cargo-chef
prepare-rust:
    FROM +rust-base
    COPY Cargo.toml Cargo.lock ./
    COPY libs libs
    COPY services services
    # Generate recipe.json for dependency caching
    RUN find . -name Cargo.toml -exec touch {} \; && \
        cargo chef prepare --recipe-path recipe.json
    SAVE ARTIFACT recipe.json

# Build Rust dependencies
build-rust-deps:
    FROM +rust-base
    COPY +prepare-rust/recipe.json ./
    RUN cargo chef cook --release --recipe-path recipe.json
    SAVE ARTIFACT /usr/local/cargo
    SAVE ARTIFACT target

# Format check for Rust
fmt-rust:
    FROM +rust-base
    RUN rustup component add rustfmt
    COPY --dir libs services Cargo.toml Cargo.lock .
    RUN cargo fmt --all -- --check

# Clippy check for Rust
clippy-rust:
    FROM +rust-base
    RUN rustup component add clippy
    COPY +build-rust-deps/cargo /usr/local/cargo
    COPY +build-rust-deps/target target
    COPY --dir libs services Cargo.toml Cargo.lock .
    RUN cargo clippy --all-targets --all-features -- -D warnings

# Test Rust code
test-rust:
    FROM +rust-base
    COPY +build-rust-deps/cargo /usr/local/cargo
    COPY +build-rust-deps/target target
    COPY --dir libs services Cargo.toml Cargo.lock .
    RUN --secret REDIS_URL=redis://localhost:6379 \
        cargo test --workspace --all-features

# Build all Rust services
build-rust-services:
    FROM +rust-base
    COPY +build-rust-deps/cargo /usr/local/cargo
    COPY +build-rust-deps/target target
    COPY --dir libs services Cargo.toml Cargo.lock .
    RUN cargo build --release --workspace
    SAVE ARTIFACT target/release AS LOCAL target/release

# Build individual service Docker images
docker-apigateway:
    FROM +build-rust-services
    FROM debian:bookworm-slim
    RUN apt-get update && apt-get install -y \
        ca-certificates \
        libssl3 \
        && rm -rf /var/lib/apt/lists/*
    COPY +build-rust-services/target/release/apigateway /usr/local/bin/
    EXPOSE 8080
    CMD ["apigateway"]
    SAVE IMAGE voltageems/apigateway:latest

docker-comsrv:
    FROM +build-rust-services
    FROM debian:bookworm-slim
    RUN apt-get update && apt-get install -y \
        ca-certificates \
        libssl3 \
        && rm -rf /var/lib/apt/lists/*
    COPY +build-rust-services/target/release/comsrv /usr/local/bin/
    EXPOSE 8091
    CMD ["comsrv"]
    SAVE IMAGE voltageems/comsrv:latest

docker-modsrv:
    FROM +build-rust-services
    FROM debian:bookworm-slim
    RUN apt-get update && apt-get install -y \
        ca-certificates \
        libssl3 \
        && rm -rf /var/lib/apt/lists/*
    COPY +build-rust-services/target/release/modsrv /usr/local/bin/
    EXPOSE 8092
    CMD ["modsrv"]
    SAVE IMAGE voltageems/modsrv:latest

docker-hissrv:
    FROM +build-rust-services
    FROM debian:bookworm-slim
    RUN apt-get update && apt-get install -y \
        ca-certificates \
        libssl3 \
        && rm -rf /var/lib/apt/lists/*
    COPY +build-rust-services/target/release/hissrv /usr/local/bin/
    EXPOSE 8093
    CMD ["hissrv"]
    SAVE IMAGE voltageems/hissrv:latest

docker-netsrv:
    FROM +build-rust-services
    FROM debian:bookworm-slim
    RUN apt-get update && apt-get install -y \
        ca-certificates \
        libssl3 \
        && rm -rf /var/lib/apt/lists/*
    COPY +build-rust-services/target/release/netsrv /usr/local/bin/
    EXPOSE 8095
    CMD ["netsrv"]
    SAVE IMAGE voltageems/netsrv:latest

docker-alarmsrv:
    FROM +build-rust-services
    FROM debian:bookworm-slim
    RUN apt-get update && apt-get install -y \
        ca-certificates \
        libssl3 \
        && rm -rf /var/lib/apt/lists/*
    COPY +build-rust-services/target/release/alarmsrv /usr/local/bin/
    EXPOSE 8094
    CMD ["alarmsrv"]
    SAVE IMAGE voltageems/alarmsrv:latest

# Build all Docker images
docker-all:
    BUILD +docker-apigateway
    BUILD +docker-comsrv
    BUILD +docker-modsrv
    BUILD +docker-hissrv
    BUILD +docker-netsrv
    BUILD +docker-alarmsrv

# Run all checks (format, lint, test)
check-all:
    BUILD +fmt-rust
    BUILD +clippy-rust
    BUILD +test-rust

# Complete CI pipeline
ci:
    BUILD +check-all
    BUILD +build-rust-services
    # BUILD +docker-all  # Skip Docker builds for now

# Development build (skip Docker images)
dev:
    BUILD +check-all
    BUILD +build-rust-services

# Clean build artifacts
clean:
    LOCALLY
    RUN rm -rf target
    RUN find . -name "*.log" -delete
    RUN find . -name "*.pid" -delete