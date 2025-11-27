#!/usr/bin/env bash
# Build complete ARM64 installer package for VoltageEMS
# Creates a self-contained installer with tools and Docker images

set -euo pipefail

# Disable macOS resource fork files from the start
export COPYFILE_DISABLE=1
export COPY_EXTENDED_ATTRIBUTES_DISABLE=1
export DSSTORE_DISABLE=1

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Paths
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
BUILD_DIR="$ROOT_DIR/build/arm64-installer"
OUTPUT_DIR="$ROOT_DIR/release"

# Version - support both date and semantic versioning
VERSION="${1:-$(date +%Y%m%d)}"
FOLDER_NAME="MonarchEdge"
PACKAGE_NAME="MonarchEdge-arm64-${VERSION}"

# Target architecture for cross-compilation
TARGET="aarch64-unknown-linux-musl"

# Detect CPU cores for parallel compilation
if command -v nproc &> /dev/null; then
    CPU_CORES=$(nproc)
elif command -v sysctl &> /dev/null; then
    CPU_CORES=$(sysctl -n hw.ncpu)
else
    CPU_CORES=4  # Fallback to 4 cores
fi

echo -e "${BLUE}================================================${NC}"
echo -e "${BLUE}    MonarchEdge ARM64 Installer Builder         ${NC}"
echo -e "${BLUE}================================================${NC}"
echo ""
echo -e "${GREEN}Detected CPU cores: $CPU_CORES${NC}"
echo ""

# Check for makeself
if ! command -v makeself &> /dev/null; then
    echo -e "${YELLOW}Warning: makeself not found. Installing...${NC}"
    if [[ "$OSTYPE" == "darwin"* ]]; then
        brew install makeself
    else
        echo -e "${RED}Please install makeself first:${NC}"
        echo "  Ubuntu/Debian: sudo apt-get install makeself"
        echo "  CentOS/RHEL: sudo yum install makeself"
        exit 1
    fi
fi

# Copy only configuration files (whitelist approach)
copy_config_files() {
    local src="$1"
    local dst="$2"

    echo "  Copying configuration files from $src to $dst"

    # Create destination directory structure
    if [[ -d "$src" ]]; then
        # Copy directory structure first
        find "$src" -type d | while read dir; do
            rel_dir="${dir#$src}"
            [[ -n "$rel_dir" ]] && mkdir -p "$dst$rel_dir"
        done

        # Copy only specific file types
        find "$src" -type f \( -name "*.yaml" -o -name "*.yml" -o -name "*.csv" -o -name "*.json" \) | while read file; do
            rel_path="${file#$src}"
            cp "$file" "$dst$rel_path"
        done
    fi
}

# Copy Docker images (tar.gz files only)
copy_docker_images() {
    local src="$1"
    local dst="$2"

    if [[ -d "$src" ]]; then
        mkdir -p "$dst"
        find "$src" -name "*.tar.gz" -type f -exec cp {} "$dst/" \;
    fi
}

# Clean and create build directory
echo -e "${YELLOW}Preparing build directory...${NC}"
rm -rf "$BUILD_DIR"
mkdir -p "$BUILD_DIR"/{tools,docker,config,scripts}
mkdir -p "$OUTPUT_DIR"

# Step 1: Build Monarch CLI tool
echo ""
echo -e "${BLUE}[1/5] Preparing Monarch CLI for ARM64...${NC}"

# Use the dedicated build script for CLI tools
if [[ -f "$SCRIPT_DIR/offline/build-monarch-arm64.sh" ]]; then
    echo "Building Monarch using build-monarch-arm64.sh..."
    "$SCRIPT_DIR/offline/build-monarch-arm64.sh"

    # Copy the built binary from offline-bundle
    if [[ -f "$ROOT_DIR/offline-bundle/cli/linux-aarch64/bin/monarch" ]]; then
        cp -v "$ROOT_DIR/offline-bundle/cli/linux-aarch64/bin/monarch" "$BUILD_DIR/tools/"
        echo -e "${GREEN}Monarch CLI built and copied successfully${NC}"
    else
        echo -e "${RED}Error: Monarch binary not found after build${NC}"
        echo "Expected location: $ROOT_DIR/offline-bundle/cli/linux-aarch64/bin/monarch"
        exit 1
    fi
else
    # Fallback: check if pre-built binary exists
    if [[ -f "$ROOT_DIR/offline-bundle/cli/linux-aarch64/bin/monarch" ]]; then
        echo "Using existing monarch binary from offline-bundle"
        cp -v "$ROOT_DIR/offline-bundle/cli/linux-aarch64/bin/monarch" "$BUILD_DIR/tools/"
        echo -e "${GREEN}Monarch CLI ready${NC}"
    elif [[ -f "$ROOT_DIR/target/aarch64-unknown-linux-musl/release/monarch" ]]; then
        echo "Using existing monarch binary from target directory"
        cp -v "$ROOT_DIR/target/aarch64-unknown-linux-musl/release/monarch" "$BUILD_DIR/tools/"
        echo -e "${GREEN}Monarch CLI ready${NC}"
    else
        echo -e "${RED}Error: build-monarch-arm64.sh not found and no pre-built binary available${NC}"
        echo "Please ensure scripts/offline/build-monarch-arm64.sh exists"
        exit 1
    fi
fi

# Make tools executable
chmod +x "$BUILD_DIR/tools/"* 2>/dev/null || true

echo -e "${GREEN}[DONE] CLI tools built${NC}"

# Step 2: Build Docker images
echo ""
echo -e "${BLUE}[2/5] Building Docker images for ARM64...${NC}"

# Always rebuild Docker images to ensure latest version
echo -e "${YELLOW}Building Docker images...${NC}"

# Clean old Docker build cache to ensure fresh build
echo "Cleaning old Docker build artifacts..."
rm -rf "$ROOT_DIR/offline-bundle/docker"
mkdir -p "$BUILD_DIR/docker"

# Build VoltageEMS services image using pre-compiled binaries
echo -e "${YELLOW}Building VoltageEMS Docker image with pre-compiled binaries...${NC}"

# Check if cargo-zigbuild is installed
if ! command -v cargo-zigbuild &> /dev/null; then
    echo -e "${YELLOW}Installing cargo-zigbuild for cross-compilation...${NC}"
    cargo install cargo-zigbuild
fi

# Check if rust target is installed
if ! rustup target list --installed | grep -q "$TARGET"; then
    echo -e "${YELLOW}Installing $TARGET target...${NC}"
    rustup target add $TARGET
fi

# Check if Swagger UI should be enabled (default: disabled for production builds)
ENABLE_SWAGGER_UI="${ENABLE_SWAGGER_UI:-0}"

# Determine build features based on environment variable
# Note: Features are combined for all services (comsrv, modsrv)
# - comsrv needs: modbus, can, [swagger-ui]
# - modsrv needs: redis, sqlite, [swagger-ui]
if [[ "$ENABLE_SWAGGER_UI" == "1" ]]; then
    CARGO_FEATURES="modbus,can,redis,sqlite,swagger-ui"
    echo -e "${GREEN}Building with Swagger UI ENABLED (set ENABLE_SWAGGER_UI=0 to disable)${NC}"
else
    CARGO_FEATURES="modbus,can,redis,sqlite"
    echo -e "${YELLOW}Building without Swagger UI (default for production)${NC}"
fi

# Build services using zigbuild
echo -e "${BLUE}Building services for ARM64 with $CPU_CORES parallel jobs...${NC}"
cd "$ROOT_DIR"
CARGO_BUILD_JOBS=$CPU_CORES cargo zigbuild --release --target $TARGET \
    --no-default-features \
    --features "$CARGO_FEATURES" \
    -p comsrv -p modsrv

# Check if binaries were built
SERVICES="comsrv modsrv"
for service in $SERVICES; do
    if [[ ! -f "$ROOT_DIR/target/$TARGET/release/$service" ]]; then
        echo -e "${RED}Error: Failed to build $service${NC}"
        exit 1
    fi
    echo -e "${GREEN}✓ Built $service${NC}"
done

# Build Docker image
echo -e "${BLUE}Creating Docker image with $CPU_CORES parallel jobs...${NC}"
docker build \
    --build-arg BUILD_JOBS=$CPU_CORES \
    --build-arg ENABLE_SWAGGER_UI="$ENABLE_SWAGGER_UI" \
    -t voltageems:latest .

if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓ VoltageEMS Docker image built${NC}"

    # Save the built image
    docker save voltageems:latest | gzip > "$BUILD_DIR/docker/voltageems.tar.gz"
    echo -e "${GREEN}✓ Saved voltageems.tar.gz${NC}"
else
    echo -e "${RED}Error: Docker build failed${NC}"
    exit 1
fi

# Pull official Redis image for ARM64
echo -e "${BLUE}Pulling official Redis 8 Alpine for ARM64...${NC}"

docker pull --platform linux/arm64 redis:8-alpine

if [ $? -eq 0 ]; then
    echo -e "${GREEN}✓ Redis image pulled successfully${NC}"

    # Save Redis image (using official tag)
    echo "Saving Redis image..."
    docker save redis:8-alpine | gzip > "$BUILD_DIR/docker/voltage-redis.tar.gz"
    echo -e "${GREEN}✓ Saved voltage-redis.tar.gz${NC}"
else
    echo -e "${RED}Error: Failed to pull Redis image${NC}"
    exit 1
fi

# Verify we have the required images
if [[ ! -f "$BUILD_DIR/docker/voltageems.tar.gz" ]]; then
    echo -e "${RED}voltageems.tar.gz not found!${NC}"
    exit 1
fi
if [[ ! -f "$BUILD_DIR/docker/voltage-redis.tar.gz" ]]; then
    echo -e "${RED}voltage-redis.tar.gz not found!${NC}"
    exit 1
fi

# Copy docker-compose.yml
if [[ -f "$ROOT_DIR/docker-compose.yml" ]]; then
    echo -e "${GREEN}Copying docker-compose configuration${NC}"
    cp -v "$ROOT_DIR/docker-compose.yml" "$BUILD_DIR/"
fi

echo -e "${GREEN}[DONE] Docker images prepared${NC}"

# Step 3: Copy configuration templates
echo ""
echo -e "${BLUE}[3/5] Copying configuration templates...${NC}"

# Copy config.template directory (primary configuration source)
if [[ -d "$ROOT_DIR/config.template" ]]; then
    echo -e "${YELLOW}Copying config.template directory...${NC}"
    copy_config_files "$ROOT_DIR/config.template" "$BUILD_DIR/config.template"
    echo -e "${GREEN}✓ Copied config.template${NC}"
elif [[ -d "$ROOT_DIR/config" ]]; then
    echo -e "${YELLOW}config.template not found, using config/ directory...${NC}"
    copy_config_files "$ROOT_DIR/config" "$BUILD_DIR/config.template"
    echo -e "${GREEN}✓ Created config.template from config/${NC}"
else
    echo -e "${YELLOW}Warning: No configuration templates found, creating minimal structure...${NC}"
    mkdir -p "$BUILD_DIR/config.template"/{comsrv,modsrv,rulesrv,alarmsrv,hissrv}
fi

# Copy docker-compose.yml
if [[ -f "$ROOT_DIR/docker-compose.yml" ]]; then
    echo -e "${YELLOW}Copying docker-compose.yml...${NC}"
    cp "$ROOT_DIR/docker-compose.yml" "$BUILD_DIR/"
    echo -e "${GREEN}✓ Copied docker-compose.yml${NC}"
else
    echo -e "${RED}Warning: docker-compose.yml not found${NC}"
fi

echo -e "${GREEN}[DONE] Configuration templates copied${NC}"

# Step 4: Copy installation script
echo ""
echo -e "${BLUE}[4/5] Copying installation script...${NC}"

# Copy the pre-existing install.sh script
if [[ -f "$ROOT_DIR/scripts/install.sh" ]]; then
    cp "$ROOT_DIR/scripts/install.sh" "$BUILD_DIR/install.sh"
    chmod +x "$BUILD_DIR/install.sh"
    echo -e "${GREEN}[DONE] Installation script copied${NC}"
else
    echo -e "${RED}Error: install.sh not found at $ROOT_DIR/scripts/install.sh${NC}"
    exit 1
fi


# Step 5: Create self-extracting installer package
echo ""
echo -e "${BLUE}[5/5] Creating self-extracting installer...${NC}"

# Create proper directory structure for packaging
cd "$BUILD_DIR/.."
TEMP_PKG_DIR="MonarchEdge-temp-$$"
mkdir -p "$TEMP_PKG_DIR"

# Move install.sh to root level
cp "$BUILD_DIR/install.sh" "$TEMP_PKG_DIR/"
chmod +x "$TEMP_PKG_DIR/install.sh"

# Copy only necessary content (whitelist approach)
# 1. Config templates (only yaml, yml, csv, json files)
if [[ -d "$BUILD_DIR/config.template" ]]; then
    copy_config_files "$BUILD_DIR/config.template" "$TEMP_PKG_DIR/config.template"
fi

# 2. Docker images (only tar.gz files)
if [[ -d "$BUILD_DIR/docker" ]]; then
    copy_docker_images "$BUILD_DIR/docker" "$TEMP_PKG_DIR/docker"
fi

# 3. CLI tools (binary files)
if [[ -d "$BUILD_DIR/tools" ]]; then
    mkdir -p "$TEMP_PKG_DIR/tools"
    cp "$BUILD_DIR/tools/monarch" "$TEMP_PKG_DIR/tools/" 2>/dev/null || true
fi

# 4. Helper scripts
if [[ -d "$BUILD_DIR/scripts" ]]; then
    mkdir -p "$TEMP_PKG_DIR/scripts"
    cp "$BUILD_DIR/scripts/"*.sh "$TEMP_PKG_DIR/scripts/" 2>/dev/null || true
    chmod +x "$TEMP_PKG_DIR/scripts/"*.sh 2>/dev/null || true
fi

# 5. docker-compose.yml
[[ -f "$BUILD_DIR/docker-compose.yml" ]] && cp "$BUILD_DIR/docker-compose.yml" "$TEMP_PKG_DIR/"

# Use makeself to create self-extracting archive (already clean from whitelist approach)
makeself --gzip "$TEMP_PKG_DIR" "$OUTPUT_DIR/${PACKAGE_NAME}.run" \
    "VoltageEMS ARM64 Installer $VERSION" \
    bash ./install.sh

# Cleanup temp directory
rm -rf "$TEMP_PKG_DIR"

# Calculate package size
RUN_SIZE=$(ls -lh "$OUTPUT_DIR/${PACKAGE_NAME}.run" 2>/dev/null | awk '{print $5}')

echo ""
echo -e "${GREEN}================================================${NC}"
echo -e "${GREEN}       Build Complete!                          ${NC}"
echo -e "${GREEN}================================================${NC}"
echo ""
echo "Package created:"
echo "  • Self-extracting installer: $OUTPUT_DIR/${PACKAGE_NAME}.run ($RUN_SIZE)"
echo ""
echo "Installation Instructions:"
echo ""
echo "  1. Copy to target machine:"
echo "     scp $OUTPUT_DIR/${PACKAGE_NAME}.run user@arm-device:/tmp/"
echo ""
echo "  2. Run installer on target:"
echo "     ssh user@arm-device"
echo "     chmod +x /tmp/${PACKAGE_NAME}.run"
echo "     sudo /tmp/${PACKAGE_NAME}.run"
echo ""
echo "  Or in one line:"
echo "     scp $OUTPUT_DIR/${PACKAGE_NAME}.run user@arm-device:/tmp/ && \\"
echo "     ssh user@arm-device 'chmod +x /tmp/${PACKAGE_NAME}.run && sudo /tmp/${PACKAGE_NAME}.run'"
echo ""
echo "After installation:"
echo "  cd /opt/MonarchEdge      # Or use ~/docker-compose.yml symlink"
echo "  docker-compose up -d      # Start services"
echo "  docker-compose ps         # Check status"
echo "  monarch sync all          # Sync configurations"
echo ""

# Cleanup
echo -e "${YELLOW}Cleaning up build directory...${NC}"
rm -rf "$BUILD_DIR"

echo -e "${GREEN}Done!${NC}"
