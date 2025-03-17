# Model Service (Modelsrv)

A real-time model execution service for the Energy Management System (EMS). This service loads model configurations from Redis, maps real-time data from Comsrv, and executes the models to produce outputs that are stored back in Redis. It also supports sending remote control and remote adjustment commands to Comsrv.

## Features

- Dynamic model loading from Redis configurations
- Real-time data mapping from Comsrv data in Redis
- Configurable update intervals
- Transformation support for input data
- Asynchronous execution using Tokio
- Automatic control actions based on model outputs
- Direct remote control and remote adjustment command support

## Architecture

The Model Service is designed to run in a Docker container and interact with:
- Redis container: For configuration, input data, and output storage
- Comsrv: Provides real-time data that is mapped to model inputs and receives control commands

## Configuration

The service is configured using a TOML file (`modelsrv.toml`). Key configuration options include:

```toml
[redis]
host = "localhost"
port = 6379
password = ""
socket = ""
prefix = "ems:"

[logging]
level = "info"
file = "/var/log/ems/modelsrv.log"
console = true

[model]
update_interval_ms = 1000
config_key_pattern = "ems:model:config:*"
data_key_pattern = "ems:data:*"
output_key_pattern = "ems:model:output:"
```

## Model Configuration Format

Models are defined in Redis as JSON strings with the following structure:

### Basic Model

```json
{
  "id": "model1",
  "name": "Battery Model",
  "description": "Real-time battery state model",
  "input_mappings": [
    {
      "source_key": "ems:data:battery",
      "source_field": "voltage",
      "target_field": "battery_voltage",
      "transform": "scale:0.001"
    },
    {
      "source_key": "ems:data:battery",
      "source_field": "current",
      "target_field": "battery_current",
      "transform": null
    }
  ],
  "output_key": "ems:model:output:battery",
  "enabled": true
}
```

### Model with Control Actions

```json
{
  "model": {
    "id": "power_flow_model",
    "name": "Power Flow Model",
    "description": "Real-time power flow model",
    "input_mappings": [
      {
        "source_key": "ems:data:pcs",
        "source_field": "active_power",
        "target_field": "pcs_power",
        "transform": null
      }
    ],
    "output_key": "ems:model:output:power_flow",
    "enabled": true
  },
  "actions": [
    {
      "id": "start_diesel_generator",
      "name": "Start Diesel Generator",
      "action_type": "RemoteControl",
      "channel": "Diesel_Serial",
      "point": "start_command",
      "value": "1",
      "conditions": [
        {
          "field": "battery_soc",
          "operator": "<",
          "value": "20"
        }
      ],
      "enabled": true
    }
  ]
}
```

## Remote Control and Adjustment

The service supports sending remote control (boolean) and remote adjustment (numeric) commands to Comsrv. These commands can be triggered:

1. Automatically by control actions defined in model configurations
2. Programmatically through the ModelEngine API

Commands are sent to Comsrv through Redis using a command queue. The command flow is:

1. Modelsrv creates a command and pushes it to the command queue (`ems:command:queue`)
2. Modelsrv creates a command status record (`ems:command:status:{command_id}`)
3. Comsrv processes the command from the queue
4. Comsrv updates the command status as it processes the command
5. Modelsrv can check the command status to determine if it was successful

## Building and Running

### Building with Cargo

```bash
cargo build --release
```

### Running the Service

```bash
./target/release/modelsrv --config modelsrv.toml
```

### Using Docker

```bash
docker build -t modelsrv .
docker run -d --name modelsrv --network ems-network modelsrv
```

## Development

### Prerequisites

- Rust 1.67 or later
- Redis server (for development)

### Testing

```bash
cargo test
```

## License

[Your License] 