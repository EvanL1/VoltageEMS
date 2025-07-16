#!/bin/bash
echo "Checking rulesrv compilation..."
cd /Users/lyf/dev/VoltageEMS
cargo check -p rulesrv 2>&1 | grep -E "(error|warning)" | head -20
echo "Check complete"