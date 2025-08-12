# VoltageEMS Makefile
# Common development and deployment tasks

.PHONY: help build test clean deploy-staging deploy-production check fmt

# Default target
help:
	@echo "VoltageEMS Development Commands:"
	@echo "  make build          - Build all services"
	@echo "  make test           - Run all tests"
	@echo "  make check          - Run code quality checks"
	@echo "  make fmt            - Format code"
	@echo "  make clean          - Clean build artifacts"
	@echo "  make up             - Start all services"
	@echo "  make down           - Stop all services"
	@echo "  make logs           - Show service logs"
	@echo "  make deploy-staging - Deploy to staging"
	@echo "  make deploy-prod    - Deploy to production"

# Development
build:
	cargo build --workspace

test:
	cargo test --workspace

check:
	cargo fmt --all -- --check
	cargo clippy --all-targets --all-features -- -D warnings
	cargo check --workspace

fmt:
	cargo fmt --all

clean:
	cargo clean
	docker-compose down -v
	rm -rf data/

# Docker operations
up:
	docker-compose up -d
	@echo "Waiting for services to start..."
	@sleep 10
	docker exec $$(docker-compose ps -q redis) sh -c "cd /scripts && sh load_all_functions.sh"
	docker-compose ps

down:
	docker-compose down

logs:
	docker-compose logs -f

# Deployment
deploy-staging:
	./scripts/deploy.sh staging

deploy-prod:
	./scripts/deploy.sh production

# Testing
integration-test:
	python -m pytest tests/test_integration.py tests/test_services.py -v

system-test:
	uv run python tests/test_system_integration.py

# Docker Integration Testing
test-docker: test-docker-modsrv test-docker-hissrv test-docker-e2e
	@echo "All Docker tests completed!"

test-docker-modsrv:
	@echo "Running modsrv Docker integration tests..."
	@./scripts/test-modsrv-docker.sh

test-docker-hissrv:
	@echo "Running hissrv Docker integration tests..."
	@./scripts/test-hissrv-docker.sh

test-docker-e2e:
	@echo "Running end-to-end Docker integration tests..."
	@./scripts/test-e2e-docker.sh

# Test with special configuration
test-docker-quick:
	@echo "Running quick Docker tests with faster intervals..."
	docker-compose -f docker-compose.yml -f docker-compose.test.yml up -d
	@sleep 5
	@./scripts/test-e2e-docker.sh

# Clean test environment
test-clean:
	docker-compose down -v
	docker system prune -f
	@echo "Test environment cleaned!"

# Database operations
redis-cli:
	docker exec -it $$(docker-compose ps -q redis) redis-cli

influx-cli:
	docker exec -it $$(docker-compose ps -q influxdb) influx

# Monitoring
monitor:
	@echo "=== Service Health Status ==="
	@curl -s http://localhost:8087/health > /dev/null && echo "✓ API Gateway" || echo "✗ API Gateway"
	@curl -s http://localhost:8081/health > /dev/null && echo "✓ ComSrv" || echo "✗ ComSrv"
	@curl -s http://localhost:8082/health > /dev/null && echo "✓ ModSrv" || echo "✗ ModSrv"
	@curl -s http://localhost:8083/health > /dev/null && echo "✓ AlarmSrv" || echo "✗ AlarmSrv"
	@curl -s http://localhost:8084/health > /dev/null && echo "✓ RuleSrv" || echo "✗ RuleSrv"
	@curl -s http://localhost:8085/health > /dev/null && echo "✓ HisSrv" || echo "✗ HisSrv"
	@curl -s http://localhost:8088/health > /dev/null && echo "✓ NetSrv" || echo "✗ NetSrv"