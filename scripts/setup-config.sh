#!/bin/bash
# Setup configuration from template
# This script is used during deployment/packaging to copy config.template to config

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

TEMPLATE_DIR="$PROJECT_ROOT/config.template"
CONFIG_DIR="$PROJECT_ROOT/config"

echo "=== VoltageEMS Configuration Setup ==="
echo "Template: $TEMPLATE_DIR"
echo "Target:   $CONFIG_DIR"
echo

# Check if template exists
if [ ! -d "$TEMPLATE_DIR" ]; then
    echo "Error: Template directory not found: $TEMPLATE_DIR"
    exit 1
fi

# Ask for confirmation if config already exists
if [ -d "$CONFIG_DIR" ] && [ "$(ls -A "$CONFIG_DIR" 2>/dev/null)" ]; then
    echo "⚠️  WARNING: Config directory already exists and is not empty!"
    echo "    This will overwrite existing configuration files."
    echo
    read -p "Continue? (y/N): " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        echo "Aborted."
        exit 0
    fi
    echo "Backing up existing config to config.backup.$(date +%Y%m%d_%H%M%S)..."
    mv "$CONFIG_DIR" "$CONFIG_DIR.backup.$(date +%Y%m%d_%H%M%S)"
fi

# Create config directory
mkdir -p "$CONFIG_DIR"

# Copy all configuration from template
echo "Copying configuration files..."
cp -r "$TEMPLATE_DIR"/* "$CONFIG_DIR/"

echo ""
echo "✅ Configuration setup complete!"
echo ""
echo "Copied files:"
echo "  - comsrv: $(find "$CONFIG_DIR/comsrv" -name "*.csv" 2>/dev/null | wc -l | xargs) CSV files"
echo "  - modsrv: $(find "$CONFIG_DIR/modsrv/products" -name "*.csv" 2>/dev/null | wc -l | xargs) product CSV files"
echo "  - rulesrv: $(find "$CONFIG_DIR/rulesrv" -name "*.yaml" 2>/dev/null | wc -l | xargs) rule files"
echo ""
echo "Next steps:"
echo "  1. Review and customize config files if needed"
echo "  2. Run: monarch init all"
echo "  3. Run: monarch sync all"
echo "  4. Start services"
