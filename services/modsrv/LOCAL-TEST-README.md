# ModSrv Local Test Environment

This guide explains how to run the ModSrv service in Docker and execute API tests locally.

## Test Architecture
1. Start ModSrv and Redis using `docker-compose.yml`.
2. Run `test-api.py` from your local Python environment.
3. Connect to the ModSrv API running inside Docker.

## Requirements
- Docker 20.10+
- Docker Compose 1.29+
- Python 3.6+

## Quick Start
```bash
chmod +x run-local-tests.sh
./run-local-tests.sh --build --clean
```
The script installs dependencies, starts the services, waits for readiness, runs the tests and then cleans up.

## Script Options
- `-b, --build` – rebuild images
- `-c, --clean` – remove containers when done
- `-l, --logs` – show modsrv logs
- `--debug` – verbose output

## Manual Steps
```bash
docker-compose up -d
python test-api.py
```
