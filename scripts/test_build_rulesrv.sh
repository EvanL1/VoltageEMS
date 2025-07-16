#!/bin/bash
set -e

echo "Testing rulesrv build..."
cd /Users/lyf/dev/VoltageEMS

# Build rulesrv
echo "Building rulesrv..."
cargo build -p rulesrv --release 2>&1

if [ $? -eq 0 ]; then
    echo "✅ Build successful!"
    ls -la target/release/rulesrv
else
    echo "❌ Build failed!"
    exit 1
fi