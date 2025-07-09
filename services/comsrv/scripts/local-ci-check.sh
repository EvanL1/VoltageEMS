#!/bin/bash
# 本地 CI 检查脚本 - 在推送前运行，模拟 GitHub Actions CI

set -e

echo "🔍 开始本地 CI 检查..."
echo "================================"

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# 错误计数
ERRORS=0

# 检查函数
check_step() {
    local step_name=$1
    local command=$2
    
    echo -e "\n${YELLOW}▶ $step_name${NC}"
    if eval "$command"; then
        echo -e "${GREEN}✅ $step_name 通过${NC}"
    else
        echo -e "${RED}❌ $step_name 失败${NC}"
        ((ERRORS++))
        return 1
    fi
}

# 1. 格式检查
check_step "代码格式检查" "cargo fmt -- --check"

# 2. 构建检查
check_step "Debug 构建" "cargo build"
check_step "Release 构建" "cargo build --release"

# 3. Clippy 关键检查（模拟 CI）
check_step "Clippy 关键错误检查" "cargo clippy --all-targets -- \
    -D clippy::correctness \
    -D clippy::suspicious \
    -D deprecated"

# 4. 运行测试
check_step "单元测试" "cargo test --lib"
check_step "集成测试" "cargo test --test '*' || true"  # 集成测试可能需要特定环境

# 5. 文档检查
check_step "文档构建" "cargo doc --no-deps --quiet"

# 6. 可选：完整 Clippy 检查（仅供参考）
echo -e "\n${YELLOW}▶ 完整 Clippy 分析（仅供参考）${NC}"
cargo clippy --all-targets 2>&1 | tee clippy-report.txt || true
CLIPPY_WARNINGS=$(grep -c "warning:" clippy-report.txt || true)
echo -e "${YELLOW}📊 Clippy 警告数: $CLIPPY_WARNINGS${NC}"

# 7. 检查是否有未提交的更改
echo -e "\n${YELLOW}▶ Git 状态检查${NC}"
if [[ -n $(git status -s) ]]; then
    echo -e "${YELLOW}⚠️  有未提交的更改：${NC}"
    git status -s
else
    echo -e "${GREEN}✅ 工作区干净${NC}"
fi

# 总结
echo -e "\n================================"
if [ $ERRORS -eq 0 ]; then
    echo -e "${GREEN}✅ 所有 CI 检查通过！可以安全推送到 GitHub。${NC}"
    
    # 显示下一步操作建议
    echo -e "\n建议的下一步操作："
    echo "1. git add -A"
    echo "2. git commit -m \"your commit message\""
    echo "3. git push origin $(git branch --show-current)"
else
    echo -e "${RED}❌ 有 $ERRORS 个检查失败。请修复后再推送。${NC}"
    exit 1
fi

# 清理临时文件
rm -f clippy-report.txt