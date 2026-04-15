<#
.SYNOPSIS
    VoltageEMS frontend Docker local deploy script

.EXAMPLE
    .\scripts\docker-deploy.ps1
    .\scripts\docker-deploy.ps1 -Tag v1.2.3
    .\scripts\docker-deploy.ps1 -NoCache
#>

param(
    [string]$Tag           = "latest",
    [switch]$NoCache,
    [string]$ContainerName = "voltage-apps",
    [int]$Port             = 8080
)

[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
[Console]::InputEncoding  = [System.Text.Encoding]::UTF8
$OutputEncoding           = [System.Text.Encoding]::UTF8

$ImageFull = "${ContainerName}:${Tag}"
$AppsDir   = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)

Write-Host ""
Write-Host "================================================" -ForegroundColor Cyan
Write-Host "   VoltageEMS Docker Deploy" -ForegroundColor Cyan
Write-Host "================================================" -ForegroundColor Cyan

# Step 0: Check Docker
Write-Host "`n[0/4] Checking Docker..." -ForegroundColor Yellow
docker version > $null 2>&1
if ($LASTEXITCODE -ne 0) {
    Write-Host "[ERROR] Docker is not running. Please start Docker Desktop." -ForegroundColor Red
    exit 1
}
Write-Host "  OK Docker is ready" -ForegroundColor Green

# Step 1: Remove old container (by name)
Write-Host "`n[1/4] Removing old container: $ContainerName ..." -ForegroundColor Yellow
$existing = docker ps -a --filter "name=$ContainerName" --format "{{.ID}}" 2> $null
if ($existing) {
    docker stop $existing > $null 2>&1
    docker rm   $existing > $null 2>&1
    Write-Host "  OK Removed: $existing" -ForegroundColor Green
} else {
    Write-Host "  No existing container found, skipping" -ForegroundColor Gray
}

# Step 2: Build image
Write-Host "`n[2/4] Building image: $ImageFull ..." -ForegroundColor Yellow

if (-not (Test-Path "$AppsDir\Dockerfile")) {
    Write-Host "[ERROR] Dockerfile not found at: $AppsDir" -ForegroundColor Red
    exit 1
}

$buildArgs = @("build", "-t", $ImageFull)
if ($NoCache) {
    $buildArgs += "--no-cache"
    Write-Host "  Using --no-cache (full rebuild)" -ForegroundColor Gray
}
$buildArgs += $AppsDir

& docker @buildArgs
if ($LASTEXITCODE -ne 0) {
    Write-Host "[ERROR] Docker build failed" -ForegroundColor Red
    exit 1
}
Write-Host "  OK Image built successfully" -ForegroundColor Green

# Step 3: Start container
Write-Host "`n[3/4] Starting container..." -ForegroundColor Yellow
docker run -d --name $ContainerName --restart unless-stopped -p "${Port}:8080" $ImageFull > $null 2>&1
if ($LASTEXITCODE -ne 0) {
    Write-Host "[ERROR] Container failed to start" -ForegroundColor Red
    docker logs --tail 20 $ContainerName
    exit 1
}
Write-Host "  OK Container started" -ForegroundColor Green

# Step 4: Verify
Write-Host "`n[4/4] Verifying..." -ForegroundColor Yellow
Start-Sleep -Seconds 2
$running = docker ps --filter "name=$ContainerName" --filter "status=running" --format "{{.Status}}" 2> $null
if ($running) {
    Write-Host "  OK Status: $running" -ForegroundColor Green
    Write-Host ""
    Write-Host "================================================" -ForegroundColor Cyan
    Write-Host "   Deploy successful!  http://localhost:$Port" -ForegroundColor Cyan
    Write-Host "================================================" -ForegroundColor Cyan
    Write-Host ""
    Write-Host "  View logs : docker logs -f $ContainerName" -ForegroundColor Gray
    Write-Host "  Stop      : docker stop $ContainerName" -ForegroundColor Gray
} else {
    Write-Host "[WARNING] Container may not be running" -ForegroundColor Yellow
    docker logs --tail 20 $ContainerName
    exit 1
}
