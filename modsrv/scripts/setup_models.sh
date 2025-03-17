#!/bin/bash

# Build and run the model configuration script
cd "$(dirname "$0")"
cargo build
./target/debug/create_model_config

echo "Model configuration setup complete!" 