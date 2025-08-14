#!/bin/bash
# Build Docker dependencies base image
# This creates a base image with all Rust dependencies pre-compiled

set -e

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

cd "$PROJECT_DIR"

echo "========================================="
echo "Building VoltageEMS Dependencies Image"
echo "========================================="

# Build the base dependencies image
echo "Building voltageems-dependencies:latest..."
docker build -f Dockerfile.base -t voltageems-dependencies:latest .

if [ $? -eq 0 ]; then
    echo ""
    echo "✅ Dependencies image built successfully!"
    echo ""
    echo "You can now build individual services faster with:"
    echo "  docker-compose build --parallel"
    echo ""
    echo "Or build a specific service:"
    echo "  docker-compose build comsrv"
else
    echo ""
    echo "❌ Failed to build dependencies image"
    exit 1
fi

# Optional: Show image size
echo ""
echo "Image info:"
docker images voltageems-dependencies:latest