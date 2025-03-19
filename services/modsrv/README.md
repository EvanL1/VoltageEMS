# ModSrv - Model Service

Real-time model execution service for VoltageEMS.

## Overview

ModSrv is a service that executes real-time models for monitoring and control of energy systems. It provides a flexible framework for creating and managing different types of models, with support for:

- Template-based model creation
- Real-time data processing
- Control operations
- Redis-based data storage and retrieval

## Requirements

- Rust 1.70 or higher
- Redis (local or remote)
- Docker and Docker Compose (for containerized deployment)

## Directory Structure

```
modsrv/
  ├── src/               # Source code
  ├── templates/         # Model templates
  ├── config/            # Configuration files
  ├── instances/         # Instance data (local storage)
  ├── Dockerfile         # Docker build file
  ├── docker-compose.yml # Docker Compose configuration
  └── Cargo.toml         # Rust project configuration
```

## Configuration

Configuration can be provided in YAML or TOML format. The service looks for configuration files in the following order:

1. Path specified by `--config` command-line argument
2. `/etc/voltageems/config/modsrv/modsrv.yaml` (Docker environment)
3. Current directory (`modsrv.yaml` or `modsrv.toml`)

Example configuration (YAML):

```yaml
redis:
  host: "localhost"  # or "redis" for Docker
  port: 6379
  password: ""
  socket: ""
  key_prefix: "ems:"
  db: 0

logging:
  level: "debug"
  file: ""
  console: true

model:
  update_interval_ms: 1000
  config_key_pattern: "ems:model:config:*"
  data_key_pattern: "ems:data:*"
  output_key_pattern: "ems:model:output:*"
  templates_dir: "templates"  # or "/opt/voltageems/modsrv/templates" for Docker

control:
  operation_key_pattern: "ems:control:operation:*"
  enabled: true

use_redis: true
storage_mode: "hybrid"
sync_interval_secs: 60
```

## Local Development

### Running Locally

To run the service locally:

```sh
# With default configuration
cargo run -- service

# With custom configuration
cargo run -- --config config/local-config.yaml service

# List available templates
cargo run -- list

# Show model information
cargo run -- info
```

### Creating Model Instances

To create a model instance from a template:

```sh
# Create a single instance
cargo run -- create <template_id> <instance_id> --name "Instance Name"

# Create multiple instances
cargo run -- create-multiple <template_id> <count> --prefix "instance" --start-index 1
```

## Docker Deployment

### Building and Running with Docker Compose

```sh
# Build and start the services
docker-compose up -d

# View logs
docker-compose logs -f modsrv

# Stop services
docker-compose down
```

### Using Docker directly

```sh
# Build the image
docker build -t voltageems/modsrv .

# Run the container
docker run -d --name modsrv \
  -v ./config:/etc/voltageems/config/modsrv \
  -v ./templates:/opt/voltageems/modsrv/templates \
  --network host \
  voltageems/modsrv
```

## Templates

Templates are stored in the `templates` directory and define the structure and behavior of model instances. Each template includes:

- Basic metadata (ID, name, description)
- Input/output mappings
- Control actions

Example template:

```yaml
id: "example_model"
name: "Example Model"
description: "A simple example model"
file_path: "templates/example.yaml"
version: "1.0.0"

input_mappings:
  - source_field: "input1"
    target_field: "model_input1"
    data_type: "string"
  - source_field: "input2"
    target_field: "model_input2"
    data_type: "float"

output_mappings:
  - source_field: "output1"
    target_field: "model_output1"
    data_type: "string"
  - source_field: "output2"
    target_field: "model_output2"
    data_type: "float"

control_actions:
  - id: "action1"
    name: "Example Action 1"
    description: "This is an example action"
    parameters:
      - name: "param1"
        description: "Example parameter"
        data_type: "string"
        default_value: "default"
```

## Control Operations

ModSrv supports control operations that can be triggered through Redis. To execute a control operation:

1. Create a control operation in Redis:
   ```
   HSET ems:control:operation:<operation_id> id <operation_id> model_id <model_id> action_id <action_id> param1 <value1> param2 <value2>
   ```
2. The service will automatically detect and execute the operation on the next update cycle.

## License

Copyright © 2024 VoltageEMS. All rights reserved.

