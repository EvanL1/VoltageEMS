#!/bin/bash
# VoltageEMS Comprehensive Test Runner
# Orchestrates different types of testing environments

set -e

# Color definitions
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
DOCKER_COMPOSE_TEST="$PROJECT_ROOT/docker-compose.test.yml"
DOCKER_COMPOSE_INTEGRATION="$PROJECT_ROOT/docker-compose.integration.yml"
DOCKER_COMPOSE_LOAD="$PROJECT_ROOT/docker-compose.load.yml"

# Default values
TEST_TYPE="all"
CLEANUP_AFTER="true"
VERBOSE="false"
PARALLEL="true"
SAVE_RESULTS="true"

# Function to print usage
usage() {
    echo "Usage: $0 [OPTIONS]"
    echo ""
    echo "Options:"
    echo "  -t, --type TYPE        Test type: unit|integration|load|all (default: all)"
    echo "  -c, --no-cleanup      Don't cleanup containers after tests"
    echo "  -v, --verbose         Verbose output"
    echo "  -s, --sequential      Run tests sequentially instead of parallel"
    echo "  -n, --no-save         Don't save test results"
    echo "  -h, --help            Show this help message"
    echo ""
    echo "Examples:"
    echo "  $0                    # Run all tests"
    echo "  $0 -t unit            # Run only unit tests"
    echo "  $0 -t integration -v  # Run integration tests with verbose output"
    echo "  $0 -t load --no-cleanup  # Run load tests and keep containers"
}

# Function to log messages
log() {
    local level=$1
    shift
    local message="$*"
    local timestamp=$(date '+%Y-%m-%d %H:%M:%S')
    
    case $level in
        INFO)
            echo -e "${GREEN}[$timestamp] INFO:${NC} $message"
            ;;
        WARN)
            echo -e "${YELLOW}[$timestamp] WARN:${NC} $message"
            ;;
        ERROR)
            echo -e "${RED}[$timestamp] ERROR:${NC} $message"
            ;;
        DEBUG)
            if [[ "$VERBOSE" == "true" ]]; then
                echo -e "${BLUE}[$timestamp] DEBUG:${NC} $message"
            fi
            ;;
    esac
}

# Function to check prerequisites
check_prerequisites() {
    log INFO "Checking prerequisites..."
    
    # Check if Docker is installed and running
    if ! command -v docker &> /dev/null; then
        log ERROR "Docker is not installed"
        exit 1
    fi
    
    if ! docker info &> /dev/null; then
        log ERROR "Docker is not running"
        exit 1
    fi
    
    # Check if docker-compose is available
    if ! command -v docker-compose &> /dev/null && ! docker compose version &> /dev/null; then
        log ERROR "Docker Compose is not available"
        exit 1
    fi
    
    # Determine docker-compose command
    if command -v docker-compose &> /dev/null; then
        DOCKER_COMPOSE_CMD="docker-compose"
    else
        DOCKER_COMPOSE_CMD="docker compose"
    fi
    
    # Check if compose files exist
    for file in "$DOCKER_COMPOSE_TEST" "$DOCKER_COMPOSE_INTEGRATION" "$DOCKER_COMPOSE_LOAD"; do
        if [[ ! -f "$file" ]]; then
            log ERROR "Docker compose file not found: $file"
            exit 1
        fi
    done
    
    log INFO "Prerequisites check passed"
}

# Function to cleanup containers and volumes
cleanup() {
    local compose_file=$1
    local project_name=$2
    
    log INFO "Cleaning up containers and volumes for $project_name..."
    
    # Stop and remove containers
    $DOCKER_COMPOSE_CMD -f "$compose_file" -p "$project_name" down --volumes --remove-orphans &> /dev/null || true
    
    # Remove dangling volumes
    docker volume prune -f &> /dev/null || true
    
    # Remove dangling networks
    docker network prune -f &> /dev/null || true
}

# Function to wait for services to be ready
wait_for_services() {
    local compose_file=$1
    local project_name=$2
    local services=("${@:3}")
    
    log INFO "Waiting for services to be ready..."
    
    local max_attempts=30
    local attempt=1
    
    for service in "${services[@]}"; do
        log DEBUG "Waiting for service: $service"
        
        while [[ $attempt -le $max_attempts ]]; do
            if $DOCKER_COMPOSE_CMD -f "$compose_file" -p "$project_name" ps "$service" 2>/dev/null | grep -q "Up"; then
                # Check health status if available
                if $DOCKER_COMPOSE_CMD -f "$compose_file" -p "$project_name" ps "$service" | grep -q "healthy\|Up"; then
                    log DEBUG "Service $service is ready"
                    break
                fi
            fi
            
            log DEBUG "Waiting for $service... (attempt $attempt/$max_attempts)"
            sleep 2
            ((attempt++))
        done
        
        if [[ $attempt -gt $max_attempts ]]; then
            log WARN "Service $service did not become ready within timeout"
        fi
        
        attempt=1
    done
}

# Function to run unit tests
run_unit_tests() {
    log INFO "Starting unit tests..."
    
    local project_name="voltageems-unit-tests"
    
    # Cleanup any existing containers
    cleanup "$DOCKER_COMPOSE_TEST" "$project_name"
    
    # Start infrastructure services
    log INFO "Starting test infrastructure..."
    $DOCKER_COMPOSE_CMD -f "$DOCKER_COMPOSE_TEST" -p "$project_name" up -d redis-test influxdb-test
    
    # Wait for infrastructure
    wait_for_services "$DOCKER_COMPOSE_TEST" "$project_name" "redis-test" "influxdb-test"
    
    # Initialize test data
    log INFO "Initializing test data..."
    $DOCKER_COMPOSE_CMD -f "$DOCKER_COMPOSE_TEST" -p "$project_name" run --rm test-data-init
    
    # Run unit tests
    log INFO "Running unit tests..."
    local unit_test_exit_code=0
    
    if [[ "$PARALLEL" == "true" ]]; then
        # Run service tests in parallel
        $DOCKER_COMPOSE_CMD -f "$DOCKER_COMPOSE_TEST" -p "$project_name" run --rm unit-tests &
        $DOCKER_COMPOSE_CMD -f "$DOCKER_COMPOSE_TEST" -p "$project_name" run --rm comsrv-test &
        $DOCKER_COMPOSE_CMD -f "$DOCKER_COMPOSE_TEST" -p "$project_name" run --rm modsrv-test &
        $DOCKER_COMPOSE_CMD -f "$DOCKER_COMPOSE_TEST" -p "$project_name" run --rm alarmsrv-test &
        $DOCKER_COMPOSE_CMD -f "$DOCKER_COMPOSE_TEST" -p "$project_name" run --rm rulesrv-test &
        $DOCKER_COMPOSE_CMD -f "$DOCKER_COMPOSE_TEST" -p "$project_name" run --rm hissrv-test &
        
        # Wait for all background jobs
        wait
        unit_test_exit_code=$?
    else
        # Run tests sequentially
        $DOCKER_COMPOSE_CMD -f "$DOCKER_COMPOSE_TEST" -p "$project_name" run --rm unit-tests || unit_test_exit_code=1
        $DOCKER_COMPOSE_CMD -f "$DOCKER_COMPOSE_TEST" -p "$project_name" run --rm comsrv-test || unit_test_exit_code=1
        $DOCKER_COMPOSE_CMD -f "$DOCKER_COMPOSE_TEST" -p "$project_name" run --rm modsrv-test || unit_test_exit_code=1
        $DOCKER_COMPOSE_CMD -f "$DOCKER_COMPOSE_TEST" -p "$project_name" run --rm alarmsrv-test || unit_test_exit_code=1
        $DOCKER_COMPOSE_CMD -f "$DOCKER_COMPOSE_TEST" -p "$project_name" run --rm rulesrv-test || unit_test_exit_code=1
        $DOCKER_COMPOSE_CMD -f "$DOCKER_COMPOSE_TEST" -p "$project_name" run --rm hissrv-test || unit_test_exit_code=1
    fi
    
    # Collect results
    if [[ "$SAVE_RESULTS" == "true" ]]; then
        log INFO "Collecting test results..."
        $DOCKER_COMPOSE_CMD -f "$DOCKER_COMPOSE_TEST" -p "$project_name" run --rm test-collector
    fi
    
    # Cleanup if requested
    if [[ "$CLEANUP_AFTER" == "true" ]]; then
        cleanup "$DOCKER_COMPOSE_TEST" "$project_name"
    fi
    
    if [[ $unit_test_exit_code -eq 0 ]]; then
        log INFO "Unit tests completed successfully"
    else
        log ERROR "Unit tests failed"
    fi
    
    return $unit_test_exit_code
}

# Function to run integration tests
run_integration_tests() {
    log INFO "Starting integration tests..."
    
    local project_name="voltageems-integration-tests"
    
    # Cleanup any existing containers
    cleanup "$DOCKER_COMPOSE_INTEGRATION" "$project_name"
    
    # Start all services
    log INFO "Starting integration test environment..."
    $DOCKER_COMPOSE_CMD -f "$DOCKER_COMPOSE_INTEGRATION" -p "$project_name" up -d
    
    # Wait for services to be ready
    wait_for_services "$DOCKER_COMPOSE_INTEGRATION" "$project_name" \
        "redis-integration" "influxdb-integration" "comsrv-integration" \
        "modsrv-integration" "alarmsrv-integration" "rulesrv-integration" \
        "hissrv-integration" "apigateway-integration"
    
    # Run integration test scenarios
    log INFO "Running integration test scenarios..."
    local integration_exit_code=0
    
    # Run integration test runner
    $DOCKER_COMPOSE_CMD -f "$DOCKER_COMPOSE_INTEGRATION" -p "$project_name" run --rm integration-test-runner || integration_exit_code=1
    
    # Run E2E test scenarios
    if [[ $integration_exit_code -eq 0 ]]; then
        log INFO "Running E2E test scenarios..."
        $DOCKER_COMPOSE_CMD -f "$DOCKER_COMPOSE_INTEGRATION" -p "$project_name" run --rm e2e-data-flow-test || integration_exit_code=1
        $DOCKER_COMPOSE_CMD -f "$DOCKER_COMPOSE_INTEGRATION" -p "$project_name" run --rm e2e-alarm-workflow-test || integration_exit_code=1
        $DOCKER_COMPOSE_CMD -f "$DOCKER_COMPOSE_INTEGRATION" -p "$project_name" run --rm e2e-rule-engine-test || integration_exit_code=1
    fi
    
    # Cleanup if requested
    if [[ "$CLEANUP_AFTER" == "true" ]]; then
        cleanup "$DOCKER_COMPOSE_INTEGRATION" "$project_name"
    fi
    
    if [[ $integration_exit_code -eq 0 ]]; then
        log INFO "Integration tests completed successfully"
    else
        log ERROR "Integration tests failed"
    fi
    
    return $integration_exit_code
}

# Function to run load tests
run_load_tests() {
    log INFO "Starting load tests..."
    
    local project_name="voltageems-load-tests"
    
    # Cleanup any existing containers
    cleanup "$DOCKER_COMPOSE_LOAD" "$project_name"
    
    # Start load test environment
    log INFO "Starting load test environment..."
    $DOCKER_COMPOSE_CMD -f "$DOCKER_COMPOSE_LOAD" -p "$project_name" up -d \
        redis-load influxdb-load modbus-load-generator prometheus grafana
    
    # Wait for infrastructure
    wait_for_services "$DOCKER_COMPOSE_LOAD" "$project_name" "redis-load" "influxdb-load"
    
    # Start VoltageEMS services
    log INFO "Starting VoltageEMS services for load testing..."
    $DOCKER_COMPOSE_CMD -f "$DOCKER_COMPOSE_LOAD" -p "$project_name" up -d \
        comsrv-load modsrv-load alarmsrv-load rulesrv-load hissrv-load nginx-load
    
    # Wait for services
    wait_for_services "$DOCKER_COMPOSE_LOAD" "$project_name" "nginx-load"
    
    # Run load tests
    log INFO "Running load tests..."
    local load_test_exit_code=0
    
    if [[ "$PARALLEL" == "true" ]]; then
        # Run different load test tools in parallel
        $DOCKER_COMPOSE_CMD -f "$DOCKER_COMPOSE_LOAD" -p "$project_name" run --rm k6-api-load &
        $DOCKER_COMPOSE_CMD -f "$DOCKER_COMPOSE_LOAD" -p "$project_name" run --rm k6-modbus-load &
        $DOCKER_COMPOSE_CMD -f "$DOCKER_COMPOSE_LOAD" -p "$project_name" run --rm k6-redis-load &
        $DOCKER_COMPOSE_CMD -f "$DOCKER_COMPOSE_LOAD" -p "$project_name" run --rm artillery-mixed-load &
        
        # Wait for all load tests
        wait
        load_test_exit_code=$?
    else
        # Run load tests sequentially
        $DOCKER_COMPOSE_CMD -f "$DOCKER_COMPOSE_LOAD" -p "$project_name" run --rm k6-api-load || load_test_exit_code=1
        $DOCKER_COMPOSE_CMD -f "$DOCKER_COMPOSE_LOAD" -p "$project_name" run --rm k6-modbus-load || load_test_exit_code=1
        $DOCKER_COMPOSE_CMD -f "$DOCKER_COMPOSE_LOAD" -p "$project_name" run --rm artillery-mixed-load || load_test_exit_code=1
    fi
    
    # Analyze results
    if [[ "$SAVE_RESULTS" == "true" ]]; then
        log INFO "Analyzing load test results..."
        $DOCKER_COMPOSE_CMD -f "$DOCKER_COMPOSE_LOAD" -p "$project_name" run --rm load-test-analyzer
    fi
    
    # Keep monitoring stack running if not cleaning up
    if [[ "$CLEANUP_AFTER" == "false" ]]; then
        log INFO "Load test environment kept running for analysis"
        log INFO "Grafana dashboard: http://localhost:3001 (admin/loadtest123)"
        log INFO "Prometheus: http://localhost:9090"
    else
        cleanup "$DOCKER_COMPOSE_LOAD" "$project_name"
    fi
    
    if [[ $load_test_exit_code -eq 0 ]]; then
        log INFO "Load tests completed successfully"
    else
        log ERROR "Load tests failed"
    fi
    
    return $load_test_exit_code
}

# Function to run all tests
run_all_tests() {
    log INFO "Running all test suites..."
    
    local overall_exit_code=0
    
    # Run tests in sequence
    run_unit_tests || overall_exit_code=1
    run_integration_tests || overall_exit_code=1
    run_load_tests || overall_exit_code=1
    
    return $overall_exit_code
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -t|--type)
            TEST_TYPE="$2"
            shift 2
            ;;
        -c|--no-cleanup)
            CLEANUP_AFTER="false"
            shift
            ;;
        -v|--verbose)
            VERBOSE="true"
            shift
            ;;
        -s|--sequential)
            PARALLEL="false"
            shift
            ;;
        -n|--no-save)
            SAVE_RESULTS="false"
            shift
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            usage
            exit 1
            ;;
    esac
done

# Main execution
main() {
    log INFO "VoltageEMS Test Runner starting..."
    log INFO "Test type: $TEST_TYPE"
    log INFO "Cleanup after: $CLEANUP_AFTER"
    log INFO "Parallel execution: $PARALLEL"
    log INFO "Save results: $SAVE_RESULTS"
    
    check_prerequisites
    
    local exit_code=0
    
    case $TEST_TYPE in
        unit)
            run_unit_tests || exit_code=1
            ;;
        integration)
            run_integration_tests || exit_code=1
            ;;
        load)
            run_load_tests || exit_code=1
            ;;
        all)
            run_all_tests || exit_code=1
            ;;
        *)
            log ERROR "Invalid test type: $TEST_TYPE"
            usage
            exit 1
            ;;
    esac
    
    if [[ $exit_code -eq 0 ]]; then
        log INFO "All tests completed successfully!"
    else
        log ERROR "Some tests failed!"
    fi
    
    exit $exit_code
}

# Run main function
main "$@"