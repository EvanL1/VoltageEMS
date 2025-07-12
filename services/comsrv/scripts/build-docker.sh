#!/bin/bash
# build-docker.sh - Build Docker image for comsrv

set -e

echo "Building comsrv Docker image..."

# Change to repository root directory
cd "$(dirname "$0")/../../.."

# Build the Docker image using comsrv Dockerfile
docker build -f services/comsrv/Dockerfile -t comsrv:latest .

echo "Docker image built successfully"