#!/bin/bash
# Start complete test environment for comsrv

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "=== Starting ComSrv Test Environment ==="
echo "Project root: $PROJECT_ROOT"

# Change to project directory
cd "$PROJECT_ROOT"

# Stop any existing containers
echo "Stopping existing containers..."
docker-compose -f docker-compose.test.yml down

# Create logs directory if it doesn't exist
mkdir -p logs

# Start all services
echo "Starting services..."
docker-compose -f docker-compose.test.yml up -d

# Wait for services to be healthy
echo "Waiting for services to be healthy..."
sleep 5

# Check service status
echo ""
echo "=== Service Status ==="
docker-compose -f docker-compose.test.yml ps

# Show Redis connection test
echo ""
echo "=== Testing Redis Connection ==="
docker exec comsrv-redis-test redis-cli --user readonly --pass readonly_password_2025 ping || echo "Redis connection failed"

# Show comsrv logs
echo ""
echo "=== ComSrv Logs (last 20 lines) ==="
docker logs comsrv-test --tail 20

echo ""
echo "=== Test Environment Started ==="
echo "- Redis: redis://comsrv:comsrv_secure_password_2025@localhost:6379"
echo "- Modbus Simulator: localhost:5020"
echo "- ComSrv API: http://localhost:3000"
echo ""
echo "To monitor logs: docker-compose -f docker-compose.test.yml logs -f"
echo "To stop: docker-compose -f docker-compose.test.yml down"