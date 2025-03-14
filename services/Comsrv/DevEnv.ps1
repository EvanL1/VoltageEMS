# Set console encoding to UTF-8
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8

Write-Host "Script execution started"
# Define variables
$imageName = "my-dev-env"
$containerName = "my-dev-container"
$codeDir = "."  
$workDir = "/workspace" 

Write-Host "Building development image..."
# Build the development image
docker build -t $imageName -f DevDockerfile .

Write-Host "Checking container status"
# Check if the container is already running
$runningContainer = docker ps -q -f "name=$containerName"

if ($runningContainer) {
    Write-Host "Container $containerName is already running."
} else {
    Write-Host "Starting development container..."
    # Start the container with necessary mappings
    docker run -d --name $containerName `
        -v ${codeDir}:${workDir} `
        -v /dev:/dev `
        --privileged `
        -p 502:502 `
        -p 6379:6379 `
        --network host `
        $imageName

    Write-Host "Container $containerName has been started."
}

Write-Host "Development environment is ready!"
Write-Host "To enter the container, run: docker exec -it $containerName bash"
Write-Host "To build the project inside container:"
Write-Host "1. cd /workspace"
Write-Host "2. mkdir -p build && cd build"
Write-Host "3. cmake .."
Write-Host "4. make -j$(nproc)"