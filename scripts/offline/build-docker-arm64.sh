#!/usr/bin/env bash
# Build Docker images for ARM64 and export as tar files
# Includes VoltageRedis and VoltageEMS services

set -euo pipefail

# Disable macOS resource fork files
export COPYFILE_DISABLE=1

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m'

# Configuration
ROOT_DIR="$(cd "$(dirname "$0")"/../.. && pwd)"
OUTPUT_DIR="$ROOT_DIR/offline-bundle/docker"
PLATFORM="linux/arm64"

# Detect CPU cores for parallel compilation
if command -v nproc &> /dev/null; then
    CPU_CORES=$(nproc)
elif command -v sysctl &> /dev/null; then
    CPU_CORES=$(sysctl -n hw.ncpu)
else
    CPU_CORES=4  # Fallback to 4 cores
fi

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}  Building Docker Images for ARM64     ${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""
echo -e "${GREEN}Detected CPU cores: $CPU_CORES${NC}"
echo ""

# Check Docker
if ! command -v docker &> /dev/null; then
    echo -e "${RED}Error: Docker not installed${NC}"
    exit 1
fi

# Check for buildx (required for cross-platform builds)
if ! docker buildx version &> /dev/null; then
    echo -e "${YELLOW}Docker buildx not found, installing...${NC}"
    docker buildx create --use --name arm64-builder
fi

# Create output directory
mkdir -p "$OUTPUT_DIR"

# Clean up old images to avoid confusion
echo -e "${YELLOW}Cleaning up old images...${NC}"
docker rmi voltage-redis:arm64 2>/dev/null && echo "  Removed voltage-redis:arm64" || true
docker rmi voltageems:arm64 2>/dev/null && echo "  Removed voltageems:arm64" || true
docker rmi voltage-apps:arm64 2>/dev/null && echo "  Removed voltage-apps:arm64" || true
# Note: We keep :latest tags as they will be overwritten by new builds

# Function to build and save image
build_and_save() {
    local dockerfile=$1
    local context=$2
    local tag=$3
    local output=$4

    echo -e "${YELLOW}Building $tag for ARM64 with $CPU_CORES parallel jobs...${NC}"

    # Check if old image exists and inform user
    if docker images | grep -q "^${tag%:*}.*${tag#*:}"; then
        echo "  Note: Existing $tag will be replaced"
    fi

    if [[ -f "$dockerfile" ]]; then
        # Build for ARM64 with parallel jobs (will automatically replace existing tag)
        docker buildx build \
            --platform "$PLATFORM" \
            --build-arg BUILD_JOBS=$CPU_CORES \
            --load \
            -f "$dockerfile" \
            -t "$tag" \
            "$context" || {
            echo -e "${RED}Failed to build $tag${NC}"
            return 1
        }

        # Save to tar.gz
        echo "Saving $tag to $output..."
        docker save "$tag" | gzip > "$output"

        # Show size
        size=$(ls -lh "$output" | awk '{print $5}')
        echo -e "${GREEN}[DONE] Saved $tag ($size)${NC}"

        return 0
    else
        echo -e "${YELLOW}Dockerfile not found: $dockerfile${NC}"
        return 1
    fi
}

# Pull official Redis image for ARM64
echo ""
echo -e "${BLUE}[1/3] Pulling official Redis 8 Alpine for ARM64...${NC}"

# Pull the base image
docker pull --platform "$PLATFORM" redis:8-alpine

if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓ Redis image pulled successfully${NC}"

    # Save Redis image (using official tag)
    echo "Saving Redis image..."
    docker save redis:8-alpine | gzip > "$OUTPUT_DIR/voltage-redis.tar.gz"

    # Show size
    size=$(ls -lh "$OUTPUT_DIR/voltage-redis.tar.gz" | awk '{print $5}')
    echo -e "${GREEN}[DONE] Saved voltage-redis.tar.gz ($size)${NC}"
else
    echo -e "${RED}Error: Failed to pull Redis image${NC}"
    exit 1
fi

# Build VoltageEMS services
echo ""
echo -e "${BLUE}[2/3] Building VoltageEMS services...${NC}"

# Use the main Dockerfile
if [[ -f "$ROOT_DIR/Dockerfile" ]]; then
    DOCKERFILE="$ROOT_DIR/Dockerfile"
else
    # Create a simple Dockerfile if none exists
    echo -e "${YELLOW}No Dockerfile found, creating minimal Dockerfile...${NC}"

    cat << 'EOF' > /tmp/Dockerfile.arm64
FROM rust:1.90-slim AS builder

WORKDIR /usr/src/app

# Install dependencies for ARM64 cross-compilation
RUN apt-get update && apt-get install -y \
    gcc-aarch64-linux-gnu \
    libc6-dev-arm64-cross \
    && rm -rf /var/lib/apt/lists/*

# Add ARM64 target
RUN rustup target add aarch64-unknown-linux-gnu

# Copy source code
COPY . .

# Build for ARM64 (without swagger-ui for production)
ENV CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc
RUN cargo build --release --target aarch64-unknown-linux-gnu \
    --no-default-features \
    --features "modbus,redis,sqlite" \
    -p comsrv -p modsrv

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy binaries from builder
COPY --from=builder /usr/src/app/target/aarch64-unknown-linux-gnu/release/comsrv /usr/local/bin/
COPY --from=builder /usr/src/app/target/aarch64-unknown-linux-gnu/release/modsrv /usr/local/bin/

RUN chmod +x /usr/local/bin/*

WORKDIR /app

# Default to comsrv
CMD ["comsrv"]
EOF

    DOCKERFILE="/tmp/Dockerfile.arm64"
fi

build_and_save \
    "$DOCKERFILE" \
    "$ROOT_DIR" \
    "voltageems:latest" \
    "$OUTPUT_DIR/voltageems.tar.gz"

# Build Frontend (Vue.js)
echo ""
echo -e "${BLUE}[3/4] Building Frontend (Vue.js) for ARM64...${NC}"

FRONTEND_DOCKERFILE="$ROOT_DIR/apps/Dockerfile"
if [[ -f "$FRONTEND_DOCKERFILE" ]]; then
    build_and_save \
        "$FRONTEND_DOCKERFILE" \
        "$ROOT_DIR/apps" \
        "voltage-apps:latest" \
        "$OUTPUT_DIR/apps.tar.gz"
else
    echo -e "${YELLOW}Warning: Frontend Dockerfile not found at $FRONTEND_DOCKERFILE${NC}"
    echo -e "${YELLOW}Skipping frontend build...${NC}"
fi

# Copy docker-compose.yml
echo ""
echo -e "${BLUE}[4/4] Copying docker-compose.yml...${NC}"

if [[ -f "$ROOT_DIR/docker-compose.yml" ]]; then
    cp "$ROOT_DIR/docker-compose.yml" "$OUTPUT_DIR/"
    echo -e "${GREEN}[DONE] Copied docker-compose.yml${NC}"
else
    echo -e "${YELLOW}Warning: docker-compose.yml not found${NC}"
fi

# Summary
echo ""
echo -e "${BLUE}========================================${NC}"
echo -e "${GREEN}  Docker Build Complete!                ${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""
echo "Built images:"
ls -lh "$OUTPUT_DIR"/*.tar.gz 2>/dev/null || echo "  No images found"
echo ""
echo "Total size: $(du -sh "$OUTPUT_DIR" | cut -f1)"
echo ""
echo "To load images on ARM64 device:"
echo "  docker load < voltage-redis.tar.gz"
echo "  docker load < voltageems.tar.gz"
if [[ -f "$OUTPUT_DIR/apps.tar.gz" ]]; then
    echo "  docker load < apps.tar.gz"
fi
echo ""
echo "To start services:"
echo "  docker-compose up -d"
