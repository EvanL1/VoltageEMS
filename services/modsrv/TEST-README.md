# ModSrv Docker Test Environment

This document describes how to run the full ModSrv API test suite inside Docker.

## Components
1. **test-api.py** – exercises all API endpoints including health checks and rule management
2. **Docker environment** – Redis, ModSrv service and a test container
3. **Helper script** – provides a command line interface for running tests

## Quick Start
```bash
chmod +x run-docker-tests.sh
./run-docker-tests.sh --build --clean
```

## Script Options
- `-b, --build` – rebuild images
- `-d, --detach` – run containers in the background
- `-c, --clean` – remove containers after tests
- `-l, --logs` – show modsrv logs
- `--debug` – verbose output

## Manual Execution
You can run Docker Compose commands directly if desired:
```bash
docker-compose -f docker-compose.test.yml build
docker-compose -f docker-compose.test.yml up
```
