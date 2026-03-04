#!/usr/bin/env bash
# Build multi-architecture installer package for VoltageEMS
# Usage: build-installer.sh [VERSION] [ARCH] [TARGET] [--services=...] [--with-swagger]
#   VERSION: Version string (default: YYYYMMDD)
#   ARCH: arm64 | amd64 (default: arm64)
#   TARGET: Rust target triple (default based on ARCH)
#   --services: Comma-separated list of services to include (optional, default: all)
#   --with-swagger: Enable Swagger UI in Rust services (comsrv, modsrv)
#
# Service names: comsrv, modsrv, hissrv, apigateway, netsrv, alarmsrv, apps, redis, influxdb
# Service groups: python/py (all Python services), rust (all Rust services)
#
# Examples:
#   ./build-installer.sh                                    # Build all services (ARM64, today's date)
#   ./build-installer.sh v1.2.0 arm64                       # Build all services (ARM64, v1.2.0)
#   ./build-installer.sh v1.2.0 arm64 --services=python     # All Python services
#   ./build-installer.sh v1.2.0 arm64 -s rust               # All Rust services
#   ./build-installer.sh v1.2.0 arm64 -s netsrv,hissrv      # Specific Python services
#   ./build-installer.sh v1.2.0 arm64 --with-swagger        # Build with Swagger UI enabled

set -euo pipefail

# Disable macOS resource fork files
export COPYFILE_DISABLE=1
export COPY_EXTENDED_ATTRIBUTES_DISABLE=1

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Paths
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
BUILD_DIR="$ROOT_DIR/build/installer"
OUTPUT_DIR="$ROOT_DIR/release"

# Parse arguments
VERSION=""
ARCH=""
TARGET=""
SELECTED_SERVICES=""
ENABLE_SWAGGER=0

# Parse all arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --services=*)
            SELECTED_SERVICES="${1#*=}"
            shift
            ;;
        -s|--services)
            SELECTED_SERVICES="$2"
            shift 2
            ;;
        --with-swagger)
            ENABLE_SWAGGER=1
            shift
            ;;
        *)
            if [[ -z "$VERSION" ]]; then
                VERSION="$1"
            elif [[ -z "$ARCH" ]]; then
                ARCH="$1"
            elif [[ -z "$TARGET" ]]; then
                TARGET="$1"
            fi
            shift
            ;;
    esac
done

# Set defaults
VERSION="${VERSION:-$(date +%Y%m%d)}"
ARCH="${ARCH:-arm64}"

# Set defaults based on architecture
case "$ARCH" in
  arm64)
    TARGET="${TARGET:-aarch64-unknown-linux-musl}"
    DOCKER_PLATFORM="linux/arm64"
    ;;
  amd64)
    TARGET="${TARGET:-x86_64-unknown-linux-musl}"
    DOCKER_PLATFORM="linux/amd64"
    ;;
  *)
    echo -e "${RED}Error: Unknown architecture '$ARCH'. Use 'arm64' or 'amd64'${NC}"
    exit 1
    ;;
esac

FOLDER_NAME="MonarchEdge"

# Expand service group shortcuts
expand_service_groups() {
    local input="$1"
    local expanded=""
    
    IFS=',' read -ra ITEMS <<< "$input"
    for item in "${ITEMS[@]}"; do
        item=$(echo "$item" | xargs)  # trim whitespace
        case "$item" in
            python|py)
                expanded="${expanded}hissrv,apigateway,netsrv,alarmsrv,"
                ;;
            rust)
                expanded="${expanded}comsrv,modsrv,"
                ;;
            *)
                expanded="${expanded}${item},"
                ;;
        esac
    done
    
    # Remove trailing comma and duplicates
    echo "$expanded" | sed 's/,$//' | tr ',' '\n' | sort -u | tr '\n' ',' | sed 's/,$//'
}

# Expand service groups if shortcuts are used
if [[ -n "$SELECTED_SERVICES" ]]; then
    SELECTED_SERVICES=$(expand_service_groups "$SELECTED_SERVICES")
fi

# Adjust package name based on selected services
if [[ -n "$SELECTED_SERVICES" ]]; then
    # Create a shorter suffix for the package name
    SERVICES_SUFFIX=$(echo "$SELECTED_SERVICES" | tr ',' '-')
    PACKAGE_NAME="MonarchEdge-${ARCH}-${VERSION}-${SERVICES_SUFFIX}"
else
    PACKAGE_NAME="MonarchEdge-${ARCH}-${VERSION}"
fi

# Service to image mapping
declare -A SERVICE_TO_IMAGE=(
    ["comsrv"]="voltageems:latest"
    ["modsrv"]="voltageems:latest"
    ["hissrv"]="voltageems-ss:latest"
    ["apigateway"]="voltageems-ss:latest"
    ["netsrv"]="voltageems-ss:latest"
    ["alarmsrv"]="voltageems-ss:latest"
    ["apps"]="voltage-apps:latest"
    ["redis"]="redis:8-alpine"
    ["influxdb"]="influxdb:3-core"
)

# Parse selected services
declare -A BUILD_IMAGES
if [[ -n "$SELECTED_SERVICES" ]]; then
    IFS=',' read -ra SERVICES_ARRAY <<< "$SELECTED_SERVICES"
    for service in "${SERVICES_ARRAY[@]}"; do
        service=$(echo "$service" | xargs)  # trim whitespace
        if [[ -n "${SERVICE_TO_IMAGE[$service]:-}" ]]; then
            image="${SERVICE_TO_IMAGE[$service]}"
            BUILD_IMAGES["$image"]=1
        else
            echo -e "${RED}Error: Unknown service '$service'${NC}"
            echo "Available services: ${!SERVICE_TO_IMAGE[@]}"
            exit 1
        fi
    done
else
    # Build all images
    BUILD_IMAGES["voltageems:latest"]=1
    BUILD_IMAGES["voltageems-ss:latest"]=1
    BUILD_IMAGES["voltage-apps:latest"]=1
    BUILD_IMAGES["redis:8-alpine"]=1
    BUILD_IMAGES["influxdb:3-core"]=1
fi

# Detect CPU cores
if command -v nproc &> /dev/null; then
    CPU_CORES=$(nproc)
elif command -v sysctl &> /dev/null; then
    CPU_CORES=$(sysctl -n hw.ncpu)
else
    CPU_CORES=4
fi

echo -e "${BLUE}================================================${NC}"
echo -e "${BLUE}    MonarchEdge ${ARCH^^} Installer Builder     ${NC}"
echo -e "${BLUE}================================================${NC}"
echo ""
echo -e "Version:      ${GREEN}$VERSION${NC}"
echo -e "Architecture: ${GREEN}$ARCH${NC}"
echo -e "Target:       ${GREEN}$TARGET${NC}"
echo -e "Platform:     ${GREEN}$DOCKER_PLATFORM${NC}"
echo -e "CPU Cores:    ${GREEN}$CPU_CORES${NC}"
if [[ "$ENABLE_SWAGGER" == "1" ]]; then
    echo -e "Swagger UI:   ${GREEN}ENABLED${NC}"
else
    echo -e "Swagger UI:   ${YELLOW}disabled${NC}"
fi
if [[ -n "$SELECTED_SERVICES" ]]; then
    echo -e "Services:     ${YELLOW}$SELECTED_SERVICES (partial build)${NC}"
    echo -e "Images:       ${YELLOW}${!BUILD_IMAGES[@]}${NC}"
else
    echo -e "Services:     ${GREEN}ALL (full build)${NC}"
fi
echo ""

# Check for makeself
if ! command -v makeself &> /dev/null; then
    echo -e "${YELLOW}Warning: makeself not found. Installing...${NC}"
    if [[ "$OSTYPE" == "darwin"* ]]; then
        brew install makeself
    else
        echo -e "${RED}Please install makeself first${NC}"
        exit 1
    fi
fi

# Helper functions
copy_config_files() {
    local src="$1"
    local dst="$2"
    if [[ -d "$src" ]]; then
        find "$src" -type d | while read dir; do
            rel_dir="${dir#$src}"
            [[ -n "$rel_dir" ]] && mkdir -p "$dst$rel_dir"
        done
        find "$src" -type f \( -name "*.yaml" -o -name "*.yml" -o -name "*.csv" -o -name "*.json" \) | while read file; do
            rel_path="${file#$src}"
            cp "$file" "$dst$rel_path"
        done
    fi
}

copy_docker_images() {
    local src="$1"
    local dst="$2"
    if [[ -d "$src" ]]; then
        mkdir -p "$dst"
        find "$src" -name "*.tar.gz" -type f -exec cp {} "$dst/" \;
    fi
}

# Build unified Python services image for ARM64
build_python_services() {
    local context="$ROOT_DIR"
    local dockerfile="$ROOT_DIR/services/python-services/Dockerfile"
    local tag="voltageems-ss:latest"
    local output="$BUILD_DIR/docker/python-services.tar.gz"

    if [[ ! -f "$dockerfile" ]]; then
        echo -e "${RED}Error: Unified Python services Dockerfile not found: $dockerfile${NC}"
        return 1
    fi

    echo -e "${BLUE}Building unified Python services image $tag for $ARCH...${NC}"
    if docker build --platform $DOCKER_PLATFORM \
        -f "$dockerfile" \
        -t "$tag" \
        "$context"; then
        docker save "$tag" | gzip > "$output"
        local size=$(ls -lh "$output" | awk '{print $5}')
        echo -e "${GREEN}✓ Saved python-services.tar.gz ($size)${NC}"
    else
        echo -e "${RED}Error: Failed to build $tag${NC}"
        return 1
    fi
}

pull_and_save_image() {
    local image=$1
    local output_name=$2
    local output_path="$BUILD_DIR/docker/$output_name"

    # 优先尝试使用 skopeo (不需要本地 Docker 参与，完美解决架构和 manifest 问题)
    if command -v skopeo &> /dev/null; then
        echo -e "${BLUE}Using skopeo to directly fetch $image for $ARCH...${NC}"
        local base_tar="${output_path%.gz}"
        
        # 补全官方镜像的完整路径 (skopeo 需要)
        local full_image="$image"
        [[ "$image" != *"/"* ]] && full_image="docker.io/library/$image"

        if skopeo copy --override-os linux --override-arch "$ARCH" \
            "docker://$full_image" \
            "docker-archive:$base_tar:$image" > /dev/null; then
            gzip -f "$base_tar"
            local size=$(ls -lh "$output_path" | awk '{print $5}')
            echo -e "${GREEN}✓ Saved $output_name using skopeo ($size)${NC}"
            return 0
        else
            echo -e "${YELLOW}Warning: skopeo failed, falling back to docker...${NC}"
        fi
    fi

    # 兜底逻辑：传统的 docker pull + save
    if docker image inspect "$image" &>/dev/null; then
        echo -e "${GREEN}Using existing local image: $image${NC}"
    else
        echo -e "${BLUE}Pulling $image for $ARCH...${NC}"
        docker pull --platform $DOCKER_PLATFORM "$image"
    fi

    docker save "$image" | gzip > "$output_path"
    local size=$(ls -lh "$output_path" | awk '{print $5}')
    echo -e "${GREEN}✓ Saved $output_name ($size)${NC}"
}

# Clean and create build directory
echo -e "${YELLOW}Preparing build directory...${NC}"
rm -rf "$BUILD_DIR"
mkdir -p "$BUILD_DIR"/{tools,docker,config,scripts}
mkdir -p "$OUTPUT_DIR"

# Step 1: Build Monarch CLI (only if Rust services are selected)
echo ""
if [[ -n "${BUILD_IMAGES[voltageems:latest]:-}" ]]; then
    echo -e "${BLUE}[1/5] Building Monarch CLI for $ARCH...${NC}"

    # Check for cargo-zigbuild
    if ! command -v cargo-zigbuild &> /dev/null; then
        echo -e "${YELLOW}Installing cargo-zigbuild...${NC}"
        cargo install cargo-zigbuild
    fi

    # Check if rust target is installed
    if ! rustup target list --installed | grep -q "$TARGET"; then
        echo -e "${YELLOW}Installing $TARGET target...${NC}"
        rustup target add $TARGET
    fi

    # Build monarch CLI
    CARGO_BUILD_JOBS=$CPU_CORES cargo zigbuild --release --target $TARGET -p monarch
    if [[ -f "$ROOT_DIR/target/$TARGET/release/monarch" ]]; then
        cp "$ROOT_DIR/target/$TARGET/release/monarch" "$BUILD_DIR/tools/"
        echo -e "${GREEN}✓ Built monarch CLI${NC}"
    else
        echo -e "${RED}Error: Failed to build monarch${NC}"
        exit 1
    fi

    chmod +x "$BUILD_DIR/tools/"* 2>/dev/null || true
else
    echo -e "${YELLOW}[1/5] Skipping Monarch CLI (Rust services not selected)${NC}"
fi

# Step 2: Build Docker images
echo ""
echo -e "${BLUE}[2/5] Building Docker images for $ARCH...${NC}"

# Build Rust services if needed
if [[ -n "${BUILD_IMAGES[voltageems:latest]:-}" ]]; then
    echo -e "${BLUE}Building Rust services...${NC}"

    SWAGGER_FLAG=""
    if [[ "$ENABLE_SWAGGER" == "1" ]]; then
        SWAGGER_FLAG="--features swagger-ui"
        echo -e "${GREEN}Building with Swagger UI ENABLED${NC}"
    else
        echo -e "${YELLOW}Building without Swagger UI (use --with-swagger to enable)${NC}"
    fi

    CARGO_BUILD_JOBS=$CPU_CORES cargo zigbuild --release --target $TARGET \
        -p comsrv -p modsrv $SWAGGER_FLAG

    for service in comsrv modsrv; do
        if [[ ! -f "$ROOT_DIR/target/$TARGET/release/$service" ]]; then
            echo -e "${RED}Error: Failed to build $service${NC}"
            exit 1
        fi
        echo -e "${GREEN}✓ Built $service${NC}"
    done

    # Build voltageems Docker image using pre-compiled binaries
    echo -e "${BLUE}Building VoltageEMS Docker image...${NC}"
    if docker build --platform $DOCKER_PLATFORM \
        --build-arg TARGET_TRIPLE=$TARGET \
        -f "$ROOT_DIR/Dockerfile" \
        -t voltageems:latest \
        "$ROOT_DIR"; then
        docker save voltageems:latest | gzip > "$BUILD_DIR/docker/voltageems.tar.gz"
        sync
        echo -e "${GREEN}✓ Saved voltageems.tar.gz${NC}"
    else
        echo -e "${RED}Error: Docker build failed${NC}"
        exit 1
    fi
else
    echo -e "${YELLOW}⊘ Skipping Rust services (not selected)${NC}"
fi

# Build Python services if needed
if [[ -n "${BUILD_IMAGES[voltageems-ss:latest]:-}" ]]; then
    echo -e "${BLUE}Building unified Python services...${NC}"
    build_python_services
else
    echo -e "${YELLOW}⊘ Skipping Python services (not selected)${NC}"
fi

# Build Frontend if needed
if [[ -n "${BUILD_IMAGES[voltage-apps:latest]:-}" ]]; then
    echo -e "${BLUE}Building Frontend (Vue.js)...${NC}"
    FRONTEND_DOCKERFILE="$ROOT_DIR/apps/Dockerfile"
    if [[ -f "$FRONTEND_DOCKERFILE" ]]; then
        echo -e "${BLUE}Building voltage-apps:latest for $ARCH...${NC}"
        if docker build --platform $DOCKER_PLATFORM \
            -f "$FRONTEND_DOCKERFILE" \
            -t voltage-apps:latest \
            "$ROOT_DIR/apps"; then
            docker save voltage-apps:latest | gzip > "$BUILD_DIR/docker/apps.tar.gz"
            sync
            size=$(ls -lh "$BUILD_DIR/docker/apps.tar.gz" | awk '{print $5}')
            echo -e "${GREEN}✓ Saved apps.tar.gz ($size)${NC}"
        else
            echo -e "${YELLOW}Warning: Frontend build failed, continuing without frontend...${NC}"
        fi
    else
        echo -e "${YELLOW}Warning: Frontend Dockerfile not found at $FRONTEND_DOCKERFILE${NC}"
        echo -e "${YELLOW}Skipping frontend build...${NC}"
    fi
else
    echo -e "${YELLOW}⊘ Skipping Frontend (not selected)${NC}"
fi

# Pull official images if needed
echo -e "${BLUE}Pulling official images...${NC}"
if [[ -n "${BUILD_IMAGES[redis:8-alpine]:-}" ]]; then
    pull_and_save_image "redis:8-alpine" "voltage-redis.tar.gz"
else
    echo -e "${YELLOW}⊘ Skipping redis:8-alpine (not selected)${NC}"
fi

if [[ -n "${BUILD_IMAGES[influxdb:3-core]:-}" ]]; then
    pull_and_save_image "influxdb:3-core" "voltage-influxdb.tar.gz"
else
    echo -e "${YELLOW}⊘ Skipping influxdb:3-core (not selected)${NC}"
fi

# Pull alpine image for upgrade container (always include)
echo -e "${BLUE}Pulling alpine:latest for upgrade container...${NC}"
pull_and_save_image "alpine:latest" "alpine.tar.gz"

# Verify images
echo -e "${YELLOW}Verifying Docker images...${NC}"

# Build list of expected images based on what was selected
declare -A EXPECTED_IMAGES
if [[ -n "${BUILD_IMAGES[voltageems:latest]:-}" ]]; then
    EXPECTED_IMAGES["voltageems"]=1
fi
if [[ -n "${BUILD_IMAGES[voltageems-ss:latest]:-}" ]]; then
    EXPECTED_IMAGES["python-services"]=1
fi
if [[ -n "${BUILD_IMAGES[voltage-apps:latest]:-}" ]]; then
    EXPECTED_IMAGES["apps"]=1
fi
if [[ -n "${BUILD_IMAGES[redis:8-alpine]:-}" ]]; then
    EXPECTED_IMAGES["voltage-redis"]=1
fi
if [[ -n "${BUILD_IMAGES[influxdb:3-core]:-}" ]]; then
    EXPECTED_IMAGES["voltage-influxdb"]=1
fi

# Verify only the images that should exist
for img in "${!EXPECTED_IMAGES[@]}"; do
    if [[ ! -f "$BUILD_DIR/docker/$img.tar.gz" ]]; then
        echo -e "${RED}✗ $img.tar.gz not found!${NC}"
        exit 1
    fi
    size=$(ls -lh "$BUILD_DIR/docker/$img.tar.gz" | awk '{print $5}')
    echo -e "${GREEN}✓ $img.tar.gz ($size)${NC}"
done

# Show summary
echo ""
echo -e "${GREEN}[DONE] Docker images prepared${NC}"
if [[ -n "$SELECTED_SERVICES" ]]; then
    echo -e "${YELLOW}Built ${#EXPECTED_IMAGES[@]} image(s) for selected services${NC}"
else
    echo -e "${GREEN}Built all ${#EXPECTED_IMAGES[@]} image(s)${NC}"
fi

# Step 3: Copy configuration templates
echo ""
echo -e "${BLUE}[3/5] Copying configuration templates...${NC}"

if [[ -d "$ROOT_DIR/config.template" ]]; then
    copy_config_files "$ROOT_DIR/config.template" "$BUILD_DIR/config.template"
    echo -e "${GREEN}✓ Copied config.template${NC}"
fi

[[ -f "$ROOT_DIR/docker-compose.yml" ]] && cp "$ROOT_DIR/docker-compose.yml" "$BUILD_DIR/"

echo -e "${GREEN}[DONE] Configuration templates copied${NC}"

# Step 4: Copy and customize installation script
echo ""
echo -e "${BLUE}[4/5] Copying installation script...${NC}"

if [[ -f "$ROOT_DIR/scripts/install.sh" ]]; then
    cp "$ROOT_DIR/scripts/install.sh" "$BUILD_DIR/install.sh"
    chmod +x "$BUILD_DIR/install.sh"

    # Customize script for target architecture
    if [[ "$ARCH" == "amd64" ]]; then
        echo -e "${YELLOW}Customizing install.sh for AMD64...${NC}"
        # Replace ARM64 references with AMD64
        sed -i.bak \
            -e 's/ARM64/AMD64/g' \
            -e 's/arm64/amd64/g' \
            -e 's/aarch64/x86_64/g' \
            "$BUILD_DIR/install.sh"
        rm -f "$BUILD_DIR/install.sh.bak"
    fi
    echo -e "${GREEN}[DONE] Installation script copied${NC}"
else
    echo -e "${RED}Error: install.sh not found${NC}"
    exit 1
fi

# Step 5: Create self-extracting installer
echo ""
echo -e "${BLUE}[5/5] Creating self-extracting installer...${NC}"

cd "$BUILD_DIR/.."
TEMP_PKG_DIR="MonarchEdge-temp-$$"
mkdir -p "$TEMP_PKG_DIR"

cp "$BUILD_DIR/install.sh" "$TEMP_PKG_DIR/"
chmod +x "$TEMP_PKG_DIR/install.sh"

[[ -d "$BUILD_DIR/config.template" ]] && copy_config_files "$BUILD_DIR/config.template" "$TEMP_PKG_DIR/config.template"
[[ -d "$BUILD_DIR/docker" ]] && copy_docker_images "$BUILD_DIR/docker" "$TEMP_PKG_DIR/docker"

mkdir -p "$TEMP_PKG_DIR/tools"
if [[ -f "$BUILD_DIR/tools/monarch" ]]; then
    cp "$BUILD_DIR/tools/monarch" "$TEMP_PKG_DIR/tools/"
    echo -e "${GREEN}✓ Included monarch CLI${NC}"
elif [[ -n "${BUILD_IMAGES[voltageems:latest]:-}" ]]; then
    # Monarch is required for Rust services
    echo -e "${RED}Error: monarch binary not found but Rust services are selected${NC}"
    rm -rf "$TEMP_PKG_DIR"
    exit 1
else
    # Monarch is optional for Python-only packages
    echo -e "${YELLOW}⊘ Skipping monarch CLI (Python-only package)${NC}"
fi

[[ -f "$BUILD_DIR/docker-compose.yml" ]] && cp "$BUILD_DIR/docker-compose.yml" "$TEMP_PKG_DIR/"

# Ensure all files are written to disk before creating archive
sync

# Create installer description
if [[ -n "$SELECTED_SERVICES" ]]; then
    INSTALLER_DESC="VoltageEMS ${ARCH^^} Partial Update ($SELECTED_SERVICES) $VERSION"
else
    INSTALLER_DESC="VoltageEMS ${ARCH^^} Full Installer $VERSION"
fi

makeself --gzip "$TEMP_PKG_DIR" "$OUTPUT_DIR/${PACKAGE_NAME}.run" \
    "$INSTALLER_DESC" \
    bash ./install.sh

rm -rf "$TEMP_PKG_DIR"

RUN_SIZE=$(ls -lh "$OUTPUT_DIR/${PACKAGE_NAME}.run" 2>/dev/null | awk '{print $5}')

echo ""
echo -e "${GREEN}================================================${NC}"
echo -e "${GREEN}       Build Complete!                          ${NC}"
echo -e "${GREEN}================================================${NC}"
echo ""
echo -e "Package: ${GREEN}$OUTPUT_DIR/${PACKAGE_NAME}.run${NC} (${YELLOW}$RUN_SIZE${NC})"
if [[ -n "$SELECTED_SERVICES" ]]; then
    echo -e "Type:    ${YELLOW}Partial Update${NC}"
    echo -e "Services: ${YELLOW}$SELECTED_SERVICES${NC}"
    echo -e "Images:   ${YELLOW}${!BUILD_IMAGES[@]}${NC}"
else
    echo -e "Type:    ${GREEN}Full Installation${NC}"
fi
echo ""
echo "Installation:"
echo "  scp ${PACKAGE_NAME}.run user@device:/tmp/"
echo "  ssh user@device 'chmod +x /tmp/${PACKAGE_NAME}.run && sudo /tmp/${PACKAGE_NAME}.run'"
echo ""

# Cleanup
rm -rf "$BUILD_DIR"
echo -e "${GREEN}Done!${NC}"
