#!/bin/bash
set -e

# Color codes for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${YELLOW}=== Running VoltageEMS Integration Tests ===${NC}"

# Get absolute path to project root for docker-compose file
PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

# Prefer using the containerized redis-cli in CI, but keep host fallback for developers
DC="docker compose -f ${PROJECT_ROOT}/docker/docker-compose-ci.yml"
RC="$DC exec -T voltage-redis redis-cli"

USE_CONTAINER=0
if $RC ping > /dev/null 2>&1; then
    echo -e "${GREEN}✓ Redis (container) is running${NC}"
    USE_CONTAINER=1
elif command -v redis-cli >/dev/null 2>&1 && redis-cli ping >/dev/null 2>&1; then
    echo -e "${GREEN}✓ Redis (host) is running${NC}"
    USE_CONTAINER=0
else
    echo -e "${RED}Error: Redis is not running${NC}"
    echo "Please start Redis first (choose one):"
    echo "  docker compose -f docker/docker-compose-ci.yml up -d voltage-redis"
    echo "  # or"
    echo "  redis-server"
    exit 1
fi

# Clean Redis test data
# Note: Using Lua EVAL for cleanup (Redis standard feature, not custom commands)
echo -e "${YELLOW}Cleaning test data...${NC}"
if [ $USE_CONTAINER -eq 1 ]; then
  $RC EVAL "
      local keys = redis.call('KEYS', 'comsrv:9999:*')
      for i=1,#keys do redis.call('DEL', keys[i]) end
      keys = redis.call('KEYS', 'modsrv:test_instance:*')
      for i=1,#keys do redis.call('DEL', keys[i]) end
      redis.call('HDEL', 'route:c2m',
          'comsrv:9999:T:1', 'comsrv:9999:T:2',
          'comsrv:9999:S:1', 'comsrv:9999:C:1')
      redis.call('HDEL', 'route:m2c',
          'modsrv:test_instance:A:1', 'modsrv:test_instance:A:2')
      return 'OK'
  " 0
else
  redis-cli EVAL "
      local keys = redis.call('KEYS', 'comsrv:9999:*')
      for i=1,#keys do redis.call('DEL', keys[i]) end
      keys = redis.call('KEYS', 'modsrv:test_instance:*')
      for i=1,#keys do redis.call('DEL', keys[i]) end
      redis.call('HDEL', 'route:c2m',
          'comsrv:9999:T:1', 'comsrv:9999:T:2',
          'comsrv:9999:S:1', 'comsrv:9999:C:1')
      redis.call('HDEL', 'route:m2c',
          'modsrv:test_instance:A:1', 'modsrv:test_instance:A:2')
      return 'OK'
  " 0
fi

echo -e "${GREEN}✓ Test data cleaned${NC}"

# Run tests with single thread to avoid race conditions
echo -e "${YELLOW}Running integration tests (single-threaded)...${NC}"
if RUST_LOG=info cargo test --workspace --test '*' -- --test-threads=1; then
    echo -e "${GREEN}✓ All integration tests passed!${NC}"
else
    echo -e "${RED}✗ Some integration tests failed${NC}"
    echo -e "${YELLOW}Tip: Run individual tests for debugging:${NC}"
    echo "  cargo test --test full_stack_e2e_test test_complete_uplink_data_flow -- --nocapture"
    exit 1
fi

# Optional: Try parallel execution to detect race conditions
echo ""
echo -e "${YELLOW}Testing parallel execution (optional)...${NC}"
if cargo test --workspace --test '*' 2>/dev/null; then
    echo -e "${GREEN}✓ Tests also pass in parallel (good isolation)${NC}"
else
    echo -e "${YELLOW}⚠ Tests fail in parallel (expected - tests share Redis state)${NC}"
    echo "  This is normal. Use --test-threads=1 for reliable results."
fi

echo ""
echo -e "${GREEN}=== Integration Test Run Complete ===${NC}"
