#!/usr/bin/env bash
# VoltageEMS ARM64 Installation Script

set -euo pipefail

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Installation directories
INSTALL_DIR="/opt/MonarchEdge"
# Allow logs to be stored on external storage if available
LOG_DIR="${LOG_DIR:-/extp/logs}"

# Save the directory where installation was launched (for cleanup later)
LAUNCH_DIR="${LAUNCH_DIR:-$(pwd)}"

# Determine which host user should own installed files. This must be resolved
# before we attempt any permission changes while running with `set -u`.
determine_install_user() {
    local resolved=""

    if [[ -n "${INSTALL_USER:-}" ]]; then
        if id "${INSTALL_USER}" &>/dev/null; then
            resolved="${INSTALL_USER}"
            echo "Using specified user: $resolved"
        else
            echo -e "${YELLOW}Warning: INSTALL_USER=${INSTALL_USER} not found on system${NC}"
        fi
    fi

    if [[ -z "$resolved" && -n "${SUDO_USER:-}" && "${SUDO_USER}" != "root" ]]; then
        resolved="$SUDO_USER"
        echo "Detected installation user: $resolved (via sudo)"
    fi

    if [[ -z "$resolved" ]] && command -v logname &>/dev/null; then
        local login_user
        login_user=$(logname 2>/dev/null || true)
        if [[ -n "$login_user" && "$login_user" != "root" && $(id -u "$login_user" 2>/dev/null || echo -1) -ge 0 ]]; then
            resolved="$login_user"
            echo "Detected installation user: $resolved (via logname)"
        fi
    fi

    if [[ -z "$resolved" ]] && command -v who &>/dev/null; then
        local logged_user
        logged_user=$(who | grep -E "pts/|tty" | head -1 | awk '{print $1}' || true)
        if [[ -n "$logged_user" && "$logged_user" != "root" && $(id -u "$logged_user" 2>/dev/null || echo -1) -ge 0 ]]; then
            resolved="$logged_user"
            echo "Detected installation user: $resolved (via who)"
        fi
    fi

    if [[ -z "$resolved" ]]; then
        local current_dir
        current_dir=$(pwd)
        if [[ "$current_dir" =~ ^/home/([^/]+) ]]; then
            local potential_user="${BASH_REMATCH[1]}"
            if id "$potential_user" &>/dev/null; then
                resolved="$potential_user"
                echo "Detected installation user: $resolved (from current directory)"
            fi
        fi
    fi

    if [[ -z "$resolved" || "$resolved" == "root" ]]; then
        if [[ -r /etc/passwd ]]; then
            while IFS=: read -r name _ user_uid _; do
                case "$user_uid" in
                    ''|*[!0-9]*) continue ;;
                esac
                if (( user_uid >= 1000 && user_uid < 65534 )) && [[ "$name" != "root" ]]; then
                    if id "$name" &>/dev/null; then
                        resolved="$name"
                        echo -e "${YELLOW}Warning: Using first available user: $resolved${NC}"
                        echo -e "${YELLOW}To specify a different user, run: INSTALL_USER=<username> $0${NC}"
                    fi
                    break
                fi
            done < /etc/passwd
        fi
    fi

    if [[ -z "$resolved" ]]; then
        resolved=${USER:-$(whoami)}
        echo "Using current user: $resolved"
    fi

    ACTUAL_USER="$resolved"
    ACTUAL_UID=$(id -u "$ACTUAL_USER")
    ACTUAL_GID=$(id -g "$ACTUAL_USER")

    echo "Using installation user: $ACTUAL_USER (UID=$ACTUAL_UID)"

    if [[ "$ACTUAL_USER" == "root" ]]; then
        echo -e "${YELLOW}Warning: Directories will be owned by root. Set INSTALL_USER=<username> to override.${NC}"
    fi
}

# Check if a Docker image has changed by comparing image digests
# Args: $1 = image name (e.g., "redis:8-alpine")
# Returns: 0 if changed or not running, 1 if unchanged
check_image_changed() {
    local image_name=$1
    local container_name=""

    # Determine container name from image name
    case "$image_name" in
        voltage-redis:*)
            container_name="voltage-redis"
            ;;
        voltageems:*)
            # voltageems image is used by multiple containers
            # Check if any container is running
            container_name=$(docker ps --filter "ancestor=$image_name" --format "{{.Names}}" | head -1)
            ;;
        *)
            echo -e "${YELLOW}Unknown image: $image_name${NC}"
            return 0  # Assume changed for unknown images
            ;;
    esac

    # If container doesn't exist or isn't running, consider it as "needs update"
    if [[ -z "$container_name" ]]; then
        return 0
    fi

    # Get the image ID currently used by the running container
    local running_image_id
    running_image_id=$(docker inspect "$container_name" --format='{{.Image}}' 2>/dev/null)

    if [[ -z "$running_image_id" ]]; then
        # Container not found or not running
        return 0
    fi

    # Get the image ID of the local image with the same tag
    local local_image_id
    local_image_id=$(docker images "$image_name" --format='{{.ID}}' 2>/dev/null)

    if [[ -z "$local_image_id" ]]; then
        # Local image not found
        return 0
    fi

    # Compare image IDs
    if [[ "$running_image_id" == "$local_image_id" ]]; then
        # Images are the same
        return 1
    else
        # Images are different
        return 0
    fi
}

# Display image change status with colors
# Args: $1 = image name, $2 = running digest, $3 = new digest
display_image_status() {
    local image_name=$1
    local running_digest=$2
    local new_digest=$3

    if [[ -z "$running_digest" ]]; then
        echo -e "  ${BLUE}$image_name${NC}: ${YELLOW}not running${NC}"
    elif [[ "$running_digest" == "$new_digest" ]]; then
        echo -e "  ${BLUE}$image_name${NC}: ${GREEN}✓ unchanged${NC}"
    else
        echo -e "  ${BLUE}$image_name${NC}: ${RED}⚠ changed${NC}"
        echo -e "    Running: ${running_digest:0:12}"
        echo -e "    New:     ${new_digest:0:12}"
    fi
}

echo -e "${BLUE}================================${NC}"
echo -e "${BLUE}  VoltageEMS ARM64 Installer   ${NC}"
echo -e "${BLUE}================================${NC}"
echo ""

# Check architecture
ARCH=$(uname -m)
if [[ "$ARCH" != "aarch64" && "$ARCH" != "arm64" ]]; then
    echo -e "${YELLOW}Warning: This installer is for ARM64. Current arch: $ARCH${NC}"
    read -p "Continue anyway? (y/N): " -n 1 -r
    echo
    [[ ! $REPLY =~ ^[Yy]$ ]] && exit 1
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

# Install EMS Console GUI (optional)
if [[ -f "tools/ems-console" ]]; then
    echo ""
    echo -e "${YELLOW}Installing EMS Console GUI...${NC}"
    $SUDO cp -v "tools/ems-console" "/usr/local/bin/ems-console"
    $SUDO chmod +x "/usr/local/bin/ems-console"

    # Install launcher script if available
    if [[ -f "scripts/ems-console-launcher.sh" ]]; then
        $SUDO cp -v "scripts/ems-console-launcher.sh" "/usr/local/bin/ems-console-launcher"
        $SUDO chmod +x "/usr/local/bin/ems-console-launcher"
        echo -e "${GREEN}✓ EMS Console launcher installed${NC}"
    fi

    echo -e "${GREEN}✓ EMS Console GUI installed${NC}"
    echo ""
    echo -e "${BLUE}EMS Console provides real-time monitoring:${NC}"
    echo "  - Channel status and point values"
    echo "  - Live data updates from Redis"
    echo "  - Requires X11/GUI environment (use SSH -X for remote access)"
    echo ""
    echo -e "${YELLOW}Usage:${NC}"
    echo "  Local: ems-console"
    echo "  Remote (with checks): ems-console-launcher"
    echo "  Remote SSH: ssh -X user@host ems-console-launcher"
else
    echo -e "${YELLOW}Note: EMS Console GUI not included in this package${NC}"
    echo "      (Optional component - system will work without it)"
fi

echo -e "${GREEN}[DONE] CLI tools installed${NC}"

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
        echo "  1. Load new images (no service interruption)"
        echo "  2. Compare image changes"
        echo "  3. Only update changed services"
        echo "  4. Protect Redis from unnecessary restarts"
        echo ""
        read -p "Proceed with smart update? (Y/n): " -n 1 -r
        echo

        # Default to Yes if user just hits Enter
        if [[ -z "$REPLY" ]] || [[ $REPLY =~ ^[Yy]$ ]]; then
            # === PHASE 1: Load new images (without affecting running containers) ===
            echo ""
            echo -e "${BLUE}Phase 1: Loading new images...${NC}"

            # Backup existing images by tagging them
            echo "Creating backup tags for current images..."
            for image in voltageems:latest redis:8-alpine; do
                if docker image inspect "$image" >/dev/null 2>&1; then
                    backup_tag="${image/:latest/:backup-$(date +%s)}"
                    docker tag "$image" "$backup_tag" 2>/dev/null || true
                    echo "  Backed up $image → $backup_tag"
                fi
            done

            # Load new images
            LOADED_IMAGES=""
            for image in docker/*.tar.gz; do
                if [[ -f "$image" ]]; then
                    echo -n "  Loading $(basename "$image")... "
                    if OUTPUT=$(docker load < "$image" 2>&1); then
                        LOADED_NAME=$(echo "$OUTPUT" | grep "Loaded image:" | sed 's/Loaded image: //')
                        if [ -n "$LOADED_NAME" ]; then
                            LOADED_IMAGES="$LOADED_IMAGES $LOADED_NAME"
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

            # === PHASE 2: Detect image changes ===
            echo ""
            echo -e "${BLUE}Phase 2: Detecting image changes...${NC}"

            REDIS_CHANGED=false
            VOLTAGEEMS_CHANGED=false

            # Check voltage-redis
            if check_image_changed "redis:8-alpine"; then
                REDIS_CHANGED=true
                echo -e "  ${RED}⚠ voltage-redis has changed${NC}"
            else
                echo -e "  ${GREEN}✓ voltage-redis unchanged${NC}"
            fi

            # Check voltageems
            if check_image_changed "voltageems:latest"; then
                VOLTAGEEMS_CHANGED=true
                echo -e "  ${RED}⚠ voltageems has changed${NC}"
            else
                echo -e "  ${GREEN}✓ voltageems unchanged${NC}"
            fi

            # === PHASE 3: Selective update with Redis protection ===
            echo ""
            echo -e "${BLUE}Phase 3: Applying updates...${NC}"

            # Handle VoltageRedis update (with explicit confirmation)
            if [ "$REDIS_CHANGED" = true ]; then
                echo ""
                echo -e "${YELLOW}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
                echo -e "${YELLOW}⚠  Redis Image Update Detected${NC}"
                echo -e "${YELLOW}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
                echo ""
                echo "  Service: VoltageRedis"
                echo "  Impact: Brief service interruption (2-5 seconds)"
                echo "  Data: Preserved (AOF + RDB persistence)"
                echo ""
                read -p "Update Redis now? (yes/NO): " redis_confirm
                echo ""

                if [[ "$redis_confirm" == "yes" ]]; then
                    echo -e "${YELLOW}Updating VoltageRedis...${NC}"

                    # Stop and remove old Redis container
                    docker stop voltage-redis 2>/dev/null || true
                    docker rm voltage-redis 2>/dev/null || true

                    # Remove old Redis image
                    OLD_REDIS_IMAGES=$(docker images | grep "redis" | grep -v "backup\|8-alpine" | awk '{print $1":"$2}')
                    for img in $OLD_REDIS_IMAGES; do
                        if [[ "$img" != "redis:8-alpine" ]]; then
                            docker rmi "$img" 2>/dev/null || true
                        fi
                    done

                    # Start new Redis container using docker-compose
                    if [[ -f "$INSTALL_DIR/docker-compose.yml" ]]; then
                        docker compose -f "$INSTALL_DIR/docker-compose.yml" up -d voltage-redis
                        echo -e "${GREEN}✓ VoltageRedis updated successfully${NC}"
                    else
                        echo -e "${YELLOW}Note: docker-compose.yml not found, start manually after installation${NC}"
                    fi
                else
                    echo -e "${BLUE}Skipped Redis update (will use old image)${NC}"
                    # Restore old image tag
                    BACKUP_REDIS=$(docker images | grep "redis.*backup" | head -1 | awk '{print $1":"$2}')
                    if [[ -n "$BACKUP_REDIS" ]]; then
                        docker tag "$BACKUP_REDIS" "redis:8-alpine"
                        echo "  Restored previous redis:8-alpine"
                    fi
                fi
            else
                echo -e "${GREEN}✓ VoltageRedis: No update needed${NC}"
            fi

            # Handle VoltageEMS services update (automatic if changed)
            if [ "$VOLTAGEEMS_CHANGED" = true ]; then
                echo ""
                echo -e "${YELLOW}Updating VoltageEMS services (comsrv, modsrv, rulesrv)...${NC}"

                # Get list of running VoltageEMS containers
                RUNNING_SERVICES=$(docker ps --filter "ancestor=voltageems:latest" --format "{{.Names}}" | grep -E "comsrv|modsrv|rulesrv" || true)

                if [[ -n "$RUNNING_SERVICES" ]]; then
                    echo "  Stopping old containers: $(echo "$RUNNING_SERVICES" | tr '\n' ' ')"
                    echo "$RUNNING_SERVICES" | xargs -r docker stop 2>/dev/null || true
                    echo "$RUNNING_SERVICES" | xargs -r docker rm 2>/dev/null || true
                fi

                # Remove old voltageems images (keep backup)
                OLD_VOLTAGEEMS_IMAGES=$(docker images | grep "voltageems" | grep -v "backup" | awk '{print $1":"$2}')
                for img in $OLD_VOLTAGEEMS_IMAGES; do
                    if [[ "$img" != "voltageems:latest" ]]; then
                        docker rmi "$img" 2>/dev/null || true
                    fi
                done

                # Restart services using docker-compose (if file exists)
                if [[ -f "$INSTALL_DIR/docker-compose.yml" ]]; then
                    docker compose -f "$INSTALL_DIR/docker-compose.yml" up -d comsrv modsrv rulesrv
                    echo -e "${GREEN}✓ VoltageEMS services updated successfully${NC}"
                else
                    echo -e "${YELLOW}Note: docker-compose.yml not found, start manually after installation${NC}"
                fi
            else
                echo -e "${GREEN}✓ VoltageEMS services: No update needed${NC}"
            fi

            # === PHASE 4: Cleanup ===
            echo ""
            echo -e "${BLUE}Phase 4: Cleaning up...${NC}"

            # Remove backup tags (only if update was successful)
            BACKUP_IMAGES=$(docker images | grep "backup-" | awk '{print $1":"$2}')
            if [[ -n "$BACKUP_IMAGES" ]]; then
                echo "$BACKUP_IMAGES" | xargs -r docker rmi 2>/dev/null || true
                echo "  Removed backup images"
            fi

            # Clean up dangling images
            docker image prune -f 2>/dev/null || true

            echo ""
            echo -e "${GREEN}[DONE] Smart update completed${NC}"

            # Summary
            echo ""
            echo -e "${BLUE}Update Summary:${NC}"
            if [ "$REDIS_CHANGED" = true ]; then
                echo "  • VoltageRedis: Updated"
            else
                echo "  • VoltageRedis: Unchanged (no restart)"
            fi
            if [ "$VOLTAGEEMS_CHANGED" = true ]; then
                echo "  • VoltageEMS services: Updated"
            else
                echo "  • VoltageEMS services: Unchanged"
            fi
        else
            echo -e "${YELLOW}Skipping image update.${NC}"
            echo -e "${GREEN}[SKIPPED] Docker images${NC}"
        fi
    else
        # No existing images - first installation
        echo "Loading Docker images (first installation)..."
        LOADED_IMAGES=""
        for image in docker/*.tar.gz; do
            if [[ -f "$image" ]]; then
                echo -n "  Loading $(basename "$image")... "
                if OUTPUT=$(docker load < "$image" 2>&1); then
                    LOADED_NAME=$(echo "$OUTPUT" | grep "Loaded image:" | sed 's/Loaded image: //')
                    if [ -n "$LOADED_NAME" ]; then
                        LOADED_IMAGES="$LOADED_IMAGES $LOADED_NAME"
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
        for image_name in voltageems:latest redis:8-alpine; do
            echo -n "  Checking $image_name... "
            if docker image inspect "$image_name" >/dev/null 2>&1; then
                CREATED=$(docker image inspect "$image_name" --format='{{.Created}}' 2>/dev/null | cut -d'T' -f1)
                echo -e "${GREEN}present${NC} (created: $CREATED)"
            else
                echo -e "${RED}missing!${NC}"
                echo -e "${RED}ERROR: Expected image $image_name was not loaded properly${NC}"
                exit 1
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
for service in comsrv modsrv alarmsrv rulesrv hissrv; do
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
    echo -e "${YELLOW}Note: Copy config.template to config and customize before use${NC}"
else
    echo -e "${YELLOW}Warning: config.template not found${NC}"
fi

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

# Initialize empty SQLite database
echo "Creating placeholder database file..."
$SUDO touch "$INSTALL_DIR"/data/voltage.db
echo "Note: Database is empty. Run 'monarch init all && monarch sync all' after installation"

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
    # Fix scripts directory if it exists
    [[ -d "$INSTALL_DIR/scripts" ]] && chown -R $ACTUAL_USER:docker "$INSTALL_DIR/scripts" 2>/dev/null || true
else
    $SUDO chown -R $ACTUAL_USER:docker "$INSTALL_DIR/data" 2>/dev/null || true
    $SUDO chown -R $ACTUAL_USER:docker "$INSTALL_DIR/config.template" 2>/dev/null || true
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
        for service in comsrv modsrv alarmsrv rulesrv hissrv; do
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
        $SUDO chmod 775 "$LOG_DIR" || echo "Warning: Could not set permissions for $LOG_DIR"

        # Fix each service subdirectory explicitly
        for service in comsrv modsrv alarmsrv rulesrv hissrv; do
            if [[ -d "$LOG_DIR/$service" ]]; then
                $SUDO chown -R ${ACTUAL_UID}:${ACTUAL_GID} "$LOG_DIR/$service" || echo "Warning: Could not set ownership for $LOG_DIR/$service"
                $SUDO chmod 775 "$LOG_DIR/$service" || echo "Warning: Could not set permissions for $LOG_DIR/$service"
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

# Set permissions for subdirectories (775 - owner and group can write)
if [[ -z "$SUDO" ]]; then
    chmod -R 775 "$INSTALL_DIR/data"
    chmod -R 775 "$LOG_DIR"
else
    $SUDO chmod -R 775 "$INSTALL_DIR/data"
    $SUDO chmod -R 775 "$LOG_DIR"
fi

# Update development .env if we're in development mode (running from source directory)
if [[ "$INSTALL_DIR" == "$(pwd)" ]] || [[ -f ".env" && -f "Cargo.toml" ]]; then
    echo "Updating development .env file with user permissions..."

    # Use the update-env-permissions.sh script if it exists
    if [[ -f "scripts/update-env-permissions.sh" ]]; then
        echo "Running update-env-permissions.sh to configure permissions..."
        bash scripts/update-env-permissions.sh
    elif [[ -f "$INSTALL_DIR/scripts/update-env-permissions.sh" ]]; then
        echo "Running update-env-permissions.sh to configure permissions..."
        bash "$INSTALL_DIR/scripts/update-env-permissions.sh"
    else
        # Fallback to inline update if script not found
        if [[ -f ".env" ]]; then
            # Check if HOST_UID and HOST_GID already exist
            if grep -q "^HOST_UID=" .env; then
                # Update existing values
                sed -i.bak "s/^HOST_UID=.*/HOST_UID=$ACTUAL_UID/" .env
                sed -i.bak "s/^HOST_GID=.*/HOST_GID=$DOCKER_GID/" .env
                rm -f .env.bak
            else
                # Add after the second line (after header comments)
                sed -i.bak "3i\\
\\
# User configuration for container permissions\\
HOST_UID=$ACTUAL_UID         # User ID for file ownership\\
HOST_GID=$DOCKER_GID         # Docker group GID for container access" .env
                rm -f .env.bak
            fi
            echo "Development .env updated with HOST_UID=$ACTUAL_UID and HOST_GID=$DOCKER_GID"
        fi
    fi
fi

# Create .env file with user configuration
echo "Creating environment configuration..."
$SUDO bash -c "cat > '$INSTALL_DIR/.env' << ENVEOF
# Auto-generated during installation
# User: $ACTUAL_USER (UID=$ACTUAL_UID)
# Date: $(date)

# User configuration for container permissions
HOST_UID=$ACTUAL_UID        # User ID for file ownership
HOST_GID=$DOCKER_GID         # Use docker group for container GID
HOST_USER=$ACTUAL_USER       # Username for reference
DOCKER_GID=$DOCKER_GID       # Docker group GID

# Redis connection
REDIS_URL=redis://voltage-redis:6379

# Logging level
RUST_LOG=info

# Unified database path for all services (container path)
VOLTAGE_DB_PATH=/app/data/voltage.db
ENVEOF"

# Set correct ownership for .env file
$SUDO chown $ACTUAL_USER:docker "$INSTALL_DIR/.env"
$SUDO chmod 640 "$INSTALL_DIR/.env"

# Run update-env-permissions.sh if it exists in production installation
if [[ "$INSTALL_DIR" != "$(pwd)" ]] && [[ -f "$INSTALL_DIR/scripts/update-env-permissions.sh" ]]; then
    echo "Running update-env-permissions.sh to finalize permissions..."
    cd "$INSTALL_DIR" && bash scripts/update-env-permissions.sh
    cd - > /dev/null
fi

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
    for service in comsrv modsrv alarmsrv rulesrv hissrv; do
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
echo "    - Redis: 6379"
echo "    - ComSrv: 6001"
echo "    - ModSrv: 6002"
echo "    - AlarmSrv: 6006"
echo "    - RuleSrv: 6003"
echo "    - HisSrv: 6004"
echo ""
echo -e "${YELLOW}Important: Configuration Setup Required${NC}"
echo "  Before starting services, you must:"
echo "  1. Copy configuration template:"
echo "     cp -r $INSTALL_DIR/config.template $INSTALL_DIR/config"
echo "  2. Customize configuration files in config/ directory"
echo "  3. Initialize and sync configurations:"
echo "     monarch init all && monarch sync all"
echo ""
echo "Quick Start:"
echo "  docker-compose up -d   - Start all services"
echo "  docker-compose down    - Stop all services"
echo "  docker-compose ps      - Check service status"
echo "  docker-compose logs -f - View service logs"
echo ""
echo "CLI Management (via monarch):"
echo ""
echo "  Configuration:"
echo "    monarch sync all              - Sync all configurations"
echo "    monarch validate all          - Validate configurations"
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
else
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

echo ""
echo -e "${GREEN}Installation complete! Thank you for using VoltageEMS.${NC}"
echo ""
