# Comsrv: Communication Server for Energy Management Systems

[![C++](https://img.shields.io/badge/language-C%2B%2B-blue.svg)](https://isocpp.org/)
[![License](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)
[![Version](https://img.shields.io/badge/version-0.4.0-orange.svg)](CHANGELOG.md)

Comsrv is a flexible, high-performance communication server designed for energy management systems. It provides a unified interface for various industrial communication protocols, focusing on Modbus TCP and Modbus RTU implementations.

## Features

- **Multi-Protocol Support**
  - Modbus TCP (Client and Server)
  - Modbus RTU (Master and Slave)
  - Extensible architecture for adding more protocols

- **Flexible Configuration**
  - JSON-based channel configuration
  - CSV-based point table definitions
  - Hot-reload of configuration files

- **High Performance**
  - Optimized polling strategies
  - Grouped register reads/writes
  - Multi-threaded communication channels

- **Robust Error Handling**
  - Automatic reconnection mechanisms
  - Detailed error logging
  - Graceful failure recovery

- **Data Exchange**
  - Redis integration for data publishing
  - Standardized data point format
  - Support for various data types

## Project Structure

```
comsrv/
├── config/                 # Configuration files
│   ├── channels.json       # Channel configuration
│   └── points/             # Point table CSV files
│       ├── pcs_di.csv
│       ├── pcs_ai.csv
│       └── ...
├── include/                # Header files
│   ├── core/               # Core functionality
│   └── protocols/          # Protocol implementations
│       ├── modbus/         # Modbus protocol
│       └── ...
├── src/                    # Source files
│   ├── core/               # Core implementation
│   └── protocols/          # Protocol implementations
├── logs/                   # Log files
├── CMakeLists.txt          # CMake build configuration
├── Dockerfile              # Production Docker configuration
├── DevDockerfile           # Development Docker configuration
├── docker-compose.yml      # Docker Compose configuration
└── start.sh                # Startup script
```

## Prerequisites

- CMake 3.10 or higher
- C++17 compatible compiler
- Required libraries:
  - libmodbus (≥ 3.1.4)
  - hiredis (≥ 0.14.0)
  - jsoncpp (≥ 1.7.4)
- (Optional) Docker and Docker Compose for containerized deployment

## Building from Source

### Using CMake

```bash
# Clone the repository
git clone https://github.com/yourusername/comsrv.git
cd comsrv

# Create a build directory
mkdir build && cd build

# Configure and build
cmake ..
make

# Install (optional)
sudo make install
```

### Using Docker (Recommended)

```bash
# Clone the repository
git clone https://github.com/yourusername/comsrv.git
cd comsrv

# Build and run using the provided script
./start.sh
```

## Configuration

### Channel Configuration (channels.json)

The `channels.json` file defines communication channels and global settings:

```json
{
  "version": "2.0",
  "title": "Channel Configuration for Energy Management System",
  "logging": {
    "level": "INFO",
    "file": "logs/comsrv.log",
    "maxSize": 10485760,
    "maxFiles": 10
  },
  "redis": {
    "host": "redis",
    "port": 6379,
    "db": 0,
    "password": "",
    "keyPrefix": "comsrv:"
  },
  "channels": [
    {
      "index": 1,
      "name": "PCS",
      "description": "Power Conversion System",
      "enabled": true,
      "protocolType": 1,
      "physicalInterfaceType": 1,
      "deviceRole": 1,
      "protocol": {
        "type": "ModbusTCP",
        "host": "192.168.1.100",
        "port": 502,
        "timeout": 1000,
        "maxRead": 125
      },
      "pointTables": {
        "di": "points/pcs_di.csv",
        "ai": "points/pcs_ai.csv",
        "do": "points/pcs_do.csv",
        "ao": "points/pcs_ao.csv"
      },
      "pollRate": 1000
    }
  ]
}
```

### Point Table Format (CSV)

Point tables use CSV format with headers. Example for a digital input (DI) point table:

```csv
name,address,slaveId,description,dataType,byteOrder,bitOffset,scale,offset,units,readOnly
BatteryFault,1001,1,Battery fault status,UINT16,AB,0,1,0,,true
ACContactorStatus,1002,1,AC contactor status,UINT16,AB,0,1,0,,true
```

Available data types:
- `UINT16`: 16-bit unsigned integer
- `INT16`: 16-bit signed integer
- `UINT32`: 32-bit unsigned integer
- `INT32`: 32-bit signed integer
- `FLOAT32`: 32-bit floating point
- `UINT64`: 64-bit unsigned integer
- `INT64`: 64-bit signed integer
- `FLOAT64`: 64-bit floating point

Byte orders:
- `AB`: Big-endian 16-bit (default)
- `BA`: Little-endian 16-bit
- `ABCD`: Big-endian 32-bit
- `CDAB`: Little-endian 32-bit swapped
- `BADC`: Big-endian 16-bit swapped
- `DCBA`: Little-endian 32-bit

## Running the Application

### Command Line Options

```
Usage: comsrv [OPTIONS]

Options:
  -c, --config DIR    Configuration directory (default: ./config)
  -l, --logs DIR      Log directory (default: ./logs)
  -v, --verbose       Enable verbose output
  -d, --daemon        Run as daemon
  -h, --help          Show this help message
```

### Example

```bash
# Run with default settings
./comsrv

# Run with custom configuration directory
./comsrv -c /path/to/config -l /path/to/logs

# Run in verbose mode
./comsrv -v

# Run as daemon
./comsrv -d
```

## Data Access

When Redis integration is enabled, all data points are published to Redis with keys following this pattern:

```
{keyPrefix}:{channelName}:{pointType}:{pointName}
```

For example:

```
comsrv:PCS:di:BatteryFault
```

## Development

A development environment is provided through `DevDockerfile`:

```bash
# Build and run the development container
docker build -t comsrv-dev -f DevDockerfile .
docker run -it --name comsrv-dev -v $(pwd):/workspace comsrv-dev
```

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request
