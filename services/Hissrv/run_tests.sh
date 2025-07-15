#!/bin/bash
# 运行 hissrv 服务的测试套件

set -e

echo "======================================"
echo "运行 HisSrv 测试套件"
echo "======================================"

# 设置颜色
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# 运行单元测试
echo -e "\n${YELLOW}运行单元测试...${NC}"
cargo test --lib -- --nocapture

# 运行集成测试
echo -e "\n${YELLOW}运行集成测试...${NC}"
cargo test --test '*' -- --nocapture

# 运行特定的测试模块
echo -e "\n${YELLOW}运行批量写入器测试...${NC}"
cargo test batch_writer_test -- --nocapture

echo -e "\n${YELLOW}运行Redis订阅器测试...${NC}"
cargo test redis_subscriber_test -- --nocapture

echo -e "\n${YELLOW}运行查询优化器测试...${NC}"
cargo test query_optimizer_test -- --nocapture

echo -e "\n${YELLOW}运行保留策略测试...${NC}"
cargo test retention_policy_test -- --nocapture

echo -e "\n${YELLOW}运行API测试...${NC}"
cargo test api_test -- --nocapture

# 运行性能基准测试（可选）
if [ "$1" == "--bench" ]; then
    echo -e "\n${YELLOW}运行性能基准测试...${NC}"
    cargo bench
fi

# 生成测试覆盖率报告（需要安装 cargo-tarpaulin）
if command -v cargo-tarpaulin &> /dev/null; then
    echo -e "\n${YELLOW}生成测试覆盖率报告...${NC}"
    cargo tarpaulin --out Html --output-dir target/coverage
    echo -e "${GREEN}覆盖率报告已生成到: target/coverage/tarpaulin-report.html${NC}"
fi

echo -e "\n${GREEN}所有测试完成！${NC}"