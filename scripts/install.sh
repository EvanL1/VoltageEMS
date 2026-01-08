#!/usr/bin/env bash
# VoltageEMS ARM64 Installation Script

set -euo pipefail

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Installation directories (configurable via environment variables)
# Default: /opt/MonarchEdge for production, can be overridden
INSTALL_DIR="${VOLTAGE_INSTALL_DIR:-${INSTALL_DIR:-/opt/MonarchEdge}}"
# Allow logs to be stored on external storage if available
LOG_DIR="${VOLTAGE_LOG_DIR:-${LOG_DIR:-$INSTALL_DIR/logs}}"

# Save the directory where installation was launched (for cleanup later)
LAUNCH_DIR="${LAUNCH_DIR:-$(pwd)}"

# =============================================================================
# Command Line Arguments
# =============================================================================
AUTO_MODE=true  # Default to auto mode for production deployments
SHOW_HELP=false

while [[ $# -gt 0 ]]; do
    case $1 in
        -i|--interactive)
            AUTO_MODE=false
            shift
            ;;
        --help|-h)
            SHOW_HELP=true
            shift
            ;;
        *)
            echo -e "${RED}Unknown option: $1${NC}"
            echo "Usage: $0 [-i|--interactive] [--help|-h]"
            exit 1
            ;;
    esac
done

if [[ "$SHOW_HELP" == true ]]; then
    echo "MonarchEdge ARM64 Installation Script"
    echo ""
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Options:"
    echo "  -i, --interactive   Interactive mode: prompt for confirmations"
    echo "  --help, -h          Show this help message"
    echo ""
    echo "Examples:"
    echo "  $0                  Automatic update (default, no prompts)"
    echo "  $0 -i               Interactive installation with prompts"
    exit 0
fi

if [[ "$AUTO_MODE" == false ]]; then
    echo -e "${BLUE}Running in INTERACTIVE mode - will prompt for confirmations${NC}"
fi

# Docker Compose V1/V2 compatibility functions
detect_docker_compose_cmd() {
    if docker compose version &>/dev/null 2>&1; then
        echo "docker compose"
    elif command -v docker-compose &>/dev/null; then
        echo "docker-compose"
    else
        echo -e "${RED}ERROR: Neither 'docker compose' (V2) nor 'docker-compose' (V1) found${NC}" >&2
        echo -e "${YELLOW}Please install Docker Compose: https://docs.docker.com/compose/install/${NC}" >&2
        return 1
    fi
}

run_docker_compose() {
    local compose_cmd
    compose_cmd=$(detect_docker_compose_cmd) || return 1

    if [[ "$compose_cmd" == "docker compose" ]]; then
        docker compose "$@"
    else
        docker-compose "$@"
    fi
}

# =============================================================================
# Smart Update Helper Functions
# =============================================================================

# Tarball filename to image name mapping
declare -A TARBALL_TO_IMAGE=(
    ["voltageems.tar.gz"]="voltageems:latest"
    ["voltage-redis.tar.gz"]="redis:8-alpine"
    ["voltage-influxdb.tar.gz"]="influxdb:2-alpine"
    ["python-services.tar.gz"]="voltageems-ss:latest"
    ["apps.tar.gz"]="voltage-apps:latest"
)

# Image to container mapping
declare -A IMAGE_TO_CONTAINERS=(
    ["redis:8-alpine"]="voltage-redis"
    ["influxdb:2-alpine"]="voltage-influxdb"
    ["voltageems:latest"]="voltageems-comsrv voltageems-modsrv"
    ["voltageems-ss:latest"]="voltageems-hissrv voltageems-apigateway voltageems-netsrv voltageems-alarmsrv"
    ["voltage-apps:latest"]="voltage-apps"
)

# Container to service name mapping (for docker-compose)
declare -A CONTAINER_TO_SERVICE=(
    ["voltage-redis"]="voltage-redis"
    ["voltage-influxdb"]="influxdb"
    ["voltageems-comsrv"]="comsrv"
    ["voltageems-modsrv"]="modsrv"
    ["voltageems-hissrv"]="hissrv"
    ["voltageems-apigateway"]="apigateway"
    ["voltageems-netsrv"]="netsrv"
    ["voltageems-alarmsrv"]="alarmsrv"
    ["voltage-apps"]="apps"
)

# Generate backup tag for an image
# Example: redis:8-alpine -> redis:backup-8-alpine-1703260800
generate_backup_tag() {
    local image=$1
    local timestamp=${BACKUP_TIMESTAMP:-$(date +%s)}
    local name="${image%:*}"      # redis, voltageems
    local tag="${image##*:}"      # 8-alpine, latest
    echo "${name}:backup-${tag}-${timestamp}"
}

# Detect if an image has changed compared to running container
# Returns: "changed", "unchanged", or "not_running"
detect_image_change() {
    local image=$1
    local containers="${IMAGE_TO_CONTAINERS[$image]:-}"

    [[ -z "$containers" ]] && echo "unknown" && return

    # Get new loaded image ID
    local new_id
    new_id=$(docker images "$image" --format '{{.ID}}' 2>/dev/null)
    [[ -z "$new_id" ]] && echo "not_loaded" && return

    # Get running container's image ID (use first container)
    local first_container
    first_container=$(echo "$containers" | awk '{print $1}')
    local running_id
    running_id=$(docker inspect "$first_container" --format '{{.Image}}' 2>/dev/null | sed 's/sha256://; s/^\(.\{12\}\).*/\1/')

    if [[ -z "$running_id" ]]; then
        echo "not_running"
    elif [[ "$new_id" == "$running_id" ]]; then
        echo "unchanged"
    else
        echo "changed"
    fi
}

# Check if all containers are running
all_containers_running() {
    local containers=$1
    for container in $containers; do
        if ! docker ps --filter "name=^${container}$" --filter "status=running" -q | grep -q .; then
            return 1
        fi
    done
    return 0
}

# =============================================================================
# Smart Image Loading - Skip unchanged images
# =============================================================================

# Extract image ID from tar.gz without loading
# Docker save format: manifest.json contains Config field with image sha256
get_tarball_image_id() {
    local tarball=$1

    # Extract manifest.json and get the Config (image ID)
    # Support both docker save and skopeo formats
    # Use timeout to prevent hanging (30 seconds max)
    local raw_config
    raw_config=$(timeout 30 sh -c "zcat '$tarball' 2>/dev/null | tar -xOf - manifest.json 2>/dev/null | \
        sed -n 's/.*\"Config\":\"\([^\"]*\)\".*/\1/p' | head -1" 2>/dev/null)
    local extract_result=$?

    if [[ -z "$raw_config" ]]; then
        return
    fi

    # Clean up the path (skopeo uses blobs/sha256/HASH, docker uses HASH.json or sha256:HASH)
    local clean_hash
    clean_hash=$(basename "$raw_config" | sed 's/\.json$//' | sed 's/sha256://')

    # Return first 12 chars
    echo "${clean_hash:0:12}"
}

# Get local image ID by image name
get_local_image_id() {
    local image=$1
    local result
    result=$(docker images "$image" --format '{{.ID}}' 2>/dev/null | head -1)
    echo "$result"
}

# Smart load: only load if image has changed
# Special handling for multi-arch images (influxdb, redis)
# Returns: 0 if loaded, 1 if skipped (unchanged), 2 if error
smart_load_image() {
    local tarball=$1
    local basename
    basename=$(basename "$tarball")
    local image="${TARBALL_TO_IMAGE[$basename]:-}"

    # If no mapping found, load anyway
    if [[ -z "$image" ]]; then
        echo -n "  Loading $basename (unknown mapping)... "
        if timeout 300 docker load < "$tarball" >/dev/null 2>&1; then
            echo -e "${GREEN}done${NC}"
            return 0
        else
            echo -e "${RED}failed${NC}"
            return 2
        fi
    fi

    # Get tarball image ID (with timeout protection)
    local tarball_id
    tarball_id=$(get_tarball_image_id "$tarball")
    local extract_result=$?

    if [[ -z "$tarball_id" ]]; then
        # Cannot extract ID, load anyway
        echo -n "  Loading $basename (cannot extract ID)... "
        if timeout 300 docker load < "$tarball" >/dev/null 2>&1; then
            echo -e "${GREEN}done${NC}"
            return 0
        else
            echo -e "${RED}failed${NC}"
            return 2
        fi
    fi

    # Get local image ID
    local local_id
    local_id=$(get_local_image_id "$image")

    # Compare IDs
    if [[ -n "$local_id" && "$tarball_id" == "$local_id" ]]; then
        echo -e "  ${GREEN}✓${NC} $basename: ${GREEN}unchanged${NC} (skipped)"
        return 1
    fi

    # IDs differ or local not found - load the image
    if [[ -z "$local_id" ]]; then
        echo -n "  Loading $basename (new)... "
    else
        echo -n "  Loading $basename (${local_id:0:12} → ${tarball_id:0:12})... "
        # CRITICAL: Remove old image first, otherwise docker load may skip it!
        docker rmi "$image" >/dev/null 2>&1 || true
    fi

    # Use timeout to prevent hanging (5 minutes max for docker load)
    if timeout 300 docker load < "$tarball" >/dev/null 2>&1; then
        echo -e "${GREEN}done${NC}"
        return 0
    else
        echo -e "${RED}failed (timeout or error)${NC}"
        return 2
    fi
}

# Update a single service with backup and rollback capability
# Args: $1=image, $2=containers (space-separated)
# Returns: 0 on success, 1 on failure
update_service() {
    local image=$1
    local containers=$2
    local backup_tag
    backup_tag=$(generate_backup_tag "$image")

    echo -e "  ${BLUE}Updating: $image${NC}"

    # Step 1: Backup current image
    echo "    Backing up → $backup_tag"
    if ! docker tag "$image" "$backup_tag" 2>/dev/null; then
        echo -e "    ${YELLOW}Warning: Could not backup (image may not exist locally)${NC}"
    fi

    # Step 2: Stop and remove old containers
    for container in $containers; do
        docker stop "$container" 2>/dev/null || true
        docker rm "$container" 2>/dev/null || true
    done

    # Step 3: Start new containers (--no-deps avoids recreating running dependencies)
    for container in $containers; do
        local service="${CONTAINER_TO_SERVICE[$container]:-$container}"
        run_docker_compose -f "$INSTALL_DIR/docker-compose.yml" up -d --no-deps "$service"
    done

    # Step 4: Health check (wait for containers to start)
    sleep 3

    if all_containers_running "$containers"; then
        echo -e "    ${GREEN}✓ Update successful${NC}"
        # Remove backup on success
        docker rmi "$backup_tag" 2>/dev/null || true
        return 0
    else
        echo -e "    ${RED}✗ Update failed, rolling back...${NC}"
        # Rollback: restore backup image and restart
        docker tag "$backup_tag" "$image" 2>/dev/null || true
        for container in $containers; do
            local service="${CONTAINER_TO_SERVICE[$container]:-$container}"
            run_docker_compose -f "$INSTALL_DIR/docker-compose.yml" up -d --no-deps "$service"
        done
        echo -e "    ${YELLOW}Rollback completed${NC}"
        return 1
    fi
}

# Confirm infrastructure update (requires explicit 'y')
confirm_infrastructure_update() {
    local image=$1
    local service_name="${image%:*}"

    echo ""
    echo -e "${YELLOW}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${YELLOW}⚠  Infrastructure Update: $service_name${NC}"
    echo -e "${YELLOW}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo ""
    echo "  Image: $image"
    echo "  Impact: Brief service interruption"
    echo "  Data: Preserved (persistent volumes)"
    echo ""

    if [[ "$AUTO_MODE" == true ]]; then
        echo -e "  ${GREEN}Auto mode: confirmed${NC}"
        return 0
    fi

    read -p "Update now? (y/N): " confirm
    [[ "$confirm" =~ ^[Yy]$ ]]
}

# Select Python services to update (batch selection)
# Args: changed services as positional arguments
# Output: selected services (space-separated)
select_python_services() {
    local -a changed_services=("$@")

    [[ ${#changed_services[@]} -eq 0 ]] && return

    echo ""
    echo -e "${YELLOW}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo -e "${YELLOW}  Python Services Update${NC}"
    echo -e "${YELLOW}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
    echo ""
    echo "  Changed services: ${changed_services[*]}"

    # Auto mode: update all changed services
    if [[ "$AUTO_MODE" == true ]]; then
        echo -e "  ${GREEN}Auto mode: updating all${NC}"
        echo "${changed_services[@]}"
        return
    fi

    echo ""
    echo "  [A]ll    - Update all changed services (default)"
    echo "  [S]elect - Choose individually"
    echo "  [N]one   - Skip all"
    echo ""
    read -p "Choice [A]: " choice

    case "${choice:-A}" in
        [Aa]*)
            echo "${changed_services[@]}"
            ;;
        [Ss]*)
            local -a selected=()
            for svc in "${changed_services[@]}"; do
                read -p "  Update $svc? (Y/n): " confirm
                if [[ -z "$confirm" || "$confirm" =~ ^[Yy]$ ]]; then
                    selected+=("$svc")
                fi
            done
            echo "${selected[*]}"
            ;;
        *)
            echo ""
            ;;
    esac
}

# Ensure core services are running (called after image update/install)
# This fixes the issue where services need manual restart after installation
ensure_core_services_running() {
    echo ""
    echo -e "${BLUE}Ensuring core services are running...${NC}"

    local services_started=0
    local core_containers=("voltage-redis" "voltageems-comsrv" "voltageems-modsrv")

    for container in "${core_containers[@]}"; do
        if ! docker ps --filter "name=^${container}$" --filter "status=running" -q 2>/dev/null | grep -q .; then
            local service="${CONTAINER_TO_SERVICE[$container]:-$container}"
            echo "  Starting $service..."
            
            # Detect docker compose command
            local compose_cmd
            if docker compose version &>/dev/null 2>&1; then
                compose_cmd="docker compose"
            elif command -v docker-compose &>/dev/null; then
                compose_cmd="docker-compose"
            else
                echo -e "    ${YELLOW}Warning: docker-compose not found${NC}"
                continue
            fi
            
            # Use timeout with direct docker compose command
            if timeout 60 $compose_cmd -f "$INSTALL_DIR/docker-compose.yml" up -d --no-deps "$service" 2>&1; then
                services_started=$((services_started + 1))
            else
                echo -e "    ${YELLOW}Warning: Failed to start $service${NC}"
            fi
        else
            echo -e "  ${GREEN}✓${NC} $container: already running"
        fi
    done

    if [[ $services_started -gt 0 ]]; then
        echo ""
        echo -e "${GREEN}✓ Started $services_started service(s)${NC}"
        # Wait for services to be healthy
        sleep 3
    fi
}

# Verify that containers are using the expected images
# This catches the case where containers exist but use old images
verify_containers_using_correct_images() {
    local images_to_check=("$@")
    local all_ok=true

    echo ""
    echo -e "${BLUE}Verifying containers are using correct images...${NC}"

    for image in "${images_to_check[@]}"; do
        local containers="${IMAGE_TO_CONTAINERS[$image]:-}"
        [[ -z "$containers" ]] && continue

        # Get expected image ID (first 12 chars)
        local expected_id
        expected_id=$(docker images "$image" --format '{{.ID}}' 2>/dev/null | head -1)
        [[ -z "$expected_id" ]] && continue

        for container in $containers; do
            # Get running container's image ID
            local running_id
            running_id=$(docker inspect "$container" --format '{{.Image}}' 2>/dev/null | sed 's/sha256://; s/^\(.\{12\}\).*/\1/')

            if [[ -z "$running_id" ]]; then
                echo -e "  ${YELLOW}○${NC} $container: not running"
            elif [[ "$expected_id" == "$running_id" ]]; then
                echo -e "  ${GREEN}✓${NC} $container: using correct image ($expected_id)"
            else
                echo -e "  ${RED}✗${NC} $container: image mismatch!"
                echo -e "      Expected: $expected_id, Running: $running_id"
                echo -e "      ${YELLOW}Forcing recreation...${NC}"
                local service="${CONTAINER_TO_SERVICE[$container]:-$container}"
                docker stop "$container" 2>/dev/null || true
                docker rm "$container" 2>/dev/null || true
                run_docker_compose -f "$INSTALL_DIR/docker-compose.yml" up -d --no-deps "$service" 2>/dev/null || true
                all_ok=false
            fi
        done
    done

    if [[ "$all_ok" == true ]]; then
        echo -e "${GREEN}✓ All containers verified${NC}"
    else
        echo -e "${YELLOW}⚠ Some containers were recreated to fix image mismatch${NC}"
    fi
}

# =============================================================================
# End of Smart Update Helper Functions
# =============================================================================

# Determine which host user should own installed files.
# Simplified: just use current user, no complex detection.
determine_install_user() {
    ACTUAL_USER=$(whoami)
    ACTUAL_UID=$(id -u)
    ACTUAL_GID=$(id -g)
}

echo -e "${BLUE}================================${NC}"
echo -e "${BLUE}  VoltageEMS ARM64 Installer   ${NC}"
echo -e "${BLUE}================================${NC}"
echo ""

# Check architecture
ARCH=$(uname -m)
if [[ "$ARCH" != "aarch64" && "$ARCH" != "arm64" ]]; then
    echo -e "${YELLOW}Warning: This installer is for ARM64. Current arch: $ARCH${NC}"
    if [[ "$AUTO_MODE" == true ]]; then
        echo -e "${YELLOW}Auto mode: continuing despite architecture mismatch${NC}"
    else
        read -p "Continue anyway? (y/N): " -n 1 -r
        echo
        [[ ! $REPLY =~ ^[Yy]$ ]] && exit 1
    fi
fi

# Resolve installation user details once up front so later steps can safely
# reference ACTUAL_USER/UID/GID without triggering `set -u` exits.
determine_install_user

# Check if we have sudo access (will be needed for some operations)
SUDO=""
if [[ $EUID -ne 0 ]]; then
    echo -e "${YELLOW}Note: Some operations will require sudo privileges${NC}"
    SUDO="sudo"
    # Test sudo access
    if ! sudo -n true 2>/dev/null; then
        echo "Please enter your password for sudo access:"
        sudo true || {
            echo -e "${RED}Error: sudo access required for installation${NC}"
            exit 1
        }
    fi
fi

# Step 1: Install CLI tools
echo -e "${YELLOW}[1/3] Installing CLI tools...${NC}"
$SUDO mkdir -p /usr/local/bin

# Install Monarch CLI
if [[ -f "tools/monarch" ]]; then
    $SUDO cp -v "tools/monarch" "/usr/local/bin/monarch"
    $SUDO chmod +x "/usr/local/bin/monarch"
    echo -e "${GREEN}✓ Monarch CLI installed${NC}"
    echo ""
    echo -e "${BLUE}Monarch provides unified management:${NC}"
    echo "  - Configuration: monarch sync, validate, export"
    echo "  - Channels: monarch channels list/status/control"
    echo "  - Models: monarch models products/instances"
    echo "  - Rules: monarch rules list/enable/execute"
    echo "  - Services: monarch services start/stop/logs"
else
    echo -e "${RED}Error: Monarch binary not found${NC}"
    exit 1
fi

echo -e "${GREEN}[DONE] CLI tools installed${NC}"

# CRITICAL: Pre-install docker-compose.yml before image updates
# This is needed because Step 2 may update Redis container and requires this file
if [[ -f docker-compose.yml ]]; then
    echo "Pre-installing docker-compose.yml for Smart Update Mode..."
    $SUDO mkdir -p "$INSTALL_DIR"
    $SUDO cp docker-compose.yml "$INSTALL_DIR/docker-compose.yml"
    echo -e "${GREEN}✓ docker-compose.yml ready for container updates${NC}"
else
    echo -e "${YELLOW}Warning: docker-compose.yml not found (will be created later)${NC}"
fi

# Step 2: Load Docker images (Smart Update Mode)
echo -e "${YELLOW}[2/3] Loading Docker images...${NC}"
if command -v docker &> /dev/null; then
    # Check if images already exist (check for both :latest and :arm64 tags)
    EXISTING_IMAGES=false
    if docker images | grep -q "voltageems.*latest\|voltageems.*arm64"; then
        EXISTING_IMAGES=true
    fi
    if docker images | grep -q "redis.*8-alpine"; then
        EXISTING_IMAGES=true
    fi

    if [ "$EXISTING_IMAGES" = true ]; then
        echo -e "${BLUE}Existing VoltageEMS images detected.${NC}"
        echo ""
        echo -e "${BLUE}Smart Update Mode:${NC}"
        echo "  1. Smart load (skip unchanged images)"
        echo "  2. Detect changes vs running containers"
        echo "  3. Update changed services (with auto-rollback)"
        echo ""

        PROCEED_UPDATE=false
        if [[ "$AUTO_MODE" == true ]]; then
            echo -e "${GREEN}Auto mode: proceeding with smart update${NC}"
            PROCEED_UPDATE=true
        else
            read -p "Proceed with smart update? (Y/n): " -n 1 -r
            echo
            if [[ -z "$REPLY" ]] || [[ $REPLY =~ ^[Yy]$ ]]; then
                PROCEED_UPDATE=true
            fi
        fi

        if [[ "$PROCEED_UPDATE" == true ]]; then
            # Set global backup timestamp for consistent naming
            BACKUP_TIMESTAMP=$(date +%s)

            # === PHASE 1: SMART LOADING ===
            echo ""
            echo -e "${BLUE}Phase 1: Smart loading images (skip unchanged)...${NC}"

            # Track loaded/skipped images
            LOADED_IMAGES=()
            SKIPPED_IMAGES=()

            # Smart load: only load images that have changed
            # Process in specific order to handle dependencies
            tarball_list=(docker/voltageems.tar.gz docker/python-services.tar.gz docker/voltage-redis.tar.gz docker/voltage-influxdb.tar.gz docker/apps.tar.gz)
            
            tarball_idx=0
            for tarball in "${tarball_list[@]}"; do
                tarball_idx=$((tarball_idx + 1))
                
                if [[ -f "$tarball" ]]; then
                    _basename=$(basename "$tarball")
                    _image="${TARBALL_TO_IMAGE[$_basename]:-}"

                    # Temporarily disable exit-on-error to capture return codes 0, 1, 2
                    set +e
                    smart_load_image "$tarball"
                    _result=$?
                    set -e

                    case $_result in
                        0)  # Loaded
                            [[ -n "$_image" ]] && LOADED_IMAGES+=("$_image")
                            ;;
                        1)  # Skipped (unchanged)
                            [[ -n "$_image" ]] && SKIPPED_IMAGES+=("$_image")
                            ;;
                        2)  # Error
                            echo -e "${RED}Failed to load $_basename${NC}"
                            exit 1
                            ;;
                    esac
                else
                    # File not found is OK (apps.tar.gz is optional)
                    if [[ "$tarball" != *"apps.tar.gz"* ]]; then
                        echo -e "${YELLOW}Warning: $tarball not found${NC}"
                    fi
                fi
            done

            # Summary
            echo ""
            echo -e "  ${GREEN}Loaded: ${#LOADED_IMAGES[@]}${NC} | ${BLUE}Skipped: ${#SKIPPED_IMAGES[@]}${NC}"

            # Detect changes vs running containers (for loaded images only)
            # Skipped images (unchanged tarball vs local) won't have changed containers
            declare -A CHANGE_STATUS
            INFRA_CHANGED=()
            RUST_CHANGED=()
            PYTHON_CHANGED=()
            FRONTEND_CHANGED=()

            # If nothing was loaded, skip detection
            if [[ ${#LOADED_IMAGES[@]} -eq 0 ]]; then
                echo ""
                echo -e "  ${GREEN}✓ All images unchanged (Image ID matches)${NC}"
                echo -e "  ${GREEN}✓ No backup or restart needed${NC}"
                
                # Skip to Phase 3 directly (no updates needed)
                echo ""
                echo -e "${BLUE}Phase 2: Confirming updates...${NC}"
                echo -e "${BLUE}No updates selected (all images unchanged).${NC}"
                
                # Skip Phase 3
                echo ""
                echo -e "${GREEN}[DONE] Smart update completed${NC}"
                echo ""
                echo -e "${BLUE}Update Summary:${NC}"
                echo "  • ${#SKIPPED_IMAGES[@]} image(s) skipped (Image ID unchanged)"
                
                # List skipped images
                for img in "${SKIPPED_IMAGES[@]}"; do
                    local_id=$(get_local_image_id "$img")
                    echo -e "    ${GREEN}✓${NC} $img (${local_id:0:12})"
                done
                
                # Ensure core services are running
                echo ""
                ensure_core_services_running
            else
                echo ""
                echo "  Detecting changes vs running containers:"

                # Check loaded images against running containers
                for image in "${LOADED_IMAGES[@]}"; do
                    status=$(detect_image_change "$image")
                    CHANGE_STATUS[$image]=$status

                    case "$status" in
                        "changed")
                            echo -e "    ${RED}⚠${NC} $image: ${RED}needs restart${NC}"
                            case "$image" in
                                redis:*|influxdb:*)
                                    INFRA_CHANGED+=("$image")
                                    ;;
                                voltageems:latest)
                                    RUST_CHANGED+=("$image")
                                    ;;
                                voltageems-ss:*)
                                    PYTHON_CHANGED+=("$image")
                                    ;;
                                voltage-apps:*)
                                    FRONTEND_CHANGED+=("$image")
                                    ;;
                            esac
                            ;;
                        "not_running")
                            echo -e "    ${YELLOW}○${NC} $image: not running (will start)"
                            # Treat not-running as needing update
                            case "$image" in
                                redis:*|influxdb:*)
                                    INFRA_CHANGED+=("$image")
                                    ;;
                                voltageems:latest)
                                    RUST_CHANGED+=("$image")
                                    ;;
                                voltageems-ss:*)
                                    PYTHON_CHANGED+=("$image")
                                    ;;
                                voltage-apps:*)
                                    FRONTEND_CHANGED+=("$image")
                                    ;;
                            esac
                            ;;
                        *)
                            echo -e "    ${GREEN}✓${NC} $image: already running latest"
                            ;;
                    esac
                done
            fi

            # === PHASE 2: DECIDE ===
            echo ""
            echo -e "${BLUE}Phase 2: Confirming updates...${NC}"

            TO_UPDATE=()
            UPDATE_SUCCESS=()
            UPDATE_FAILED=()

            # Infrastructure: explicit confirmation required
            for image in "${INFRA_CHANGED[@]}"; do
                if confirm_infrastructure_update "$image"; then
                    TO_UPDATE+=("$image")
                else
                    echo -e "  ${BLUE}Skipped: $image${NC}"
                fi
            done

            # Rust core: auto-confirm
            for image in "${RUST_CHANGED[@]}"; do
                echo -e "  ${GREEN}Auto-confirmed: $image (Rust core)${NC}"
                TO_UPDATE+=("$image")
            done

            # Python services: unified image
            if [[ ${#PYTHON_CHANGED[@]} -gt 0 ]]; then
                # Check if unified Python services image changed
                for img in "${PYTHON_CHANGED[@]}"; do
                    if [[ "$img" == "voltageems-ss:latest" ]]; then
                        echo -e "  ${GREEN}Auto-confirmed: $img (unified Python services)${NC}"
                        TO_UPDATE+=("$img")
                    fi
                done
            fi

            # Frontend: auto-confirm (optional service)
            for image in "${FRONTEND_CHANGED[@]}"; do
                echo -e "  ${GREEN}Auto-confirmed: $image (Frontend)${NC}"
                TO_UPDATE+=("$image")
            done

            # === PHASE 3: EXECUTE ===
            if [[ ${#TO_UPDATE[@]} -eq 0 ]]; then
                echo ""
                echo -e "${BLUE}No updates selected.${NC}"
            else
                echo ""
                echo -e "${BLUE}Phase 3: Executing updates...${NC}"
                echo "  Services to update: ${#TO_UPDATE[@]}"
                echo ""

                for image in "${TO_UPDATE[@]}"; do
                    containers="${IMAGE_TO_CONTAINERS[$image]:-}"
                    if [[ -n "$containers" ]]; then
                        if update_service "$image" "$containers"; then
                            UPDATE_SUCCESS+=("$image")
                        else
                            UPDATE_FAILED+=("$image")
                        fi
                    fi
                done
            fi

            # Cleanup
            echo ""
            echo -e "${BLUE}Cleaning up...${NC}"
            docker image prune -f 2>/dev/null || true

            # Summary
            echo ""
            echo -e "${GREEN}[DONE] Smart update completed${NC}"
            echo ""
            echo -e "${BLUE}Update Summary:${NC}"

            if [[ ${#UPDATE_SUCCESS[@]} -gt 0 ]]; then
                echo -e "  ${GREEN}✓ Updated:${NC} ${UPDATE_SUCCESS[*]}"
            fi
            if [[ ${#UPDATE_FAILED[@]} -gt 0 ]]; then
                echo -e "  ${RED}✗ Failed:${NC} ${UPDATE_FAILED[*]}"
            fi

            # Show skipped (unchanged) images
            if [[ ${#SKIPPED_IMAGES[@]} -gt 0 ]]; then
                echo "  • ${#SKIPPED_IMAGES[@]} image(s) skipped (unchanged)"
            fi

            # Ensure core services are running (even if images unchanged)
            ensure_core_services_running

            # Verify updated containers are using correct images
            if [[ ${#UPDATE_SUCCESS[@]} -gt 0 ]]; then
                verify_containers_using_correct_images "${UPDATE_SUCCESS[@]}"
            fi
        else
            echo -e "${YELLOW}Skipping image update.${NC}"
            echo -e "${GREEN}[SKIPPED] Docker images${NC}"

            # Still ensure core services are running
            ensure_core_services_running
        fi
    else
        # No existing images - first installation
        echo "Loading Docker images (first installation)..."
        FRESH_LOADED_IMAGES=""
        for tarball in docker/*.tar.gz; do
            if [[ -f "$tarball" ]]; then
                echo -n "  Loading $(basename "$tarball")... "
                if OUTPUT=$(docker load < "$tarball" 2>&1); then
                    LOADED_NAME=$(echo "$OUTPUT" | grep "Loaded image:" | sed 's/Loaded image: //')
                    if [ -n "$LOADED_NAME" ]; then
                        FRESH_LOADED_IMAGES="$FRESH_LOADED_IMAGES $LOADED_NAME"
                        echo -e "${GREEN}success${NC}"
                    else
                        echo -e "${GREEN}success${NC}"
                    fi
                else
                    echo -e "${RED}failed${NC}"
                    echo "    Error: $OUTPUT"
                    exit 1
                fi
            fi
        done

        # Verify loaded images
        echo "Verifying loaded images..."
        # NOTE: voltage-apps:latest is optional
        for image_name in voltageems:latest redis:8-alpine influxdb:2-alpine voltageems-ss:latest voltage-apps:latest; do
            echo -n "  Checking $image_name... "
            if docker image inspect "$image_name" >/dev/null 2>&1; then
                CREATED=$(docker image inspect "$image_name" --format='{{.Created}}' 2>/dev/null | cut -d'T' -f1)
                echo -e "${GREEN}present${NC} (created: $CREATED)"
            else
                if [[ "$image_name" == "voltage-apps:latest" ]]; then
                    echo -e "${YELLOW}missing (optional, skipping)${NC}"
                else
                    echo -e "${RED}missing!${NC}"
                    echo -e "${RED}ERROR: Expected image $image_name was not loaded properly${NC}"
                    exit 1
                fi
            fi
        done

        echo -e "${GREEN}[DONE] Docker images loaded${NC}"
    fi
else
    echo -e "${RED}Docker not installed. Please install Docker first.${NC}"
    echo "Run: curl -fsSL https://get.docker.com | sh"
    exit 1
fi

# Step 3: Setup directories and configuration
echo -e "${YELLOW}[3/3] Setting up configuration...${NC}"

# Check if external storage is available
if [[ -d "/extp" ]] && [[ -w "/extp" || -n "$SUDO" ]]; then
    echo "External storage detected at /extp"
    LOG_DIR="/extp/logs"
    echo "Logs will be stored at: $LOG_DIR"

    # Ensure /extp has appropriate permissions for creating subdirectories
    if [[ -n "$SUDO" ]]; then
        echo "Setting initial permissions for /extp directory..."
        $SUDO chmod 755 "/extp" 2>/dev/null || true
        # Note: Full permission and ownership fix will happen after user detection
    fi
else
    echo "No external storage found, using default location"
    LOG_DIR="$INSTALL_DIR/logs"
fi

# Create all necessary directories
echo "Creating installation directories..."
$SUDO mkdir -p "$INSTALL_DIR"/data

# Create log directories (permissions will be set after user detection)
echo "Creating log directories..."
$SUDO mkdir -p "$LOG_DIR"
# Create log directories for all services
for service in comsrv modsrv hissrv apigateway netsrv alarmsrv; do
    $SUDO mkdir -p "$LOG_DIR/$service"
done

# Install scripts directory (utility scripts)
if [[ -d "scripts" ]] && [[ "$INSTALL_DIR" != "$(pwd)" ]]; then
    echo "Installing utility scripts..."
    $SUDO mkdir -p "$INSTALL_DIR/scripts"

    # Copy update-env-permissions.sh if it exists
    if [[ -f "scripts/update-env-permissions.sh" ]]; then
        $SUDO cp "scripts/update-env-permissions.sh" "$INSTALL_DIR/scripts/"
        $SUDO chmod +x "$INSTALL_DIR/scripts/update-env-permissions.sh"
        echo -e "${GREEN}✓ Utility scripts installed${NC}"
    fi
fi

# Install config.template directory (only config files)
if [[ -d "config.template" ]]; then
    echo "Installing configuration templates..."
    $SUDO mkdir -p "$INSTALL_DIR/config.template"

    # Copy only configuration files (yaml, yml, csv, json)
    find config.template -type f \( -name "*.yaml" -o -name "*.yml" -o -name "*.csv" -o -name "*.json" \) | while read file; do
        # Get relative path
        rel_path="${file#config.template/}"
        # Create directory structure
        $SUDO mkdir -p "$INSTALL_DIR/config.template/$(dirname "$rel_path")"
        # Copy file to template directory only
        $SUDO cp "$file" "$INSTALL_DIR/config.template/$rel_path"
    done

    echo -e "${GREEN}✓ Configuration templates installed${NC}"
else
    echo -e "${YELLOW}Warning: config.template not found${NC}"
fi

# Install Python service configuration files
# Python services configs go directly to config/ directory (not config.template/)
# Try to copy from services/{service}/config/ first (development), then from installer package config/ (production)
echo "Installing Python service configuration files..."
PYTHON_SERVICES="hissrv apigateway netsrv alarmsrv"
for service in $PYTHON_SERVICES; do
    SERVICE_CONFIG_DIR="services/$service/config"
    INSTALLER_CONFIG_DIR="config/$service"
    
    # Try to copy from services directory first (development scenario)
    if [[ -d "$SERVICE_CONFIG_DIR" ]]; then
        echo "  Copying $service configuration files from services directory..."
        $SUDO mkdir -p "$INSTALL_DIR/config/$service"
        
        # Copy all files and directories from service config directory
        find "$SERVICE_CONFIG_DIR" -type f \( -name "*.yaml" -o -name "*.yml" -o -name "*.json" -o -name "*.conf" -o -name "*.ini" -o -name "*.txt" \) | while read file; do
            # Get relative path from service config directory
            rel_path="${file#$SERVICE_CONFIG_DIR/}"
            # Create directory structure if needed
            if [[ "$rel_path" == *"/"* ]]; then
                $SUDO mkdir -p "$INSTALL_DIR/config/$service/$(dirname "$rel_path")"
            fi
            # Copy file
            $SUDO cp "$file" "$INSTALL_DIR/config/$service/$rel_path"
        done
        
        # Also copy directories if they exist (for nested config structures)
        find "$SERVICE_CONFIG_DIR" -mindepth 1 -type d | while read dir; do
            rel_dir="${dir#$SERVICE_CONFIG_DIR/}"
            $SUDO mkdir -p "$INSTALL_DIR/config/$service/$rel_dir"
        done
        
        echo -e "    ${GREEN}✓ $service configuration files copied to $INSTALL_DIR/config/$service/${NC}"
    # Fallback: copy from installer package config/ directory (production scenario)
    elif [[ -d "$INSTALLER_CONFIG_DIR" ]]; then
        echo "  Copying $service configuration files from installer package..."
        $SUDO mkdir -p "$INSTALL_DIR/config/$service"
        $SUDO cp -r "$INSTALLER_CONFIG_DIR"/* "$INSTALL_DIR/config/$service/" 2>/dev/null || true
        echo -e "    ${GREEN}✓ $service configuration files copied from installer package${NC}"
    else
        echo -e "    ${YELLOW}⚠ $service config directory not found: $SERVICE_CONFIG_DIR or $INSTALLER_CONFIG_DIR${NC}"
    fi
done

echo -e "${GREEN}✓ Python service configuration files installed${NC}"

# Create a symlink if logs are external
if [[ "$LOG_DIR" != "$INSTALL_DIR/logs" ]]; then
    echo "Creating symlink for logs..."
    # Remove existing logs directory if it's not a symlink
    if [[ -d "$INSTALL_DIR/logs" ]] && [[ ! -L "$INSTALL_DIR/logs" ]]; then
        echo "Removing existing logs directory to create symlink..."
        $SUDO rm -rf "$INSTALL_DIR/logs"
    fi
    # Create the symlink
    $SUDO ln -sfn "$LOG_DIR" "$INSTALL_DIR/logs"
    echo "Linked $INSTALL_DIR/logs -> $LOG_DIR"
fi

# Database initialization with safety check
echo "Checking database status..."
DB_FILE="$INSTALL_DIR/data/voltage.db"

if [[ -f "$DB_FILE" ]]; then
    # Database exists - check if it has data
    DB_SIZE=$(stat -c%s "$DB_FILE" 2>/dev/null || stat -f%z "$DB_FILE" 2>/dev/null || echo "0")

    if [[ "$DB_SIZE" -gt 0 ]]; then
        # Format size for display (handle both Linux and macOS)
        if command -v numfmt &>/dev/null; then
            DB_SIZE_DISPLAY=$(numfmt --to=iec $DB_SIZE 2>/dev/null)
        else
            DB_SIZE_DISPLAY="${DB_SIZE}B"
        fi

        echo -e "${BLUE}Existing database detected (${DB_SIZE_DISPLAY})${NC}"
        echo ""
        echo "Options:"
        echo "  1. Add missing tables only (safe upgrade, preserves data)"
        echo "  2. Skip database initialization"
        echo ""
        echo -e "${YELLOW}Note: Database reset is disabled for safety. To reset manually:${NC}"
        echo "      rm $DB_FILE && monarch init"
        echo ""

        if [[ "$AUTO_MODE" == true ]]; then
            echo -e "${GREEN}Auto mode: using safe upgrade (option 1)${NC}"
            DB_OPTION=1
        else
            read -p "Choose option [1]: " DB_OPTION
            DB_OPTION=${DB_OPTION:-1}
        fi

        case $DB_OPTION in
            1)
                echo -e "${YELLOW}Running safe schema upgrade...${NC}"
                monarch init  # IF NOT EXISTS ensures safety
                echo -e "${GREEN}✓ Schema upgraded (existing data preserved)${NC}"
                ;;
            2)
                echo -e "${BLUE}Skipped database initialization${NC}"
                ;;
            *)
                echo -e "${YELLOW}Invalid option. Using safe upgrade (option 1)...${NC}"
                monarch init
                ;;
        esac
    else
        # Empty database file
        echo "Empty database file detected, initializing..."
        monarch init
    fi
else
    # No database - first installation
    echo "Creating new database..."
    $SUDO touch "$DB_FILE"
    $SUDO chown $ACTUAL_USER:docker "$DB_FILE" 2>/dev/null || true
    monarch init
fi

# Set permissions using docker group for secure access
echo "Setting up permissions..."

if [[ -z "${ACTUAL_USER:-}" ]]; then
    echo -e "${RED}Error: Failed to determine installation user. Aborting.${NC}"
    exit 1
fi

# Check if docker group exists and get its GID
DOCKER_GROUP=$(getent group docker 2>/dev/null)
if [[ -n "$DOCKER_GROUP" ]]; then
    DOCKER_GID=$(echo "$DOCKER_GROUP" | cut -d: -f3)
    echo "Docker group found (GID=$DOCKER_GID)"
else
    echo "Warning: docker group not found, creating it..."
    $SUDO groupadd docker 2>/dev/null || true
    DOCKER_GID=$(getent group docker | cut -d: -f3)
fi

# Set ownership: actual_user:docker
echo "Setting ownership to $ACTUAL_USER:docker..."

# Set ownership for main directory first
if [[ -z "$SUDO" ]]; then
    chown $ACTUAL_USER:docker "$INSTALL_DIR"
else
    $SUDO chown $ACTUAL_USER:docker "$INSTALL_DIR"
fi

# Then set ownership for subdirectories
if [[ -z "$SUDO" ]]; then
    chown -R $ACTUAL_USER:docker "$INSTALL_DIR/data" 2>/dev/null || true
    chown -R $ACTUAL_USER:docker "$INSTALL_DIR/config.template" 2>/dev/null || true
    chown -R $ACTUAL_USER:docker "$INSTALL_DIR/config" 2>/dev/null || true
    # Fix scripts directory if it exists
    [[ -d "$INSTALL_DIR/scripts" ]] && chown -R $ACTUAL_USER:docker "$INSTALL_DIR/scripts" 2>/dev/null || true
else
    $SUDO chown -R $ACTUAL_USER:docker "$INSTALL_DIR/data" 2>/dev/null || true
    $SUDO chown -R $ACTUAL_USER:docker "$INSTALL_DIR/config.template" 2>/dev/null || true
    $SUDO chown -R $ACTUAL_USER:docker "$INSTALL_DIR/config" 2>/dev/null || true
    # Fix scripts directory if it exists
    [[ -d "$INSTALL_DIR/scripts" ]] && $SUDO chown -R $ACTUAL_USER:docker "$INSTALL_DIR/scripts" 2>/dev/null || true
fi

# Special handling for log directories (may be external)
echo "Fixing log directory permissions..."

# Get numeric UID and GID for the actual user and docker group
ACTUAL_UID=$(id -u $ACTUAL_USER)
ACTUAL_GID=$DOCKER_GID  # Use the docker GID we detected earlier

echo "Using numeric IDs for permissions: UID=$ACTUAL_UID, GID=$ACTUAL_GID"

# Fix /extp ownership if it exists (parent of external log directory)
if [[ "$LOG_DIR" == "/extp/logs" ]] && [[ -d "/extp" ]]; then
    echo "Fixing /extp parent directory ownership..."
    if [[ -z "$SUDO" ]]; then
        chown ${ACTUAL_UID}:${ACTUAL_GID} "/extp" 2>/dev/null || echo "Warning: Could not set ownership for /extp (may need sudo)"
        chmod 755 "/extp" 2>/dev/null || true
    else
        $SUDO chown ${ACTUAL_UID}:${ACTUAL_GID} "/extp" || echo "Warning: Could not set ownership for /extp"
        $SUDO chmod 755 "/extp" || true
    fi
    echo "Set /extp ownership to ${ACTUAL_UID}:${ACTUAL_GID}"
fi

if [[ -d "$LOG_DIR" ]]; then
    if [[ -z "$SUDO" ]]; then
        # Fix main log directory using numeric IDs
        chown ${ACTUAL_UID}:${ACTUAL_GID} "$LOG_DIR" || echo "Warning: Could not set ownership for $LOG_DIR"
        chmod 775 "$LOG_DIR" || echo "Warning: Could not set permissions for $LOG_DIR"

        # Fix each service subdirectory explicitly
        for service in comsrv modsrv hissrv apigateway netsrv alarmsrv; do
            if [[ -d "$LOG_DIR/$service" ]]; then
                chown -R ${ACTUAL_UID}:${ACTUAL_GID} "$LOG_DIR/$service" || echo "Warning: Could not set ownership for $LOG_DIR/$service"
                chmod 775 "$LOG_DIR/$service" || echo "Warning: Could not set permissions for $LOG_DIR/$service"
            fi
        done

        # Fix symlink if it exists
        [[ -L "$INSTALL_DIR/logs" ]] && chown -h ${ACTUAL_UID}:${ACTUAL_GID} "$INSTALL_DIR/logs" 2>/dev/null || true
    else
        # Fix main log directory using numeric IDs
        $SUDO chown ${ACTUAL_UID}:${ACTUAL_GID} "$LOG_DIR" || echo "Warning: Could not set ownership for $LOG_DIR"
        $SUDO chmod 777 "$LOG_DIR" || echo "Warning: Could not set permissions for $LOG_DIR"

        # Fix each service subdirectory explicitly
        for service in comsrv modsrv hissrv apigateway netsrv alarmsrv; do
            if [[ -d "$LOG_DIR/$service" ]]; then
                $SUDO chown -R ${ACTUAL_UID}:${ACTUAL_GID} "$LOG_DIR/$service" || echo "Warning: Could not set ownership for $LOG_DIR/$service"
                $SUDO chmod -R 777 "$LOG_DIR/$service" || echo "Warning: Could not set permissions for $LOG_DIR/$service"
            fi
        done

        # Fix symlink if it exists
        [[ -L "$INSTALL_DIR/logs" ]] && $SUDO chown -h ${ACTUAL_UID}:${ACTUAL_GID} "$INSTALL_DIR/logs" 2>/dev/null || true
    fi
    echo -e "${GREEN}✓ Log directory permissions fixed (UID=$ACTUAL_UID, GID=$ACTUAL_GID)${NC}"
else
    echo -e "${YELLOW}Warning: Log directory $LOG_DIR does not exist${NC}"
fi

# Set permissions for main directory (755 - owner can write, others can read/execute)
if [[ -z "$SUDO" ]]; then
    chmod 755 "$INSTALL_DIR"
else
    $SUDO chmod 755 "$INSTALL_DIR"
fi

# Set permissions for subdirectories (777 for logs to allow any container user to write)
if [[ -z "$SUDO" ]]; then
    chmod -R 775 "$INSTALL_DIR/data"
    chmod -R 777 "$LOG_DIR"
    [[ -d "$INSTALL_DIR/config" ]] && chmod -R 775 "$INSTALL_DIR/config" || true
else
    $SUDO chmod -R 775 "$INSTALL_DIR/data"
    $SUDO chmod -R 777 "$LOG_DIR"
    [[ -d "$INSTALL_DIR/config" ]] && $SUDO chmod -R 775 "$INSTALL_DIR/config" || true
fi

# Create system-wide environment variables for Docker Compose
echo "Creating system environment variables..."
$SUDO tee /etc/profile.d/monarchedge.sh > /dev/null << EOF
# VoltageEMS Docker environment variables
# Generated by install.sh on $(date)
# User: $ACTUAL_USER (UID=$ACTUAL_UID, GID=$ACTUAL_GID)
export HOST_UID=$ACTUAL_UID
export HOST_GID=$ACTUAL_GID
EOF
$SUDO chmod 644 /etc/profile.d/monarchedge.sh
echo -e "${GREEN}✓ Environment variables exported to /etc/profile.d/monarchedge.sh${NC}"

echo "Permissions configured:"
echo "  User: $ACTUAL_USER (UID=$ACTUAL_UID)"
echo "  Group: docker (GID=$DOCKER_GID)"
echo "  Mode: 775 (directories), 664 (files)"

# Add user to docker group if not already
if ! groups $ACTUAL_USER 2>/dev/null | grep -q docker; then
    echo "Adding $ACTUAL_USER to docker group..."
    $SUDO usermod -aG docker $ACTUAL_USER
    echo "IMPORTANT: User must logout and login for group changes to take effect!"
fi

# Additional config samples are not needed - already handled above

# Install docker-compose.yml
if [[ -f docker-compose.yml ]]; then
    echo "Installing docker-compose.yml..."
    $SUDO cp docker-compose.yml "$INSTALL_DIR/docker-compose.yml"
    $SUDO chown $ACTUAL_USER:docker "$INSTALL_DIR/docker-compose.yml"
    $SUDO chmod 644 "$INSTALL_DIR/docker-compose.yml"
    echo -e "${GREEN}docker-compose.yml installed${NC}"
else
    echo -e "${YELLOW}Warning: docker-compose.yml not found${NC}"
fi

# Create soft link to user's home directory for convenience
echo "Creating docker-compose.yml symlink in user home directory..."
if [[ -n "$ACTUAL_USER" ]]; then
    USER_HOME=$(eval echo ~$ACTUAL_USER)
    if [[ -d "$USER_HOME" ]]; then
        # Create symlink in user's home directory (as the target user to avoid permission issues)
        if command -v sudo &> /dev/null; then
            # Use sudo to create symlink as the target user
            sudo -u "$ACTUAL_USER" ln -sf "$INSTALL_DIR/docker-compose.yml" "$USER_HOME/docker-compose.yml" 2>/dev/null
            if [[ $? -eq 0 ]]; then
                echo -e "${GREEN}Created symlink: $USER_HOME/docker-compose.yml → $INSTALL_DIR/docker-compose.yml${NC}"
            else
                echo -e "${YELLOW}Note: Could not create symlink in home directory (permission denied)${NC}"
                echo -e "${YELLOW}You can manually create it later with:${NC}"
                echo -e "${YELLOW}  ln -sf $INSTALL_DIR/docker-compose.yml ~/docker-compose.yml${NC}"
            fi
        else
            # Try without sudo (might fail due to permissions)
            ln -sf "$INSTALL_DIR/docker-compose.yml" "$USER_HOME/docker-compose.yml" 2>/dev/null
            if [[ $? -eq 0 ]]; then
                echo -e "${GREEN}Created symlink: $USER_HOME/docker-compose.yml → $INSTALL_DIR/docker-compose.yml${NC}"
            else
                echo -e "${YELLOW}Note: Could not create symlink in home directory${NC}"
                echo -e "${YELLOW}You can manually create it later with:${NC}"
                echo -e "${YELLOW}  ln -sf $INSTALL_DIR/docker-compose.yml ~/docker-compose.yml${NC}"
            fi
        fi
    fi
fi

echo -e "${GREEN}[DONE] Configuration installed${NC}"

echo ""
echo -e "${GREEN}================================${NC}"
echo -e "${GREEN}  Installation Complete!        ${NC}"
echo -e "${GREEN}================================${NC}"
echo ""
echo "Installed components:"
echo "  • CLI Tool: monarch (unified management)"
echo "  • Docker Images: voltage-redis, voltageems"
echo "  • Installation directory: $INSTALL_DIR"
if [[ "$LOG_DIR" != "$INSTALL_DIR/logs" ]]; then
    echo "  • Log directory: $LOG_DIR (symlinked from $INSTALL_DIR/logs)"
else
    echo "  • Log directory: $LOG_DIR"
fi
echo ""

# Display actual permissions for verification
echo -e "${BLUE}Directory Permissions:${NC}"
echo "--------------------------------------------"
ls -ld "$INSTALL_DIR" 2>/dev/null | awk '{printf "%-20s %s %s:%s\n", $9":", $1, $3, $4}'

if [[ -d "$INSTALL_DIR/data" ]]; then
    ls -ld "$INSTALL_DIR/data" 2>/dev/null | awk '{printf "%-20s %s %s:%s\n", "├── data:", $1, $3, $4}'
fi

if [[ -L "$INSTALL_DIR/logs" ]]; then
    LINK_INFO=$(ls -ld "$INSTALL_DIR/logs" 2>/dev/null)
    TARGET=$(readlink "$INSTALL_DIR/logs" 2>/dev/null)
    echo "$LINK_INFO" | awk -v target="$TARGET" '{printf "%-20s %s %s:%s -> %s\n", "├── logs:", $1, $3, $4, target}'
elif [[ -d "$INSTALL_DIR/logs" ]]; then
    ls -ld "$INSTALL_DIR/logs" 2>/dev/null | awk '{printf "%-20s %s %s:%s\n", "├── logs:", $1, $3, $4}'
fi

if [[ -d "$INSTALL_DIR/config.template" ]]; then
    ls -ld "$INSTALL_DIR/config.template" 2>/dev/null | awk '{printf "%-20s %s %s:%s\n", "├── config.template:", $1, $3, $4}'
fi

if [[ -f "$INSTALL_DIR/docker-compose.yml" ]]; then
    ls -l "$INSTALL_DIR/docker-compose.yml" 2>/dev/null | awk '{printf "%-20s %s %s:%s\n", "└── docker-compose:", $1, $3, $4}'
fi
echo "--------------------------------------------"

# If using external log directory, show its permissions too
if [[ "$LOG_DIR" != "$INSTALL_DIR/logs" ]] && [[ -d "$LOG_DIR" ]]; then
    echo ""
    echo -e "${BLUE}External Log Directory Permissions:${NC}"
    echo "--------------------------------------------"
    ls -ld "$LOG_DIR" 2>/dev/null | awk '{printf "%-25s %s %s:%s\n", $9":", $1, $3, $4}'

    # Show service subdirectories
    for service in comsrv modsrv hissrv apigateway netsrv alarmsrv; do
        if [[ -d "$LOG_DIR/$service" ]]; then
            ls -ld "$LOG_DIR/$service" 2>/dev/null | awk -v svc="├── $service:" '{printf "%-25s %s %s:%s\n", svc, $1, $3, $4}'
        fi
    done
    echo "--------------------------------------------"
fi

echo ""

# Check if permissions might need attention
MAIN_OWNER=$(stat -c "%U" "$INSTALL_DIR" 2>/dev/null || stat -f "%Su" "$INSTALL_DIR" 2>/dev/null || echo "unknown")
if [[ "$MAIN_OWNER" == "root" ]]; then
    echo -e "${YELLOW}⚠ Note: Directory is owned by root${NC}"
    echo -e "${YELLOW}  To change owner: sudo chown -R <user>:docker $INSTALL_DIR${NC}"
    echo ""
fi

echo "Permission Configuration:"
echo "  • Directories owned by: $ACTUAL_USER:docker"
echo "  • Ensure your user is in docker group:"
echo -e "    ${YELLOW}sudo usermod -aG docker \$USER${NC}"
echo "    (logout and login for changes to take effect)"
echo ""
echo "Network Configuration:"
echo -e "${YELLOW}  • Using host network mode for optimal performance${NC}"
echo "  • Services available on localhost:"
echo "    - Redis: 6379          (data store)"
echo "    - InfluxDB: 8086       (time-series database)"
echo "    - ComSrv: 6001         (communication - Rust)"
echo "    - ModSrv: 6002         (model + rules - Rust)"
echo "    - HisSrv: 6004         (history - Python)"
echo "    - APIGateway: 6005     (gateway - Python)"
echo "    - NetSrv: 6006         (network - Python)"
echo "    - AlarmSrv: 6007       (alarm - Python)"
echo "    - Frontend: 8080       (Vue.js + nginx)"
echo ""
echo -e "${YELLOW}Important: Configuration Setup Required${NC}"
echo "  Before starting services, you must:"
echo "  1. Copy Rust service configuration template:"
echo "     cp -r $INSTALL_DIR/config.template/comsrv $INSTALL_DIR/config/comsrv"
echo "     cp -r $INSTALL_DIR/config.template/modsrv $INSTALL_DIR/config/modsrv"
echo "  2. Customize configuration files in config/ directory:"
echo "     - Rust services: config/comsrv/, config/modsrv/ (copy from config.template first)"
echo "     - Python services: config/hissrv/, config/apigateway/, config/netsrv/, config/alarmsrv/ (already installed)"
echo "  3. Sync configurations to database:"
echo "     monarch sync"
echo ""
echo -e "${BLUE}Database Management:${NC}"
echo "  monarch init          - Add missing tables (safe, preserves data)"
echo "  monarch init --force  - Reset database (WARNING: deletes all data)"
echo "  monarch sync          - Sync configuration files to database"
echo ""
echo "Quick Start:"
echo -e "  ${YELLOW}source /etc/profile.d/monarchedge.sh${NC}  - Load environment variables (or re-login)"
echo "  docker-compose up -d   - Start all services"
echo "  docker-compose down    - Stop all services"
echo "  docker-compose ps      - Check service status"
echo "  docker-compose logs -f - View service logs"
echo ""
echo "CLI Management (via monarch):"
echo ""
echo "  Configuration:"
echo "    monarch sync                  - Sync all configurations"
echo "    monarch status                - Show sync status"
echo "    monarch export modsrv         - Export configuration from database"
echo ""
echo "  Channels:"
echo "    monarch channels list         - List all channels"
echo "    monarch channels status 1     - Get channel status"
echo "    monarch channels reload       - Reload configurations"
echo ""
echo "  Models:"
echo "    monarch models products list  - List products"
echo "    monarch models instances list - List instances"
echo "    monarch models products import pv - Import product"
echo ""
echo "  Rules:"
echo "    monarch rules list            - List all rules"
echo "    monarch rules enable R001     - Enable a rule"
echo "    monarch rules test R001       - Test a rule"
echo ""
echo "  Services:"
echo "    monarch services start        - Start all services"
echo "    monarch services stop         - Stop all services"
echo "    monarch services logs comsrv  - View service logs"
echo ""
echo -e "${YELLOW}Note: Using host network mode - ensure ports are not in use${NC}"
echo ""

# Offer to clean up installer package
echo -e "${BLUE}================================${NC}"
echo -e "${BLUE}  Cleanup                       ${NC}"
echo -e "${BLUE}================================${NC}"
echo ""

# Try to detect installer package location
INSTALLER_NAME=""
POSSIBLE_LOCATIONS=(
    "$LAUNCH_DIR/MonarchEdge-arm64-*.run"
    "/tmp/MonarchEdge-arm64-*.run"
    "$HOME/MonarchEdge-arm64-*.run"
    "$HOME/Downloads/MonarchEdge-arm64-*.run"
)

# Search for installer in common locations
for pattern in "${POSSIBLE_LOCATIONS[@]}"; do
    # Use nullglob to handle no matches gracefully
    shopt -s nullglob
    for file in $pattern; do
        if [[ -f "$file" ]]; then
            INSTALLER_NAME="$file"
            break 2
        fi
    done
    shopt -u nullglob
done

if [[ -n "$INSTALLER_NAME" ]]; then
    echo -e "${YELLOW}Installer package detected:${NC}"
    echo "  Location: $INSTALLER_NAME"
    echo "  Size: $(du -h "$INSTALLER_NAME" 2>/dev/null | cut -f1)"
    echo ""

    if [[ "$AUTO_MODE" == true ]]; then
        echo -e "${BLUE}Auto mode: keeping installer package${NC}"
    else
        read -p "Do you want to delete the installer package? (y/N): " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            if rm -f "$INSTALLER_NAME" 2>/dev/null; then
                echo -e "${GREEN}✓ Installer package deleted${NC}"
            else
                echo -e "${YELLOW}Warning: Failed to delete installer (may need sudo)${NC}"
                echo "  You can manually delete it with:"
                echo "  $SUDO rm -f '$INSTALLER_NAME'"
            fi
        else
            echo -e "${BLUE}Installer package kept at: $INSTALLER_NAME${NC}"
        fi
    fi
else
    if [[ "$AUTO_MODE" != true ]]; then
        echo -e "${BLUE}No installer package found in common locations.${NC}"
        echo ""
        read -p "Do you want to specify the installer location for cleanup? (y/N): " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            read -p "Enter full path to installer: " INSTALLER_PATH
            if [[ -f "$INSTALLER_PATH" ]]; then
                read -p "Delete $INSTALLER_PATH? (y/N): " -n 1 -r
                echo
                if [[ $REPLY =~ ^[Yy]$ ]]; then
                    if rm -f "$INSTALLER_PATH" 2>/dev/null; then
                        echo -e "${GREEN}✓ Installer deleted${NC}"
                    else
                        echo -e "${YELLOW}Failed to delete (may need sudo)${NC}"
                        echo "  Try: $SUDO rm -f '$INSTALLER_PATH'"
                    fi
                fi
            else
                echo -e "${YELLOW}File not found: $INSTALLER_PATH${NC}"
            fi
        fi
    fi
fi

echo ""
echo -e "${GREEN}Installation complete! Thank you for using VoltageEMS.${NC}"
echo ""
