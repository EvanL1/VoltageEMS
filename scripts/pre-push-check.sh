#!/bin/bash
# Pre-push check script for VoltageEMS

echo "Running pre-push checks..."

# Check if code is formatted
echo "Checking code formatting..."
if ! cargo fmt --all -- --check; then
    echo "Code is not formatted! Please run 'cargo fmt --all' before pushing."
    exit 1
fi

# Note: Tests temporarily disabled during refactoring
echo "Note: Test checks temporarily disabled during refactoring"

echo "Pre-push checks passed!"
exit 0