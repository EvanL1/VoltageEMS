#!/bin/bash

# Check if Docker is running
if ! docker info > /dev/null 2>&1; then
    echo "Docker is not running. Please start Docker and try again."
    exit 1
fi

# Define variables
imageName="my-dev-env"
containerName="my-dev-container"
codeDir="$(pwd)"
workDir="/workspace"

echo "Building development image..."
# Build the development image
docker build -t $imageName -f DevDockerfile .

echo "Checking container status"
# Check if the container is already running
runningContainer=$(docker ps -q -f "name=$containerName")

if [ -n "$runningContainer" ]; then
    echo "Container $containerName is already running."
else
    echo "Starting development container..."
    # Start the container with necessary mappings
    docker run -d --name $containerName \
        -v ${codeDir}:${workDir} \
        -v /dev:/dev \
        --privileged \
        -p 502:502 \
        -p 6379:6379 \
        --network host \
        $imageName

    echo "Container $containerName has been started."
fi

echo "Development environment is ready!"
echo "To enter the container, run: docker exec -it $containerName bash"
echo "To build the project inside container:"
echo "1. cd /workspace"
echo "2. mkdir -p build && cd build"
echo "3. cmake .."
echo "4. make -j$(nproc)"