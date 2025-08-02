#!/usr/bin/env bash
set -e

echo "🔧 Setting up Git hooks..."

# 设置 Git 使用自定义 hooks 目录
git config core.hooksPath .githooks

echo "✅ Git hooks configured successfully!"
echo ""
echo "📋 Available hooks:"
echo "  - pre-commit: Runs formatting, clippy, and tests"
echo "  - pre-push: Runs strict checks and security audits"
echo "  - commit-msg: Validates commit message format"
echo ""
echo "🛠️  Recommended tools to install:"
echo "  cargo install cargo-audit    # Security vulnerability scanner"
echo "  cargo install cargo-udeps    # Find unused dependencies"
echo "  cargo install cargo-outdated # Check for outdated dependencies"